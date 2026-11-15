//! Bounded support output built from allowlisted fields, never raw API/SQL/log data.
use crate::{app::Session, model::*, probes::registry::Diagnostic};
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf, sync::atomic::Ordering};
pub fn snapshot(session: &Session, screen: &str) -> Value {
    let store = &session.store;
    let now = session.clock.now();
    let sources: Vec<_> = store
        .watches
        .iter()
        .map(|(kind, c)| {
            json!({
                "kind": kind, "epoch": c.epoch, "status": c.status,
                "objects": c.objects.len(), "bytes": c.bytes, "staging_bytes": c.staging_bytes,
                "events_received": c.received, "syncs": c.syncs, "errors": c.errors,
                "last_sync_age_ms": c.last_sync.map(|t| now.0.saturating_sub(t.0)),
                "last_error": c.last_error,
                "scope":c.source.as_ref().map(|s|&s.scope),"api_version":c.source.as_ref().map(|s|&s.api_version),"resource":c.source.as_ref().map(|s|&s.resource),
            })
        })
        .collect();
    let samples: Vec<_> = store.samples.iter().take(128).map(|(key, o)| json!({
        "key": key, "latest_attempt": o.attempt,
        "age_ms": o.last_good.as_ref().map(|v| now.0.saturating_sub(v.at.0)),
        "last_good": o.last_good.as_ref().map(|s| json!({
            "ticket": s.ticket, "role": s.value.role, "server_version": s.value.server_version,
            "system_id": s.value.system_id, "timeline": s.value.timeline,
            "current": s.value.current, "replay": s.value.replay,
            "archive": s.value.archive, "connections": s.value.connections,
        })),
        "freshness": format!("{:?}", o.freshness(now, session.budgets.stale_after())),
    })).collect();
    let diagnostics: Vec<_> = store.diagnostics.iter().take(128).map(|((key, class), o)| json!({
        "key": key, "class": class, "latest_attempt": o.attempt,
        "last_good": o.last_good.as_ref().map(|s| json!({
            "ticket": s.ticket, "age_ms": now.0.saturating_sub(s.at.0),
            "summary": match &s.value {
                Diagnostic::Capabilities(c) => json!({ "views": c.views.len(), "functions": c.functions.len() }),
                Diagnostic::Medium(m) => json!({
                    "slot_count": m.slot_count, "returned_slots": m.slots.len(),
                    "replication_peers": m.replication.len(), "settings_count": m.settings.len(),
                }),
                Diagnostic::Deep(d) => json!({
                    "connections": d.connections, "wait_groups": d.waits.len(),
                    "blocker_rows": d.blockers.len(), "long_transactions": d.long_transactions.len(),
                }),
            },
        })),
    })).collect();
    let log_sources: Vec<_> = session
        .logs
        .sources
        .iter()
        .map(|(source, s)| {
            json!({
                "source": source, "generation": s.generation, "active": s.active, "state": s.state,
                "lines": s.lines.len(), "bytes": s.bytes, "evicted": s.evicted,
            })
        })
        .collect();
    json!({
        "schema_version": 1, "binary_version": env!("CARGO_PKG_VERSION"), "view": session.view,
        "scope": {
            "epoch": store.scope, "context": session.config().context, "namespace": session.namespace,
            "all_namespaces": session.config().all_namespaces, "selected": store.selected,
            "topology_epochs": store.topology.iter().take(128)
                .map(|(k, v)| json!({ "cluster": k, "epoch": v })).collect::<Vec<_>>(),
        },
        "profile": session.config().profile,
        "redaction": crate::capabilities::envelope(session.config().profile).redaction,
        "redacted_fields": [
            "passwords", "tokens", "connection_strings", "query_text", "client_addresses",
            "log_messages", "event_messages", "postgres_user_database_application_names", "file_paths",
        ],
        "truncated": store.samples.len() > 128 || store.diagnostics.len() > 128 || store.clusters().count() > 128 || store.pooler_samples.len() > 128 || store.watches.values().flat_map(|w|w.objects.values()).filter(|o|matches!(o,Object::Evidence(e) if matches!(e.data,crate::evidence::Data::Operational(_)))).count() > 128,
        "budgets": session.budgets, "sources": sources, "samples": samples, "diagnostics": diagnostics,
        "operational":{"schema_version":1,"clusters":store.clusters().take(128).map(|c|json!({"identity":c.id,"quality":c.operational.availability,"generation":c.operational.generation,"image":c.operational.image,"operator_version":c.operational.operator_version,"operator_hash":c.operational.operator_hash,"conditions":c.operational.conditions.len(),"plugins":c.operational.plugins.len(),"reported_ready":c.operational.ready,"reported_instances":c.operational.desired,"latest_success_ms":c.operational.latest_success_ms,"latest_failure_ms":c.operational.latest_failure_ms,"reported_recovery_point_ms":c.operational.recovery_point_ms})).collect::<Vec<_>>(),"resources":store.watches.values().flat_map(|w|w.objects.values()).filter_map(|o|if let Object::Evidence(e)=o {if let crate::evidence::Data::Operational(value)=&e.data {Some(json!({"identity":e.id,"kind":e.kind,"association":e.association,"summary":operational_summary(value)}))}else{None}}else{None}).take(128).collect::<Vec<_>>()},
        "auxiliary":store.auxiliary.iter().map(|(key,o)|json!({"source":key,"attempt":o.attempt,"freshness":format!("{:?}",o.freshness(now,crate::auxiliary::stale_after(key))),"sample":o.last_good.as_ref().map(|s|json!({"identity":s.ticket,"age_ms":now.0.saturating_sub(s.at.0),"metadata":s.value}))})).collect::<Vec<_>>(),
        "poolers":store.pooler_samples.iter().take(128).map(|(key,o)|json!({"pod":key,"identity":o.latest_ticket,"attempt":o.attempt,"sample":o.last_good.as_ref().map(|s|json!({"version":s.value.version,"age_ms":now.0.saturating_sub(s.at.0),"pools":s.value.pools.len(),"waiting":s.value.waiting()}))})).collect::<Vec<_>>(),
        "capabilities": crate::capabilities::observed_for(store, session.config(), session.sql.clone()),
        "health": { "reasons": session.health.reasons().len(), "evaluations": session.health.evaluations },
        "scheduler": {
            "classes": session.scheduler.stats, "pending": session.pending_targets(),
            "omitted": session.scheduler.omitted, "shedding": session.scheduler.shedding(now),
        },
        "runtime": {
            "queue_bytes": session.queue_bytes(), "queue_high_bytes": session.max_queue_bytes,
            "cache_bytes": store.bytes(), "task_count": session.task_count(),
            "rejected": store.rejected, "cleanup_errors": session.cleanup_errors,
        },
        "logs": {
            "sources": log_sources, "timeline_schema_version":1,"scope":session.log_scope,"source_toggles":session.log_sources,"max_timeline_rows":crate::logs::timeline::MAX_ROWS,"max_index_bytes":crate::logs::timeline::MAX_INDEX_BYTES,"omitted_streams":session.log_omitted,"event_index_bytes":session.event_logs.bytes,"bytes": session.logs.bytes, "high_bytes": session.logs.high_bytes,
            "queue_drops": session.log_dropped.load(Ordering::Relaxed),
            "truncated": session.logs.truncated, "rejected": session.logs.rejected,
        },
        "screen": crate::logs::redact(screen, session.config().profile).chars().take(8192).collect::<String>(),
    })
}
pub fn write(session: &Session, screen: &str) -> Result<PathBuf, ErrorClass> {
    // Screens containing diagnostic/log/event payloads are omitted from dumps regardless of display opt-in.
    let screen = if matches!(
        session.view,
        crate::navigation::View::Logs
            | crate::navigation::View::Waits
            | crate::navigation::View::Events
            | crate::navigation::View::Replication
            | crate::navigation::View::Poolers
            | crate::navigation::View::Connections
            | crate::navigation::View::Roles
            | crate::navigation::View::Databases
            | crate::navigation::View::Subscriptions
    ) {
        "[content-bearing view omitted]"
    } else {
        screen
    };
    let bytes = serde_json::to_vec_pretty(&snapshot(session, screen))
        .map_err(|_| ErrorClass::InvalidData)?;
    if bytes.len() > session.budgets.dump_bytes {
        return Err(ErrorClass::OutputLimit);
    }
    let dir = &session.config().paths.dumps;
    std::fs::create_dir_all(dir).map_err(|_| ErrorClass::Unavailable)?;
    let path = dir.join(format!(
        "cnpg-watch-{}-{}.json",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
        session.store.scope.0
    ));
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(|_| ErrorClass::Unavailable)?;
    file.write_all(&bytes)
        .map_err(|_| ErrorClass::Unavailable)?;
    Ok(path)
}

