//! Deterministic, freshness-aware health reasons and their navigation targets.
use crate::{
    budget::Budgets,
    evidence::{Association, Data, Evidence},
    model::*,
    navigation::View,
    reducer::Store,
    ui,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{Hash, Hasher},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    Warning,
    Info,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reason {
    pub id: String,
    pub severity: Severity,
    pub cluster: Key,
    pub message: String,
    pub evidence: Key,
    pub confidence: &'static str,
    pub freshness: &'static str,
    pub target: View,
    pub hint: &'static str,
}
pub fn cluster_key<'a>(store: &'a Store, e: &'a Evidence) -> Option<&'a Key> {
    // Events must resolve their current involved UID before label hints can associate them.
    if !matches!(
        e.data,
        Data::Event {
            involved: Some(_),
            ..
        }
    ) {
        match &e.association {
            Association::Label(k) if store.cluster(k).is_some() => return Some(k),
            Association::Owner { key, uid }
                if store.cluster(key).is_some_and(|c| &c.id.uid == uid) =>
            {
                return Some(key);
            }
            Association::Conflict | Association::Owner { .. } => return None,
            _ => {}
        }
    }
    match &e.data {
        Data::Event {
            involved: Some(id), ..
        } => {
            if let Some(c) = store.cluster(&id.key).filter(|c| c.id == *id) {
                return Some(&c.id.key);
            }
            let pod = store
                .pod(&id.key)
                .filter(|p| {
                    p.id == *id
                        && p.cluster
                            .as_ref()
                            .is_some_and(|k| store.pods_for(k).any(|current| current.id == p.id))
                })
                .and_then(|p| p.cluster.as_ref());
            if pod.is_some() {
                return pod;
            }
            if let Data::Event {
                involved_kind: Some(kind),
                ..
            } = &e.data
            {
                return store
                    .evidence_at(*kind, &id.key)
                    .filter(|object| object.id == *id)
                    .and_then(|object| cluster_key(store, object));
            }
            None
        }
        Data::Endpoints { service, .. } => store
            .evidence(Kind::Service)
            .find(|s| s.id.key == *service)
            .and_then(|s| cluster_key(store, s)),
        _ => None,
    }
}
#[derive(Default)]
pub struct Engine {
    stamp: Option<(u64, u64, Option<i64>)>,
    reasons: Vec<Reason>,
    pending: BTreeMap<Key, (String, Tick)>,
    fingerprints: BTreeMap<Key, u64>,
    by_cluster: BTreeMap<Key, Vec<Reason>>,
    pub evaluations: u64,
}
impl Engine {
    /// Reference path for validation and benchmarks: bypasses all memoized results,
    /// preserving only convergence history, which is part of the evaluator state.
    pub fn recompute(&mut self, store: &Store, now: Tick, budgets: &Budgets) -> &[Reason] {
        self.stamp = None;
        self.fingerprints.clear();
        self.update(store, now, budgets)
    }
    pub fn reasons(&self) -> &[Reason] {
        &self.reasons
    }
    pub fn update(&mut self, store: &Store, now: Tick, budgets: &Budgets) -> &[Reason] {
        let stamp = (store.revision, now.0 / 1000, store.utc_ms.map(|t| t / 1000));
        if self.stamp == Some(stamp) {
            return &self.reasons;
        }
        let mut grouped: BTreeMap<Key, Vec<&Evidence>> = store
            .clusters()
            .map(|c| (c.id.key.clone(), Vec::new()))
            .collect();
        for cache in store.watches.values() {
            for object in cache.objects.values() {
                if let Object::Evidence(e) = object
                    && let Some(k) = cluster_key(store, e)
                    && let Some(items) = grouped.get_mut(k)
                {
                    items.push(e);
                }
            }
        }
        self.by_cluster.retain(|key, _| grouped.contains_key(key));
        self.fingerprints.retain(|key, _| grouped.contains_key(key));
        let service_keys: BTreeSet<_> = store
            .evidence(Kind::Service)
            .chain(store.evidence(Kind::OperatorService))
            .map(|s| s.id.key.clone())
            .collect();
        self.pending.retain(|k, _| service_keys.contains(k));
        for cluster in store.clusters() {
            let evidence = &grouped[&cluster.id.key];
            let mut hash = std::hash::DefaultHasher::new();
            // A cluster's normalized source changes and one-second age bucket are its invalidation key.
            cluster.hash(&mut hash);
            (now.0 / 1000).hash(&mut hash);
            store.utc_ms.map(|t| t / 1000).hash(&mut hash);
            store.auxiliary.hash(&mut hash);
            (store.selected.as_ref() == Some(&cluster.id.key)).hash(&mut hash);
            for e in evidence {
                e.hash(&mut hash);
            }
            for p in store.pods_for(&cluster.id.key) {
                p.hash(&mut hash);
                store.samples.get(&p.id.key).hash(&mut hash);
                store.pooler_samples.get(&p.id.key).hash(&mut hash);
            }
            for ((pod, class), observation) in &store.diagnostics {
                if store
                    .pod(pod)
                    .is_some_and(|p| p.cluster.as_ref() == Some(&cluster.id.key))
                {
                    class.hash(&mut hash);
                    observation.hash(&mut hash);
                }
            }
            for (kind, cache) in &store.watches {
                kind.hash(&mut hash);
                cache.status.hash(&mut hash);
                cache.source.hash(&mut hash);
            }
            for kind in [
                Kind::Deployment,
                Kind::OperatorPod,
                Kind::OperatorService,
                Kind::OperatorEndpointSlice,
                Kind::ValidatingWebhook,
                Kind::MutatingWebhook,
            ] {
                for item in store.evidence(kind) {
                    item.hash(&mut hash);
                }
            }
            let fingerprint = hash.finish();
            if self.fingerprints.get(&cluster.id.key) == Some(&fingerprint) {
                continue;
            }
            self.evaluations += 1;
            let reasons = evaluate(store, cluster, evidence, now, budgets, &mut self.pending);
            self.by_cluster.insert(cluster.id.key.clone(), reasons);
            self.fingerprints
                .insert(cluster.id.key.clone(), fingerprint);
        }
        self.reasons = self.by_cluster.values().flatten().cloned().collect();
        self.reasons.sort_by(|a, b| {
            (&a.severity, &a.cluster, &a.id).cmp(&(&b.severity, &b.cluster, &b.id))
        });
        self.stamp = Some(stamp);
        &self.reasons
    }
}
fn evaluate(
    store: &Store,
    cluster: &Cluster,
    evidence: &[&Evidence],
    now: Tick,
    budgets: &Budgets,
    pending: &mut BTreeMap<Key, (String, Tick)>,
) -> Vec<Reason> {
    let mut reasons = Vec::new();
    let key = &cluster.id.key;
    let mut add = |id: &str,
                   severity: Severity,
                   message: String,
                   evidence: Key,
                   target: View,
                   confidence,
                   hint| {
        reasons.push(Reason {
            id: format!("{id}:{evidence}"),
            severity,
            cluster: key.clone(),
            message,
            evidence,
            target,
            confidence,
            freshness: "current",
            hint,
        });
    };
    let synced = |kind| {
        store
            .watches
            .get(&kind)
            .is_some_and(|w| w.status == SyncState::Synced)
    };
    for kind in store.watches.keys() {
        if let Some(w) = store.watches.get(kind)
            && w.status != SyncState::Synced
        {
            add(
                &format!("source.{kind:?}"),
                Severity::Info,
                format!(
                    "{kind:?} evidence {:?}; cluster observation continues",
                    w.status
                ),
                key.clone(),
                View::Doctor,
                "unknown",
                "Review scoped doctor access and source health",
            );
        }
    }
    for pod in store.pods_for(key).filter(|p| p.role.is_some()) {
        if synced(Kind::Pod) && pod.ready == Some(false) {
            add(
                "instance.not_ready",
                Severity::Warning,
                "Instance is not Ready".into(),
                pod.id.key.clone(),
                View::Instances,
                "observed",
                "Inspect pod conditions and selected logs",
            );
        }
        if let Lag::Bytes(bytes) = ui::lag(store, pod, now, budgets)
            && bytes > 64 * 1024 * 1024
            && cluster
                .operational
                .replica
                .delay
                .as_deref()
                .is_none_or(|d| ["0", "0s", "0ms"].contains(&d))
        {
            add(
                "replica.byte_lag",
                Severity::Warning,
                format!("Compatible replica WAL lag {bytes} B"),
                pod.id.key.clone(),
                View::Replication,
                "sampled",
                "Inspect replay/receive progress and replication slots",
            );
        }
    }
    if store.selected.as_ref() == Some(key) {
        for pod in store.pods_for(key).filter(|p| p.role.is_some()) {
            if let Some(o) = store.samples.get(&pod.id.key) {
                if let Attempt::Failed(e) = o.attempt {
                    add(
                        "sql.failure",
                        Severity::Info,
                        format!(
                            "SQL {:?}; retained sample {:?}",
                            e,
                            o.freshness(now, budgets.stale_after())
                        ),
                        pod.id.key.clone(),
                        View::Doctor,
                        "latest-attempt",
                        "Check credentials/capabilities and allow bounded retry; Kubernetes evidence remains independent",
                    );
                }
                if matches!(o.freshness(now, budgets.stale_after()), Freshness::Fresh(_))
                    && let Some(s) = &o.last_good
                {
                    if let Some(c) = &s.value.connections
                        && c.pressure_percent().is_some_and(|p| p >= 85)
                    {
                        add(
                            "connections.pressure",
                            Severity::Warning,
                            format!(
                                "Client connection pressure {:?}%; deep work is shed",
                                c.pressure_percent()
                            ),
                            pod.id.key.clone(),
                            View::Connections,
                            "sampled",
                            "Inspect application pooling and reserved connection policy",
                        );
                    }
                    if matches!(store.archive_deltas.get(&pod.id.key),Some(ArchiveDelta::Counts{failed,..})if *failed>0)
                    {
                        add(
                            "archive.failure_delta",
                            Severity::Warning,
                            "New archive failures since the prior compatible reset epoch".into(),
                            pod.id.key.clone(),
                            View::Replication,
                            "sampled",
                            "Inspect archiver messages and backup destination; cumulative failures are not a current alarm",
                        );
                    }
                }
            }
            if let Some(o) = store
                .diagnostics
                .get(&(pod.id.key.clone(), crate::probes::registry::Class::Medium))
                && matches!(
                    o.freshness(now, std::time::Duration::from_secs(90)),
                    Freshness::Fresh(_)
                )
                && let Some(s) = &o.last_good
                && store.valid_ticket(&s.ticket)
                && let crate::probes::registry::Diagnostic::Medium(m) = &s.value
            {
                for slot in &m.slots {
                    if slot.wal_status.as_deref() == Some("lost") {
                        add(
                            "replication.slot_lost",
                            Severity::Warning,
                            format!("Replication slot {} lost required WAL", slot.slot_name),
                            pod.id.key.clone(),
                            View::Replication,
                            "sampled",
                            "Inspect consumer recovery and retained WAL policy",
                        );
                    } else if !slot.active
                        && slot
                            .retained_bytes
                            .as_ref()
                            .and_then(|b| b.parse::<u64>().ok())
                            .is_some_and(|b| b > 1024 * 1024 * 1024)
                    {
                        add(
                            "replication.slot_retention",
                            Severity::Warning,
                            format!("Inactive slot {} retains over 1 GiB", slot.slot_name),
                            pod.id.key.clone(),
                            View::Replication,
                            "sampled",
                            "Inspect consumer health and storage capacity; do not drop slots blindly",
                        );
                    }
                }
            }
        }
    }
    for pod in store.pods_for(key).filter(|p| p.pooler.is_some()) {
        if let Some(o) = store.pooler_samples.get(&pod.id.key)
            && matches!(
                o.freshness(
                    now,
                    std::time::Duration::from_millis(crate::pooler::STALE_MS)
                ),
                Freshness::Fresh(_)
            )
            && let Some(s) = &o.last_good
            && crate::pooler::valid(store, &s.ticket)
            && s.value.waiting() > 0
        {
            add(
                "pooler.waiting",
                Severity::Warning,
                format!("Pooler has {} waiting clients", s.value.waiting()),
                pod.id.key.clone(),
                View::Poolers,
                "sampled",
                "Inspect pool capacity and application transactions; direct database health is independent",
            );
        }
    }
    for e in evidence {
        match &e.data {
            Data::Pvc {
                phase: Some(phase), ..
            } if synced(Kind::Pvc) && phase != "Bound" => add(
                "storage.unbound",
                Severity::Warning,
                format!("PVC is {phase}"),
                e.id.key.clone(),
                View::Storage,
                "observed",
                "Inspect PVC events and storage provisioner",
            ),
            Data::Pooler {
                desired: Some(desired),
                ready: Some(ready),
                ..
            } if synced(Kind::Pooler) && ready < desired => add(
                "pooler.not_ready",
                Severity::Warning,
                format!("Pooler ready {ready}/{desired}; statistics unknown"),
                e.id.key.clone(),
                View::Poolers,
                "observed",
                "Inspect pooler pod readiness and logs",
            ),
            Data::Event {
                warning: true,
                reason,
                ..
            } if synced(Kind::Event) => add(
                "event.warning",
                Severity::Warning,
                format!("Kubernetes warning: {reason}"),
                e.id.key.clone(),
                View::Events,
                "observed",
                "Inspect related events and logs; event is not a current outage assertion",
            ),
            Data::Service {
                read_write: true, ..
            } => {
                let compatible = synced(Kind::Service)
                    && synced(Kind::EndpointSlice)
                    && synced(Kind::Pod)
                    && synced(Kind::Cluster);
                let primary = cluster
                    .primary
                    .as_ref()
                    .and_then(|name| Key::new(&key.namespace, name).ok())
                    .and_then(|k| store.pods_for(key).find(|p| p.id.key == k));
                let slices: Vec<_> = evidence
                    .iter()
                    .filter_map(|s| match &s.data {
                        Data::Endpoints { service, endpoints } if service == &e.id.key => {
                            Some(endpoints)
                        }
                        _ => None,
                    })
                    .flatten()
                    .collect();
                let targets: Vec<_> = slices
                    .iter()
                    .filter(|s| s.ready == Some(true))
                    .filter_map(|s| s.target.as_ref())
                    .collect();
                let known = compatible
                    && primary.is_some()
                    && slices
                        .iter()
                        .all(|s| s.ready.is_some() && s.target.is_some());
                let mismatch = known
                    && (targets.is_empty()
                        || targets.iter().any(|t| Some(*t) != primary.map(|p| &p.id)));
                if mismatch {
                    let signature = format!("{:?}{:?}{:?}", e.id, primary.map(|p| &p.id), targets);
                    let start = pending
                        .entry(e.id.key.clone())
                        .or_insert((signature.clone(), now));
                    if start.0 != signature {
                        *start = (signature, now);
                    }
                    if now.0.saturating_sub(start.1.0) >= budgets.service_convergence_ms {
                        add("service.primary_mismatch",Severity::Critical,"Read-write service has no ready primary endpoint or points at a different incarnation".into(),e.id.key.clone(),View::Services,"persistent","Inspect EndpointSlices, primary readiness, and failover convergence");
                    } else {
                        add(
                            "service.converging",
                            Severity::Info,
                            "Read-write endpoints are converging; mismatch is pending".into(),
                            e.id.key.clone(),
                            View::Services,
                            "pending",
                            "Allow the bounded convergence window before declaring a mismatch",
                        );
                    }
                } else {
                    pending.remove(&e.id.key);
                    if !known {
                        add(
                            "service.unknown",
                            Severity::Info,
                            "Read-write endpoint identity/readiness is unknown".into(),
                            e.id.key.clone(),
                            View::Services,
                            "unknown",
                            "Wait for independent sources to synchronize",
                        );
                    }
                }
            }
            _ => {}
        }
    }
    reasons.extend(crate::operational::health::evaluate(
        store, cluster, evidence, now, budgets, pending,
    ));
    reasons
}
