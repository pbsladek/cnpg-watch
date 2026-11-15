mod support;
use cnpg_watch::{
    adapters,
    budget::Budgets,
    cli::{Options, Profile},
    collectors,
    config::{Config, Paths},
    evidence::{Association, Data, Endpoint, Evidence, Labels},
    health::Engine,
    logs::{self, Book, Framer, Level, Message, Source, Update},
    model::*,
    navigation::{Filter, History},
    reducer::{Store, WatchUpdate},
};
use serde_json::json;
fn evidence(kind: Kind, name: &str, data: Data) -> Object {
    Object::Evidence(Evidence {
        id: Identity {
            key: Key::new("app-db", name).unwrap(),
            uid: Uid::new(format!("uid-{name}")).unwrap(),
        },
        kind,
        labels: Labels::new(),
        association: Association::Label(support::cluster().id().key.clone()),
        data,
        observed_at: Tick(0),
    })
}
fn apply(store: &mut Store, object: Object) {
    let kind = object.kind();
    store.enable_sources([kind]);
    support::watch(store, kind, WatchUpdate::Apply(object));
    store.watches.get_mut(&kind).unwrap().status = SyncState::Synced;
}
#[test]
fn complete_relist_updates_pod_index_and_recreated_cluster_rejects_old_owner() {
    let mut store = support::seed();
    let c = store.clusters().next().unwrap().clone();
    let mut p = store.pods().next().unwrap().clone();
    p.cluster_uid = Some(c.id.uid.clone());
    apply(&mut store, Object::Pod(p.clone()));
    assert_eq!(store.pods_for(&c.id.key).count(), 1);
    let mut replacement = c.clone();
    replacement.id.uid = Uid::new("new-cluster").unwrap();
    apply(&mut store, Object::Cluster(replacement));
    assert_eq!(store.pods_for(&c.id.key).count(), 0);
    assert!(!store.valid_ticket(&support::ticket(&store, 1)));
    support::watch(&mut store, Kind::Pod, WatchUpdate::Init);
    support::watch(&mut store, Kind::Pod, WatchUpdate::InitDone);
    assert_eq!(store.pods_for(&c.id.key).count(), 0);
    support::watch(&mut store, Kind::Pod, WatchUpdate::Init);
    p.id.key.name = "payments-2".into();
    p.cluster_uid = Some(Uid::new("new-cluster").unwrap());
    support::watch(
        &mut store,
        Kind::Pod,
        WatchUpdate::InitApply(Object::Pod(p)),
    );
    support::watch(&mut store, Kind::Pod, WatchUpdate::InitDone);
    assert_eq!(store.pods_for(&c.id.key).count(), 1);
}
#[test]
fn overlapping_event_scopes_share_a_watch_and_operator_permissions_are_separate() {
    for (all, ns, operator, count) in [
        (false, "app-db", "app-db", 1),
        (false, "app-db", "cnpg-system", 2),
        (true, "app-db", "cnpg-system", 1),
    ] {
        let config = Config::resolve(
            Options {
                namespace: if all { None } else { Some(ns.into()) },
                all_namespaces: Some(all),
                operator_namespace: Some(operator.into()),
                ..Default::default()
            },
            Paths::discover().unwrap(),
        )
        .unwrap();
        let plans = collectors::plans(&config, ns);
        assert_eq!(
            plans
                .iter()
                .filter(|p| matches!(p.kind, Kind::Event | Kind::OperatorEvent))
                .count(),
            count
        );
        assert!(
            plans
                .iter()
                .filter(|p| p.kind == Kind::Deployment)
                .all(|p| p.namespace.as_deref() == Some(operator))
        );
    }
}
#[test]
fn service_mismatch_requires_persistent_fresh_identity_and_clears_after_recovery() {
    let mut store = support::seed();
    let primary = store.pods().next().unwrap().id.clone();
    apply(
        &mut store,
        evidence(
            Kind::Service,
            "payments-rw",
            Data::Service {
                selector: Labels::new(),
                read_write: true,
            },
        ),
    );
    let service = Key::new("app-db", "payments-rw").unwrap();
    let mut other = primary.clone();
    other.uid = Uid::new("old-primary-incarnation").unwrap();
    apply(
        &mut store,
        evidence(
            Kind::EndpointSlice,
            "payments-rw-1",
            Data::Endpoints {
                service: service.clone(),
                endpoints: vec![Endpoint {
                    target: Some(other),
                    ready: Some(true),
                }],
            },
        ),
    );
    let mut engine = Engine::default();
    let budgets = Budgets::default();
    assert!(
        engine
            .update(&store, Tick(0), &budgets)
            .iter()
            .any(|r| r.id.starts_with("service.converging"))
    );
    assert!(
        !engine
            .update(&store, Tick(4999), &budgets)
            .iter()
            .any(|r| r.id.starts_with("service.primary_mismatch"))
    );
    assert!(
        engine
            .update(&store, Tick(5000), &budgets)
            .iter()
            .any(|r| r.id.starts_with("service.primary_mismatch"))
    );
    apply(
        &mut store,
        evidence(
            Kind::EndpointSlice,
            "payments-rw-1",
            Data::Endpoints {
                service,
                endpoints: vec![Endpoint {
                    target: Some(primary),
                    ready: Some(true),
                }],
            },
        ),
    );
    assert!(
        !engine
            .update(&store, Tick(5100), &budgets)
            .iter()
            .any(|r| r.id.starts_with("service.primary_mismatch"))
    );
}
#[test]
fn optional_forbidden_source_never_invalidates_compatible_wal_or_declares_operator_outage() {
    let mut store = support::seed();
    let ticket = support::ticket(&store, 1);
    store.begin_probe(ticket.clone());
    store.finish_probe(ticket, Ok(support::sample()), Tick(0));
    store.enable_sources([Kind::Pvc]);
    support::watch(
        &mut store,
        Kind::Pvc,
        WatchUpdate::Error(ErrorClass::Forbidden),
    );
    let pod = store.pods().next().unwrap();
    assert_eq!(
        cnpg_watch::ui::lag(&store, pod, Tick(1), &Budgets::default()),
        Lag::NotApplicable
    );
    let mut health = Engine::default();
    assert!(
        health
            .update(&store, Tick(1), &Budgets::default())
            .iter()
            .all(|r| r.severity == cnpg_watch::health::Severity::Info)
    );
}
#[test]
fn event_expiry_and_old_uid_association_are_independent_of_quiet_object_age() {
    let mut store = support::seed();
    let mut pod = store.pods().next().unwrap().id.clone();
    pod.uid = Uid::new("gone-pod").unwrap();
    let mut event = evidence(
        Kind::Event,
        "warning",
        Data::Event {
            warning: true,
            reason: "FailedMount".into(),
            message: "details".into(),
            involved: Some(pod),
            involved_kind: Some(Kind::Pod),
            last_seen_ms: Some(1000),
            count: 1,
        },
    );
    if let Object::Evidence(e) = &mut event {
        e.association = Association::Unrelated;
    }
    apply(&mut store, event);
    let e = store.evidence(Kind::Event).next().unwrap();
    assert!(cnpg_watch::health::cluster_key(&store, e).is_none());
    store.expire_events(Tick(20), 2000, 500);
    assert_eq!(store.evidence(Kind::Event).count(), 0);
    assert_eq!(store.clusters().count(), 1);
    assert_eq!(store.pods().count(), 1);
}
#[test]
fn adapters_keep_unknown_values_and_reject_conflicting_ownership() {
    let raw=serde_json::from_value(json!({"apiVersion":"v1","kind":"Service","metadata":{"namespace":"app-db","name":"rw","uid":"service-1","labels":{"cnpg.io/cluster":"payments"},"ownerReferences":[{"apiVersion":"postgresql.cnpg.io/v1","kind":"Cluster","name":"another","uid":"another-id"}]},"spec":{"selector":{"cnpg.io/cluster":"payments","cnpg.io/instanceRole":"primary"}}})).unwrap();
    let Object::Evidence(e) = adapters::service(raw).unwrap() else {
        panic!()
    };
    assert_eq!(e.association, Association::Conflict);
    let raw=serde_json::from_value(json!({"apiVersion":"postgresql.cnpg.io/v1","kind":"Pooler","metadata":{"namespace":"app-db","name":"pool","uid":"pool-1"},"spec":{"cluster":{"name":"payments"},"instances":1}})).unwrap();
    let Object::Evidence(e) = adapters::pooler(raw).unwrap() else {
        panic!()
    };
    assert!(matches!(e.data, Data::Pooler { ready: None, .. }));
}
#[test]
fn filters_are_bounded_validated_and_preserve_label_and_inverse_semantics() {
    let labels = Labels::from([("team".into(), "db".into())]);
    for (filter, yes, no) in [
        ("/pay.*", "payments", "orders"),
        ("/! healthy", "degraded", "healthy"),
        ("/-f pyms", "payments", "orders"),
    ] {
        let f = Filter::parse(filter).unwrap();
        assert!(f.matches(yes, &labels));
        assert!(!f.matches(no, &labels));
    }
    assert!(
        Filter::parse("/-l team=db,env!=prod")
            .unwrap()
            .matches("anything", &labels)
    );
    assert!(Filter::parse("/[").is_err());
    assert!(Filter::parse(&"x".repeat(513)).is_err());
    let mut history = History::default();
    for i in 0..100 {
        history.push(format!(":namespace test-{i}"));
    }
    for _ in 0..50 {
        history.previous();
    }
    assert_eq!(history.previous(), Some(":namespace test-68"));
}
#[test]
fn log_framing_retention_redaction_generations_and_expiry_remain_bounded() {
    let mut framer = Framer::new(16);
    let mut framed = Vec::new();
    for _ in 0..100 {
        framer.push(&[b'x'; 1024], |line, truncated| {
            framed.push((line.to_vec(), truncated))
        });
        assert!(framer.retained() <= 16);
    }
    framer.push(b"\nhello\n", |line, t| framed.push((line.to_vec(), t)));
    assert_eq!(framed[0].0.len(), 16);
    assert!(framed[0].1);
    assert_eq!(framed[1].0, b"hello");
    let line=logs::parse(br#"{"level":"error","msg":"password='do not expose' postgres://role:password@host/db 192.0.2.1","query":"select secret","user":"private-user"}"#,false,Profile::Secure,1);
    assert!(
        !line.text.contains("do not expose")
            && !line.text.contains("role:password")
            && !line.text.contains("192.0.2.1")
            && !line.text.contains("select secret")
    );
    assert_eq!(line.level, Level::Error);
    let mut book = Book::default();
    let source = Source {
        channel: cnpg_watch::logs::Channel::Postgres,
        scope: ScopeEpoch(1),
        key: Key::new("app-db", "payments-1").unwrap(),
        uid: Uid::new("pod-1").unwrap(),
        container: "postgres".into(),
    };
    book.activate(source.clone(), 2, Tick(0));
    let budgets = Budgets {
        log_source_bytes: 1024,
        log_total_bytes: 2048,
        ..Default::default()
    };
    for i in 0..1000 {
        book.accept(
            Message {
                source: source.clone(),
                generation: 2,
                update: Update::Lines(vec![logs::parse(b"safe line", false, Profile::Standard, i)]),
            },
            Tick(1),
            &budgets,
        );
        assert!(book.bytes <= 1024);
    }
    let before = book.bytes;
    book.accept(
        Message {
            source: source.clone(),
            generation: 1,
            update: Update::Lines(vec![line]),
        },
        Tick(2),
        &budgets,
    );
    assert_eq!(book.bytes, before);
    assert_eq!(book.rejected, 1);
    book.retain_active(&Default::default(), Tick(3));
    book.enforce(Tick(budgets.log_idle_ms + 4), &budgets);
    assert!(book.sources.is_empty() && book.bytes == 0);
}
#[tokio::test(start_paused = true)]
async fn initialization_deadline_terminates_only_its_own_watch_and_cancellation_reclaims_queue() {
    let (tx, mut rx) = cnpg_watch::queue::channel(8, 4096);
    let budgets = Budgets {
        watch_sync_ms: 10,
        ..Default::default()
    };
    let cancel = tokio_util::sync::CancellationToken::new();
    let stream = futures::stream::pending::<
        Result<
            kube::runtime::watcher::Event<kube::api::DynamicObject>,
            kube::runtime::watcher::Error,
        >,
    >();
    let error = collectors::consume(
        stream,
        adapters::service,
        Kind::Service,
        ScopeEpoch(1),
        WatchEpoch(0),
        &tx,
        &cancel,
        &budgets,
    )
    .await;
    assert_eq!(error, ErrorClass::Timeout);
    assert!(matches!(
        rx.try_recv().unwrap().value.update,
        WatchUpdate::Error(ErrorClass::Timeout)
    ));
    assert_eq!(rx.queued_bytes(), 0);
}

#[test]
fn cached_health_matches_full_recomputation_through_changes_and_time() {
    let mut store = support::seed();
    let budgets = Budgets::default();
    let mut cached = Engine::default();
    let mut oracle = Engine::default();
    for step in 0..40 {
        let mut pod = store.pods().next().unwrap().clone();
        pod.ready = Some(step % 3 != 0);
        apply(&mut store, Object::Pod(pod));
        let now = Tick(step * 1000);
        let expected = oracle.recompute(&store, now, &budgets).to_vec();
        assert_eq!(cached.update(&store, now, &budgets), expected);
        let evaluated = cached.evaluations;
        cached.update(&store, now, &budgets);
        assert_eq!(cached.evaluations, evaluated);
    }
}
#[test]
fn empty_live_endpoint_slice_is_valid_evidence() {
    let value = json!({"apiVersion":"discovery.k8s.io/v1","kind":"EndpointSlice","metadata":{"name":"payments-rw","namespace":"app-db","uid":"slice-uid","labels":{"kubernetes.io/service-name":"payments-rw"}},"addressType":"IPv4","endpoints":null});
    assert!(adapters::endpoints(serde_json::from_value(value).unwrap()).is_ok());
}

#[test]
fn captured_fleet_resource_shapes_normalize_through_production_adapters() {
    macro_rules! check {
        ($file:literal, $adapter:path) => {
            for value in serde_json::from_slice::<Vec<serde_json::Value>>(include_bytes!(concat!(
                "../fixtures/cnpg/1.30.0/fleet/",
                $file
            )))
            .unwrap()
            {
                $adapter(serde_json::from_value(value).unwrap()).unwrap();
            }
        };
    }
    check!("Cluster.json", adapters::cluster);
    check!("Pod.json", adapters::pod);
    check!("OperatorPod.json", adapters::operator_pod);
    check!("Event.json", adapters::event);
    check!("Pvc.json", adapters::pvc);
    check!("Service.json", adapters::service);
    check!("EndpointSlice.json", adapters::endpoints);
    check!("Pooler.json", adapters::pooler);
    check!("Deployment.json", adapters::deployment);
    check!("Lease.json", adapters::lease);
}
