use crate::{
    capabilities::{self, Scope},
    cli::RbacArgs,
    config::Config,
    model::Key,
};
use anyhow::{Result, bail, ensure};
use k8s_openapi::{
    api::rbac::v1::{PolicyRule, RoleRef, Subject},
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use kube::{
    Api, Client,
    api::{DynamicObject, Patch, PatchParams, PostParams},
    core::{ApiResource, GroupVersionKind},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MANAGER: &str = "cnpg-watch";
const LABEL: &str = "app.kubernetes.io/managed-by";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub api_version: String,
    pub kind: String,
    pub metadata: ObjectMeta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<PolicyRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref: Option<RoleRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects: Option<Vec<Subject>>,
}
impl Manifest {
    fn id(&self) -> String {
        format!(
            "{}/{}/{}",
            self.kind,
            self.metadata.namespace.as_deref().unwrap_or("<cluster>"),
            self.metadata.name.as_deref().unwrap_or("")
        )
    }
    fn api(&self, client: Client) -> Api<DynamicObject> {
        let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(
            "rbac.authorization.k8s.io",
            "v1",
            &self.kind,
        ));
        match &self.metadata.namespace {
            Some(ns) => Api::namespaced_with(client, ns, &resource),
            None => Api::all_with(client, &resource),
        }
    }
}
fn name(value: &str) -> Result<()> {
    Key::new("scope", value)
        .map_err(|_| anyhow::anyhow!("invalid Kubernetes object/subject name"))?;
    Ok(())
}
fn subject(config: &Config, args: &RbacArgs) -> Result<Subject> {
    if let Some(sa) = &args.service_account {
        name(sa)?;
        let ns = args
            .service_account_namespace
            .as_ref()
            .or(config.namespace.as_ref())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "service-account-namespace is required without an explicit database namespace"
                )
            })?;
        name(ns)?;
        Ok(Subject {
            kind: "ServiceAccount".into(),
            name: sa.clone(),
            namespace: Some(ns.clone()),
            api_group: None,
        })
    } else {
        let (kind, value) = if let Some(user) = &args.user {
            ("User", user)
        } else if let Some(group) = &args.group {
            ("Group", group)
        } else {
            bail!("an explicit service-account, user, or group subject is required")
        };
        ensure!(
            !value.is_empty()
                && value.len() <= 253
                && !value.chars().any(|c| c.is_control() || c.is_whitespace()),
            "invalid subject"
        );
        Ok(Subject {
            kind: kind.into(),
            name: value.clone(),
            namespace: None,
            api_group: Some("rbac.authorization.k8s.io".into()),
        })
    }
}
fn labels() -> BTreeMap<String, String> {
    BTreeMap::from([
        (LABEL.into(), MANAGER.into()),
        ("cnpg-watch.io/schema-version".into(), "1".into()),
    ])
}
fn pair(
    objects: &mut Vec<Manifest>,
    base: &str,
    namespace: Option<String>,
    rules: Vec<PolicyRule>,
    subject: &Subject,
) {
    if rules.is_empty() {
        return;
    }
    let kind = if namespace.is_some() {
        "Role"
    } else {
        "ClusterRole"
    };
    let meta = ObjectMeta {
        name: Some(base.into()),
        namespace,
        labels: Some(labels()),
        ..Default::default()
    };
    objects.push(Manifest {
        api_version: "rbac.authorization.k8s.io/v1".into(),
        kind: kind.into(),
        metadata: meta.clone(),
        rules: Some(stable_rules(&rules)),
        role_ref: None,
        subjects: None,
    });
    objects.push(Manifest {
        api_version: "rbac.authorization.k8s.io/v1".into(),
        kind: format!("{kind}Binding"),
        metadata: meta,
        rules: None,
        role_ref: Some(RoleRef {
            api_group: "rbac.authorization.k8s.io".into(),
            kind: kind.into(),
            name: base.into(),
        }),
        subjects: Some(vec![subject.clone()]),
    });
}
pub fn generate(config: &Config, args: &RbacArgs) -> Result<Vec<Manifest>> {
    name(&args.name)?;
    ensure!(args.name.len() <= 230, "RBAC name too long");
    ensure!(
        config.all_namespaces || config.namespace.is_some(),
        "setup requires an explicit --namespace or --all-namespaces"
    );
    let target = subject(config, args)?;
    let mut db = Vec::new();
    let mut op = Vec::new();
    let mut global = Vec::new();
    for spec in capabilities::CATALOG {
        if !capabilities::enabled(spec.feature, config.profile) {
            continue;
        }
        for r in spec.kubernetes {
            let rules = match r.scope {
                Scope::Database => &mut db,
                Scope::Operator if config.operator_watch && config.operator_namespace.is_some() => {
                    &mut op
                }
                Scope::Cluster if args.include_cluster_resources || config.cluster_resources => {
                    &mut global
                }
                _ => continue,
            };
            rules.push(PolicyRule {
                api_groups: Some(vec![r.group.into()]),
                resources: Some(vec![r.resource.into()]),
                verbs: r.verbs.iter().map(|v| (*v).into()).collect(),
                ..Default::default()
            });
        }
    }
    let mut objects = Vec::new();
    let ns = if config.all_namespaces {
        None
    } else {
        config.namespace.clone()
    };
    if !config.all_namespaces && config.operator_namespace == config.namespace {
        db.extend(op);
        op = Vec::new();
    }
    pair(&mut objects, &args.name, ns, db, &target);
    pair(
        &mut objects,
        &format!("{}-operator", args.name),
        config.operator_namespace.clone(),
        op,
        &target,
    );
    pair(
        &mut objects,
        &format!("{}-cluster", args.name),
        None,
        global,
        &target,
    );
    let mut secrets: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    if config.profile != crate::cli::Profile::Minimal {
        for key in &config.certificate_secrets {
            secrets
                .entry(key.namespace.clone())
                .or_default()
                .push(key.name.clone());
        }
    }
    for (index, (namespace, mut names)) in secrets.into_iter().enumerate() {
        names.sort();
        names.dedup();
        pair(
            &mut objects,
            &format!("{}-certificates-{index}", args.name),
            Some(namespace),
            vec![PolicyRule {
                api_groups: Some(vec![String::new()]),
                resources: Some(vec!["secrets".into()]),
                resource_names: Some(names),
                verbs: vec!["get".into()],
                ..Default::default()
            }],
            &target,
        );
    }
    Ok(objects)
}
pub fn yaml(objects: &[Manifest]) -> Result<String> {
    let mut out = String::new();
    for (i, object) in objects.iter().enumerate() {
        if i > 0 {
            out.push_str("---\n");
        }
        out.push_str(&serde_yaml::to_string(object)?);
    }
    Ok(out)
}
/// Keep generated manifests readable; comparisons separately expand permission products.
fn stable_rules(rules: &[PolicyRule]) -> Vec<PolicyRule> {
    let mut map = BTreeMap::new();
    for rule in rules {
        let mut rule = rule.clone();
        rule.verbs.sort();
        rule.verbs.dedup();
        for values in [
            &mut rule.api_groups,
            &mut rule.resources,
            &mut rule.resource_names,
            &mut rule.non_resource_urls,
        ]
        .into_iter()
        .flatten()
        {
            values.sort();
            values.dedup();
        }
        map.insert(serde_json::to_string(&rule).unwrap(), rule);
    }
    map.into_values().collect()
}
/// Expand permission products so grouping/order do not produce spurious diffs.
pub fn canonical_rules(rules: &[PolicyRule]) -> Vec<PolicyRule> {
    let mut unique = BTreeMap::new();
    for rule in rules {
        let mut names = rule.resource_names.clone();
        if let Some(n) = &mut names {
            n.sort();
            n.dedup();
        }
        for verb in &rule.verbs {
            if let Some(urls) = &rule.non_resource_urls {
                for url in urls {
                    let atom = PolicyRule {
                        verbs: vec![verb.clone()],
                        non_resource_urls: Some(vec![url.clone()]),
                        ..Default::default()
                    };
                    unique.insert(serde_json::to_string(&atom).unwrap(), atom);
                }
            }
            for group in rule.api_groups.iter().flatten() {
                for resource in rule.resources.iter().flatten() {
                    let atom = PolicyRule {
                        api_groups: Some(vec![group.clone()]),
                        resources: Some(vec![resource.clone()]),
                        resource_names: names.clone(),
                        verbs: vec![verb.clone()],
                        ..Default::default()
                    };
                    unique.insert(serde_json::to_string(&atom).unwrap(), atom);
                }
            }
        }
    }
    unique.into_values().collect()
}
fn keys<T: Serialize>(items: &[T]) -> BTreeSet<String> {
    items
        .iter()
        .map(|v| serde_json::to_string(v).unwrap())
        .collect()
}
#[derive(Debug, Serialize)]
pub struct Difference {
    pub object: String,
    pub status: &'static str,
    pub missing: Vec<String>,
    pub extra_managed_rules: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub applied: bool,
    pub would_change: bool,
    pub matching: Vec<String>,
    pub missing: Vec<String>,
    pub changed: Vec<Difference>,
    pub extra_managed_rules: Vec<Difference>,
}

