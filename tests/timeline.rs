mod support;
use cnpg_watch::{
    budget::Budgets,
    cli::Profile,
    evidence::{Association, Data, Evidence},
    logs::{
        self, Book, Channel, Message, Source, Update,
        timeline::{self, Scope, Sources, Window},
    },
    model::*,
    reducer::WatchUpdate,
};
fn operator(store: &mut cnpg_watch::reducer::Store) -> Source {
    let source = Source {
        channel: Channel::Operator,
        scope: store.scope,
        key: Key::new("cnpg-system", "controller").unwrap(),
        uid: Uid::new("op-uid").unwrap(),
        container: "manager".into(),
    };
    store.enable_sources([Kind::OperatorPod]);
    support::watch(
        store,
        Kind::OperatorPod,
        WatchUpdate::Apply(Object::Evidence(Evidence {
            id: source.identity(),
            kind: Kind::OperatorPod,
            labels: Default::default(),
            association: Association::Unrelated,
            data: Data::OperatorPod {
                ready: Some(true),
                containers: vec!["manager".into()],
            },
            observed_at: Tick(0),
        })),
    );
    source
}
fn put(book: &mut Book, source: &Source, raw: &str, generation: u64, budgets: &Budgets) {
    book.accept(
        Message {
            source: source.clone(),
            generation,
            update: Update::Lines(vec![logs::parse(
                raw.as_bytes(),
                false,
                Profile::Secure,
                999,
            )]),
        },
        Tick(1),
        budgets,
    );
}
#[test]
fn metadata_only_correlation_timestamp_disorder_namespace_and_uid_fences() {
    let mut store = support::seed();
    let source = operator(&mut store);
    let mut book = Book::default();
    let budgets = Budgets::default();
    book.activate(source.clone(), 1, Tick(0));
    let context = |time: &str, extra: &str| {
        format!(
            r#"{{"ts":"2026-11-15T00:00:{time}Z","msg":"safe","cluster":{{"namespace":"app-db","name":"payments"}},{extra}}}"#
        )
    };
    put(
        &mut book,
        &source,
        &context("02", r#""reconcileID":"opaque-private-id""#),
        1,
        &budgets,
    );
    put(
        &mut book,
        &source,
        r#"{"ts":"2026-11-15T00:00:01Z","msg":"second","reconcileID":"opaque-private-id"}"#,
        1,
        &budgets,
    );
    put(
        &mut book,
        &source,
        r#"{"ts":"2026-11-15T00:00:03Z","msg":"payments in text alone must not correlate"}"#,
        1,
        &budgets,
    );
    put(
        &mut book,
        &source,
        r#"{"ts":"2026-11-15T00:00:03Z","cluster":{"namespace":"other","name":"payments"},"msg":"wrong namespace"}"#,
        1,
        &budgets,
    );
    put(
        &mut book,
        &source,
        r#"{"ts":"2026-11-15T00:00:04Z","cluster":{"namespace":"app-db","name":"payments","uid":"old-uid"},"reconcileID":"opaque-private-id","msg":"old uid"}"#,
        1,
        &budgets,
    );
    put(
        &mut book,
        &source,
        &context(
            "59",
            r#""msg":"password=PASSWORD_MARKER query: QUERY_MARKER""#,
        ),
        1,
        &budgets,
    );
    let projection = timeline::project(
        &store,
        &book,
        &Scope::Cluster,
        Sources::default(),
        Window::Follow,
    );
    assert_eq!(projection.entries.len(), 3);
    assert!(
        projection
            .entries
            .windows(2)
            .all(|p| p[0].timestamp_ms <= p[1].timestamp_ms)
    );
    assert!(
        projection
            .entries
            .iter()
            .all(|e| e.source.key.namespace == "cnpg-system")
    );
    let text = serde_json::to_string(
        &book
            .sources
            .values()
            .flat_map(|s| s.lines.iter())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for marker in ["opaque-private-id", "PASSWORD_MARKER", "QUERY_MARKER"] {
        assert!(!text.contains(marker), "{marker}");
    }
    assert_eq!(
        timeline::project(
            &store,
            &book,
            &Scope::Unmatched,
            Sources::default(),
            Window::Follow
        )
        .entries
        .len(),
        3
    );
    let switches = Sources {
        operator: false,
        ..Default::default()
    };
    assert!(
        timeline::project(&store, &book, &Scope::Operator, switches, Window::Follow)
            .entries
            .is_empty()
    );
    let mut old = source.clone();
    old.uid = Uid::new("old-op").unwrap();
    book.activate(old.clone(), 1, Tick(1));
    put(
        &mut book,
        &old,
        &context("05", r#""msg":"old controller""#),
        1,
        &budgets,
    );
    assert_eq!(
        timeline::project(
            &store,
            &book,
            &Scope::Cluster,
            Sources::default(),
            Window::Follow
        )
        .entries
        .len(),
        3
    );
    store.switch_scope();
    assert!(
        timeline::project(
            &store,
            &book,
            &Scope::Cluster,
            Sources::default(),
            Window::Follow
        )
        .entries
        .is_empty()
    );
}
#[test]
fn pause_uses_global_arrival_order_across_sources_eviction_and_source_generations() {
    let store = support::fleet(1);
    let mut book = Book::default();
    let budgets = Budgets {
        log_source_bytes: 128 * 1024,
        ..Default::default()
    };
    let pods: Vec<_> = store.pods().take(2).collect();
    let sources: Vec<_> = pods
        .into_iter()
        .map(|p| Source {
            channel: Channel::Postgres,
            scope: store.scope,
            key: p.id.key.clone(),
            uid: p.id.uid.clone(),
            container: "postgres".into(),
        })
        .collect();
    for s in &sources {
        book.activate(s.clone(), 1, Tick(0));
    }
    put(&mut book, &sources[0], "first", 1, &budgets);
    put(&mut book, &sources[1], "second", 1, &budgets);
    let cutoff = book.last_sequence();
    put(&mut book, &sources[0], "later", 1, &budgets);
    let frozen = timeline::project(
        &store,
        &book,
        &Scope::Selected,
        Sources::default(),
        Window::Paused(cutoff),
    );
    assert_eq!(frozen.entries.len(), 2);
    assert!(frozen.entries.iter().all(|e| e.line.text != "later"));
    for i in 0..1500 {
        put(&mut book, &sources[0], &format!("line {i}"), 1, &budgets);
    }
    let follow = timeline::project(
        &store,
        &book,
        &Scope::Selected,
        Sources::default(),
        Window::Follow,
    );
    assert_eq!(follow.entries.len(), timeline::MAX_ROWS);
    assert!(follow.index_bytes <= timeline::MAX_INDEX_BYTES);
    assert!(book.bytes <= budgets.log_total_bytes);
    let previous = book.bytes;
    book.activate(sources[0].clone(), 2, Tick(2));
    put(&mut book, &sources[0], "obsolete", 1, &budgets);
    assert!(book.bytes < previous);
    assert!(
        !book.sources[&sources[0]]
            .lines
            .iter()
            .any(|l| l.text == "obsolete")
    );
    let last = book.last_sequence();
    book.clear();
    book.activate(sources[0].clone(), 3, Tick(3));
    put(&mut book, &sources[0], "new", 3, &budgets);
    assert!(book.last_sequence() > last);
}
#[test]
fn malformed_json_is_redacted_before_retention_and_events_are_immutable_deduplicated_records() {
    for raw in [
        r#"{"password":"SECRET_MARKER""#,
        r#"{"query":"QUERY_MARKER""#,
        r#"{"database":"DATABASE_MARKER""#,
    ] {
        let line = logs::parse(raw.as_bytes(), false, Profile::Secure, 1);
        assert!(!line.text.contains("MARKER"));
        assert!(line.context.is_none());
    }
    let mut store = support::seed();
    let cluster = store.clusters().next().unwrap().id.clone();
    let event = Evidence {
        id: Identity {
            key: Key::new("app-db", "event").unwrap(),
            uid: Uid::new("event").unwrap(),
        },
        kind: Kind::Event,
        labels: Default::default(),
        association: Association::Unrelated,
        data: Data::Event {
            warning: true,
            reason: "Fixture".into(),
            message: "password=SECRET_MARKER".into(),
            involved: Some(cluster),
            involved_kind: Some(Kind::Cluster),
            last_seen_ms: Some(1794700800000),
            count: 1,
        },
        observed_at: Tick(0),
    };
    store.enable_sources([Kind::Event]);
    support::watch(
        &mut store,
        Kind::Event,
        WatchUpdate::Apply(Object::Evidence(event.clone())),
    );
    let mut book = Book::default();
    let mut events = logs::events::Events::default();
    let budgets = Budgets::default();
    events.sync(
        &store,
        &mut book,
        &budgets,
        logs::events::Selection {
            cluster: true,
            operator: false,
        },
        Profile::Secure,
        Tick(0),
    );
    let cutoff = book.last_sequence();
    events.sync(
        &store,
        &mut book,
        &budgets,
        logs::events::Selection {
            cluster: true,
            operator: false,
        },
        Profile::Secure,
        Tick(1),
    );
    assert_eq!(book.last_sequence(), cutoff);
    let mut event = event;
    if let Data::Event { count, .. } = &mut event.data {
        *count = 2;
    }
    support::watch(
        &mut store,
        Kind::Event,
        WatchUpdate::Apply(Object::Evidence(event)),
    );
    events.sync(
        &store,
        &mut book,
        &budgets,
        logs::events::Selection {
            cluster: true,
            operator: false,
        },
        Profile::Secure,
        Tick(2),
    );
    assert!(book.last_sequence() > cutoff);
    let frozen = timeline::project(
        &store,
        &book,
        &Scope::Cluster,
        Sources::default(),
        Window::Paused(cutoff),
    );
    assert_eq!(frozen.entries.len(), 1);
    assert!(frozen.entries[0].line.text.contains("count 1"));
    assert!(!frozen.entries[0].line.text.contains("SECRET_MARKER"));
    assert!(events.bytes <= timeline::MAX_INDEX_BYTES);
}
#[test]
fn reconcile_anchor_flood_and_event_flood_remain_bounded() {
    let mut store = support::seed();
    let first = operator(&mut store);
    let mut second = first.clone();
    second.key.name = "controller-two".into();
    second.uid = Uid::new("operator-two").unwrap();
    let mut evidence = store.evidence(Kind::OperatorPod).next().unwrap().clone();
    evidence.id = second.identity();
    support::watch(
        &mut store,
        Kind::OperatorPod,
        WatchUpdate::Apply(Object::Evidence(evidence)),
    );
    let mut book = Book::default();
    let budgets = Budgets::default();
    for source in [&first, &second] {
        book.activate(source.clone(), 1, Tick(0));
        let lines=(0..600).map(|i|{let raw=format!(r#"{{"ts":"2026-11-15T00:00:00Z","cluster":{{"namespace":"app-db","name":"payments"}},"reconcileID":"private-id-{i}","msg":"safe"}}"#);logs::parse(raw.as_bytes(),false,Profile::Secure,i)}).collect();
        book.accept(
            Message {
                source: source.clone(),
                generation: 1,
                update: Update::Lines(lines),
            },
            Tick(1),
            &budgets,
        );
    }
    let projected = timeline::project(
        &store,
        &book,
        &Scope::Cluster,
        Sources::default(),
        Window::Follow,
    );
    assert_eq!(projected.entries.len(), timeline::MAX_ROWS);
    assert!(projected.index_bytes <= timeline::MAX_INDEX_BYTES);
    assert!(projected.omitted > 0);
    assert!(book.bytes <= budgets.log_total_bytes);
    assert!(
        projected
            .entries
            .iter()
            .all(|e| !e.line.text.contains("private-id"))
    );
    let cluster = store.clusters().next().unwrap().id.clone();
    store.enable_sources([Kind::Event]);
    for i in 0..1200 {
        let event = Evidence {
            id: Identity {
                key: Key::new("app-db", format!("event-{i}")).unwrap(),
                uid: Uid::new(format!("event-{i}")).unwrap(),
            },
            kind: Kind::Event,
            labels: Default::default(),
            association: Association::Unrelated,
            data: Data::Event {
                warning: false,
                reason: "Fixture".into(),
                message: "safe".into(),
                involved: Some(cluster.clone()),
                involved_kind: Some(Kind::Cluster),
                last_seen_ms: Some(1794700800000 + i),
                count: 1,
            },
            observed_at: Tick(0),
        };
        support::watch(
            &mut store,
            Kind::Event,
            WatchUpdate::Apply(Object::Evidence(event)),
        );
    }
    let mut events = logs::events::Events::default();
    events.sync(
        &store,
        &mut book,
        &budgets,
        logs::events::Selection {
            cluster: true,
            operator: false,
        },
        Profile::Secure,
        Tick(2),
    );
    assert!(events.omitted > 0);
    assert!(events.bytes <= timeline::MAX_INDEX_BYTES);
    assert!(book.bytes <= budgets.log_total_bytes);
}
