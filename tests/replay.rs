mod support;
use cnpg_watch::{
    budget::Budgets,
    model::*,
    reducer::{Store, WatchEvent, WatchUpdate},
    ui::{self, ViewCache},
};
use proptest::prelude::*;
use std::time::Duration;
use support::*;

#[test]
fn recreated_pod_cannot_inherit_late_result_or_delete() {
    let mut store = seed();
    let old = ticket(&store, 1);
    assert!(store.begin_probe(old.clone()));
    let mut replacement = pod();
    if let Object::Pod(p) = &mut replacement {
        p.id.uid = Uid::new("replacement").unwrap();
    }
    watch(
        &mut store,
        Kind::Pod,
        WatchUpdate::Apply(replacement.clone()),
    );
    let fresh = ticket(&store, 2);
    assert!(store.begin_probe(fresh.clone()));
    assert!(store.finish_probe(fresh.clone(), Ok(sample()), Tick(10)));
    assert!(!store.finish_probe(old.clone(), Ok(sample()), Tick(20)));
    watch(&mut store, Kind::Pod, WatchUpdate::Delete(old.pod));
    assert_eq!(
        store.pod(&fresh.pod.key).unwrap().id,
        replacement.id().clone()
    );
    assert_eq!(
        store.samples[&fresh.pod.key].last_good.as_ref().unwrap().at,
        Tick(10)
    );
}

#[test]
fn relist_commits_membership_only_after_complete_initialization() {
    let mut store = seed();
    let object = pod();
    watch(&mut store, Kind::Pod, WatchUpdate::Init);
    watch(
        &mut store,
        Kind::Pod,
        WatchUpdate::Error(ErrorClass::Transport),
    );
    assert!(store.pod(&object.id().key).is_some());
    watch(&mut store, Kind::Pod, WatchUpdate::InitDone);
    assert!(store.pod(&object.id().key).is_some());
    watch(&mut store, Kind::Pod, WatchUpdate::Init);
    watch(
        &mut store,
        Kind::Pod,
        WatchUpdate::InitApply(object.clone()),
    );
    watch(
        &mut store,
        Kind::Pod,
        WatchUpdate::InitApply(object.clone()),
    );
    watch(&mut store, Kind::Pod, WatchUpdate::InitDone);
    assert_eq!(store.pods().count(), 1);
    watch(&mut store, Kind::Pod, WatchUpdate::Init);
    watch(&mut store, Kind::Pod, WatchUpdate::InitDone);
    assert!(store.pod(&object.id().key).is_none());
}