pub async fn reconcile(
    client: Client,
    desired: Vec<Manifest>,
    apply: bool,
    prune: bool,
) -> Result<Report> {
    let mut report = Report {
        schema_version: 1,
        applied: apply,
        would_change: false,
        matching: Vec::new(),
        missing: Vec::new(),
        changed: Vec::new(),
        extra_managed_rules: Vec::new(),
    };
    // Read and validate every object before issuing any write.
    let mut planned = Vec::new();
    for object in desired {
        let api = object.api(client.clone());
        let name = object.metadata.name.as_deref().unwrap();
        let live = api.get_opt(name).await.map_err(|e| safe("read RBAC", &e))?;
        let live = live
            .map(|o| serde_json::from_value::<Manifest>(serde_json::to_value(o).unwrap()))
            .transpose()
            .map_err(|_| anyhow::anyhow!("invalid live RBAC object; contents suppressed"))?;
        if let Some(existing) = &live {
            ensure!(
                existing
                    .metadata
                    .labels
                    .as_ref()
                    .and_then(|l| l.get(LABEL))
                    .map(String::as_str)
                    == Some(MANAGER),
                "refusing to adopt unmanaged RBAC object {}",
                object.id()
            );
            ensure!(
                existing.role_ref == object.role_ref,
                "immutable roleRef differs for {}; choose a new --name",
                object.id()
            );
        }
        if apply {
            for s in object
                .subjects
                .iter()
                .flatten()
                .filter(|s| s.kind == "ServiceAccount")
            {
                let accounts: Api<k8s_openapi::api::core::v1::ServiceAccount> =
                    Api::namespaced(client.clone(), s.namespace.as_deref().unwrap());
                ensure!(
                    accounts
                        .get_opt(&s.name)
                        .await
                        .map_err(|e| safe("validate service account", &e))?
                        .is_some(),
                    "service account subject does not exist"
                );
            }
        }
        let desired_keys = content(&object);
        let live_keys = live.as_ref().map(content).unwrap_or_default();
        let missing: Vec<_> = desired_keys.difference(&live_keys).cloned().collect();
        let extra: Vec<_> = live_keys.difference(&desired_keys).cloned().collect();
        let write = live.is_none() || !missing.is_empty() || (prune && !extra.is_empty());
        if live.is_none() {
            report.missing.push(object.id());
        } else if missing.is_empty() && extra.is_empty() {
            report.matching.push(object.id());
        } else {
            report.changed.push(Difference {
                object: object.id(),
                status: if write { "changed" } else { "extra_preserved" },
                missing,
                extra_managed_rules: extra.clone(),
            });
        }
        if !extra.is_empty() {
            report.extra_managed_rules.push(Difference {
                object: object.id(),
                status: if prune { "prune" } else { "preserve" },
                missing: Vec::new(),
                extra_managed_rules: extra,
            });
        }
        report.would_change |= write;
        let mut merged = object;
        if let Some(existing) = &live {
            merged.metadata.resource_version = existing.metadata.resource_version.clone();
            if !prune {
                if let Some(rules) = &mut merged.rules {
                    rules.extend(existing.rules.clone().unwrap_or_default());
                    *rules = stable_rules(rules);
                }
                if let Some(subjects) = &mut merged.subjects {
                    subjects.extend(existing.subjects.clone().unwrap_or_default());
                    *subjects = canonical_subjects(subjects);
                }
            }
        }
        planned.push((api, merged, live.is_some(), write));
    }
    if apply {
        for (api, object, exists, write) in planned {
            if !write {
                continue;
            }
            let value = serde_json::to_value(&object)?;
            if exists {
                api.patch(
                    object.metadata.name.as_deref().unwrap(),
                    &PatchParams::default(),
                    &Patch::Merge(&value),
                )
                .await
                .map_err(|e| safe("update RBAC", &e))?;
            } else {
                let dynamic: DynamicObject = serde_json::from_value(value)?;
                api.create(&PostParams::default(), &dynamic)
                    .await
                    .map_err(|e| safe("create RBAC", &e))?;
            }
        }
    }
    Ok(report)
}
fn canonical_subjects(subjects: &[Subject]) -> Vec<Subject> {
    let mut map = BTreeMap::new();
    for subject in subjects {
        let mut subject = subject.clone();
        if subject.kind == "ServiceAccount" && subject.api_group.as_deref() == Some("") {
            subject.api_group = None;
        }
        map.insert(serde_json::to_string(&subject).unwrap(), subject);
    }
    map.into_values().collect()
}
fn content(object: &Manifest) -> BTreeSet<String> {
    let mut out = keys(&canonical_rules(
        object.rules.as_deref().unwrap_or_default(),
    ));
    out.extend(keys(&canonical_subjects(
        object.subjects.as_deref().unwrap_or_default(),
    )));
    out
}
fn safe(action: &str, error: &kube::Error) -> anyhow::Error {
    anyhow::anyhow!(
        "{action}: {}; details suppressed",
        crate::probes::api_error(error)
    )
}