fn operational_summary(value: &crate::operational::Resource) -> Value {
    use crate::operational::Resource;
    match value {
        Resource::Backup {
            phase,
            method,
            started_ms,
            stopped_ms,
            failure_reported,
            ..
        } => {
            json!({"phase":phase,"method":method,"started_ms":started_ms,"stopped_ms":stopped_ms,"failure_reported":failure_reported})
        }
        Resource::Schedule {
            suspended,
            last_run_ms,
            next_run_ms,
            failure_reported,
            ..
        } => {
            json!({"suspended":suspended,"reported_last_ms":last_run_ms,"reported_next_ms":next_run_ms,"failure_reported":failure_reported})
        }
        Resource::Managed {
            reconciliation: r, ..
        } => {
            json!({"generation":r.generation,"observed_generation":r.observed_generation,"applied":r.applied,"conditions":r.conditions.len(),"message_present":r.message_present})
        }
        Resource::Quorum {
            standby_names,
            standby_number,
            ..
        } => json!({"standby_names":standby_names.len(),"reported_standby_number":standby_number}),
        Resource::Catalog {
            images,
            component_entries,
            global,
        } => json!({"images":images.len(),"components":component_entries,"cluster_scoped":global}),
        Resource::Webhook {
            services,
            ca_present,
            external_clients,
            rules,
            ..
        } => {
            json!({"routing_services":services,"ca_present":ca_present,"external_clients":external_clients,"cnpg_applicable_hooks":rules})
        }
    }
}
