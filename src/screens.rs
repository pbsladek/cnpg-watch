//! Render-only rows are built from accepted evidence; no collector or setup I/O.
#[path = "screens/logs.rs"]
mod log_rows;
mod operational;
use crate::{
    app::Session,
    evidence::{Data, Labels},
    model::*,
    navigation::{Filter, View},
    probes::registry::{Class, Diagnostic},
    ui,
};
use ratatui::Frame;
pub mod render;
pub use render::{Dashboard, Secondary};
#[derive(Clone)]
pub struct ScreenRow {
    pub id: String,
    pub key: Key,
    pub cells: Vec<String>,
    pub target: Option<View>,
    pub severity: Option<crate::health::Severity>,
}
impl ScreenRow {
    pub fn severity(mut self, severity: crate::health::Severity) -> Self {
        self.severity = Some(severity);
        self
    }
}
#[derive(Default)]
pub struct Content {
    pub title: String,
    pub headers: Vec<&'static str>,
    pub rows: Vec<ScreenRow>,
    pub empty: String,
    pub extra: String,
    pub dashboard: Option<Dashboard>,
}
fn row(id: impl ToString, key: &Key, cells: Vec<String>, target: Option<View>) -> ScreenRow {
    ScreenRow {
        id: id.to_string(),
        key: key.clone(),
        cells,
        target,
        severity: None,
    }
}
pub fn content(
    session: &Session,
    view: View,
    filter: &Filter,
    log_window: crate::logs::timeline::Window,
    severity: Option<crate::logs::Level>,
) -> Content {
    if view == View::Logs {
        return log_rows::content(session, view, filter, log_window, severity);
    }
    if matches!(
        view,
        View::Backups
            | View::Databases
            | View::Roles
            | View::Subscriptions
            | View::Plugins
            | View::Certs
    ) {
        return operational::content(session, view, filter, log_window, severity);
    }
    let store = &session.store;
    let now = session.clock.now();
    let mut out=Content{title:format!("{view:?}"),empty:"No matching rows (filter active), no associated evidence, or source not synchronized; see :doctor".into(),..Default::default()};
    let mut push = |row: ScreenRow, labels: &Labels| {
        if filter.matches(&row.cells.join(" "), labels) {
            out.rows.push(row);
        }
    };
    match view {
        View::Fleet => {
            out.headers = vec!["cluster", "phase", "instances ready", "SQL", "health"];
            for c in store.clusters() {
                let pods: Vec<_> = store
                    .pods_for(&c.id.key)
                    .filter(|p| p.role.is_some())
                    .collect();
                let readiness = if store
                    .watches
                    .get(&Kind::Pod)
                    .is_some_and(|w| w.status == SyncState::Synced)
                {
                    format!(
                        "{}/{}",
                        pods.iter().filter(|p| p.ready == Some(true)).count(),
                        pods.len()
                    )
                } else {
                    "unavailable".into()
                };
                let sample = pods
                    .iter()
                    .filter_map(|p| store.samples.get(&p.id.key))
                    .filter_map(|o| o.last_good.as_ref())
                    .min_by_key(|s| s.at);
                let sql = sample.map_or_else(
                    || "never sampled".into(),
                    |s| {
                        format!(
                            "{} {}s",
                            if store.selected.as_ref() == Some(&c.id.key) {
                                "sampled"
                            } else {
                                "cached"
                            },
                            now.age(s.at).as_secs()
                        )
                    },
                );
                let reason = session
                    .health
                    .reasons()
                    .iter()
                    .find(|r| r.cluster == c.id.key)
                    .map_or_else(
                        || "no current finding".into(),
                        |r| format!("{:?}: {}", r.severity, r.message),
                    );
                push(
                    row(
                        format!("cluster:{}", c.id.key),
                        &c.id.key,
                        vec![
                            c.id.key.to_string(),
                            c.phase.clone().unwrap_or_else(|| "unknown".into()),
                            readiness,
                            sql,
                            reason,
                        ],
                        Some(View::Instances),
                    ),
                    &c.labels,
                );
            }
            if out.rows.is_empty() {
                out.empty = format!(
                    "Clusters {:?}; zero visible clusters or filter matched none",
                    store.watches.get(&Kind::Cluster).map(|w| w.status)
                );
            }
        }
        View::Instances => {
            out.headers = vec!["instance", "role", "ready", "LSN", "lag", "sample"];
            let derived = ui::derive(store, now, &session.budgets);
            for r in instances(&derived).rows {
                let labels = store.pod(&r.key).map(|p| &p.labels);
                push(r, labels.unwrap_or(&Labels::new()));
            }
            out.extra = derived.archive;
        }
        View::Health | View::Hints => {
            out.headers = vec![
                "evidence",
                "severity",
                "reason / hint",
                "confidence",
                "target",
            ];
            for r in session.health.reasons() {
                let message = if view == View::Hints {
                    r.hint.to_owned()
                } else {
                    r.message.clone()
                };
                push(
                    row(
                        &r.id,
                        &r.evidence,
                        vec![
                            r.evidence.to_string(),
                            format!("{:?}", r.severity),
                            crate::logs::redact(&message, session.config().profile),
                            format!("{} / {}", r.confidence, r.freshness),
                            format!("{:?}", r.target),
                        ],
                        Some(r.target),
                    )
                    .severity(r.severity),
                    &Labels::new(),
                );
            }
            out.extra="Findings clear with recovery/expiry. Missing optional access is independent of cluster availability.".into();
        }
        View::Events | View::Storage | View::Services | View::Poolers | View::Operator => {
            out.headers = vec!["resource", "evidence", "state / details"];
            let kinds = match view {
                View::Events => vec![Kind::Event],
                View::Storage => vec![Kind::Pvc],
                View::Services => vec![Kind::Service, Kind::EndpointSlice],
                View::Poolers => vec![Kind::Pooler, Kind::PoolerDeployment],
                _ => vec![
                    Kind::Deployment,
                    Kind::OperatorPod,
                    Kind::OperatorEvent,
                    Kind::Lease,
                    Kind::OperatorService,
                    Kind::OperatorEndpointSlice,
                    Kind::ValidatingWebhook,
                    Kind::MutatingWebhook,
                ],
            };
            for kind in kinds {
                for e in store.evidence(kind) {
                    if view != View::Operator
                        && crate::health::cluster_key(store, e) != store.selected.as_ref()
                    {
                        continue;
                    }
                    if let Data::Operational(resource) = &e.data
                        && matches!(
                            resource.as_ref(),
                            crate::operational::Resource::Webhook { rules: 0, .. }
                        )
                    {
                        continue;
                    }
                    let (summary, target) = match &e.data {
                        Data::Operational(value) => (
                            crate::operational::summary(
                                value,
                                session.config().profile == crate::cli::Profile::Secure,
                            ),
                            None,
                        ),
                        Data::Event {
                            warning,
                            reason,
                            message,
                            count,
                            ..
                        } => (
                            format!(
                                "{} {reason}: {} (count {count})",
                                if *warning { "warning" } else { "normal" },
                                crate::logs::redact(message, session.config().profile)
                            ),
                            Some(View::Logs),
                        ),
                        Data::Pvc {
                            phase,
                            requested,
                            capacity,
                            reported_state,
                        } => (
                            format!(
                                "{} requested {} capacity {}; role {:?}; CNPG state {reported_state:?}; association {:?}",
                                phase.as_deref().unwrap_or("unknown"),
                                requested.as_deref().unwrap_or("unknown"),
                                capacity.as_deref().unwrap_or("unknown"),
                                e.labels.get("cnpg.io/pvcRole"),
                                e.association
                            ),
                            None,
                        ),
                        Data::Service {
                            read_write,
                            selector,
                        } => (
                            format!(
                                "{} selector {selector:?}",
                                if *read_write { "read-write" } else { "service" }
                            ),
                            None,
                        ),
                        Data::Endpoints { service, endpoints } => (
                            format!(
                                "service {} ready {}/{}; missing identity {}",
                                service,
                                endpoints.iter().filter(|e| e.ready == Some(true)).count(),
                                endpoints.len(),
                                endpoints.iter().filter(|e| e.target.is_none()).count()
                            ),
                            None,
                        ),
                        Data::Pooler {
                            desired,
                            ready,
                            mode,
                            phase,
                            type_,
                            image,
                            unavailable,
                        } => {
                            let pods: Vec<_> = crate::health::cluster_key(store, e)
                                .into_iter()
                                .flat_map(|k| store.pods_for(k))
                                .filter(|p| {
                                    p.pooler.as_ref() == Some(&e.id.key)
                                        && crate::pooler::owns_pod(store, &e.id, p)
                                })
                                .collect();
                            let state = if store
                                .watches
                                .get(&Kind::Pod)
                                .is_some_and(|w| w.status == SyncState::Synced)
                            {
                                format!(
                                    "UID-owned pod-ready {}/{}",
                                    pods.iter().filter(|p| p.ready == Some(true)).count(),
                                    desired.map_or_else(|| "unknown".into(), |n| n.to_string())
                                )
                            } else {
                                "pod readiness unavailable".into()
                            };
                            (
                                format!(
                                    "{state}; CR ready {ready:?}; unavailable {unavailable:?}; phase {phase:?}; type {type_:?}; image {image:?}; mode {}; {}",
                                    mode.as_deref().unwrap_or("unknown"),
                                    crate::pooler::summary(
                                        store,
                                        &e.id.key,
                                        now,
                                        session.config().pooler_stats,
                                        session.config().profile
                                    )
                                ),
                                Some(View::Logs),
                            )
                        }
                        Data::Deployment {
                            desired,
                            ready,
                            images,
                            watch_namespaces,
                            reconciliation,
                            ..
                        } => (
                            format!(
                                "ready {:?}/{:?}; images {}; WATCH_NAMESPACE {:?}; generation {:?}/{:?}; {}; association is inferred",
                                ready,
                                desired,
                                images.join(","),
                                watch_namespaces,
                                reconciliation.observed_generation,
                                reconciliation.generation,
                                reconciliation.state()
                            ),
                            None,
                        ),
                        Data::OperatorPod { ready, .. } => {
                            (format!("ready {ready:?}"), Some(View::Logs))
                        }
                        Data::ReplicaSet { deployment } => {
                            (format!("owner Deployment {deployment:?}"), None)
                        }
                        Data::Lease { holder, renewed_ms } => (
                            format!(
                                "holder {} renewed {:?}; visibility does not prove database outage",
                                holder.as_deref().unwrap_or("unknown"),
                                renewed_ms
                            ),
                            None,
                        ),
                    };
                    push(
                        row(
                            format!("{kind:?}:{}", e.id.key),
                            &e.id.key,
                            vec![e.id.key.to_string(), format!("{kind:?}"), summary],
                            target,
                        ),
                        &e.labels,
                    );
                }
            }
            if view == View::Operator {
                out.extra=session.operator.as_ref().map_or_else(||"Operator disabled or unresolved".into(),|o|format!("Discovery {:?}; candidates {:?}; namespace {:?}. No operator outage inferred from access gaps.",o.state,o.candidates,o.namespace));
            }
        }
        View::Logs => unreachable!("log projection dispatched above"),
        View::Connections | View::Waits | View::Replication | View::Replicas => {
            out.headers = vec!["instance", "evidence", "details / sample state"];
            let class = if matches!(view, View::Replication | View::Replicas) {
                Class::Medium
            } else {
                Class::Deep
            };
            for p in store.selected_pods().filter(|p| p.role.is_some()) {
                if let Some(o) = store.diagnostics.get(&(p.id.key.clone(), class)) {
                    let state = format!(
                        "{:?} / {:?}",
                        o.attempt,
                        o.freshness(
                            now,
                            Duration::from_millis(if class == Class::Medium {
                                90_000
                            } else {
                                30_000
                            })
                        )
                    );
                    if let Some(last) = &o.last_good {
                        match &last.value {
                            Diagnostic::Medium(m) => {
                                push(
                                    row(
                                        format!("medium:{}", p.id.key),
                                        &p.id.key,
                                        vec![
                                            p.id.key.to_string(),
                                            "replication settings".into(),
                                            format!(
                                                "slots returned {}/{}; sync setting {}; {state}",
                                                m.slots.len(),
                                                m.slot_count,
                                                m.settings
                                                    .get("synchronous_standby_names")
                                                    .map_or("unknown", String::as_str)
                                            ),
                                        ],
                                        None,
                                    ),
                                    &p.labels,
                                );
                                for slot in &m.slots {
                                    push(
                                        row(
                                            format!("slot:{}:{}", p.id.key, slot.slot_name),
                                            &p.id.key,
                                            vec![
                                                p.id.key.to_string(),
                                                format!(
                                                    "slot {} {}",
                                                    slot.slot_name, slot.slot_type
                                                ),
                                                format!(
                                                    "active {} WAL {:?} retained {:?} B; {state}",
                                                    slot.active,
                                                    slot.wal_status,
                                                    slot.retained_bytes
                                                ),
                                            ],
                                            None,
                                        ),
                                        &p.labels,
                                    );
                                }
                                for peer in &m.replication {
                                    push(
                                        row(
                                            format!("peer:{}:{}", p.id.key, peer.application_name),
                                            &p.id.key,
                                            vec![
                                                p.id.key.to_string(),
                                                "peer".into(),
                                                format!(
                                                    "{} {} {} replay {:?}; {state}",
                                                    peer.application_name,
                                                    peer.state,
                                                    peer.sync_state,
                                                    peer.replay_lsn
                                                ),
                                            ],
                                            None,
                                        ),
                                        &p.labels,
                                    );
                                }
                            }
                            Diagnostic::Deep(d) => {
                                push(
                                    row(
                                        format!("connections:{}", p.id.key),
                                        &p.id.key,
                                        vec![
                                            p.id.key.to_string(),
                                            "connections".into(),
                                            format!(
                                                "clients {} max {} active {} idle-in-tx {} pressure {:?}%; {state}",
                                                d.connections.clients,
                                                d.connections.max_connections,
                                                d.connections.active,
                                                d.connections.idle_in_transaction,
                                                d.connections.pressure_percent()
                                            ),
                                        ],
                                        None,
                                    ),
                                    &p.labels,
                                );
                                if view == View::Waits {
                                    for w in &d.waits {
                                        push(
                                            row(
                                                format!(
                                                    "wait:{}:{:?}:{:?}",
                                                    p.id.key, w.wait_event_type, w.wait_event
                                                ),
                                                &p.id.key,
                                                vec![
                                                    p.id.key.to_string(),
                                                    "wait".into(),
                                                    format!(
                                                        "{:?}/{:?}: {}",
                                                        w.wait_event_type, w.wait_event, w.count
                                                    ),
                                                ],
                                                None,
                                            ),
                                            &p.labels,
                                        );
                                    }
                                    for b in &d.blockers {
                                        push(
                                            row(
                                                format!("blocker:{}:{}", p.id.key, b.pid),
                                                &p.id.key,
                                                vec![
                                                    p.id.key.to_string(),
                                                    "blocker".into(),
                                                    format!(
                                                        "pid {} blocked by {:?}",
                                                        b.pid, b.blocking_pids
                                                    ),
                                                ],
                                                None,
                                            ),
                                            &p.labels,
                                        );
                                    }
                                    for t in &d.long_transactions {
                                        let query = if session.config().profile
                                            == crate::cli::Profile::Incident
                                            && session.config().show_query_text
                                        {
                                            t.query_text.as_deref().unwrap_or("[query text absent]")
                                        } else {
                                            "[query text redacted]"
                                        };
                                        push(
                                            row(
                                                format!("transaction:{}:{}", p.id.key, t.pid),
                                                &p.id.key,
                                                vec![
                                                    p.id.key.to_string(),
                                                    "transaction".into(),
                                                    format!(
                                                        "pid {} tx {:?} ms query {:?} ms {query}",
                                                        t.pid, t.xact_age_ms, t.query_age_ms
                                                    ),
                                                ],
                                                None,
                                            ),
                                            &p.labels,
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    } else {
                        push(
                            row(
                                format!("never:{}", p.id.key),
                                &p.id.key,
                                vec![p.id.key.to_string(), "never sampled".into(), state],
                                None,
                            ),
                            &p.labels,
                        );
                    }
                } else {
                    push(
                        row(
                            format!("disabled:{}", p.id.key),
                            &p.id.key,
                            vec![
                                p.id.key.to_string(),
                                "never sampled".into(),
                                if class == Class::Deep
                                    && session.config().profile != crate::cli::Profile::Incident
                                {
                                    "disabled by profile; select incident".into()
                                } else {
                                    "capability check pending or unsupported; see :doctor".into()
                                },
                            ],
                            None,
                        ),
                        &p.labels,
                    );
                }
            }
            out.extra = format!(
                "{:?}; overload shedding {}; query text opt-in {}; budgets remain enforced",
                session.scheduler.stats,
                session.scheduler.shedding(now),
                session.config().show_query_text
            );
        }
        View::Backups
        | View::Databases
        | View::Roles
        | View::Subscriptions
        | View::Plugins
        | View::Certs => unreachable!("operational projection dispatched above"),
        View::Doctor | View::Profile => {}
    }
    if view == View::Operator {
        let key = crate::auxiliary::Key::Metrics;
        if let Some(observation) = store.auxiliary.get(&key) {
            let owner = Key::new(
                session
                    .config()
                    .operator_namespace
                    .as_deref()
                    .unwrap_or(&session.namespace),
                "operator-metrics",
            )
            .expect("validated scope");
            let sample = observation.last_good.as_ref().map_or_else(
                || "not yet sampled".into(),
                |s| match &s.value {
                    crate::auxiliary::Sample::Metrics(values) => values
                        .iter()
                        .map(|(metric, value)| format!("{}={}", metric.name(), value.get()))
                        .collect::<Vec<_>>()
                        .join("; "),
                    _ => "incompatible sample".into(),
                },
            );
            out.rows.push(row(
                "operator-metrics",
                &owner,
                vec![
                    "configured operator metrics".into(),
                    "optional enrichment".into(),
                    format!(
                        "{sample}; {:?}; {:?}; counters are cumulative, labels omitted",
                        observation.attempt,
                        observation.freshness(now, crate::auxiliary::stale_after(&key))
                    ),
                ],
                None,
            ));
        } else if session.config().operator_metrics_url.is_none() {
            out.extra
                .push_str(" Optional metrics disabled (no configured URL).");
        }
    }
    if view == View::Replicas
        && let Some(cluster) = store.selected.as_ref().and_then(|k| store.cluster(k))
    {
        out.extra = format!(
            "Replica configuration {:?}; current/target {:?}/{:?}; durable/safe promotion is not inferred. {}",
            cluster.operational.replica,
            cluster.primary,
            cluster.operational.target_primary,
            out.extra
        );
        for e in store
            .evidence(Kind::FailoverQuorum)
            .filter(|e| crate::health::cluster_key(store, e) == Some(&cluster.id.key))
        {
            if let Data::Operational(resource) = &e.data {
                out.rows.push(row(
                    format!("quorum:{}", e.id.key),
                    &e.id.key,
                    vec![
                        e.id.key.to_string(),
                        "reported quorum".into(),
                        crate::operational::summary(
                            resource,
                            session.config().profile == crate::cli::Profile::Secure,
                        ),
                    ],
                    None,
                ));
            }
        }
    }
    if matches!(view, View::Poolers | View::Connections) {
        for observation in store
            .pooler_samples
            .values()
            .filter(|o| crate::pooler::valid(store, &o.latest_ticket))
        {
            let Some(sample) = &observation.last_good else {
                continue;
            };
            let state = format!(
                "{:?}; {:?}",
                observation.freshness(now, Duration::from_millis(crate::pooler::STALE_MS)),
                observation.attempt
            );
            for (index, pool) in sample.value.pools.iter().enumerate() {
                let identity = if session.config().profile == crate::cli::Profile::Secure {
                    "[identifiers redacted]".into()
                } else {
                    format!("{}/{}", pool.database, pool.user)
                };
                let cells = vec![
                    sample.ticket.pod.key.to_string(),
                    "PgBouncer pool".into(),
                    format!(
                        "{identity} clients active {} waiting {}; servers active {} idle {}; max wait {:?} us; {state}",
                        pool.clients_active,
                        pool.clients_waiting,
                        pool.servers_active,
                        pool.servers_idle,
                        pool.max_wait_us
                    ),
                ];
                if filter.matches(&cells.join(" "), &Labels::new()) {
                    out.rows.push(row(
                        format!("pool:{}:{index}", sample.ticket.pod.key),
                        &sample.ticket.pod.key,
                        cells,
                        None,
                    ));
                }
            }
            for (index, stats) in sample.value.stats.iter().enumerate() {
                let database = if session.config().profile == crate::cli::Profile::Secure {
                    "[redacted]"
                } else {
                    &stats.database
                };
                let cells = vec![
                    sample.ticket.pod.key.to_string(),
                    "PgBouncer stats".into(),
                    format!(
                        "{database} total queries {} transactions {} wait {:?} us; average query rate {:?}/s; {state}",
                        stats.total_queries,
                        stats.total_transactions,
                        stats.total_wait_us,
                        stats.average_queries_per_second
                    ),
                ];
                if filter.matches(&cells.join(" "), &Labels::new()) {
                    out.rows.push(row(
                        format!("pool-stats:{}:{index}", sample.ticket.pod.key),
                        &sample.ticket.pod.key,
                        cells,
                        None,
                    ));
                }
            }
        }
    }
    if matches!(view, View::Fleet | View::Instances) {
        out.dashboard = Some(dashboard(
            store,
            session.health.reasons(),
            now,
            session.config().profile,
            session.config().pooler_stats,
            view == View::Fleet,
        ));
    }
    out
}
use std::time::Duration;
pub fn draw(
    frame: &mut Frame<'_>,
    content: &Content,
    header: &str,
    status: &str,
    selected: Option<&str>,
    help: bool,
) {
    render::draw(
        frame,
        content,
        header,
        status,
        selected,
        help,
        Secondary::Events,
    );
}

/// Bounded overview from canonical facts; shared by the real app and load workload.
pub fn dashboard(
    store: &crate::reducer::Store,
    reasons: &[crate::health::Reason],
    now: Tick,
    profile: crate::cli::Profile,
    stats: bool,
    fleet: bool,
) -> Dashboard {
    let relevant = |key: &Key| fleet || store.selected.as_ref() == Some(key);
    let health = reasons
        .iter()
        .filter(|r| relevant(&r.cluster))
        .take(3)
        .map(|r| format!("{:?}: {}", r.severity, r.message))
        .collect::<Vec<_>>()
        .join(" | ");
    let health = if health.is_empty() {
        "No current findings; inspect source/SQL availability before inferring health".into()
    } else {
        health
    };
    let poolers = store
        .evidence(Kind::Pooler)
        .filter(|e| crate::health::cluster_key(store, e).is_some_and(relevant))
        .take(3)
        .map(|e| {
            format!(
                "{}: {}",
                e.id.key,
                crate::pooler::summary(store, &e.id.key, now, stats, profile)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut events = Vec::with_capacity(4);
    for event in store
        .evidence(Kind::Event)
        .filter(|e| crate::health::cluster_key(store, e).is_some_and(relevant))
    {
        let time = if let Data::Event { last_seen_ms, .. } = event.data {
            last_seen_ms
        } else {
            None
        };
        events.push((time, event));
        events.sort_by_key(|(time, _)| std::cmp::Reverse(*time));
        events.truncate(3);
    }
    let events = events
        .into_iter()
        .filter_map(|(_, e)| {
            if let Data::Event {
                reason, message, ..
            } = &e.data
            {
                Some(
                    crate::logs::redact(&format!("{}: {reason}: {message}", e.id.key), profile)
                        .chars()
                        .take(256)
                        .collect::<String>(),
                )
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Dashboard {
        health,
        poolers: if poolers.is_empty() {
            format!(
                "No associated poolers; source {:?}",
                store.watches.get(&Kind::Pooler).map(|w| w.status)
            )
        } else {
            poolers
        },
        events: if events.is_empty() {
            format!(
                "No associated events; source {:?}",
                store.watches.get(&Kind::Event).map(|w| w.status)
            )
        } else {
            events
        },
    }
}

pub fn instances(view: &ui::ViewModel) -> Content {
    Content {
        title: "Instances".into(),
        headers: vec!["instance", "role", "ready", "LSN", "lag", "sample"],
        rows: view
            .rows
            .iter()
            .map(|r| {
                row(
                    format!("pod:{}", r.key),
                    &r.key,
                    vec![
                        r.key.name.clone(),
                        r.role.clone(),
                        r.ready.clone(),
                        r.lsn.clone(),
                        r.lag.clone(),
                        r.state.clone(),
                    ],
                    Some(View::Logs),
                )
            })
            .collect(),
        empty: view.empty.clone(),
        extra: view.archive.clone(),
        dashboard: None,
    }
}
