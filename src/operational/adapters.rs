//! Explicit allowlists for installed CNPG resource schemas.
use super::*;
use crate::{
    adapters,
    evidence::{Association, Data, Evidence},
    model::*,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::DynamicObject;
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Ref {
    name: Option<String>,
    kind: Option<String>,
    major: Option<u32>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawCondition {
    #[serde(rename = "type")]
    type_: String,
    status: String,
    reason: Option<String>,
    observed_generation: Option<i64>,
    last_transition_time: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawPlugin {
    name: String,
    version: Option<String>,
    enabled: Option<bool>,
    is_w_a_l_archiver: Option<bool>,
    capabilities: Option<Vec<String>>,
    operator_capabilities: Option<Vec<String>>,
    wal_capabilities: Option<Vec<String>>,
    backup_capabilities: Option<Vec<String>>,
    restore_job_hook_capabilities: Option<Vec<String>>,
    status: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawReplica {
    enabled: Option<bool>,
    source: Option<String>,
    min_apply_delay: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Synchronous {
    method: Option<String>,
    number: Option<u32>,
    data_durability: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Postgres {
    synchronous: Synchronous,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct BackupConfig {
    retention_policy: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct CatalogImage {
    major: u32,
    image: String,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawSpec {
    cluster: Option<Ref>,
    name: Option<String>,
    dbname: Option<String>,
    method: Option<String>,
    schedule: Option<String>,
    suspend: Option<bool>,
    replica: RawReplica,
    external_clusters: Option<Vec<Ref>>,
    plugins: Option<Vec<RawPlugin>>,
    backup: BackupConfig,
    postgresql: Postgres,
    image_catalog_ref: CatalogRefRaw,
    images: Option<Vec<CatalogImage>>,
    component_images: Option<Vec<serde::de::IgnoredAny>>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct CatalogRefRaw {
    name: Option<String>,
    kind: Option<String>,
    major: Option<u32>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Instance {
    pod_name: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Certificates {
    server_t_l_s_secret: Option<String>,
    client_c_a_secret: Option<String>,
    server_c_a_secret: Option<String>,
    replication_t_l_s_secret: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawStatus {
    current_primary: Option<String>,
    target_primary: Option<String>,
    phase: Option<String>,
    method: Option<String>,
    started_at: Option<String>,
    stopped_at: Option<String>,
    instance_i_d: Option<Instance>,
    error: Option<String>,
    command_error: Option<String>,
    last_check_time: Option<String>,
    last_schedule_time: Option<String>,
    next_schedule_time: Option<String>,
    applied: Option<bool>,
    observed_generation: Option<i64>,
    message: Option<String>,
    conditions: Option<Vec<RawCondition>>,
    primary: Option<String>,
    standby_names: Option<Vec<String>>,
    standby_number: Option<u32>,
    image: Option<String>,
    ready_instances: Option<u32>,
    instances: Option<u32>,
    cloud_native_p_g_operator_hash: Option<String>,
    last_successful_backup: Option<String>,
    last_failed_backup: Option<String>,
    first_recoverability_point: Option<String>,
    dangling_p_v_c: Option<Vec<String>>,
    unusable_p_v_c: Option<Vec<String>>,
    resizing_p_v_c: Option<Vec<String>>,
    initializing_p_v_c: Option<Vec<String>>,
    healthy_p_v_c: Option<Vec<String>>,
    plugin_status: Option<Vec<RawPlugin>>,
    certificates: Certificates,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ServiceRef {
    name: String,
    namespace: String,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ClientConfig {
    service: Option<ServiceRef>,
    url: Option<String>,
    ca_bundle: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Webhook {
    rules: Option<Vec<ApiRule>>,
    client_config: ClientConfig,
    failure_policy: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ApiRule {
    api_groups: Option<Vec<String>>,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Raw {
    spec: RawSpec,
    status: RawStatus,
    webhooks: Option<Vec<Webhook>>,
}

fn bounded(value: Option<String>, limit: usize) -> Result<Option<String>, InvalidValue> {
    if value
        .as_ref()
        .is_some_and(|v| v.len() > limit || v.chars().any(char::is_control))
    {
        return Err(InvalidValue);
    }
    Ok(value)
}
fn names(values: Option<Vec<String>>, limit: usize) -> Result<Vec<String>, InvalidValue> {
    let values = values.unwrap_or_default();
    if values.len() > limit {
        return Err(InvalidValue);
    }
    values
        .into_iter()
        .map(|v| bounded(Some(v), 253).map(Option::unwrap))
        .collect()
}
fn time(value: Option<String>) -> Result<Option<i64>, InvalidValue> {
    value
        .map(|v| {
            chrono::DateTime::parse_from_rfc3339(&v)
                .map(|v| v.timestamp_millis())
                .map_err(|_| InvalidValue)
        })
        .transpose()
}
fn conditions(values: Option<Vec<RawCondition>>) -> Result<Vec<Condition>, InvalidValue> {
    let values = values.unwrap_or_default();
    if values.len() > 32 {
        return Err(InvalidValue);
    }
    values
        .into_iter()
        .map(|v| {
            Ok(Condition {
                type_: bounded(Some(v.type_), 128)?.unwrap(),
                status: bounded(Some(v.status), 16)?.unwrap(),
                reason: bounded(v.reason, 128)?,
                observed_generation: v.observed_generation,
                changed_ms: time(v.last_transition_time)?,
            })
        })
        .collect()
}
pub fn reconciliation(
    meta: &ObjectMeta,
    status: serde_json::Value,
) -> Result<Reconciliation, InvalidValue> {
    let raw: RawStatus = serde_json::from_value(status).map_err(|_| InvalidValue)?;
    let mut result = reconcile(meta, &raw)?;
    result.conditions = conditions(raw.conditions)?;
    Ok(result)
}
fn reconcile(meta: &ObjectMeta, status: &RawStatus) -> Result<Reconciliation, InvalidValue> {
    if meta.generation.is_some_and(|g| g < 0) || status.observed_generation.is_some_and(|g| g < 0) {
        return Err(InvalidValue);
    }
    Ok(Reconciliation {
        generation: meta.generation,
        observed_generation: status.observed_generation,
        applied: status.applied,
        conditions: vec![],
        message_present: status.message.as_ref().is_some_and(|s| !s.is_empty()),
    })
}
pub fn cluster(
    meta: &ObjectMeta,
    data: serde_json::Value,
) -> Result<(Option<String>, Option<String>, ClusterContext), InvalidValue> {
    let raw: Raw = serde_json::from_value(data).map_err(|_| InvalidValue)?;
    let Raw { spec, status, .. } = raw;
    let mut plugins = Vec::new();
    let configured = spec.plugins.unwrap_or_default();
    let reported = status.plugin_status.unwrap_or_default();
    if configured.len() > 32 || reported.len() > 32 {
        return Err(InvalidValue);
    }
    for (configured, raw) in configured
        .into_iter()
        .map(|p| (true, p))
        .chain(reported.into_iter().map(|p| (false, p)))
    {
        let name = bounded(Some(raw.name), 253)?.unwrap();
        let version = bounded(raw.version, 128)?;
        let mut capabilities = names(raw.capabilities, 64)?;
        for (prefix, values) in [
            ("operator", raw.operator_capabilities),
            ("wal", raw.wal_capabilities),
            ("backup", raw.backup_capabilities),
            ("restore", raw.restore_job_hook_capabilities),
        ] {
            for value in names(values, 64)? {
                capabilities.push(format!("{prefix}:{value}"));
            }
        }
        if capabilities.len() > 128 {
            return Err(InvalidValue);
        }
        if let Some(existing) = plugins.iter_mut().find(|p: &&mut Plugin| p.name == name) {
            existing.version = version.or(existing.version.take());
            if existing
                .capabilities
                .len()
                .saturating_add(capabilities.len())
                > 128
            {
                return Err(InvalidValue);
            }
            existing.capabilities.extend(capabilities);
            existing.reported_status |= raw.status.is_some();
        } else {
            plugins.push(Plugin {
                name,
                version,
                configured,
                enabled: raw.enabled,
                archiver: raw.is_w_a_l_archiver,
                capabilities,
                reported_status: raw.status.is_some(),
            });
        }
    }
    let mut secrets = Vec::new();
    let cert = status.certificates;
    for value in [
        cert.server_t_l_s_secret,
        cert.client_c_a_secret,
        cert.server_c_a_secret,
        cert.replication_t_l_s_secret,
    ]
    .into_iter()
    .flatten()
    {
        Key::new(meta.namespace.as_deref().ok_or(InvalidValue)?, &value)?;
        secrets.push(value);
    }
    secrets.sort();
    secrets.dedup();
    let replica = Replica {
        enabled: spec.replica.enabled,
        source: bounded(spec.replica.source, 253)?,
        delay: bounded(spec.replica.min_apply_delay, 128)?,
        external_names: names(
            spec.external_clusters
                .map(|v| v.into_iter().filter_map(|r| r.name).collect()),
            64,
        )?,
        sync_method: bounded(spec.postgresql.synchronous.method, 64)?,
        sync_number: spec.postgresql.synchronous.number,
        durability: bounded(spec.postgresql.synchronous.data_durability, 64)?,
    };
    let context = ClusterContext {
        availability: Availability::Available,
        created_ms: meta
            .creation_timestamp
            .as_ref()
            .map(|t| t.0.as_millisecond()),
        generation: meta.generation,
        conditions: conditions(status.conditions)?,
        target_primary: bounded(status.target_primary, 253)?,
        image: bounded(status.image, 512)?,
        operator_version: bounded(
            meta.annotations
                .as_ref()
                .and_then(|a| a.get("cnpg.io/operatorVersion").cloned()),
            64,
        )?,
        operator_hash: bounded(status.cloud_native_p_g_operator_hash, 128)?,
        ready: status.ready_instances,
        desired: status.instances,
        retention: bounded(spec.backup.retention_policy, 64)?,
        latest_success_ms: time(status.last_successful_backup)?,
        latest_failure_ms: time(status.last_failed_backup)?,
        recovery_point_ms: time(status.first_recoverability_point)?,
        dangling: names(status.dangling_p_v_c, 1024)?,
        unusable: names(status.unusable_p_v_c, 1024)?,
        resizing: names(status.resizing_p_v_c, 1024)?,
        initializing: names(status.initializing_p_v_c, 1024)?,
        healthy: names(status.healthy_p_v_c, 1024)?,
        replica,
        plugins,
        catalog: CatalogRef {
            name: bounded(spec.image_catalog_ref.name, 253)?,
            kind: bounded(spec.image_catalog_ref.kind, 64)?,
            major: spec.image_catalog_ref.major,
        },
        certificate_secrets: secrets,
    };
    Ok((
        bounded(status.current_primary, 253)?,
        bounded(status.phase, 1024)?,
        context,
    ))
}
pub fn adapt(mut raw: DynamicObject, kind: Kind) -> Result<Object, InvalidValue> {
    let global = matches!(
        kind,
        Kind::ClusterImageCatalog | Kind::ValidatingWebhook | Kind::MutatingWebhook
    );
    if global {
        raw.metadata.namespace = Some("cluster-scope".into());
    }
    let id = adapters::identity(&raw.metadata)?;
    let labels = adapters::labels(&raw.metadata)?;
    let mut association = adapters::association(&raw.metadata, &labels)?;
    let data: Raw = serde_json::from_value(raw.data).map_err(|_| InvalidValue)?;
    if let Some(name) = data.spec.cluster.and_then(|r| r.name) {
        let key = Key::new(&id.key.namespace, name)?;
        let conflict = match &association {
            Association::Label(old) | Association::Owner { key: old, .. } => *old != key,
            Association::Conflict => true,
            Association::Unrelated => false,
        };
        if conflict {
            association = Association::Conflict;
        } else if matches!(association, Association::Unrelated) {
            association = Association::Label(key);
        }
    }
    if kind == Kind::FailoverQuorum && matches!(association, Association::Unrelated) {
        association = Association::Label(id.key.clone());
    }
    let status = data.status;
    let body = match kind {
        Kind::Backup => Resource::Backup {
            phase: bounded(status.phase, 64)?,
            method: bounded(status.method.or(data.spec.method), 64)?,
            started_ms: time(status.started_at)?,
            stopped_ms: time(status.stopped_at)?,
            instance: bounded(status.instance_i_d.and_then(|i| i.pod_name), 253)?,
            failure_reported: status.error.as_ref().is_some_and(|v| !v.is_empty())
                || status.command_error.as_ref().is_some_and(|v| !v.is_empty()),
        },
        Kind::ScheduledBackup => Resource::Schedule {
            schedule: bounded(data.spec.schedule, 256)?,
            suspended: data.spec.suspend,
            method: bounded(data.spec.method, 64)?,
            last_check_ms: time(status.last_check_time)?,
            last_run_ms: time(status.last_schedule_time)?,
            next_run_ms: time(status.next_schedule_time)?,
            failure_reported: status.error.as_ref().is_some_and(|v| !v.is_empty()),
        },
        Kind::Database | Kind::DatabaseRole | Kind::Publication | Kind::Subscription => {
            let mut reconciliation = reconcile(&raw.metadata, &status)?;
            reconciliation.conditions = conditions(status.conditions)?;
            Resource::Managed {
                name: bounded(data.spec.name, 253)?,
                database: bounded(data.spec.dbname, 253)?,
                reconciliation,
            }
        }
        Kind::FailoverQuorum => Resource::Quorum {
            primary: bounded(status.primary, 253)?,
            method: bounded(status.method, 64)?,
            standby_names: names(status.standby_names, 1024)?,
            standby_number: status.standby_number,
        },
        Kind::ImageCatalog | Kind::ClusterImageCatalog => {
            let images = data.spec.images.unwrap_or_default();
            let components = data.spec.component_images.unwrap_or_default();
            if images.len() > 64 || components.len() > 64 {
                return Err(InvalidValue);
            }
            Resource::Catalog {
                images: images
                    .into_iter()
                    .map(|v| Ok((v.major, bounded(Some(v.image), 512)?.unwrap())))
                    .collect::<Result<_, InvalidValue>>()?,
                component_entries: components.len(),
                global,
            }
        }
        Kind::ValidatingWebhook | Kind::MutatingWebhook => {
            let hooks = data.webhooks.unwrap_or_default();
            if hooks.len() > 64 {
                return Err(InvalidValue);
            }
            let mut services = Vec::new();
            let mut relevant = 0;
            let mut policies = Vec::new();
            let mut ca = true;
            let mut external = 0;
            for hook in hooks {
                let rules = hook.rules.unwrap_or_default();
                if rules.len() > 64 {
                    return Err(InvalidValue);
                }
                if !rules.iter().any(|r| {
                    r.api_groups
                        .as_ref()
                        .is_some_and(|g| g.iter().any(|g| g == "postgresql.cnpg.io" || g == "*"))
                }) {
                    continue;
                }
                relevant += 1;
                if let Some(s) = hook.client_config.service {
                    services.push(Key::new(s.namespace, s.name)?);
                } else if hook.client_config.url.is_some() {
                    external += 1;
                }
                ca &= hook
                    .client_config
                    .ca_bundle
                    .as_ref()
                    .is_some_and(|b| !b.is_empty());
                if let Some(p) = bounded(hook.failure_policy, 32)? {
                    policies.push(p);
                }
            }
            Resource::Webhook {
                rules: relevant,
                services,
                failure_policies: policies,
                ca_present: ca,
                external_clients: external,
            }
        }
        _ => return Err(InvalidValue),
    };
    Ok(Object::Evidence(Evidence {
        id,
        kind,
        labels,
        association,
        data: Data::Operational(Box::new(body)),
        observed_at: Tick(0),
    }))
}