#[test]
fn old_scope_and_watch_epochs_cannot_repopulate_new_state() {
    let mut store = seed();
    let previous = store.scope;
    store.switch_scope();
    store.watch(WatchEvent {
        scope: previous,
        watch: WatchEpoch(10),
        kind: Kind::Pod,
        update: WatchUpdate::Apply(pod()),
    });
    assert_eq!(store.pods().count(), 0);
    store.watch(WatchEvent {
        scope: store.scope,
        watch: WatchEpoch(2),
        kind: Kind::Pod,
        update: WatchUpdate::Init,
    });
    store.watch(WatchEvent {
        scope: store.scope,
        watch: WatchEpoch(1),
        kind: Kind::Pod,
        update: WatchUpdate::InitApply(pod()),
    });
    assert!(
        store.watches[&Kind::Pod]
            .staging
            .as_ref()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn failed_refresh_retains_value_and_deadlines_advance_without_new_data() {
    let mut store = seed();
    let request = ticket(&store, 1);
    store.begin_probe(request.clone());
    store.finish_probe(request.clone(), Ok(sample()), Tick(0));
    let mut cache = ViewCache::default();
    assert!(
        cache.get(&store, Tick(0), &Budgets::default()).rows[0]
            .state
            .contains("fresh")
    );
    assert!(
        cache.get(&store, Tick(10_000), &Budgets::default()).rows[0]
            .state
            .contains("stale")
    );
    let next = ticket(&store, 2);
    store.begin_probe(next.clone());
    store.finish_probe(next, Err(ErrorClass::Timeout), Tick(10));
    let observation = &store.samples[&request.pod.key];
    assert!(observation.last_good.is_some());
    assert!(matches!(
        observation.freshness(Tick(10), Duration::from_secs(10)),
        Freshness::Stale(_)
    ));
    assert_eq!(store.watches[&Kind::Pod].status, SyncState::Synced);
}

#[test]
fn capacity_failure_keeps_last_complete_set_and_drops_staging() {
    let mut store = Store::new(Budgets {
        cache_entries: 1,
        ..Budgets::default()
    });
    watch(&mut store, Kind::Pod, WatchUpdate::Apply(pod()));
    watch(&mut store, Kind::Pod, WatchUpdate::Init);
    watch(&mut store, Kind::Pod, WatchUpdate::InitApply(pod()));
    assert_eq!(
        store.watches[&Kind::Pod].status,
        SyncState::Failed(ErrorClass::Capacity)
    );
    assert_eq!(store.pods().count(), 1);
    assert!(store.watches[&Kind::Pod].staging.is_none());
}

proptest! {
    #![proptest_config(ProptestConfig { rng_seed: proptest::test_runner::RngSeed::Fixed(0x243f6a8885a308d3), ..ProptestConfig::default() })]
    #[test]
    fn lsn_round_trip_and_checked_deltas(value in any::<u64>(), other in any::<u64>()) {
        prop_assert_eq!(Lsn(value).to_string().parse::<Lsn>().unwrap(),Lsn(value));
        prop_assert_eq!(Lsn(value).checked_lag(Lsn(other)),value.checked_sub(other));
    }
    #[test]
    fn lifecycle_sequences_preserve_bounds_and_cached_view_equivalence(actions in prop::collection::vec(0u8..7,1..100)) {
        let mut store=seed();let mut cache=ViewCache::default();let mut now=0;
        for action in actions {
            now+=1000;
            match action {
                0=>watch(&mut store,Kind::Pod,WatchUpdate::Apply(pod())),
                1=>watch(&mut store,Kind::Pod,WatchUpdate::Init),
                2=>watch(&mut store,Kind::Pod,WatchUpdate::InitApply(pod())),
                3=>watch(&mut store,Kind::Pod,WatchUpdate::InitDone),
                4=>watch(&mut store,Kind::Pod,WatchUpdate::Error(ErrorClass::Transport)),
                5=>watch(&mut store,Kind::Pod,WatchUpdate::Delete(pod().id().clone())),
                _=>{},
            }
            let expected=ui::derive(&store,Tick(now),&Budgets::default());
            prop_assert_eq!(cache.get(&store,Tick(now),&Budgets::default()),&expected);
            prop_assert!(store.bytes()<=Budgets::default().cache_bytes);
            prop_assert!(store.entries()<=Budgets::default().cache_entries);
        }
    }
}

#[test]
fn malformed_lsn_is_never_zero_lag() {
    for value in [
        "",
        "1",
        "/1",
        "1/",
        "-1/1",
        "100000000/0",
        "0/100000000",
        "0/G",
        "1/2/3",
    ] {
        assert!(value.parse::<Lsn>().is_err(), "{value}");
    }
}

#[test]
fn captured_protocol_replay_has_stepwise_membership_expectations() {
    #[derive(serde::Deserialize)]
    struct Replay {
        schema_version: u32,
        events: Vec<WatchEvent>,
        expected_pods: Vec<usize>,
    }
    let replay: Replay =
        serde_json::from_str(include_str!("../fixtures/replay/relist-missed-delete.json")).unwrap();
    assert_eq!(replay.schema_version, 1);
    assert_eq!(replay.events.len(), replay.expected_pods.len());
    let mut store = Store::new(Budgets::default());
    for (index, (event, count)) in replay
        .events
        .into_iter()
        .zip(replay.expected_pods)
        .enumerate()
    {
        store.watch(event);
        assert_eq!(store.pods().count(), count, "replay step {index}");
    }
}

#[test]
fn sql_role_change_invalidates_other_timeline_dependent_evidence() {
    let mut store = seed();
    let first = ticket(&store, 1);
    store.begin_probe(first.clone());
    store.finish_probe(first, Ok(sample()), Tick(0));
    let next = ticket(&store, 2);
    store.begin_probe(next.clone());
    let mut changed = sample();
    changed.role = Role::Replica;
    changed.current = None;
    changed.replay = Some(Lsn(1));
    store.finish_probe(next.clone(), Ok(changed), Tick(1));
    assert_eq!(store.topology[&next.cluster.key], TopologyEpoch(1));
    assert!(!store.valid_ticket(&next));
}

#[test]
fn actual_probe_fixture_has_version_identity_grant_and_schema_checks() {
    let bytes = include_bytes!("../fixtures/postgres/18/primary-fast.json");
    assert!(cnpg_watch::probes::parse(bytes, "watch_monitor").is_ok());
    assert_eq!(
        cnpg_watch::probes::parse(bytes, "other_role"),
        Err(ErrorClass::Forbidden)
    );
    let mut raw: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    raw["server_version"] = 170000.into();
    assert_eq!(
        cnpg_watch::probes::parse(&serde_json::to_vec(&raw).unwrap(), "watch_monitor"),
        Err(ErrorClass::Unsupported)
    );
    raw["server_version"] = 180004.into();
    raw.as_object_mut().unwrap().remove("archive");
    assert_eq!(
        cnpg_watch::probes::parse(&serde_json::to_vec(&raw).unwrap(), "watch_monitor"),
        Err(ErrorClass::InvalidData)
    );
    assert_eq!(
        cnpg_watch::probes::parse(b"{}\n{}", "watch_monitor"),
        Err(ErrorClass::InvalidData)
    );
}
