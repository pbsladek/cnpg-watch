mod support;
use cnpg_watch::{
    adapters,
    budget::Budgets,
    evidence::{Association, Data, Endpoint, Evidence},
    health::{self, Engine, Severity},
    model::*,
    operational::{
        self,
        health::{Routing, webhook_routing},
    },
    reducer::WatchUpdate,
};
use serde_json::json;
fn apply(store: &mut cnpg_watch::reducer::Store, kind: Kind, value: serde_json::Value) {
    let object = operational::adapt(serde_json::from_value(value).unwrap(), kind).unwrap();
    store.enable_sources([kind]);
    support::watch(store, kind, WatchUpdate::Init);
    support::watch(store, kind, WatchUpdate::InitApply(object));
    support::watch(store, kind, WatchUpdate::InitDone);
}
fn managed(generation: i64, observed: i64, applied: bool) -> serde_json::Value {
    json!({"apiVersion":"postgresql.cnpg.io/v1","kind":"DatabaseRole","metadata":{"namespace":"app-db","name":"role","uid":"role-uid","generation":generation},"spec":{"cluster":{"name":"payments"},"name":"private_role_marker"},"status":{"observedGeneration":observed,"applied":applied,"message":"SECRET_CONTROLLER_PAYLOAD"}})
}
#[test]
fn generation_backup_ordering_and_incremental_health_recover() {
    let mut store = support::seed();
    let budgets = Budgets::default();
    let mut cached = Engine::default();
    let mut full = Engine::default();
    for (step, (generation, observed, applied, expected)) in [
        (2, 1, false, "managed.pending_generation"),
        (2, 2, false, "managed.failed"),
        (3, 2, false, "managed.pending_generation"),
        (3, 3, true, "none"),
    ]
    .into_iter()
    .enumerate()
    {
        apply(
            &mut store,
            Kind::DatabaseRole,
            managed(generation, observed, applied),
        );
        let now = Tick(step as u64 * 1000);
        let reasons = cached.update(&store, now, &budgets);
        assert_eq!(reasons, full.recompute(&store, now, &budgets));
        assert_eq!(
            reasons
                .iter()
                .filter(|r| r.id.starts_with("managed."))
                .count(),
            usize::from(expected != "none")
        );
        if expected != "none" {
            let r = reasons.iter().find(|r| r.id.starts_with(expected)).unwrap();
            assert_eq!(
                r.severity,
                if expected == "managed.failed" {
                    Severity::Warning
                } else {
                    Severity::Info
                }
            );
        }
        let object = store.evidence(Kind::DatabaseRole).next().unwrap();
        let text = serde_json::to_string(object).unwrap();
        assert!(!text.contains("SECRET_CONTROLLER_PAYLOAD"));
    }
    let backup = |name: &str, phase: &str, time: &str| json!({"apiVersion":"postgresql.cnpg.io/v1","kind":"Backup","metadata":{"namespace":"app-db","name":name,"uid":name},"spec":{"cluster":{"name":"payments"},"method":"plugin"},"status":{"phase":phase,"stoppedAt":time,"error":"SECRET_BACKUP_PAYLOAD"}});
    apply(
        &mut store,
        Kind::Backup,
        backup("failed", "failed", "2026-11-15T01:00:00Z"),
    );
    assert!(
        cached
            .update(&store, Tick(5000), &budgets)
            .iter()
            .any(|r| r.id.starts_with("backup.failed"))
    );
    let completed = operational::adapt(
        serde_json::from_value(backup("good", "completed", "2026-11-15T02:00:00Z")).unwrap(),
        Kind::Backup,
    )
    .unwrap();
    support::watch(&mut store, Kind::Backup, WatchUpdate::Apply(completed));
    let reasons = cached.update(&store, Tick(6000), &budgets);
    assert!(!reasons.iter().any(|r| r.id.starts_with("backup.failed")));
    assert_eq!(reasons, full.recompute(&store, Tick(6000), &budgets));
    assert!(
        !serde_json::to_string(&store.evidence(Kind::Backup).collect::<Vec<_>>())
            .unwrap()
            .contains("SECRET_BACKUP_PAYLOAD")
    );
}
#[test]
fn malformed_optional_cluster_fields_preserve_topology_and_quorum_uses_real_root_status() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/cnpg/1.30.0/cluster.json")).unwrap();
    value["spec"]["replica"] = json!({"enabled":"invalid"});
    let Object::Cluster(cluster) =
        adapters::cluster(serde_json::from_value(value).unwrap()).unwrap()
    else {
        panic!()
    };
    assert!(cluster.primary.is_some());
    assert_eq!(
        cluster.operational.availability,
        Availability::Unavailable(ErrorClass::InvalidData)
    );
    let value = json!({"apiVersion":"postgresql.cnpg.io/v1","kind":"FailoverQuorum","metadata":{"namespace":"app-db","name":"payments","uid":"q"},"status":{"primary":"payments-1","method":"any","standbyNames":["payments-2"],"standbyNumber":1}});
    let Object::Evidence(e) =
        operational::adapt(serde_json::from_value(value).unwrap(), Kind::FailoverQuorum).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        e.association,
        Association::Label(Key::new("app-db", "payments").unwrap())
    );
    assert!(
        matches!(e.data,Data::Operational(ref r) if matches!(r.as_ref(),operational::Resource::Quorum{standby_number:Some(1),..}))
    );
}
#[test]
fn optional_routing_needs_covered_watch_and_current_ready_target_and_events_reject_old_uid() {
    let mut store = support::seed();
    let cluster = store.clusters().next().unwrap().id.clone();
    let mut pod = store.pods().next().unwrap().clone();
    pod.id.key.namespace = "cnpg-system".into();
    let pod_identity = pod.id.clone();
    support::watch(&mut store, Kind::Pod, WatchUpdate::Apply(Object::Pod(pod)));
    let pod = pod_identity;
    let service = Key::new("cnpg-system", "webhook").unwrap();
    store.enable_sources([Kind::OperatorService, Kind::OperatorEndpointSlice]);
    for kind in [Kind::OperatorService, Kind::OperatorEndpointSlice] {
        support::watch(
            &mut store,
            kind,
            WatchUpdate::Source(SourceSpec {
                scope: WatchScope::Namespace("app-db".into()),
                selector: None,
                api_version: "v1".into(),
                resource: "fixture".into(),
            }),
        );
        support::watch(&mut store, kind, WatchUpdate::Init);
        support::watch(&mut store, kind, WatchUpdate::InitDone);
    }
    assert_eq!(webhook_routing(&store, &service), Routing::Unknown);
    support::watch(
        &mut store,
        Kind::OperatorService,
        WatchUpdate::Source(SourceSpec {
            scope: WatchScope::Namespace("cnpg-system".into()),
            selector: None,
            api_version: "v1".into(),
            resource: "services".into(),
        }),
    );
    assert_eq!(webhook_routing(&store, &service), Routing::MissingService);
    let evidence = |kind, key: Key, data| {
        Object::Evidence(Evidence {
            id: Identity {
                key,
                uid: Uid::new("fixture").unwrap(),
            },
            kind,
            labels: Default::default(),
            association: Association::Unrelated,
            data,
            observed_at: Tick(0),
        })
    };
    support::watch(
        &mut store,
        Kind::OperatorService,
        WatchUpdate::Apply(evidence(
            Kind::OperatorService,
            service.clone(),
            Data::Service {
                selector: Default::default(),
                read_write: false,
            },
        )),
    );
    support::watch(
        &mut store,
        Kind::OperatorEndpointSlice,
        WatchUpdate::Source(SourceSpec {
            scope: WatchScope::Namespace("cnpg-system".into()),
            selector: None,
            api_version: "discovery.k8s.io/v1".into(),
            resource: "endpointslices".into(),
        }),
    );
    support::watch(
        &mut store,
        Kind::OperatorEndpointSlice,
        WatchUpdate::Apply(evidence(
            Kind::OperatorEndpointSlice,
            Key::new("cnpg-system", "slice").unwrap(),
            Data::Endpoints {
                service: service.clone(),
                endpoints: vec![Endpoint {
                    target: Some(pod.clone()),
                    ready: Some(true),
                }],
            },
        )),
    );
    assert_eq!(webhook_routing(&store, &service), Routing::Ready);
    let mut p = store.pod(&pod.key).unwrap().clone();
    p.ready = Some(false);
    support::watch(&mut store, Kind::Pod, WatchUpdate::Apply(Object::Pod(p)));
    assert_eq!(webhook_routing(&store, &service), Routing::Unknown);
    let event = Evidence {
        id: Identity {
            key: Key::new("app-db", "event").unwrap(),
            uid: Uid::new("event").unwrap(),
        },
        kind: Kind::Event,
        labels: Default::default(),
        association: Association::Label(cluster.key),
        data: Data::Event {
            warning: true,
            reason: "Fixture".into(),
            message: "message".into(),
            involved: Some(Identity {
                uid: Uid::new("old-uid").unwrap(),
                ..pod
            }),
            involved_kind: Some(Kind::Pod),
            last_seen_ms: None,
            count: 1,
        },
        observed_at: Tick(0),
    };
    assert!(health::cluster_key(&store, &event).is_none());
}
#[test]
fn captured_cnpg130_operational_shapes_and_served_plural_are_consumed() {
    for (kind, raw) in [
        (
            Kind::Backup,
            include_str!("../fixtures/cnpg/1.30.0/backups.json"),
        ),
        (
            Kind::ScheduledBackup,
            include_str!("../fixtures/cnpg/1.30.0/scheduledbackups.json"),
        ),
        (
            Kind::Database,
            include_str!("../fixtures/cnpg/1.30.0/databases.json"),
        ),
        (
            Kind::DatabaseRole,
            include_str!("../fixtures/cnpg/1.30.0/databaseroles.json"),
        ),
        (
            Kind::Publication,
            include_str!("../fixtures/cnpg/1.30.0/publications.json"),
        ),
        (
            Kind::ImageCatalog,
            include_str!("../fixtures/cnpg/1.30.0/imagecatalogs.json"),
        ),
        (
            Kind::ClusterImageCatalog,
            include_str!("../fixtures/cnpg/1.30.0/clusterimagecatalogs.json"),
        ),
    ] {
        let values: Vec<kube::api::DynamicObject> = serde_json::from_str(raw).unwrap();
        assert!(!values.is_empty());
        for value in values {
            let object = operational::adapt(value, kind).unwrap();
            assert!(object.bytes() < Budgets::default().object_bytes);
            if let Object::Evidence(e) = object
                && let Data::Operational(r) = e.data
                && let operational::Resource::Managed { reconciliation, .. } = *r
            {
                assert!(reconciliation.current());
                assert_eq!(reconciliation.applied, Some(true));
            }
        }
    }
    let discovery: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/cnpg/1.30.0/operational-discovery.json"
    ))
    .unwrap();
    let role = discovery["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["kind"] == "DatabaseRole")
        .unwrap();
    assert_eq!(role["name"], "databaseroles");
    assert_eq!(role["namespaced"], true);
    for text in [
        include_str!("../fixtures/cnpg/1.30.0/subscriptions.json"),
        include_str!("../fixtures/cnpg/1.30.0/failoverquorums.json"),
    ] {
        assert!(
            serde_json::from_str::<Vec<serde_json::Value>>(text)
                .unwrap()
                .is_empty()
        );
    }
}
#[test]
fn cluster_condition_failures_need_current_generation() {
    let mut store = support::seed();
    let mut cluster = store.clusters().next().unwrap().clone();
    cluster.operational.generation = Some(2);
    cluster.operational.conditions = vec![operational::Condition {
        type_: "ContinuousArchiving".into(),
        status: "False".into(),
        observed_generation: Some(1),
        ..Default::default()
    }];
    let mut engine = Engine::default();
    for current in [false, true] {
        cluster.operational.conditions[0].observed_generation = Some(if current { 2 } else { 1 });
        support::watch(
            &mut store,
            Kind::Cluster,
            WatchUpdate::Apply(Object::Cluster(cluster.clone())),
        );
        let reason = engine
            .update(&store, Tick(1), &Budgets::default())
            .iter()
            .find(|r| r.id.starts_with("cluster.condition_"))
            .unwrap();
        assert_eq!(
            reason.severity,
            if current {
                Severity::Warning
            } else {
                Severity::Info
            }
        );
    }
}
#[test]
fn opaque_plugin_and_external_connection_payloads_are_discarded_and_duplicate_capabilities_are_capped()
 {
    let mut value = json!({"apiVersion":"postgresql.cnpg.io/v1","kind":"Cluster","metadata":{"namespace":"app-db","name":"payments","uid":"uid","generation":1},"spec":{"externalClusters":[{"name":"upstream","connectionParameters":{"password":"SECRET_PAYLOAD"}}],"plugins":[{"name":"fixture-plugin","parameters":{"token":"SECRET_PAYLOAD"}}],"replica":{"enabled":true,"source":"upstream","minApplyDelay":"5min"}},"status":{"currentPrimary":"payments-1","pluginStatus":[{"name":"fixture-plugin","version":"v1","capabilities":["backup"],"status":"SECRET_PAYLOAD"}]}});
    let object = adapters::cluster(serde_json::from_value(value.clone()).unwrap()).unwrap();
    let encoded = serde_json::to_string(&object).unwrap();
    assert!(!encoded.contains("SECRET_PAYLOAD"));
    let Object::Cluster(cluster) = object else {
        panic!()
    };
    assert_eq!(cluster.operational.plugins.len(), 1);
    assert_eq!(cluster.operational.replica.external_names, vec!["upstream"]);
    assert_eq!(cluster.operational.replica.delay.as_deref(), Some("5min"));
    value["status"]["pluginStatus"] = json!(
        (0..32)
            .map(|_| json!({"name":"fixture-plugin","capabilities":vec!["backup";64]}))
            .collect::<Vec<_>>()
    );
    let Object::Cluster(cluster) =
        adapters::cluster(serde_json::from_value(value).unwrap()).unwrap()
    else {
        panic!()
    };
    assert!(cluster.primary.is_some());
    assert_eq!(
        cluster.operational.availability,
        Availability::Unavailable(ErrorClass::InvalidData)
    );
}
