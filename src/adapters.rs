//! All dynamic field access lives at this boundary.
use crate::model::{
    Cluster, Controller, Identity, InvalidValue, Key, Object, OwnerKind, Pod, Role, Uid,
};
use k8s_openapi::{api::core::v1::Pod as KubePod, apimachinery::pkg::apis::meta::v1::ObjectMeta};
use kube::core::DynamicObject;
use serde::Deserialize;

pub(crate) fn identity(meta: &ObjectMeta) -> Result<Identity, InvalidValue> {
    Ok(Identity {
        key: Key::new(
            meta.namespace.clone().ok_or(InvalidValue)?,
            meta.name.clone().ok_or(InvalidValue)?,
        )?,
        uid: Uid::new(meta.uid.clone().ok_or(InvalidValue)?)?,
    })
}
pub fn cluster(raw: DynamicObject) -> Result<Object, InvalidValue> {
    let id = identity(&raw.metadata)?;
    let labels = labels(&raw.metadata)?;
    let field = |name: &str| -> Result<Option<String>, InvalidValue> {
        match raw.data.get("status").and_then(|s| s.get(name)) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(s)) => {
                bounded(Some(s.clone()), if name == "phase" { 1024 } else { 253 })
            }
            _ => Err(InvalidValue),
        }
    };
    let primary = field("currentPrimary")?;
    let phase = field("phase")?;
    let operational = match crate::operational::cluster(&raw.metadata, raw.data) {
        Ok((_, _, context)) => context,
        Err(_) => crate::operational::ClusterContext {
            availability: crate::model::Availability::Unavailable(
                crate::model::ErrorClass::InvalidData,
            ),
            ..Default::default()
        },
    };
    Ok(Object::Cluster(Cluster {
        id,
        primary,
        phase,
        labels,
        operational: Box::new(operational),
    }))
}
pub fn pod(raw: KubePod) -> Result<Object, InvalidValue> {
    let id = identity(&raw.metadata)?;
    let labels = labels(&raw.metadata)?;
    let cluster = labels
        .get("cnpg.io/cluster")
        .map(|v| Key::new(id.key.namespace.clone(), v.clone()))
        .transpose()?;
    let role = match labels.get("cnpg.io/instanceRole").map(String::as_str) {
        Some("primary") => Some(Role::Primary),
        Some("replica") => Some(Role::Replica),
        _ => None,
    };
    let cluster_uid = raw
        .metadata
        .owner_references
        .as_ref()
        .and_then(|r| {
            r.iter()
                .find(|r| r.kind == "Cluster" && r.api_version.starts_with("postgresql.cnpg.io/"))
        })
        .map(|r| Uid::new(&r.uid))
        .transpose()?;
    let backup_owner = raw.metadata.owner_references.as_ref().and_then(|refs| {
        refs.iter()
            .find(|r| r.kind == "Backup" && r.api_version.starts_with("postgresql.cnpg.io/"))
    });
    let backup = backup_owner
        .map(|r| r.name.as_str())
        .or_else(|| labels.get("cnpg.io/backupName").map(String::as_str))
        .map(|name| Key::new(&id.key.namespace, name))
        .transpose()?;
    let backup_uid = backup_owner.map(|r| Uid::new(&r.uid)).transpose()?;
    let controller = raw
        .metadata
        .owner_references
        .as_ref()
        .and_then(|owners| owners.iter().find(|r| r.controller == Some(true)))
        .and_then(|r| {
            let kind = match (
                r.api_version.split('/').next().unwrap_or(""),
                r.kind.as_str(),
            ) {
                ("apps", "ReplicaSet") => OwnerKind::ReplicaSet,
                ("apps", "Deployment") => OwnerKind::Deployment,
                ("postgresql.cnpg.io", "Cluster") => OwnerKind::Cluster,
                ("postgresql.cnpg.io", "Backup") => OwnerKind::Backup,
                ("batch", "Job") => OwnerKind::Job,
                _ => return None,
            };
            Some((r, kind))
        })
        .map(|(r, kind)| {
            Ok(Controller {
                kind,
                identity: Identity {
                    key: Key::new(&id.key.namespace, &r.name)?,
                    uid: Uid::new(&r.uid)?,
                },
            })
        })
        .transpose()?;
    let containers = raw
        .spec
        .as_ref()
        .map(|s| {
            s.containers
                .iter()
                .map(|c| c.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if containers.len() > 32 || containers.iter().any(|c| c.len() > 63) {
        return Err(InvalidValue);
    }
    let status = raw.status;
    let ready = status
        .as_ref()
        .and_then(|s| s.conditions.as_ref())
        .and_then(|cs| cs.iter().find(|c| c.type_ == "Ready"))
        .and_then(|c| match c.status.as_str() {
            "True" => Some(true),
            "False" => Some(false),
            _ => None,
        });
    let address = status
        .as_ref()
        .and_then(|s| s.pod_ip.as_ref())
        .and_then(|s| s.parse().ok());
    let restarts = status
        .and_then(|s| s.container_statuses)
        .unwrap_or_default()
        .into_iter()
        .try_fold(0u32, |total, c| {
            total
                .checked_add(c.restart_count.try_into().map_err(|_| InvalidValue)?)
                .ok_or(InvalidValue)
        })?;
    Ok(Object::Pod(Pod {
        id: id.clone(),
        cluster,
        role,
        ready,
        restarts,
        address,
        pooler: labels
            .get("cnpg.io/poolerName")
            .map(|v| Key::new(labels_namespace(&id), v))
            .transpose()?,
        labels,
        cluster_uid,
        backup,
        backup_uid,
        controller,
        containers,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixtures_are_normalized_without_leaking_raw_json() {
        let c = cluster(
            serde_json::from_str(include_str!("../fixtures/cnpg/1.30.0/cluster.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(c.id().key.name, "payments");
        let p =
            pod(serde_json::from_str(include_str!("../fixtures/cnpg/1.30.0/pod.json")).unwrap())
                .unwrap();
        assert!(matches!(
            p,
            Object::Pod(Pod {
                role: Some(Role::Primary),
                ready: Some(true),
                ..
            })
        ));
    }
    #[test]
    fn missing_optional_status_is_unknown_but_identity_is_required() {
        let mut raw: DynamicObject =
            serde_json::from_str(include_str!("../fixtures/cnpg/1.30.0/cluster.json")).unwrap();
        raw.data = serde_json::json!({});
        assert!(matches!(
            cluster(raw.clone()).unwrap(),
            Object::Cluster(Cluster { primary: None, .. })
        ));
        raw.metadata.uid = None;
        assert!(cluster(raw).is_err());
    }
}

fn labels_namespace(id: &Identity) -> String {
    id.key.namespace.clone()
}
pub fn labels(meta: &ObjectMeta) -> Result<crate::evidence::Labels, InvalidValue> {
    let labels = meta.labels.clone().unwrap_or_default();
    if labels.len() > 64
        || labels.iter().any(|(k, v)| {
            k.len() > 253 || v.len() > 253 || k.chars().chain(v.chars()).any(char::is_control)
        })
    {
        return Err(InvalidValue);
    }
    Ok(labels)
}
use crate::{
    evidence::{Association, Data, Endpoint, Evidence, Labels},
    model::{Kind, Tick},
};
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Ref {
    kind: Option<String>,
    name: Option<String>,
    namespace: Option<String>,
    uid: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawEvidence {
    spec: Spec,
    status: Status,
    #[serde(rename = "type")]
    type_: Option<String>,
    reason: Option<String>,
    message: Option<String>,
    count: Option<i32>,
    involved_object: Option<Ref>,
    last_timestamp: Option<String>,
    event_time: Option<String>,
    endpoints: Option<Vec<RawEndpoint>>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Spec {
    selector: Option<Selector>,
    instances: Option<u32>,
    cluster: Option<Ref>,
    #[serde(rename = "type")]
    type_: Option<String>,
    resources: Resources,
    pgbouncer: Pgbouncer,
    replicas: Option<u32>,
    template: Option<Template>,
    holder_identity: Option<String>,
    renew_time: Option<String>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Selector {
    Labels(Labels),
    Deployment {
        #[serde(rename = "matchLabels")]
        _match_labels: Labels,
    },
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Pgbouncer {
    pool_mode: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Resources {
    requests: Labels,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Template {
    spec: TemplateSpec,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct TemplateSpec {
    containers: Vec<Container>,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Container {
    image: Option<String>,
    env: Vec<Env>,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Env {
    name: String,
    value: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Status {
    phase: Option<String>,
    capacity: Labels,
    ready_instances: Option<u32>,
    unavailable_instances: Option<u32>,
    image: Option<String>,
    ready_replicas: Option<u32>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawEndpoint {
    target_ref: Option<Ref>,
    conditions: Conditions,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Conditions {
    ready: Option<bool>,
}
fn bounded(value: Option<String>, limit: usize) -> Result<Option<String>, InvalidValue> {
    if value.as_ref().is_some_and(|v| v.len() > limit) {
        Err(InvalidValue)
    } else {
        Ok(value.map(|s| {
            s.chars()
                .filter(|c| !c.is_control() || *c == '\t')
                .collect()
        }))
    }
}
fn time_ms(value: Option<&str>) -> Option<i64> {
    value
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|v| v.timestamp_millis())
}
pub(crate) fn association(meta: &ObjectMeta, labels: &Labels) -> Result<Association, InvalidValue> {
    let ns = meta.namespace.as_deref().ok_or(InvalidValue)?;
    let label = labels
        .get("cnpg.io/cluster")
        .map(|n| Key::new(ns, n))
        .transpose()?;
    let owners: Vec<_> = meta
        .owner_references
        .iter()
        .flatten()
        .filter(|o| o.kind == "Cluster" && o.api_version.starts_with("postgresql.cnpg.io/"))
        .collect();
    if owners.len() > 1 {
        return Ok(Association::Conflict);
    }
    if let Some(owner) = owners.first() {
        let key = Key::new(ns, &owner.name)?;
        if label.as_ref().is_some_and(|l| l != &key) {
            return Ok(Association::Conflict);
        }
        return Ok(Association::Owner {
            key,
            uid: Uid::new(&owner.uid)?,
        });
    }
    Ok(label.map_or(Association::Unrelated, Association::Label))
}
fn target(reference: Ref, namespace: &str) -> Result<Option<Identity>, InvalidValue> {
    match (reference.name, reference.uid) {
        (Some(name), Some(uid)) => Ok(Some(Identity {
            key: Key::new(reference.namespace.as_deref().unwrap_or(namespace), name)?,
            uid: Uid::new(uid)?,
        })),
        _ => Ok(None),
    }
}
fn evidence(raw: DynamicObject, kind: Kind) -> Result<Object, InvalidValue> {
    let id = identity(&raw.metadata)?;
    let labels = labels(&raw.metadata)?;
    let mut association = association(&raw.metadata, &labels)?;
    let reconciliation = if matches!(kind, Kind::Deployment | Kind::PoolerDeployment) {
        Some(crate::operational::reconciliation(
            &raw.metadata,
            raw.data
                .get("status")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        )?)
    } else {
        None
    };
    let data: RawEvidence = serde_json::from_value(raw.data).map_err(|_| InvalidValue)?;
    let body = match kind {
        Kind::Event | Kind::OperatorEvent => {
            let involved_kind = data
                .involved_object
                .as_ref()
                .and_then(|r| r.kind.as_deref())
                .and_then(|kind| match kind {
                    "Cluster" => Some(Kind::Cluster),
                    "Pod" => Some(Kind::Pod),
                    "Pooler" => Some(Kind::Pooler),
                    "Backup" => Some(Kind::Backup),
                    "ScheduledBackup" => Some(Kind::ScheduledBackup),
                    "Database" => Some(Kind::Database),
                    "DatabaseRole" => Some(Kind::DatabaseRole),
                    "Publication" => Some(Kind::Publication),
                    "Subscription" => Some(Kind::Subscription),
                    "PersistentVolumeClaim" => Some(Kind::Pvc),
                    "Service" => Some(Kind::Service),
                    _ => None,
                });
            let involved = data
                .involved_object
                .map(|r| target(r, &id.key.namespace))
                .transpose()?
                .flatten();
            Data::Event {
                warning: data.type_.as_deref() == Some("Warning"),
                reason: bounded(data.reason, 253)?.unwrap_or_default(),
                message: bounded(data.message, 4096)?.unwrap_or_default(),
                involved,
                involved_kind,
                last_seen_ms: time_ms(
                    data.last_timestamp
                        .as_deref()
                        .or(data.event_time.as_deref()),
                )
                .or_else(|| {
                    raw.metadata
                        .creation_timestamp
                        .as_ref()
                        .map(|t| t.0.as_millisecond())
                }),
                count: data
                    .count
                    .unwrap_or(1)
                    .try_into()
                    .map_err(|_| InvalidValue)?,
            }
        }
        Kind::Pvc => Data::Pvc {
            phase: bounded(data.status.phase, 128)?,
            requested: data.spec.resources.requests.get("storage").cloned(),
            capacity: data.status.capacity.get("storage").cloned(),
            reported_state: bounded(
                raw.metadata
                    .annotations
                    .as_ref()
                    .and_then(|a| a.get("cnpg.io/pvcStatus").cloned()),
                128,
            )?,
        },
        Kind::Service | Kind::OperatorService => {
            let selector = match data.spec.selector {
                Some(Selector::Labels(v)) => v,
                _ => Labels::new(),
            };
            if selector.len() > 64 || selector.iter().any(|(k, v)| k.len() > 253 || v.len() > 253) {
                return Err(InvalidValue);
            }
            if matches!(association, Association::Unrelated)
                && let Some(cluster) = selector.get("cnpg.io/cluster")
            {
                association = Association::Label(Key::new(&id.key.namespace, cluster)?);
            }
            Data::Service {
                read_write: selector
                    .get("cnpg.io/instanceRole")
                    .is_some_and(|v| v == "primary"),
                selector,
            }
        }
        Kind::EndpointSlice | Kind::OperatorEndpointSlice => {
            let raw_endpoints = data.endpoints.unwrap_or_default();
            if raw_endpoints.len() > 1000 {
                return Err(InvalidValue);
            }
            let service = Key::new(
                &id.key.namespace,
                labels
                    .get("kubernetes.io/service-name")
                    .ok_or(InvalidValue)?,
            )?;
            let endpoints = raw_endpoints
                .into_iter()
                .map(|e| {
                    Ok(Endpoint {
                        target: e
                            .target_ref
                            .map(|r| target(r, &id.key.namespace))
                            .transpose()?
                            .flatten(),
                        ready: e.conditions.ready,
                    })
                })
                .collect::<Result<_, InvalidValue>>()?;
            Data::Endpoints { service, endpoints }
        }
        Kind::Pooler => {
            if let Some(cluster) = data.spec.cluster.and_then(|c| c.name) {
                let key = Key::new(&id.key.namespace, cluster)?;
                if association_key(&association).is_some_and(|k| k != &key) {
                    association = Association::Conflict;
                } else if matches!(association, Association::Unrelated) {
                    association = Association::Label(key);
                }
            }
            Data::Pooler {
                desired: data.spec.instances,
                ready: data.status.ready_instances,
                type_: bounded(data.spec.type_, 128)?,
                mode: bounded(data.spec.pgbouncer.pool_mode, 128)?,
                unavailable: data.status.unavailable_instances,
                phase: bounded(data.status.phase, 128)?,
                image: bounded(
                    data.status.image.or_else(|| {
                        data.spec
                            .template
                            .and_then(|t| t.spec.containers.into_iter().find_map(|c| c.image))
                    }),
                    512,
                )?,
            }
        }
        Kind::Deployment | Kind::PoolerDeployment => {
            let containers = data.spec.template.unwrap_or_default().spec.containers;
            if containers.len() > 32 {
                return Err(InvalidValue);
            }
            let watch = containers
                .iter()
                .flat_map(|c| c.env.iter())
                .find(|e| e.name == "WATCH_NAMESPACE")
                .and_then(|e| e.value.clone());
            let images = containers
                .into_iter()
                .map(|c| bounded(c.image, 512).map(|v| v.unwrap_or_default()))
                .collect::<Result<_, _>>()?;
            Data::Deployment {
                desired: data.spec.replicas,
                ready: data.status.ready_replicas,
                images,
                watch_namespaces: bounded(watch, 1024)?,
                reconciliation: reconciliation.unwrap_or_default(),
                pooler_owner: raw
                    .metadata
                    .owner_references
                    .as_ref()
                    .and_then(|refs| {
                        refs.iter().find(|r| {
                            r.kind == "Pooler" && r.api_version.starts_with("postgresql.cnpg.io/")
                        })
                    })
                    .map(|r| {
                        Ok(Identity {
                            key: Key::new(&id.key.namespace, &r.name)?,
                            uid: Uid::new(&r.uid)?,
                        })
                    })
                    .transpose()?,
            }
        }
        Kind::PoolerReplicaSet => Data::ReplicaSet {
            deployment: raw
                .metadata
                .owner_references
                .as_ref()
                .and_then(|refs| {
                    refs.iter()
                        .find(|r| r.kind == "Deployment" && r.api_version.starts_with("apps/"))
                })
                .map(|r| {
                    Ok(Identity {
                        key: Key::new(&id.key.namespace, &r.name)?,
                        uid: Uid::new(&r.uid)?,
                    })
                })
                .transpose()?,
        },
        Kind::Lease => Data::Lease {
            holder: bounded(data.spec.holder_identity, 253)?,
            renewed_ms: time_ms(data.spec.renew_time.as_deref()),
        },
        _ => return Err(InvalidValue),
    };
    Ok(Object::Evidence(Evidence {
        id,
        kind,
        labels,
        association,
        data: body,
        observed_at: Tick(0),
    }))
}
fn association_key(value: &Association) -> Option<&Key> {
    match value {
        Association::Label(k) | Association::Owner { key: k, .. } => Some(k),
        _ => None,
    }
}
macro_rules! adapter {($($name:ident => $kind:ident),+)=>{$(pub fn $name(raw:DynamicObject)->Result<Object,InvalidValue>{evidence(raw,Kind::$kind)})+};}
adapter!(event=>Event,operator_event=>OperatorEvent,pvc=>Pvc,service=>Service,endpoints=>EndpointSlice,pooler=>Pooler,deployment=>Deployment,lease=>Lease,operator_service=>OperatorService,operator_endpoints=>OperatorEndpointSlice,pooler_deployment=>PoolerDeployment,pooler_replicaset=>PoolerReplicaSet);
macro_rules! operational_adapter {($($name:ident => $kind:ident),+)=>{$(pub fn $name(raw:DynamicObject)->Result<Object,InvalidValue>{crate::operational::adapt(raw,Kind::$kind)})+};}
operational_adapter!(backup=>Backup,scheduled_backup=>ScheduledBackup,database=>Database,database_role=>DatabaseRole,publication=>Publication,subscription=>Subscription,quorum=>FailoverQuorum,image_catalog=>ImageCatalog,cluster_image_catalog=>ClusterImageCatalog,validating_webhook=>ValidatingWebhook,mutating_webhook=>MutatingWebhook);
pub fn operator_pod(raw: DynamicObject) -> Result<Object, InvalidValue> {
    let raw: k8s_openapi::api::core::v1::Pod =
        serde_json::from_value(serde_json::to_value(raw).map_err(|_| InvalidValue)?)
            .map_err(|_| InvalidValue)?;
    let containers = raw
        .spec
        .as_ref()
        .map(|s| {
            s.containers
                .iter()
                .map(|c| c.name.clone())
                .take(32)
                .collect()
        })
        .unwrap_or_default();
    let id = identity(&raw.metadata)?;
    let labels = labels(&raw.metadata)?;
    let Object::Pod(p) = pod(raw)? else {
        unreachable!()
    };
    Ok(Object::Evidence(Evidence {
        id,
        kind: Kind::OperatorPod,
        labels,
        association: Association::Unrelated,
        data: Data::OperatorPod {
            ready: p.ready,
            containers,
        },
        observed_at: Tick(0),
    }))
}
