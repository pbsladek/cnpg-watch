mod support;
use cnpg_watch::{
    budget::Budgets,
    cli::Profile,
    model::*,
    probes::{
        self,
        registry::{self, Bundle, Class, Diagnostic},
    },
    scheduler::Scheduler,
};
fn capabilities() -> Diagnostic {
    registry::parse(
        Bundle::Capabilities,
        include_bytes!("../fixtures/postgres/18/capabilities.json"),
        "watch_monitor",
        Profile::Incident,
    )
    .unwrap()
}
fn cache_capabilities(store: &mut cnpg_watch::reducer::Store, request: u64, now: Tick) {
    let mut t = support::ticket(store, request);
    t.class = Class::Capabilities;
    assert!(store.begin_probe(t.clone()));
    assert!(store.finish_diagnostic(t, Ok(capabilities()), now));
}
#[test]
fn all_registered_variants_have_explicit_limits_and_capability_gates() {
    let Diagnostic::Capabilities(c) = capabilities() else {
        panic!()
    };
    for bundle in [
        Bundle::Capabilities,
        Bundle::Fast,
        Bundle::FastBasic,
        Bundle::Medium,
        Bundle::Deep,
        Bundle::DeepText,
    ] {
        let spec = bundle.spec();
        assert!(spec.timeout_ms <= 2000 && spec.max_rows <= 128 && spec.schema_version == 1);
        assert!(!bundle.sql().to_ascii_uppercase().contains("SELECT *"));
        assert!(c.allows(bundle));
    }
    let mut missing = c.clone();
    missing
        .views
        .iter_mut()
        .find(|v| v.name == "pg_stat_activity")
        .unwrap()
        .columns
        .remove("pid");
    assert_eq!(missing.select_fast(), Some(Bundle::FastBasic));
    assert!(!missing.allows(Bundle::Deep));
    missing
        .views
        .iter_mut()
        .find(|v| v.name == "pg_replication_slots")
        .unwrap()
        .readable = false;
    assert!(!missing.allows(Bundle::Medium));
    let mut unsupported: serde_json::Value =
        serde_json::from_slice(include_bytes!("../fixtures/postgres/18/capabilities.json"))
            .unwrap();
    unsupported["server_version"] = 190000.into();
    assert_eq!(
        registry::parse(
            Bundle::Capabilities,
            &serde_json::to_vec(&unsupported).unwrap(),
            "watch_monitor",
            Profile::Incident
        ),
        Err(ErrorClass::Unsupported)
    );
    unsupported["server_version"] = 180004.into();
    unsupported["superuser"] = true.into();
    assert_eq!(
        registry::parse(
            Bundle::Capabilities,
            &serde_json::to_vec(&unsupported).unwrap(),
            "watch_monitor",
            Profile::Incident
        ),
        Err(ErrorClass::Forbidden)
    );
}
#[test]
fn medium_and_deep_schema_limits_unknowns_and_query_text_are_enforced() {
    assert!(matches!(
        registry::parse(
            Bundle::Medium,
            include_bytes!("../fixtures/postgres/18/medium.json"),
            "watch_monitor",
            Profile::Standard
        ),
        Ok(Diagnostic::Medium(_))
    ));
    let mut deep: serde_json::Value =
        serde_json::from_slice(include_bytes!("../fixtures/postgres/18/deep.json")).unwrap();
    deep["long_transactions"] = serde_json::json!([{ "pid":42,"backend_type":"client backend","state":"active","wait_event_type":null,"wait_event":null,"xact_age_ms":5,"query_age_ms":5,"query_text":null }]);
    deep["long_transactions"][0]["query_text"] =
        "select 'visible' /* password='never expose' */".into();
    let bytes = serde_json::to_vec(&deep).unwrap();
    let Diagnostic::Deep(hidden) =
        registry::parse(Bundle::Deep, &bytes, "watch_monitor", Profile::Incident).unwrap()
    else {
        panic!()
    };
    assert!(
        hidden
            .long_transactions
            .iter()
            .all(|t| t.query_text.is_none())
    );
    let Diagnostic::Deep(secure) =
        registry::parse(Bundle::DeepText, &bytes, "watch_monitor", Profile::Secure).unwrap()
    else {
        panic!()
    };
    assert!(
        secure
            .long_transactions
            .iter()
            .all(|t| t.query_text.is_none())
    );
    let Diagnostic::Deep(explicit) =
        registry::parse(Bundle::DeepText, &bytes, "watch_monitor", Profile::Incident).unwrap()
    else {
        panic!()
    };
    let text = explicit.long_transactions[0].query_text.as_ref().unwrap();
    assert!(text.contains("visible") && !text.contains("never expose"));
    deep["blockers"] = serde_json::json!(
        (0..21)
            .map(|_| serde_json::json!({"pid":10,"blocking_pids":[]}))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        registry::parse(
            Bundle::Deep,
            &serde_json::to_vec(&deep).unwrap(),
            "watch_monitor",
            Profile::Incident
        ),
        Err(ErrorClass::InvalidData)
    );
    assert_eq!(
        registry::parse(
            Bundle::Medium,
            b"{broken",
            "watch_monitor",
            Profile::Standard
        ),
        Err(ErrorClass::InvalidData)
    );
}
#[test]
fn diagnostic_results_are_fenced_by_scope_uid_topology_and_active_request() {
    let mut store = support::seed();
    let mut ticket = support::ticket(&store, 1);
    ticket.class = Class::Medium;
    assert!(store.begin_probe(ticket.clone()));
    let medium = registry::parse(
        Bundle::Medium,
        include_bytes!("../fixtures/postgres/18/medium.json"),
        "watch_monitor",
        Profile::Standard,
    )
    .unwrap();
    let mut wrong = ticket.clone();
    wrong.request = RequestId(99);
    assert!(!store.finish_diagnostic(wrong, Ok(medium.clone()), Tick(0)));
    assert!(store.finish_diagnostic(ticket.clone(), Ok(medium.clone()), Tick(0)));
    let mut failed = ticket.clone();
    failed.request = RequestId(2);
    assert!(store.begin_probe(failed.clone()));
    assert!(store.finish_diagnostic(failed, Err(ErrorClass::Overloaded), Tick(1)));
    let o = &store.diagnostics[&(ticket.pod.key.clone(), Class::Medium)];
    assert!(o.last_good.is_some());
    assert!(matches!(
        o.freshness(Tick(2), std::time::Duration::from_secs(90)),
        Freshness::Stale(_)
    ));
    store.switch_scope();
    assert!(!store.finish_diagnostic(ticket, Ok(medium), Tick(3)));
    assert!(store.diagnostics.is_empty());
}
#[test]
fn archive_deltas_never_compare_counter_resets_or_incompatible_timelines() {
    let mut store = support::seed();
    let t = support::ticket(&store, 1);
    let key = t.pod.key.clone();
    let mut first = support::sample();
    first.archive.stats_reset = Some("2026-11-15T00:00:00Z".into());
    store.begin_probe(t.clone());
    store.finish_probe(t, Ok(first.clone()), Tick(0));
    let mut second = first.clone();
    second.archive.archived_count += 2;
    second.archive.failed_count += 1;
    let t = support::ticket(&store, 2);
    store.begin_probe(t.clone());
    store.finish_probe(t, Ok(second.clone()), Tick(1000));
    assert_eq!(
        store.archive_deltas[&key],
        ArchiveDelta::Counts {
            archived: 2,
            failed: 1,
            elapsed_ms: 1000
        }
    );
    second.archive.stats_reset = Some("2026-11-15T00:00:01Z".into());
    let t = support::ticket(&store, 3);
    store.begin_probe(t.clone());
    store.finish_probe(t, Ok(second.clone()), Tick(2000));
    assert_eq!(store.archive_deltas[&key], ArchiveDelta::Reset);
    second.timeline += 1;
    let t = support::ticket(&store, 4);
    store.begin_probe(t.clone());
    store.finish_probe(t, Ok(second), Tick(3000));
    assert_eq!(store.archive_deltas[&key], ArchiveDelta::Unknown);
}
#[tokio::test(start_paused = true)]
async fn class_admission_is_fair_gated_and_has_no_catch_up_burst() {
    let mut store = support::seed();
    let budgets = Budgets::default();
    let mut scheduler = Scheduler::with_registry(
        budgets,
        std::time::Duration::from_secs(2),
        17,
        Profile::Incident,
        false,
    );
    scheduler.set_diagnostic_view(true);
    scheduler.reconcile(&store, Tick(0));
    let ticket = scheduler.admit(&store, Tick(0)).unwrap();
    assert_eq!(ticket.class, Class::Capabilities);
    store.begin_probe(ticket.clone());
    assert!(scheduler.admit(&store, Tick(0)).is_none());
    store.finish_diagnostic(ticket.clone(), Ok(capabilities()), Tick(1));
    scheduler.finish(&ticket, Ok(()), Tick(1));
    scheduler.reconcile(&store, Tick(1));
    let mut classes = Vec::new();
    for time in 2..8 {
        let now = Tick(time);
        if let Some(ticket) = scheduler.admit(&store, now) {
            classes.push(ticket.class);
            store.begin_probe(ticket.clone());
            if ticket.class == Class::Fast {
                store.finish_probe(ticket.clone(), Ok(support::sample()), now);
            } else {
                let bundle = scheduler.bundle(&store, &ticket, now).unwrap();
                let bytes = if ticket.class == Class::Medium {
                    &include_bytes!("../fixtures/postgres/18/medium.json")[..]
                } else {
                    &include_bytes!("../fixtures/postgres/18/deep.json")[..]
                };
                store.finish_diagnostic(
                    ticket.clone(),
                    Ok(registry::parse(bundle, bytes, "watch_monitor", Profile::Incident).unwrap()),
                    now,
                );
            }
            scheduler.finish(&ticket, Ok(()), now);
        }
    }
    assert!(
        classes.contains(&Class::Fast)
            && classes.contains(&Class::Medium)
            && classes.contains(&Class::Deep)
    );
    scheduler.reconcile(&store, Tick(500000));
    let ticket = scheduler.admit(&store, Tick(500000)).unwrap();
    assert_eq!(ticket.class, Class::Capabilities);
    store.begin_probe(ticket.clone());
    store.finish_diagnostic(ticket.clone(), Ok(capabilities()), Tick(500000));
    scheduler.finish(&ticket, Ok(()), Tick(500000));
    assert!(scheduler.admit(&store, Tick(500000)).is_some());
}
#[test]
fn connection_pressure_sheds_deep_without_disabling_fast_or_medium() {
    let mut store = support::seed();
    cache_capabilities(&mut store, 1, Tick(0));
    let mut scheduler = Scheduler::with_registry(
        Budgets::default(),
        std::time::Duration::from_secs(2),
        9,
        Profile::Incident,
        false,
    );
    scheduler.set_diagnostic_view(true);
    scheduler.reconcile(&store, Tick(1));
    scheduler.pressure(Some(95));
    let ticket = scheduler.admit(&store, Tick(1)).unwrap();
    assert_ne!(ticket.class, Class::Deep);
    let pressure = registry::Connections {
        clients: 27,
        active: 2,
        idle_in_transaction: 1,
        max_connections: 30,
        reserved: 0,
        super_reserved: 3,
    };
    assert_eq!(pressure.pressure_percent(), Some(100));
}
#[test]
fn actual_fast_fixture_still_parses_when_pressure_fields_are_absent() {
    assert!(
        probes::parse(
            include_bytes!("../fixtures/postgres/18/primary-fast.json"),
            "watch_monitor"
        )
        .is_ok()
    );
}

#[test]
fn capability_recapture_after_topology_change_does_not_compare_obsolete_fast_role() {
    let mut store = support::seed();
    let ticket = support::ticket(&store, 1);
    store.begin_probe(ticket.clone());
    store.finish_probe(ticket, Ok(support::sample()), Tick(0));
    let cluster = store.selected.clone().unwrap();
    store.topology.entry(cluster.clone()).or_default().0 += 1;
    let epoch = store.topology[&cluster];
    let mut value = capabilities();
    if let Diagnostic::Capabilities(c) = &mut value {
        c.stamp.in_recovery = true;
        c.stamp.timeline += 1;
    }
    let mut ticket = support::ticket(&store, 2);
    ticket.class = Class::Capabilities;
    store.begin_probe(ticket.clone());
    assert!(store.finish_diagnostic(ticket, Ok(value), Tick(1)));
    assert_eq!(store.topology[&cluster], epoch);
}
