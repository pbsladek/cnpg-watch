//! Current, generation-aware operational findings; no networking or wall-clock reads.
use super::Resource;
use crate::{
    budget::Budgets,
    evidence::{Data, Evidence},
    health::{Reason, Severity},
    model::*,
    navigation::View,
    reducer::Store,
};
use std::collections::BTreeMap;

pub fn evaluate(
    store: &Store,
    cluster: &Cluster,
    evidence: &[&Evidence],
    now: Tick,
    budgets: &Budgets,
    pending: &mut BTreeMap<Key, (String, Tick)>,
) -> Vec<Reason> {
    let mut reasons = Vec::new();
    let mut add = |id: &str, severity, message: String, source: &Key, target, confidence, hint| {
        reasons.push(Reason {
            id: format!("{id}:{source}"),
            severity,
            cluster: cluster.id.key.clone(),
            message,
            evidence: source.clone(),
            confidence,
            freshness: "current",
            target,
            hint,
        })
    };
    if matches!(
        cluster.operational.availability,
        Availability::Unavailable(_)
    ) {
        add(
            "cluster.operational_fields",
            Severity::Info,
            "Operational Cluster fields failed validation; core topology remains available".into(),
            &cluster.id.key,
            View::Doctor,
            "unknown",
            "Review source/version posture; malformed optional fields do not hide direct PostgreSQL evidence",
        );
    }
    let synced = |kind| {
        store
            .watches
            .get(&kind)
            .is_some_and(|w| w.status == SyncState::Synced)
    };
    if synced(Kind::Cluster) {
        for condition in &cluster.operational.conditions {
            if condition.status != "False"
                || !["Ready", "ContinuousArchiving"].contains(&condition.type_.as_str())
            {
                continue;
            }
            let current = cluster.operational.generation.is_some()
                && condition.observed_generation == cluster.operational.generation;
            add(
                if current {
                    "cluster.condition_failed"
                } else {
                    "cluster.condition_generation_unknown"
                },
                if current {
                    Severity::Warning
                } else {
                    Severity::Info
                },
                format!(
                    "Cluster condition {} reports False; {}",
                    condition.type_,
                    if current {
                        "current generation"
                    } else {
                        "observed generation is older or unknown"
                    }
                ),
                &cluster.id.key,
                if condition.type_ == "ContinuousArchiving" {
                    View::Backups
                } else {
                    View::Health
                },
                if current {
                    "current-generation"
                } else {
                    "uncertain"
                },
                "Compare current controller reconciliation and direct database evidence",
            );
        }
    }
    let successful = evidence
        .iter()
        .filter_map(|e| match &e.data {
            Data::Operational(resource) => match resource.as_ref() {
                Resource::Backup {
                    phase,
                    stopped_ms,
                    started_ms,
                    ..
                } if phase.as_deref() == Some("completed") => stopped_ms.or(*started_ms),
                _ => None,
            },
            _ => None,
        })
        .chain(cluster.operational.latest_success_ms)
        .max();
    for e in evidence {
        if !synced(e.kind) {
            continue;
        }
        let Data::Operational(resource) = &e.data else {
            continue;
        };
        match resource.as_ref() {
            Resource::Backup {
                phase,
                stopped_ms,
                started_ms,
                ..
            } if phase.as_deref() == Some("failed") => {
                let failure = stopped_ms.or(*started_ms);
                if successful.is_none() || failure.zip(successful).is_some_and(|(f, s)| f > s) {
                    add("backup.failed",Severity::Warning,"A backup attempt failed without a later reported success; recovery readiness is not established".into(),&e.id.key,View::Backups,"controller-report","Inspect backup method, plugin capabilities and archiver evidence; verify a restore separately");
                } else if failure.is_none() {
                    add(
                        "backup.history_unknown",
                        Severity::Info,
                        "Failed backup has no ordering timestamp; a successful backup also exists"
                            .into(),
                        &e.id.key,
                        View::Backups,
                        "uncertain",
                        "Inspect backup timestamps rather than treating history as a current outage",
                    );
                }
            }
            Resource::Backup { phase, .. }
                if phase
                    .as_deref()
                    .is_some_and(|p| ["pending", "started", "running"].contains(&p)) =>
            {
                add(
                    "backup.active",
                    Severity::Info,
                    "Backup is active or pending; success is not yet reported".into(),
                    &e.id.key,
                    View::Backups,
                    "controller-report",
                    "Inspect backup progress and its selected logs",
                )
            }
            Resource::Schedule {
                failure_reported: true,
                suspended,
                ..
            } if *suspended != Some(true) => add(
                "backup.schedule_error",
                Severity::Warning,
                "Backup scheduler reports an error".into(),
                &e.id.key,
                View::Backups,
                "controller-report",
                "Inspect schedule validation and controller events; missing next-run evidence stays unknown",
            ),
            Resource::Managed {
                reconciliation: r, ..
            } => {
                let target = match e.kind {
                    Kind::Database => View::Databases,
                    Kind::DatabaseRole => View::Roles,
                    _ => View::Subscriptions,
                };
                if r.current() && r.applied == Some(false) {
                    add(
                        "managed.failed",
                        Severity::Warning,
                        "Managed object application failed for its current generation".into(),
                        &e.id.key,
                        target,
                        "current-generation",
                        "Inspect controller events and role/database dependencies; arbitrary controller payloads are not exposed",
                    );
                } else if r
                    .generation
                    .zip(r.observed_generation)
                    .is_some_and(|(g, o)| o < g)
                {
                    add("managed.pending_generation",Severity::Info,"Desired generation has not yet been observed; old application status is retained as historical".into(),&e.id.key,target,"pending","Allow reconciliation and inspect controller events if progress stops");
                }
            }
            Resource::Quorum {
                primary: Some(primary),
                ..
            } if cluster.primary.as_ref() != Some(primary) => add(
                "replica.quorum_old_primary",
                Severity::Info,
                "Quorum report names a different primary; promotion safety is unknown".into(),
                &e.id.key,
                View::Replicas,
                "uncertain",
                "Compare current primary, compatible SQL timelines and the latest quorum report",
            ),
            _ => {}
        }
    }
    for name in &cluster.operational.unusable {
        if let Ok(key) = Key::new(&cluster.id.key.namespace, name) {
            add(
                "storage.unusable",
                Severity::Warning,
                "Cluster reports an unusable PVC".into(),
                &key,
                View::Storage,
                "controller-report",
                "Inspect PVC role/coherence and events; do not delete PGDATA or WAL volumes blindly",
            );
        }
    }
    for plugin in &cluster.operational.plugins {
        if plugin.configured
            && plugin.enabled != Some(false)
            && plugin.version.is_none()
            && plugin.capabilities.is_empty()
        {
            add(
                "plugin.unreported",
                Severity::Info,
                format!(
                    "Plugin {} is configured without reported capability/version evidence",
                    plugin.name
                ),
                &cluster.id.key,
                View::Plugins,
                "unknown",
                "Inspect plugin availability and CNPG-I capabilities; missing status alone does not prove a database outage",
            );
        }
    }
    for (key, observation) in &store.auxiliary {
        let fresh = matches!(
            observation.freshness(now, crate::auxiliary::stale_after(key)),
            Freshness::Fresh(_)
        );
        if let Some(sample) = &observation.last_good {
            match (&sample.value, key) {
                (
                    crate::auxiliary::Sample::Certificate(cert),
                    crate::auxiliary::Key::Certificate(key),
                ) if fresh
                    && key.namespace == cluster.id.key.namespace
                    && cluster.operational.certificate_secrets.contains(&key.name) =>
                {
                    if let Some(utc) = store.utc_ms {
                        if cert.not_after_ms <= utc {
                            add("certificate.expired",Severity::Warning,"Current sampled certificate has expired; TLS trust/chain validity is not assessed".into(),key,View::Certs,"sampled-certificate","Review operator-managed certificate rotation; private keys are never collected");
                        } else if cert.not_after_ms.saturating_sub(utc) <= 7 * 86400 * 1000 {
                            add(
                                "certificate.expiring",
                                Severity::Warning,
                                "Current sampled certificate expires within seven days".into(),
                                key,
                                View::Certs,
                                "sampled-certificate",
                                "Review certificate rotation and the latest sample age",
                            );
                        } else if cert.not_before_ms > utc {
                            add(
                                "certificate.not_yet_valid",
                                Severity::Warning,
                                "Sampled certificate is not yet valid by the supplied wall clock"
                                    .into(),
                                key,
                                View::Certs,
                                "sampled-certificate",
                                "Check clocks and certificate rotation; metadata does not verify the TLS chain",
                            );
                        }
                    }
                }
                (crate::auxiliary::Sample::Metrics(metrics), crate::auxiliary::Key::Metrics)
                    if fresh =>
                {
                    if metrics
                        .get(&crate::auxiliary::Metric::QueueDepth)
                        .is_some_and(|v| v.get() > 100.)
                    {
                        add("operator.queue_depth",Severity::Info,"Optional metrics report aggregate queue depth above 100; database availability is independent".into(),&cluster.id.key,View::Operator,"sampled-gauge","Inspect operator queue and reconciliation; cumulative error counters alone are not a current alarm");
                    }
                }
                _ => {}
            }
        }
    }
    for deployment in store.evidence(Kind::Deployment) {
        let Data::Deployment {
            desired,
            ready,
            reconciliation,
            ..
        } = &deployment.data
        else {
            continue;
        };
        if !synced(Kind::Deployment) {
            continue;
        }
        if reconciliation
            .generation
            .zip(reconciliation.observed_generation)
            .is_some_and(|(g, o)| o < g)
        {
            add(
                "operator.pending_generation",
                Severity::Info,
                "Operator Deployment has not observed its desired generation".into(),
                &deployment.id.key,
                View::Operator,
                "pending",
                "Inspect operator rollout and events; database availability is independent",
            );
        } else if reconciliation.current() && desired.zip(*ready).is_some_and(|(d, r)| r < d) {
            add("operator.not_ready",Severity::Warning,"Operator Deployment reports fewer ready replicas than desired for the current generation".into(),&deployment.id.key,View::Operator,"current-generation","Inspect operator readiness and admission routing; direct PostgreSQL health remains independent");
        }
    }
    for hook in [Kind::ValidatingWebhook, Kind::MutatingWebhook]
        .into_iter()
        .flat_map(|kind| store.evidence(kind))
    {
        if !synced(hook.kind) {
            continue;
        }
        let Data::Operational(resource) = &hook.data else {
            continue;
        };
        let Resource::Webhook { services, .. } = resource.as_ref() else {
            continue;
        };
        for service in services {
            let status = webhook_routing(store, service);
            match status {
                Routing::NoReadyEndpoints | Routing::MissingService => {
                    let signature = format!("{status:?}");
                    let since = pending
                        .entry(service.clone())
                        .or_insert((signature.clone(), now));
                    if since.0 != signature {
                        *since = (signature, now);
                    }
                    if now.0.saturating_sub(since.1.0) >= budgets.service_convergence_ms {
                        add(
                            "operator.admission_route",
                            Severity::Warning,
                            format!(
                                "Admission routing {status:?}; this is endpoint evidence, not a TLS/application probe"
                            ),
                            service,
                            View::Operator,
                            "persistent",
                            "Inspect webhook service, EndpointSlices and operator pod readiness",
                        );
                    }
                }
                Routing::Unknown => {
                    pending.remove(service);
                }
                Routing::Ready => {
                    pending.remove(service);
                }
            }
        }
    }
    reasons
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routing {
    Ready,
    NoReadyEndpoints,
    MissingService,
    Unknown,
}
pub fn webhook_routing(store: &Store, service: &Key) -> Routing {
    let services = [Kind::OperatorService, Kind::Service];
    let visible = services.iter().any(|k| {
        store.watches.get(k).is_some_and(|w| {
            w.status == SyncState::Synced
                && w.source
                    .as_ref()
                    .is_some_and(|s| s.covers_namespace(&service.namespace) && s.selector.is_none())
        })
    });
    let found = services
        .into_iter()
        .filter_map(|k| store.evidence_at(k, service))
        .any(|_| true);
    if !found {
        return if visible {
            Routing::MissingService
        } else {
            Routing::Unknown
        };
    }
    let kinds = [Kind::OperatorEndpointSlice, Kind::EndpointSlice];
    let synchronized = kinds.iter().any(|k| {
        store.watches.get(k).is_some_and(|w| {
            w.status == SyncState::Synced
                && w.source
                    .as_ref()
                    .is_some_and(|s| s.covers_namespace(&service.namespace))
        })
    });
    if !synchronized {
        return Routing::Unknown;
    }
    let mut unknown = false;
    for e in kinds.into_iter().flat_map(|k| store.evidence(k)) {
        if let Data::Endpoints {
            service: key,
            endpoints,
        } = &e.data
            && key == service
        {
            for endpoint in endpoints {
                if endpoint.ready == Some(true) {
                    if let Some(id) = &endpoint.target {
                        let current = id.key.namespace == service.namespace
                            && (store
                                .pod(&id.key)
                                .is_some_and(|p| p.id == *id && p.ready == Some(true))
                                || store.evidence(Kind::OperatorPod).any(|p| {
                                    p.id == *id
                                        && matches!(
                                            p.data,
                                            Data::OperatorPod {
                                                ready: Some(true),
                                                ..
                                            }
                                        )
                                }));
                        if current {
                            return Routing::Ready;
                        }
                    }
                    unknown = true;
                } else if endpoint.ready.is_none() {
                    unknown = true;
                }
            }
        }
    }
    if unknown {
        Routing::Unknown
    } else {
        Routing::NoReadyEndpoints
    }
}
