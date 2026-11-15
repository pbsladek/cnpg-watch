#![allow(dead_code)]
use cnpg_watch::{
    budget::Budgets,
    model::*,
    reducer::{Store, WatchEvent, WatchUpdate},
};

pub fn cluster() -> Object {
    cnpg_watch::adapters::cluster(
        serde_json::from_str(include_str!("../../fixtures/cnpg/1.30.0/cluster.json")).unwrap(),
    )
    .unwrap()
}
pub fn pod() -> Object {
    cnpg_watch::adapters::pod(
        serde_json::from_str(include_str!("../../fixtures/cnpg/1.30.0/pod.json")).unwrap(),
    )
    .unwrap()
}
pub fn watch(store: &mut Store, kind: Kind, update: WatchUpdate) {
    store.watch(WatchEvent {
        scope: store.scope,
        watch: WatchEpoch(0),
        kind,
        update,
    });
}
pub fn seed() -> Store {
    let mut store = Store::new(Budgets::default());
    for object in [cluster(), pod()] {
        let kind = object.kind();
        watch(&mut store, kind, WatchUpdate::Init);
        watch(&mut store, kind, WatchUpdate::InitApply(object));
        watch(&mut store, kind, WatchUpdate::InitDone);
    }
    store
}
pub fn ticket(store: &Store, request: u64) -> ProbeTicket {
    let pod = store.pods().next().unwrap();
    let cluster = store.cluster(pod.cluster.as_ref().unwrap()).unwrap();
    ProbeTicket {
        scope: store.scope,
        class: Default::default(),
        topology: store
            .topology
            .get(&cluster.id.key)
            .copied()
            .unwrap_or_default(),
        request: RequestId(request),
        cluster: cluster.id.clone(),
        pod: pod.id.clone(),
    }
}
pub fn sample() -> FastSample {
    FastSample {
        server_version: 180004,
        role: Role::Primary,
        system_id: 42,
        timeline: 1,
        current: Some(Lsn(0x100)),
        received: None,
        replay: None,
        server_time: "2026-11-14T00:00:00Z".into(),
        last_replayed_transaction_age_ms: None,
        peers: vec![Peer {
            application: "payments-2".into(),
            address: None,
            replay: Some(Lsn(0x100)),
        }],
        connections: None,
        archive: Archive {
            archived_count: 1,
            failed_count: 0,
            stats_reset: None,
            last_archived_time: None,
            last_failed_time: None,
        },
    }
}
pub fn fleet(count: usize) -> Store {
    let mut store = Store::new(Budgets::default());
    for kind in [Kind::Cluster, Kind::Pod] {
        watch(&mut store, kind, WatchUpdate::Init);
    }
    for index in 0..count {
        let key = Key::new("benchmark", format!("cluster-{index:04}")).unwrap();
        let object = Object::Cluster(Cluster {
            id: Identity {
                key: key.clone(),
                uid: Uid::new(format!("c-{index}")).unwrap(),
            },
            primary: Some(format!("cluster-{index:04}-1")),
            phase: Some("Ready".into()),
            operational: Default::default(),
            labels: Default::default(),
        });
        watch(&mut store, Kind::Cluster, WatchUpdate::InitApply(object));
        for instance in 1..=3 {
            let pod = Object::Pod(Pod {
                id: Identity {
                    key: Key::new("benchmark", format!("cluster-{index:04}-{instance}")).unwrap(),
                    uid: Uid::new(format!("p-{index}-{instance}")).unwrap(),
                },
                cluster: Some(key.clone()),
                role: Some(if instance == 1 {
                    Role::Primary
                } else {
                    Role::Replica
                }),
                ready: Some(true),
                restarts: 0,
                address: None,
                labels: Default::default(),
                pooler: None,
                backup: None,
                backup_uid: None,
                controller: None,
                containers: vec!["postgres".into()],
                cluster_uid: None,
            });
            watch(&mut store, Kind::Pod, WatchUpdate::InitApply(pod));
        }
    }
    for kind in [Kind::Cluster, Kind::Pod] {
        watch(&mut store, kind, WatchUpdate::InitDone);
    }
    store
}

