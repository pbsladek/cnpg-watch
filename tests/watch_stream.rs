mod support;
use cnpg_watch::{
    adapters,
    budget::Budgets,
    collectors,
    model::*,
    queue,
    reducer::{WatchEvent, WatchUpdate},
};
use kube::{core::DynamicObject, runtime::watcher};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn interrupted_adapter_initialization_cannot_finish_partial_list() {
    let mut raw: DynamicObject =
        serde_json::from_str(include_str!("../fixtures/cnpg/1.30.0/cluster.json")).unwrap();
    raw.data = serde_json::json!({"status":{"currentPrimary":42}});
    let stream = futures::stream::iter(vec![
        Ok(watcher::Event::Init),
        Ok(watcher::Event::InitApply(raw)),
        Ok(watcher::Event::InitDone),
    ]);
    let (tx, mut rx) = queue::channel(8, 8192);
    let stop = CancellationToken::new();
    assert_eq!(
        collectors::consume(
            stream,
            adapters::cluster,
            Kind::Cluster,
            ScopeEpoch(0),
            WatchEpoch(0),
            &tx,
            &stop,
            &Budgets::default()
        )
        .await,
        ErrorClass::InvalidData
    );
    let mut store = support::seed();
    while let Some(packet) = rx.try_recv() {
        store.watch(packet.value.clone());
    }
    assert_eq!(store.clusters().count(), 1);
    assert_eq!(
        store.watches[&Kind::Cluster].status,
        SyncState::Failed(ErrorClass::InvalidData)
    );
    assert!(store.watches[&Kind::Cluster].staging.is_none());
}

#[tokio::test]
async fn deletes_need_only_identity_and_capacity_errors_are_explicit() {
    let mut raw: DynamicObject =
        serde_json::from_str(include_str!("../fixtures/cnpg/1.30.0/cluster.json")).unwrap();
    let (tx, mut rx) = queue::channel(8, 8192);
    let stop = CancellationToken::new();
    raw.data = serde_json::json!({"status":"not a valid status"});
    let stream = futures::stream::iter(vec![Ok(watcher::Event::Delete(raw))]);
    collectors::consume(
        stream,
        adapters::cluster,
        Kind::Cluster,
        ScopeEpoch(0),
        WatchEpoch(0),
        &tx,
        &stop,
        &Budgets::default(),
    )
    .await;
    assert!(matches!(
        rx.try_recv().unwrap().value.update,
        WatchUpdate::Delete(_)
    ));
    let raw = serde_json::from_str(include_str!("../fixtures/cnpg/1.30.0/cluster.json")).unwrap();
    let stream = futures::stream::iter(vec![Ok(watcher::Event::Apply(raw))]);
    let budgets = Budgets {
        object_bytes: 1,
        ..Budgets::default()
    };
    assert_eq!(
        collectors::consume(
            stream,
            adapters::cluster,
            Kind::Cluster,
            ScopeEpoch(0),
            WatchEpoch(0),
            &tx,
            &stop,
            &budgets
        )
        .await,
        ErrorClass::Capacity
    );
}

#[test]
fn deleting_a_cluster_discards_failed_never_observed_samples_too() {
    let mut store = support::seed();
    let ticket = support::ticket(&store, 1);
    store.begin_probe(ticket.clone());
    store.finish_probe(ticket.clone(), Err(ErrorClass::Authentication), Tick(0));
    store.watch(WatchEvent {
        scope: store.scope,
        watch: WatchEpoch(0),
        kind: Kind::Cluster,
        update: WatchUpdate::Delete(ticket.cluster),
    });
    assert!(store.samples.is_empty());
}
