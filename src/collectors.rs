use crate::{
    adapters,
    budget::Budgets,
    model::*,
    queue::Sender,
    reducer::{WatchEvent, WatchUpdate},
};
use futures::{Stream, StreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api, Client, Resource,
    api::DynamicObject,
    core::{ApiResource, GroupVersionKind},
    runtime::{WatchStreamExt, watcher},
};
use std::{fmt::Debug, time::Duration};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub struct WatchInputs {
    pub client: Client,
    pub namespace: String,
    pub all: bool,
    pub cluster: Option<String>,
    pub scope: ScopeEpoch,
    pub budgets: Budgets,
}

pub fn spawn(
    tasks: &mut JoinSet<()>,
    inputs: WatchInputs,
    tx: Sender<WatchEvent>,
    cancel: CancellationToken,
) {
    let WatchInputs {
        client,
        namespace,
        all,
        cluster,
        scope,
        budgets,
    } = inputs;
    let cluster = cluster.as_deref();
    let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(
        "postgresql.cnpg.io",
        "v1",
        "Cluster",
    ));
    let clusters: Api<DynamicObject> = if all {
        Api::all_with(client.clone(), &resource)
    } else {
        Api::namespaced_with(client.clone(), &namespace, &resource)
    };
    let pods: Api<Pod> = if all {
        Api::all(client)
    } else {
        Api::namespaced(client, &namespace)
    };
    let mut cluster_config = watcher::Config::default().timeout(30);
    if let Some(name) = cluster {
        cluster_config = cluster_config.fields(&format!("metadata.name={name}"));
    }
    let selector = cluster.map_or_else(
        || "cnpg.io/podRole=instance,cnpg.io/cluster".into(),
        |name| format!("cnpg.io/podRole=instance,cnpg.io/cluster={name}"),
    );
    let pod_config = watcher::Config::default().timeout(30).labels(&selector);
    tasks.spawn(collect(
        clusters,
        cluster_config,
        adapters::cluster,
        Kind::Cluster,
        scope,
        tx.clone(),
        cancel.clone(),
        budgets.clone(),
    ));
    tasks.spawn(collect(
        pods,
        pod_config,
        adapters::pod,
        Kind::Pod,
        scope,
        tx,
        cancel,
        budgets,
    ));
}

fn classify(error: &watcher::Error) -> ErrorClass {
    match error {
        watcher::Error::InitialListFailed(e)
        | watcher::Error::WatchStartFailed(e)
        | watcher::Error::WatchFailed(e) => crate::probes::api_error(e),
        watcher::Error::WatchError(e) if e.code == 403 => ErrorClass::Forbidden,
        watcher::Error::WatchError(e) if e.code == 404 => ErrorClass::Unavailable,
        _ => ErrorClass::Transport,
    }
}

