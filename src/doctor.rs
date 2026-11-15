//! Bounded non-bootstrap preflight. SSAR decisions and live API evidence stay distinct.
use crate::{
    capabilities::{self, Feature, Requirement, Scope},
    cli::{Output, Profile},
    config::Config,
    model::{Availability, ErrorClass, SyncState},
    probes::{self, PodExec},
    reducer::Store,
};
use futures::{StreamExt, stream};
use k8s_openapi::api::{
    apps::v1::Deployment,
    authorization::v1::{ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec},
    core::v1::Pod,
};
use kube::{
    Api, Client, ResourceExt,
    api::{DynamicObject, ListParams, LogParams, PostParams, WatchParams},
    core::{ApiResource, GroupVersionKind},
};
use serde::Serialize;
use std::{collections::BTreeSet, io::IsTerminal, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Available,
    Forbidden,
    Unknown,
    Absent,
    Syncing,
    Empty,
    DisabledByProfile,
    DisabledByConfig,
    Unfinished,
    Ambiguous,
    Unavailable,
}
#[derive(Debug, Clone, Serialize)]
pub struct Access {
    pub feature: Feature,
    pub group: &'static str,
    pub resource: &'static str,
    pub verb: &'static str,
    pub namespace: Option<String>,
    pub state: State,
    pub evidence: &'static str,
}
#[derive(Debug, Serialize)]
pub struct Finding {
    pub feature: Feature,
    pub state: State,
    pub message: String,
    pub suggestion: String,
}
#[derive(Debug, Serialize)]
pub struct Operator {
    pub namespace: Option<String>,
    pub state: State,
    pub candidates: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct BundleCheck {
    pub id: &'static str,
    pub state: State,
    pub selected: bool,
}
#[derive(Debug, Serialize)]
pub struct AuxiliaryFinding {
    pub source: crate::auxiliary::Key,
    pub state: State,
    pub sample: Option<crate::auxiliary::Sample>,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub context: Option<String>,
    pub namespace: Option<String>,
    pub all_namespaces: bool,
    pub profile: Profile,
    pub envelope: capabilities::Envelope,
    pub capabilities: Vec<capabilities::Capability>,
    pub access: Vec<Access>,
    pub operator: Operator,
    pub findings: Vec<Finding>,
    pub setup_suggestions: Vec<String>,
    pub budgets: crate::budget::Budgets,
    pub terminal: State,
    pub transport: State,
    pub postgres_role: State,
    pub probe_error: Option<ErrorClass>,
    pub query_bundles: Vec<BundleCheck>,
    pub pooler_stats: State,
    pub auxiliary: Vec<AuxiliaryFinding>,
    pub cnpg_api: State,
}
fn state(error: ErrorClass) -> State {
    match error {
        ErrorClass::Forbidden => State::Forbidden,
        ErrorClass::Unavailable => State::Absent,
        _ => State::Unavailable,
    }
}
fn api(requirement: &Requirement, client: Client, namespace: Option<&str>) -> Api<DynamicObject> {
    let mut resource = ApiResource::from_gvk(&GroupVersionKind::gvk(
        requirement.group,
        requirement.version,
        requirement.kind,
    ));
    resource.plural = requirement.resource.split('/').next().unwrap().into();
    match namespace {
        Some(ns) => Api::namespaced_with(client, ns, &resource),
        None => Api::all_with(client, &resource),
    }
}
async fn ssar(
    client: Client,
    r: &Requirement,
    verb: &str,
    namespace: Option<&str>,
) -> (State, &'static str) {
    let reviews: Api<SelfSubjectAccessReview> = Api::all(client);
    let (resource, sub) = r
        .resource
        .split_once('/')
        .map_or((r.resource, None), |(r, s)| (r, Some(s)));
    let review = SelfSubjectAccessReview {
        spec: SelfSubjectAccessReviewSpec {
            resource_attributes: Some(ResourceAttributes {
                group: Some(r.group.into()),
                version: Some(r.version.into()),
                resource: Some(resource.into()),
                subresource: sub.map(Into::into),
                verb: Some(verb.into()),
                namespace: namespace.map(Into::into),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    match reviews.create(&PostParams::default(), &review).await {
        Ok(result) => match result.status {
            Some(s) if s.allowed => (State::Available, "ssar"),
            Some(s)
                if s.denied == Some(true)
                    || s.evaluation_error.as_deref().unwrap_or("").is_empty() =>
            {
                (State::Forbidden, "ssar")
            }
            _ => (State::Unknown, "ssar_inconclusive"),
        },
        Err(_) => (State::Unknown, "ssar_unavailable"),
    }
}
async fn check(
    client: Client,
    feature: Feature,
    r: Requirement,
    verb: &'static str,
    namespace: Option<String>,
) -> Access {
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let (decision, evidence) = ssar(client.clone(), &r, verb, namespace.as_deref()).await;
        if decision == State::Forbidden || (decision == State::Available && verb != "list") {
            return (decision, evidence);
        }
        if r.resource.contains('/') || verb == "get" {
            return (State::Unknown, "no_safe_named_fallback");
        }
        let target = api(&r, client, namespace.as_deref());
        if verb == "list" {
            match target.list(&ListParams::default().limit(1)).await {
                Ok(list) => (
                    if list.items.is_empty() {
                        State::Empty
                    } else {
                        State::Available
                    },
                    "live_list_fallback",
                ),
                Err(e) => (state(probes::api_error(&e)), "live_list_fallback"),
            }
        } else if verb == "watch" {
            match target.watch(&WatchParams::default().timeout(1), "0").await {
                Ok(_) => (State::Available, "live_watch_fallback"),
                Err(e) => (state(probes::api_error(&e)), "live_watch_fallback"),
            }
        } else {
            (State::Unknown, "no_nonmutating_fallback")
        }
    })
    .await
    .unwrap_or((State::Unknown, "deadline"));
    Access {
        feature,
        group: r.group,
        resource: r.resource,
        verb,
        namespace,
        state: result.0,
        evidence: result.1,
    }
}
pub async fn operator(client: Client, config: &Config) -> Operator {
    if !config.operator_watch {
        return Operator {
            namespace: None,
            state: State::DisabledByProfile,
            candidates: vec![],
        };
    }
    let api: Api<Deployment> = match &config.operator_namespace {
        Some(ns) => Api::namespaced(client, ns),
        None => Api::all(client),
    };
    let selector = config
        .operator_selector
        .as_deref()
        .unwrap_or("app.kubernetes.io/name=cloudnative-pg");
    match tokio::time::timeout(
        Duration::from_secs(5),
        api.list(&ListParams::default().labels(selector).limit(20)),
    )
    .await
    {
        Ok(Ok(list)) => {
            let candidates: Vec<_> = list
                .items
                .iter()
                .map(|d| format!("{}/{}", d.namespace().unwrap_or_default(), d.name_any()))
                .collect();
            let ambiguous = list
                .metadata
                .continue_
                .as_ref()
                .is_some_and(|s| !s.is_empty())
                || candidates.len() > 1;
            let state = if ambiguous {
                State::Ambiguous
            } else if candidates.is_empty() {
                State::Empty
            } else {
                State::Available
            };
            let namespace = config.operator_namespace.clone().or_else(|| {
                if !ambiguous {
                    list.items.first().and_then(|d| d.namespace())
                } else {
                    None
                }
            });
            Operator {
                namespace,
                state,
                candidates,
            }
        }
        Ok(Err(e)) => Operator {
            namespace: config.operator_namespace.clone(),
            state: state(probes::api_error(&e)),
            candidates: vec![],
        },
        Err(_) => Operator {
            namespace: config.operator_namespace.clone(),
            state: State::Unknown,
            candidates: vec![],
        },
    }
}
pub async fn inspect(config: &Config) -> Report {
    inspect_with(config, false).await
}
pub async fn inspect_with(config: &Config, include_cluster_resources: bool) -> Report {
    let connected = tokio::time::timeout(
        Duration::from_secs(10),
        crate::kubernetes::client(config, &config.budgets),
    )
    .await;
    let (client, namespace) = match connected {
        Ok(Ok(pair)) => pair,
        other => {
            let error = match other {
                Ok(Err(e)) => e,
                _ => ErrorClass::Timeout,
            };
            let mut report = empty_report(config);
            report.findings.push(Finding {
                feature: Feature::Topology,
                state: state(error),
                message: format!("Kubernetes client unavailable: {error}; details suppressed"),
                suggestion: "Select a valid --kube-context and authenticate with your provider"
                    .into(),
            });
            return report;
        }
    };
    inspect_client(config, client, namespace, include_cluster_resources).await
}
fn empty_report(config: &Config) -> Report {
    Report {
        schema_version: 1,
        context: config.context.clone(),
        namespace: config.namespace.clone(),
        all_namespaces: config.all_namespaces,
        profile: config.profile,
        envelope: capabilities::envelope(config.profile),
        capabilities: capabilities::initial_for(config),
        access: vec![],
        operator: Operator {
            namespace: config.operator_namespace.clone(),
            state: State::Unknown,
            candidates: vec![],
        },
        findings: vec![],
        setup_suggestions: vec![],
        budgets: config.budgets.clone(),
        terminal: if std::io::stdout().is_terminal() {
            State::Available
        } else {
            State::Unavailable
        },
        transport: State::Unknown,
        postgres_role: if config.password_file.is_none() && config.profile != Profile::Minimal {
            State::Unavailable
        } else {
            State::Unknown
        },
        probe_error: if config.password_file.is_none() && config.profile != Profile::Minimal {
            Some(ErrorClass::Authentication)
        } else {
            None
        },
        query_bundles: vec![],
        pooler_stats: if config.profile == Profile::Minimal {
            State::DisabledByProfile
        } else if !config.pooler_stats {
            State::DisabledByConfig
        } else {
            State::Unknown
        },
        auxiliary: vec![],
        cnpg_api: State::Unknown,
    }
}
/// Run the same preflight using an existing scoped client (also used by live contracts).
pub async fn inspect_client(
    config: &Config,
    client: Client,
    namespace: String,
    include_cluster_resources: bool,
) -> Report {
    let mut effective = config.clone();
    effective.cluster_resources |= include_cluster_resources;
    let config = &effective;
    let mut report = empty_report(config);
    if !config.all_namespaces {
        report.namespace = Some(namespace.clone());
    }
    report.operator = operator(client.clone(), config).await;
    let mut work = Vec::new();
    let mut seen = BTreeSet::new();
    for spec in capabilities::CATALOG {
        if !report
            .capabilities
            .iter()
            .any(|c| c.spec.feature == spec.feature && c.requested)
        {
            continue;
        }
        for r in spec.kubernetes {
            let ns = match r.scope {
                Scope::Database if !config.all_namespaces => Some(namespace.clone()),
                Scope::Database => None,
                Scope::Operator if config.operator_watch => match &report.operator.namespace {
                    Some(ns) => Some(ns.clone()),
                    None => continue,
                },
                Scope::Operator => continue,
                Scope::Cluster if include_cluster_resources || config.cluster_resources => None,
                Scope::Cluster => continue,
            };
            for verb in r.verbs {
                if seen.insert((spec.feature, r.group, r.resource, *verb, ns.clone())) {
                    work.push((spec.feature, *r, *verb, ns.clone()));
                }
            }
        }
    }
    report.access = stream::iter(work)
        .map(|(f, r, v, n)| check(client.clone(), f, r, v, n))
        .buffered(8)
        .collect()
        .await;
    // A direct resource request establishes API visibility even if discovery is forbidden.
    let r = &capabilities::CATALOG[0].kubernetes[0];
    let clusters = api(
        r,
        client.clone(),
        if config.all_namespaces {
            None
        } else {
            Some(&namespace)
        },
    );
    let mut params = ListParams::default().limit(2);
    if let Some(selector) = &config.selector {
        params = params.labels(selector);
    }
    if let Some(cluster) = &config.cluster {
        params = params.fields(&format!("metadata.name={cluster}"));
    }
    let selected = match tokio::time::timeout(Duration::from_secs(5), clusters.list(&params)).await
    {
        Ok(Ok(list)) => {
            report.cnpg_api = if list.items.is_empty() {
                State::Empty
            } else {
                State::Available
            };
            if list.items.len() == 1 && list.metadata.continue_.as_deref().unwrap_or("").is_empty()
            {
                list.items.into_iter().next()
            } else {
                None
            }
        }
        Ok(Err(e)) => {
            report.cnpg_api = state(probes::api_error(&e));
            None
        }
        Err(_) => {
            report.cnpg_api = State::Unknown;
            None
        }
    };
    let selected_cluster = selected.as_ref().map(|object| {
        (
            object.namespace().unwrap_or(namespace.clone()),
            object.name_any(),
        )
    });
    if config.profile == Profile::Minimal {
        report.transport = State::DisabledByProfile;
        report.postgres_role = State::DisabledByProfile;
    } else if let Some(object) = selected.as_ref() {
        let ns = object.namespace().unwrap_or(namespace.clone());
        if let Some(pod) = object
            .data
            .pointer("/status/currentPrimary")
            .and_then(|v| v.as_str())
        {
            if let Ok(key) = crate::model::Key::new(ns, pod) {
                let pods: Api<Pod> = Api::namespaced(client.clone(), &key.namespace);
                let owned =
                    match tokio::time::timeout(Duration::from_secs(2), pods.get(&key.name)).await {
                        Ok(Ok(pod)) => {
                            pod.metadata
                                .labels
                                .as_ref()
                                .and_then(|l| l.get("cnpg.io/cluster"))
                                .map(String::as_str)
                                == Some(object.name_any().as_str())
                        }
                        _ => false,
                    };
                if !owned {
                    report.findings.push(Finding {
                        feature: Feature::Topology,
                        state: State::Unknown,
                        message:
                            "Primary ownership labels could not be validated; SQL check skipped"
                                .into(),
                        suggestion: "Check CNPG pod visibility and cnpg.io/cluster labels".into(),
                    });
                }
                if let Some(path) = &config.password_file
                    && owned
                {
                    match probes::password_file(path).await {
                        Ok(password) => {
                            let executor = PodExec::new(
                                client.clone(),
                                password,
                                config.db_role.clone(),
                                config.budgets.clone(),
                            );
                            use crate::probes::registry::{self, Bundle, Diagnostic};
                            let detected = async {
                                let bytes = executor
                                    .execute_bundle(
                                        &key,
                                        Bundle::Capabilities,
                                        &CancellationToken::new(),
                                    )
                                    .await?;
                                let Diagnostic::Capabilities(c) = registry::parse(
                                    Bundle::Capabilities,
                                    &bytes,
                                    &config.db_role,
                                    config.profile,
                                )?
                                else {
                                    return Err(ErrorClass::InvalidData);
                                };
                                for bundle in [
                                    Bundle::Capabilities,
                                    Bundle::Fast,
                                    Bundle::FastBasic,
                                    Bundle::Medium,
                                    Bundle::Deep,
                                    Bundle::DeepText,
                                ] {
                                    let enabled = match bundle {
                                        Bundle::Deep | Bundle::DeepText => {
                                            config.profile == Profile::Incident
                                        }
                                        _ => true,
                                    };
                                    report.query_bundles.push(BundleCheck {
                                        id: bundle.spec().id,
                                        state: if !enabled {
                                            State::DisabledByProfile
                                        } else if c.allows(bundle) {
                                            State::Available
                                        } else {
                                            State::Unavailable
                                        },
                                        selected: c.select_fast() == Some(bundle),
                                    });
                                }
                                let bundle = c.select_fast().ok_or(ErrorClass::Unsupported)?;
                                let bytes = executor
                                    .execute_bundle(&key, bundle, &CancellationToken::new())
                                    .await?;
                                probes::parse(&bytes, &config.db_role)?;
                                Ok::<_, ErrorClass>(())
                            }
                            .await;
                            match detected {
                                Ok(()) => {
                                    report.transport = State::Available;
                                    report.postgres_role = State::Available;
                                    for access in report.access.iter_mut().filter(|a| {
                                        a.resource == "pods/exec" && a.state == State::Unknown
                                    }) {
                                        access.state = State::Available;
                                        access.evidence = "live_readonly_transport";
                                    }
                                }
                                Err(e) => {
                                    report.transport = if matches!(
                                        e,
                                        ErrorClass::Authentication
                                            | ErrorClass::Unsupported
                                            | ErrorClass::InvalidData
                                    ) {
                                        State::Available
                                    } else {
                                        state(e)
                                    };
                                    report.postgres_role = state(e);
                                    report.probe_error = Some(e);
                                }
                            }
                        }
                        Err(error) => {
                            report.postgres_role = State::Unavailable;
                            report.probe_error = Some(error);
                        }
                    }
                }
                // Non-mutating log fallback reads at most one bounded line, never exposes it.
                for access in report.access.iter_mut().filter(|a| {
                    a.resource == "pods/log"
                        && a.state == State::Unknown
                        && a.namespace.as_deref() == Some(key.namespace.as_str())
                }) {
                    let pods: Api<Pod> = Api::namespaced(client.clone(), &key.namespace);
                    let params = LogParams {
                        container: Some("postgres".into()),
                        tail_lines: Some(1),
                        limit_bytes: Some(4096),
                        ..Default::default()
                    };
                    access.state = match tokio::time::timeout(
                        Duration::from_secs(2),
                        pods.logs(&key.name, &params),
                    )
                    .await
                    {
                        Ok(Ok(_)) => State::Available,
                        Ok(Err(e)) => state(probes::api_error(&e)),
                        Err(_) => State::Unknown,
                    };
                    access.evidence = "bounded_live_log_fallback";
                }
            }
        } else {
            report.transport = State::Syncing;
        }
    }
    if config.profile != Profile::Minimal {
        let loader = crate::auxiliary::Loader::new(client.clone(), config.budgets.clone());
        let mut sources: Vec<_> = config
            .certificate_secrets
            .iter()
            .cloned()
            .map(crate::auxiliary::Key::Certificate)
            .collect();
        if config.operator_watch && config.operator_metrics_url.is_some() {
            sources.push(crate::auxiliary::Key::Metrics);
        }
        for source in sources {
            let result = loader
                .execute(config, &source, &CancellationToken::new())
                .await;
            report.auxiliary.push(AuxiliaryFinding {
                source,
                state: result
                    .as_ref()
                    .map_or_else(|e| state(*e), |_| State::Available),
                sample: result.ok(),
            });
        }
        if config.pooler_stats
            && let Some((ns, cluster)) = selected_cluster
        {
            let pods: Api<Pod> = Api::namespaced(client.clone(), &ns);
            report.pooler_stats = match tokio::time::timeout(
                Duration::from_secs(5),
                pods.list(
                    &ListParams::default()
                        .labels(&format!("cnpg.io/cluster={cluster},cnpg.io/poolerName"))
                        .limit(2),
                ),
            )
            .await
            {
                Ok(Ok(list)) => {
                    if let Some(pod) = list.items.first() {
                        let cluster = selected
                            .as_ref()
                            .and_then(|raw| crate::adapters::cluster(raw.clone()).ok())
                            .and_then(|object| {
                                if let crate::model::Object::Cluster(cluster) = object {
                                    Some(cluster)
                                } else {
                                    None
                                }
                            });
                        let verified = if let Some(cluster) = cluster {
                            crate::pooler::verify_owner(
                                client.clone(),
                                pod.clone(),
                                cluster,
                                &config.budgets,
                            )
                            .await
                        } else {
                            Err(ErrorClass::InvalidData)
                        };
                        if let Ok(key) = verified {
                            crate::pooler::execute(
                                client.clone(),
                                &key,
                                &config.pooler_socket,
                                &config.budgets,
                                &CancellationToken::new(),
                            )
                            .await
                            .map_or_else(state, |_| State::Available)
                        } else {
                            State::Unknown
                        }
                    } else {
                        State::Empty
                    }
                }
                Ok(Err(e)) => state(probes::api_error(&e)),
                Err(_) => State::Unknown,
            };
        }
    }
    for cap in &mut report.capabilities {
        if !cap.requested {
            continue;
        }
        let missing: Vec<_> = report
            .access
            .iter()
            .filter(|a| {
                a.feature == cap.spec.feature
                    && matches!(
                        a.state,
                        State::Forbidden | State::Unknown | State::Absent | State::Unavailable
                    )
            })
            .collect();
        if cap.spec.implemented {
            let detected = match cap.spec.feature {
                Feature::Topology => report.cnpg_api,
                Feature::FastSql => report.postgres_role,
                Feature::PoolerStats => report.pooler_stats,
                Feature::Certificates | Feature::OperatorMetrics => {
                    let metrics = cap.spec.feature == Feature::OperatorMetrics;
                    let sources: Vec<_> = report
                        .auxiliary
                        .iter()
                        .filter(|r| matches!(r.source, crate::auxiliary::Key::Metrics) == metrics)
                        .collect();
                    if sources.iter().any(|r| r.state != State::Available) {
                        sources
                            .iter()
                            .find(|r| r.state != State::Available)
                            .map_or(State::Unknown, |r| r.state)
                    } else if sources.is_empty() {
                        State::Unknown
                    } else {
                        State::Available
                    }
                }
                Feature::MediumSql | Feature::DeepSql => {
                    let id = if cap.spec.feature == Feature::MediumSql {
                        "postgres18.medium.v1"
                    } else {
                        "postgres18.deep.v1"
                    };
                    report
                        .query_bundles
                        .iter()
                        .find(|b| b.id == id)
                        .map_or(report.postgres_role, |b| b.state)
                }
                Feature::Operator => report.operator.state,
                _ => {
                    let access: Vec<_> = report
                        .access
                        .iter()
                        .filter(|a| a.feature == cap.spec.feature)
                        .collect();
                    if access.iter().any(|a| a.state == State::Absent) {
                        State::Absent
                    } else if !access.is_empty()
                        && access
                            .iter()
                            .all(|a| matches!(a.state, State::Available | State::Empty))
                    {
                        if access
                            .iter()
                            .filter(|a| a.verb == "list")
                            .all(|a| a.state == State::Empty)
                            && access.iter().any(|a| a.verb == "list")
                        {
                            State::Empty
                        } else {
                            State::Available
                        }
                    } else {
                        State::Unknown
                    }
                }
            };
            cap.state = if missing.iter().any(|a| a.state == State::Forbidden) {
                "forbidden"
            } else {
                state_name(detected)
            };
            cap.availability = match cap.state {
                "available" | "empty" => Availability::Available,
                "forbidden" => Availability::Unavailable(ErrorClass::Forbidden),
                "unavailable" | "absent" => Availability::Unavailable(
                    if matches!(
                        cap.spec.feature,
                        Feature::FastSql | Feature::MediumSql | Feature::DeepSql
                    ) {
                        report.probe_error.unwrap_or(ErrorClass::Unavailable)
                    } else {
                        ErrorClass::Unavailable
                    },
                ),
                _ => Availability::Unknown,
            };
        }
        if !missing.is_empty() || !cap.spec.implemented || cap.state != "available" {
            let suggestion = if cap.spec.feature == Feature::OperatorMetrics {
                "Check the explicitly configured metrics endpoint, network reachability and TLS; Kubernetes credentials are never forwarded".into()
            } else if cap.spec.feature == Feature::PoolerStats {
                "cnpg-watch setup pooler-role --pooler YOUR_POOLER --dry-run".into()
            } else if cap.spec.feature == Feature::FastSql
                && !missing.iter().any(|a| a.state == State::Forbidden)
            {
                "cnpg-watch setup postgres-role --role-profile monitor --print-sql".into()
            } else {
                let scope = if config.all_namespaces {
                    "--all-namespaces".to_owned()
                } else {
                    format!("--namespace {namespace}")
                };
                let operator = report
                    .operator
                    .namespace
                    .as_ref()
                    .map(|ns| format!(" --operator-namespace {ns}"))
                    .unwrap_or_default();
                let profile = match config.profile {
                    Profile::Minimal => "minimal",
                    Profile::Standard => "standard",
                    Profile::Incident => "incident",
                    Profile::Secure => "secure",
                };
                let certificates = if cap.spec.feature == Feature::Certificates {
                    config
                        .certificate_secrets
                        .iter()
                        .map(|key| format!(" --certificate-secret {key}"))
                        .collect::<String>()
                } else {
                    String::new()
                };
                let cluster = if matches!(cap.spec.feature, Feature::ClusterPosture) {
                    " --cluster-resources"
                } else {
                    ""
                };
                format!(
                    "cnpg-watch {certificates}{cluster} setup kubernetes-rbac {scope}{operator} --profile {profile} --user YOUR_EXPLICIT_USER --dry-run"
                )
            };
            report.findings.push(Finding {
                feature: cap.spec.feature,
                state: if !cap.spec.implemented {
                    State::Unfinished
                } else if missing.iter().any(|a| a.state == State::Forbidden) {
                    State::Forbidden
                } else {
                    State::Unknown
                },
                message: format!(
                    "{}; {} permission checks need attention",
                    cap.spec.reduced_mode,
                    missing.len()
                ),
                suggestion: suggestion.clone(),
            });
            if suggestion.starts_with("cnpg-watch ") {
                report.setup_suggestions.push(suggestion);
            }
        }
    }
    if config.operator_watch && !matches!(report.operator.state, State::Available) {
        report.findings.push(Finding{feature:Feature::Operator,state:report.operator.state,message:format!("Operator discovery is {}; database observation is independent",state_name(report.operator.state)),suggestion:"cnpg-watch doctor --operator-namespace YOUR_OPERATOR_NAMESPACE --operator-selector YOUR_LABEL_SELECTOR".into()});
    }
    report.setup_suggestions.sort();
    report.setup_suggestions.dedup();
    report
}
pub fn state_name(state: State) -> &'static str {
    match state {
        State::Available => "available",
        State::Forbidden => "forbidden",
        State::Unknown => "unknown",
        State::Absent => "absent",
        State::Syncing => "syncing",
        State::Empty => "empty",
        State::DisabledByProfile => "disabled_by_profile",
        State::DisabledByConfig => "disabled_by_config",
        State::Unfinished => "unfinished",
        State::Ambiguous => "ambiguous",
        State::Unavailable => "unavailable",
    }
}
pub fn text(report: &Report) -> String {
    let mut text = format!(
        "cnpg-watch doctor (schema {})\nProfile: {:?}; namespace: {}\nCNPG API: {}; operator: {}; transport: {}; role: {}\n",
        report.schema_version,
        report.profile,
        report.namespace.as_deref().unwrap_or("all"),
        state_name(report.cnpg_api),
        state_name(report.operator.state),
        state_name(report.transport),
        state_name(report.postgres_role)
    );
    for cap in &report.capabilities {
        text.push_str(&format!(
            "{:?}: {} — {}\n",
            cap.spec.feature, cap.state, cap.spec.reduced_mode
        ));
    }
    for bundle in &report.query_bundles {
        text.push_str(&format!(
            "{}: {}{}\n",
            bundle.id,
            state_name(bundle.state),
            if bundle.selected { " (selected)" } else { "" }
        ));
    }
    text.push_str(&format!(
        "PgBouncer fixed SHOW console: {} (pgbouncer/local-peer)\n",
        state_name(report.pooler_stats)
    ));
    for source in &report.auxiliary {
        text.push_str(&format!(
            "{:?}: {} (public metadata only)\n",
            source.source,
            state_name(source.state)
        ));
    }
    for finding in &report.findings {
        text.push_str(&format!(
            "\n{:?}: {}\n  {}\n",
            finding.feature, finding.message, finding.suggestion
        ));
    }
    text
}
pub async fn run(
    config: &Config,
    output: Output,
    include_cluster_resources: bool,
) -> anyhow::Result<()> {
    let report = inspect_with(config, include_cluster_resources).await;
    match output {
        Output::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        Output::Text => print!("{}", text(&report)),
    };
    Ok(())
}
/// Snapshot of current accepted evidence; opening a TUI panel performs no I/O.
pub fn live_text(store: &Store, config: &Config, sql: Availability) -> String {
    let profile = config.profile;
    let caps = capabilities::observed_for(store, config, sql);
    let mut text = format!(
        "Profile {:?}\nRedaction {:?}; Secret reads require explicit named certificate configuration; query text requires incident opt-in; selected SQL only\n",
        profile,
        capabilities::envelope(profile).redaction
    );
    for cap in caps {
        text.push_str(&format!(
            "{:?}: {} ({:?})\n",
            cap.spec.feature, cap.state, cap.availability
        ));
    }
    for ((pod, class), observation) in &store.diagnostics {
        text.push_str(&format!("{pod} {class:?}: {:?}\n", observation.attempt));
        if let Some(sample) = &observation.last_good
            && let crate::probes::registry::Diagnostic::Capabilities(c) = &sample.value
        {
            text.push_str(&format!(
                "PG {} timeline {} recovery {} capability epoch {}\n",
                c.stamp.server_version,
                c.stamp.timeline,
                c.stamp.in_recovery,
                sample.ticket.topology.0
            ));
            for bundle in [
                crate::probes::registry::Bundle::Fast,
                crate::probes::registry::Bundle::Medium,
                crate::probes::registry::Bundle::Deep,
            ] {
                text.push_str(&format!(
                    "  {}: {}\n",
                    bundle.spec().id,
                    if c.allows(bundle) {
                        "supported"
                    } else {
                        "missing view/column/function grant"
                    }
                ));
            }
        }
    }
    for kind in store.watches.keys() {
        let s = store
            .watches
            .get(kind)
            .map(|w| w.status)
            .unwrap_or(SyncState::Syncing);
        text.push_str(&format!("{kind:?}: {s:?}\n"));
    }
    text.push_str("\nRun cnpg-watch doctor --output json for active preflight.\nReview RBAC: cnpg-watch setup kubernetes-rbac --namespace NAMESPACE --user YOUR_EXPLICIT_USER --dry-run\nReview grants: cnpg-watch setup postgres-role --print-sql\nNo setup actions are available in the TUI.\nd / p / Esc: close panel\n");
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Request, Response};
    use kube::client::Body;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    #[tokio::test]
    async fn ssar_failure_falls_back_only_to_safe_reads_and_preserves_unknown() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let captured = seen.clone();
        let service = tower::service_fn(move |request: Request<Body>| {
            captured
                .lock()
                .unwrap()
                .push((request.method().to_string(), request.uri().to_string()));
            let is_ssar = request.uri().path().ends_with("selfsubjectaccessreviews");
            async move {
                let value = if is_ssar {
                    json!({"kind":"Status","apiVersion":"v1","status":"Failure","reason":"Forbidden","code":403,"message":"ssar disabled"})
                } else {
                    json!({"apiVersion":"v1","kind":"PodList","metadata":{},"items":[]})
                };
                Ok::<_, std::convert::Infallible>(
                    Response::builder()
                        .status(if is_ssar { 403 } else { 200 })
                        .body(Body::from(serde_json::to_vec(&value).unwrap()))
                        .unwrap(),
                )
            }
        });
        let client = Client::new(service, "app-db");
        let pod = capabilities::CATALOG[0].kubernetes[1];
        let list = check(
            client.clone(),
            Feature::Topology,
            pod,
            "list",
            Some("app-db".into()),
        )
        .await;
        assert_eq!(list.state, State::Empty);
        let get = check(
            client.clone(),
            Feature::Topology,
            pod,
            "get",
            Some("app-db".into()),
        )
        .await;
        assert_eq!(get.state, State::Unknown);
        let exec = capabilities::CATALOG[1].kubernetes[0];
        let create = check(
            client,
            Feature::FastSql,
            exec,
            "create",
            Some("app-db".into()),
        )
        .await;
        assert_eq!(create.state, State::Unknown);
        assert!(seen.lock().unwrap().iter().all(|(method, path)| {
            method == "GET"
                || (method == "POST"
                    && path
                        .split('?')
                        .next()
                        .unwrap()
                        .ends_with("selfsubjectaccessreviews"))
        }));
    }
    #[tokio::test]
    async fn operator_ambiguity_never_selects_a_candidate() {
        let service = tower::service_fn(|_: Request<Body>| async {
            let value = json!({"apiVersion":"apps/v1","kind":"DeploymentList","metadata":{},"items":[{"metadata":{"namespace":"op-a","name":"one"}},{"metadata":{"namespace":"op-b","name":"two"}}]});
            Ok::<_, std::convert::Infallible>(Response::new(Body::from(
                serde_json::to_vec(&value).unwrap(),
            )))
        });
        let config = Config::resolve(
            crate::cli::Options::default(),
            crate::config::Paths::discover().unwrap(),
        )
        .unwrap();
        let op = operator(Client::new(service, "app-db"), &config).await;
        assert_eq!(op.state, State::Ambiguous);
        assert!(op.namespace.is_none());
        assert_eq!(op.candidates.len(), 2);
    }
}