/// Operational cardinality for replay/benchmark workloads, using production decoders.
pub fn operational_fleet(count: usize) -> Store {
    let mut store = fleet(count);
    for kind in [Kind::Backup, Kind::DatabaseRole] {
        watch(&mut store, kind, WatchUpdate::Init);
        for index in 0..count {
            let cluster = format!("cluster-{index:04}");
            let (api_kind, status) = if kind == Kind::Backup {
                (
                    "Backup",
                    serde_json::json!({"phase":"completed","stoppedAt":"2026-11-15T00:00:00Z"}),
                )
            } else {
                (
                    "DatabaseRole",
                    serde_json::json!({"applied":true,"observedGeneration":1}),
                )
            };
            let value = serde_json::json!({"apiVersion":"postgresql.cnpg.io/v1","kind":api_kind,"metadata":{"namespace":"benchmark","name":format!("{cluster}-{}",api_kind.to_lowercase()),"uid":format!("{api_kind}-{index}"),"generation":1},"spec":{"cluster":{"name":cluster},"name":"fixture"},"status":status});
            let object =
                cnpg_watch::operational::adapt(serde_json::from_value(value).unwrap(), kind)
                    .unwrap();
            watch(&mut store, kind, WatchUpdate::InitApply(object));
        }
        watch(&mut store, kind, WatchUpdate::InitDone);
    }
    store
}

pub fn add_pooler(store: &mut Store) {
    let cluster = store.clusters().next().unwrap().id.clone();
    let ns = &cluster.key.namespace;
    let pool = format!("{}-pool", cluster.key.name);
    let pooler = serde_json::json!({"apiVersion":"postgresql.cnpg.io/v1","kind":"Pooler","metadata":{"namespace":ns,"name":pool,"uid":"load-pooler"},"spec":{"cluster":{"name":cluster.key.name},"instances":2,"type":"rw","pgbouncer":{"poolMode":"transaction"}}});
    let deployment = serde_json::json!({"apiVersion":"apps/v1","kind":"Deployment","metadata":{"namespace":ns,"name":pool,"uid":"load-deployment","ownerReferences":[{"apiVersion":"postgresql.cnpg.io/v1","kind":"Pooler","name":pool,"uid":"load-pooler"}]},"spec":{},"status":{}});
    let rs = serde_json::json!({"apiVersion":"apps/v1","kind":"ReplicaSet","metadata":{"namespace":ns,"name":"load-rs","uid":"load-rs","ownerReferences":[{"apiVersion":"apps/v1","kind":"Deployment","name":pool,"uid":"load-deployment"}]}});
    for object in [
        cnpg_watch::adapters::pooler(serde_json::from_value(pooler).unwrap()).unwrap(),
        cnpg_watch::adapters::pooler_deployment(serde_json::from_value(deployment).unwrap())
            .unwrap(),
        cnpg_watch::adapters::pooler_replicaset(serde_json::from_value(rs).unwrap()).unwrap(),
    ] {
        let kind = object.kind();
        store.enable_sources([kind]);
        watch(store, kind, WatchUpdate::Init);
        watch(store, kind, WatchUpdate::InitApply(object));
        watch(store, kind, WatchUpdate::InitDone);
    }
    for i in 0..2 {
        let pod = serde_json::json!({"apiVersion":"v1","kind":"Pod","metadata":{"namespace":ns,"name":format!("{pool}-{i}"),"uid":format!("load-pool-{i}"),"labels":{"cnpg.io/cluster":cluster.key.name,"cnpg.io/poolerName":pool},"ownerReferences":[{"apiVersion":"apps/v1","kind":"ReplicaSet","name":"load-rs","uid":"load-rs","controller":true}]},"spec":{"containers":[{"name":"pgbouncer"}]},"status":{"conditions":[{"type":"Ready","status":"True"}]}});
        watch(
            store,
            Kind::Pod,
            WatchUpdate::Apply(
                cnpg_watch::adapters::pod(serde_json::from_value(pod).unwrap()).unwrap(),
            ),
        );
    }
}