#[allow(clippy::too_many_arguments)]
async fn collect<K>(
    api: Api<K>,
    config: watcher::Config,
    adapt: fn(K) -> Result<Object, InvalidValue>,
    kind: Kind,
    scope: ScopeEpoch,
    tx: Sender<WatchEvent>,
    cancel: CancellationToken,
    budgets: Budgets,
) where
    K: Resource + Clone + serde::de::DeserializeOwned + Debug + Send + 'static,
{
    let mut epoch = WatchEpoch(0);
    loop {
        let stream = watcher(api.clone(), config.clone()).default_backoff();
        let outcome = consume(stream, adapt, kind, scope, epoch, &tx, &cancel, &budgets).await;
        if cancel.is_cancelled()
            || matches!(
                outcome,
                ErrorClass::Forbidden
                    | ErrorClass::Unavailable
                    | ErrorClass::Capacity
                    | ErrorClass::Cancelled
            )
        {
            return;
        }
        tokio::select! {_=cancel.cancelled()=>return,_=tokio::time::sleep(Duration::from_secs(2))=>{}}
        epoch.0 = epoch.0.checked_add(1).expect("watch epoch exhausted");
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn consume<K: Resource>(
    stream: impl Stream<Item = Result<watcher::Event<K>, watcher::Error>>,
    adapt: fn(K) -> Result<Object, InvalidValue>,
    kind: Kind,
    scope: ScopeEpoch,
    watch: WatchEpoch,
    tx: &Sender<WatchEvent>,
    cancel: &CancellationToken,
    budgets: &Budgets,
) -> ErrorClass {
    futures::pin_mut!(stream);
    let mut syncing =
        Some(tokio::time::Instant::now() + Duration::from_millis(budgets.watch_sync_ms));
    loop {
        let event = tokio::select! {
            _=cancel.cancelled()=>return ErrorClass::Cancelled,
            _=async {if let Some(deadline)=syncing {tokio::time::sleep_until(deadline).await}else{std::future::pending::<()>().await}}=>{
                let _=tx.send(WatchEvent{scope,watch,kind,update:WatchUpdate::Error(ErrorClass::Timeout)},256,cancel).await;
                return ErrorClass::Timeout;
            },
            event=stream.next()=>event,
        };
        let Some(event) = event else {
            let _ = tx
                .send(
                    WatchEvent {
                        scope,
                        watch,
                        kind,
                        update: WatchUpdate::Error(ErrorClass::Transport),
                    },
                    256,
                    cancel,
                )
                .await;
            return ErrorClass::Transport;
        };
        let update = match event {
            Ok(event) => match normalize(event, adapt, budgets.object_bytes, kind) {
                Ok(Some(update)) => update,
                Ok(None) => continue,
                Err(class) => {
                    let _ = tx
                        .send(
                            WatchEvent {
                                scope,
                                watch,
                                kind,
                                update: WatchUpdate::Error(class),
                            },
                            256,
                            cancel,
                        )
                        .await;
                    return class;
                }
            },
            Err(error) => {
                let class = classify(&error);
                if tx
                    .send(
                        WatchEvent {
                            scope,
                            watch,
                            kind,
                            update: WatchUpdate::Error(class),
                        },
                        256,
                        cancel,
                    )
                    .await
                    .is_err()
                {
                    return ErrorClass::Cancelled;
                }
                // Recreate the stream so an interrupted list cannot finish as a partial set.
                return class;
            }
        };
        match &update {
            WatchUpdate::Init => {
                syncing =
                    Some(tokio::time::Instant::now() + Duration::from_millis(budgets.watch_sync_ms))
            }
            WatchUpdate::InitDone => syncing = None,
            _ => {}
        }
        let bytes = match &update {
            WatchUpdate::Apply(o) | WatchUpdate::InitApply(o) => o.bytes() + 256,
            _ => 512,
        };
        if tx
            .send(
                WatchEvent {
                    scope,
                    watch,
                    kind,
                    update,
                },
                bytes,
                cancel,
            )
            .await
            .is_err()
        {
            return ErrorClass::Cancelled;
        }
    }
}

fn normalize<K: Resource>(
    event: watcher::Event<K>,
    adapt: fn(K) -> Result<Object, InvalidValue>,
    limit: usize,
    kind: Kind,
) -> Result<Option<WatchUpdate>, ErrorClass> {
    let object = |raw| {
        let value = adapt(raw).map_err(|_| ErrorClass::InvalidData)?;
        if value.bytes() > limit {
            return Err(ErrorClass::Capacity);
        }
        Ok(value)
    };
    let relevant = |object: &Object| !matches!(object,Object::Evidence(e) if matches!(&e.data,crate::evidence::Data::Operational(value) if matches!(value.as_ref(),crate::operational::Resource::Webhook{rules:0,..})));
    let update = match event {
        watcher::Event::Init => Some(WatchUpdate::Init),
        watcher::Event::InitDone => Some(WatchUpdate::InitDone),
        watcher::Event::Apply(raw) => {
            let value = object(raw)?;
            Some(if relevant(&value) {
                WatchUpdate::Apply(value)
            } else {
                WatchUpdate::Delete(value.id().clone())
            })
        }
        watcher::Event::InitApply(raw) => {
            let value = object(raw)?;
            relevant(&value).then_some(WatchUpdate::InitApply(value))
        }
        watcher::Event::Delete(raw) => {
            let mut meta = raw.meta().clone();
            if matches!(
                kind,
                Kind::ClusterImageCatalog | Kind::ValidatingWebhook | Kind::MutatingWebhook
            ) {
                meta.namespace = Some("cluster-scope".into());
            }
            Some(WatchUpdate::Delete(
                adapters::identity(&meta).map_err(|_| ErrorClass::InvalidData)?,
            ))
        }
    };
    Ok(update)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WatchPlan {
    pub kind: Kind,
    pub group: &'static str,
    pub version: &'static str,
    pub resource_kind: &'static str,
    pub namespace: Option<String>,
    pub labels: Option<String>,
    pub fields: Option<String>,
}
pub fn plans(config: &crate::config::Config, namespace: &str) -> Vec<WatchPlan> {
    let ns = if config.all_namespaces {
        None
    } else {
        Some(namespace.into())
    };
    let mut plans = vec![
        WatchPlan {
            kind: Kind::Cluster,
            group: "postgresql.cnpg.io",
            version: "v1",
            resource_kind: "Cluster",
            namespace: ns.clone(),
            labels: config.selector.clone(),
            fields: config
                .cluster
                .as_ref()
                .map(|n| format!("metadata.name={n}")),
        },
        WatchPlan {
            kind: Kind::Pod,
            group: "",
            version: "v1",
            resource_kind: "Pod",
            namespace: ns.clone(),
            labels: Some(config.cluster.as_ref().map_or_else(
                || "cnpg.io/cluster".into(),
                |n| format!("cnpg.io/cluster={n}"),
            )),
            fields: None,
        },
    ];
    for (kind, group, resource_kind, labels) in [
        (Kind::Event, "", "Event", None),
        (
            Kind::Pvc,
            "",
            "PersistentVolumeClaim",
            Some("cnpg.io/cluster"),
        ),
        (Kind::Service, "", "Service", Some("cnpg.io/cluster")),
        (
            Kind::EndpointSlice,
            "discovery.k8s.io",
            "EndpointSlice",
            Some("kubernetes.io/service-name"),
        ),
    ] {
        plans.push(WatchPlan {
            kind,
            group,
            version: "v1",
            resource_kind,
            namespace: ns.clone(),
            labels: labels.map(Into::into),
            fields: None,
        });
    }
    if config.profile != crate::cli::Profile::Minimal {
        for (kind, resource_kind) in [
            (Kind::PoolerDeployment, "Deployment"),
            (Kind::PoolerReplicaSet, "ReplicaSet"),
        ] {
            plans.push(WatchPlan {
                kind,
                group: "apps",
                version: "v1",
                resource_kind,
                namespace: ns.clone(),
                labels: Some("cnpg.io/poolerName".into()),
                fields: None,
            });
        }
        plans.push(WatchPlan {
            kind: Kind::Pooler,
            group: "postgresql.cnpg.io",
            version: "v1",
            resource_kind: "Pooler",
            namespace: ns.clone(),
            labels: None,
            fields: None,
        });
    }
    if config.profile != crate::cli::Profile::Minimal {
        for (kind, resource_kind) in [
            (Kind::Backup, "Backup"),
            (Kind::ScheduledBackup, "ScheduledBackup"),
            (Kind::Database, "Database"),
            (Kind::DatabaseRole, "DatabaseRole"),
            (Kind::Publication, "Publication"),
            (Kind::Subscription, "Subscription"),
            (Kind::FailoverQuorum, "FailoverQuorum"),
            (Kind::ImageCatalog, "ImageCatalog"),
        ] {
            plans.push(WatchPlan {
                kind,
                group: "postgresql.cnpg.io",
                version: "v1",
                resource_kind,
                namespace: ns.clone(),
                labels: None,
                fields: None,
            });
        }
        if config.cluster_resources {
            for (kind, group, resource_kind) in [
                (
                    Kind::ClusterImageCatalog,
                    "postgresql.cnpg.io",
                    "ClusterImageCatalog",
                ),
                (
                    Kind::ValidatingWebhook,
                    "admissionregistration.k8s.io",
                    "ValidatingWebhookConfiguration",
                ),
                (
                    Kind::MutatingWebhook,
                    "admissionregistration.k8s.io",
                    "MutatingWebhookConfiguration",
                ),
            ] {
                plans.push(WatchPlan {
                    kind,
                    group,
                    version: "v1",
                    resource_kind,
                    namespace: None,
                    labels: None,
                    fields: None,
                });
            }
        }
    }
    if config.operator_watch
        && let Some(operator) = &config.operator_namespace
    {
        let selector = Some(
            config
                .operator_selector
                .clone()
                .unwrap_or_else(|| "app.kubernetes.io/name=cloudnative-pg".into()),
        );
        for (kind, group, resource_kind, labels) in [
            (Kind::Deployment, "apps", "Deployment", selector.clone()),
            (Kind::OperatorPod, "", "Pod", selector),
            (Kind::Lease, "coordination.k8s.io", "Lease", None),
            (Kind::OperatorService, "", "Service", None),
            (
                Kind::OperatorEndpointSlice,
                "discovery.k8s.io",
                "EndpointSlice",
                None,
            ),
        ] {
            plans.push(WatchPlan {
                kind,
                group,
                version: "v1",
                resource_kind,
                namespace: Some(operator.clone()),
                labels,
                fields: None,
            });
        }
        // The database event stream already covers this namespace in these modes.
        if !config.all_namespaces && namespace != operator {
            plans.push(WatchPlan {
                kind: Kind::OperatorEvent,
                group: "",
                version: "v1",
                resource_kind: "Event",
                namespace: Some(operator.clone()),
                labels: None,
                fields: None,
            });
        }
    }
    plans.sort();
    plans.dedup();
    plans
}
pub fn describe(
    config: &crate::config::Config,
    namespace: &str,
) -> impl Iterator<Item = (Kind, SourceSpec)> {
    plans(config, namespace)
        .into_iter()
        .map(|plan| (plan.kind, source_spec(&plan)))
}
fn source_spec(plan: &WatchPlan) -> SourceSpec {
    let scope = if matches!(
        plan.kind,
        Kind::ClusterImageCatalog | Kind::ValidatingWebhook | Kind::MutatingWebhook
    ) {
        WatchScope::Cluster
    } else {
        plan.namespace
            .clone()
            .map_or(WatchScope::AllNamespaces, WatchScope::Namespace)
    };
    let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(
        plan.group,
        plan.version,
        plan.resource_kind,
    ));
    SourceSpec {
        scope,
        selector: plan.labels.clone(),
        api_version: resource.api_version,
        resource: resource.plural,
    }
}
pub fn spawn_config(
    tasks: &mut JoinSet<()>,
    client: Client,
    config: &crate::config::Config,
    namespace: &str,
    scope: ScopeEpoch,
    tx: Sender<WatchEvent>,
    cancel: &CancellationToken,
) -> std::collections::BTreeMap<Kind, CancellationToken> {
    let mut owned = std::collections::BTreeMap::new();
    let discovery = std::sync::Arc::new(tokio::sync::OnceCell::new());
    for plan in plans(config, namespace) {
        let mut options = watcher::Config::default().timeout(30).page_size(128);
        if let Some(labels) = &plan.labels {
            options = options.labels(labels);
        }
        if let Some(fields) = &plan.fields {
            options = options.fields(fields);
        }
        let stop = cancel.child_token();
        owned.insert(plan.kind, stop.clone());
        if plan.kind == Kind::Pod {
            let api: Api<Pod> = plan.namespace.as_ref().map_or_else(
                || Api::all(client.clone()),
                |ns| Api::namespaced(client.clone(), ns),
            );
            tasks.spawn(collect(
                api,
                options,
                adapters::pod,
                plan.kind,
                scope,
                tx.clone(),
                stop,
                config.budgets.clone(),
            ));
            continue;
        }
        let adapt = match plan.kind {
            Kind::Cluster => adapters::cluster,
            Kind::Event => adapters::event,
            Kind::OperatorEvent => adapters::operator_event,
            Kind::Pvc => adapters::pvc,
            Kind::Service => adapters::service,
            Kind::EndpointSlice => adapters::endpoints,
            Kind::Pooler => adapters::pooler,
            Kind::Deployment => adapters::deployment,
            Kind::OperatorPod => adapters::operator_pod,
            Kind::Lease => adapters::lease,
            Kind::OperatorService => adapters::operator_service,
            Kind::OperatorEndpointSlice => adapters::operator_endpoints,
            Kind::Backup => adapters::backup,
            Kind::ScheduledBackup => adapters::scheduled_backup,
            Kind::Database => adapters::database,
            Kind::DatabaseRole => adapters::database_role,
            Kind::Publication => adapters::publication,
            Kind::Subscription => adapters::subscription,
            Kind::FailoverQuorum => adapters::quorum,
            Kind::ImageCatalog => adapters::image_catalog,
            Kind::ClusterImageCatalog => adapters::cluster_image_catalog,
            Kind::ValidatingWebhook => adapters::validating_webhook,
            Kind::MutatingWebhook => adapters::mutating_webhook,
            Kind::PoolerDeployment => adapters::pooler_deployment,
            Kind::PoolerReplicaSet => adapters::pooler_replicaset,
            Kind::Pod => unreachable!(),
        };
        let discovery = discovery.clone();
        let client = client.clone();
        let tx = tx.clone();
        let budgets = config.budgets.clone();
        tasks.spawn(async move {
            let mut resource=ApiResource::from_gvk(&GroupVersionKind::gvk(plan.group,plan.version,plan.resource_kind));
            if plan.group=="postgresql.cnpg.io" {
                let result=tokio::select! {
                    _=stop.cancelled()=>return,
                    result=discovery.get_or_init(||async {
                        tokio::time::timeout(std::time::Duration::from_secs(5),client.list_api_group_resources("postgresql.cnpg.io/v1")).await
                            .map_err(|_|ErrorClass::Timeout).and_then(|r|r.map_err(|e|crate::probes::api_error(&e)))
                    })=>result,
                };
                if let Ok(list)=result {
                    if let Some(served)=list.resources.iter().find(|r|r.kind==plan.resource_kind && !r.name.contains('/')) {resource.plural=served.name.clone();}
                    else {
                        let _=tx.send(WatchEvent {scope,watch:WatchEpoch(0),kind:plan.kind,update:WatchUpdate::Error(ErrorClass::Unavailable)},256,&stop).await;
                        return;
                    }
                }
                // Discovery denial/transient errors do not block direct access to registered endpoints.
            }
            let mut spec=source_spec(&plan);spec.resource=resource.plural.clone();
            let _=tx.send(WatchEvent {scope,watch:WatchEpoch(0),kind:plan.kind,update:WatchUpdate::Source(spec)},512,&stop).await;
            let api:Api<DynamicObject>=plan.namespace.as_ref().map_or_else(||Api::all_with(client.clone(),&resource),|ns|Api::namespaced_with(client.clone(),ns,&resource));
            collect(api,options,adapt,plan.kind,scope,tx,stop,budgets).await;
        });
    }
    owned
}

#[cfg(test)]
mod operational_tests {
    use super::*;
    #[test]
    fn global_delete_has_model_identity_and_irrelevant_webhooks_leave_no_cache_entry() {
        let relevant = serde_json::json!({"apiVersion":"admissionregistration.k8s.io/v1","kind":"ValidatingWebhookConfiguration","metadata":{"name":"fixture","uid":"uid"},"webhooks":[{"name":"cnpg.example.test","rules":[{"apiGroups":["postgresql.cnpg.io"],"apiVersions":["v1"],"operations":["CREATE"],"resources":["clusters"]}],"clientConfig":{"service":{"namespace":"cnpg-system","name":"webhook"}},"admissionReviewVersions":["v1"],"sideEffects":"None"}]});
        let raw: DynamicObject = serde_json::from_value(relevant.clone()).unwrap();
        assert!(
            matches!(normalize(watcher::Event::Delete(raw),adapters::validating_webhook,65536,Kind::ValidatingWebhook).unwrap(),Some(WatchUpdate::Delete(id)) if id.key.namespace=="cluster-scope")
        );
        let mut other = relevant;
        other["webhooks"][0]["rules"][0]["apiGroups"] = serde_json::json!(["example.test"]);
        let raw: DynamicObject = serde_json::from_value(other).unwrap();
        assert!(
            normalize(
                watcher::Event::InitApply(raw.clone()),
                adapters::validating_webhook,
                65536,
                Kind::ValidatingWebhook
            )
            .unwrap()
            .is_none()
        );
        assert!(matches!(
            normalize(
                watcher::Event::Apply(raw),
                adapters::validating_webhook,
                65536,
                Kind::ValidatingWebhook
            )
            .unwrap(),
            Some(WatchUpdate::Delete(_))
        ));
    }
}
