//! Single-writer state transitions; no networking, terminal calls, or hidden clocks.
use crate::probes::registry::{Class, Diagnostic};
use crate::{budget::Budgets, model::*};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatchUpdate {
    Source(SourceSpec),
    Init,
    InitApply(Object),
    InitDone,
    Apply(Object),
    Delete(Identity),
    Error(ErrorClass),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEvent {
    pub scope: ScopeEpoch,
    pub watch: WatchEpoch,
    pub kind: Kind,
    pub update: WatchUpdate,
}
#[derive(Debug, Clone, Default)]
pub struct Changes {
    pub changed: bool,
    pub cancel: Vec<RequestId>,
}

#[derive(Debug, Clone)]
pub struct Store {
    pub scope: ScopeEpoch,
    pub utc_ms: Option<i64>,
    pub watches: BTreeMap<Kind, Cache>,
    pub topology: BTreeMap<Key, TopologyEpoch>,
    pod_index: BTreeMap<Key, BTreeSet<Key>>,
    pub samples: BTreeMap<Key, Observation<FastSample>>,
    pub diagnostics: BTreeMap<(Key, Class), Observation<Diagnostic>>,
    active_auxiliary: BTreeMap<crate::auxiliary::Key, crate::auxiliary::Ticket>,
    active_poolers: BTreeMap<Key, crate::pooler::Ticket>,
    pub auxiliary: BTreeMap<
        crate::auxiliary::Key,
        Observation<crate::auxiliary::Sample, crate::auxiliary::Ticket>,
    >,
    pub pooler_samples: BTreeMap<Key, crate::pooler::Observation>,
    pub archive_deltas: BTreeMap<Key, ArchiveDelta>,
    pub active: BTreeMap<Key, ProbeTicket>,
    pub selected: Option<Key>,
    pub rejected: u64,
    pub revision: u64,
    budgets: Budgets,
    sample_bytes: usize,
    diagnostic_bytes: usize,
    pooler_bytes: usize,
    auxiliary_bytes: usize,
}
impl Store {
    pub fn new(budgets: Budgets) -> Self {
        Self {
            scope: ScopeEpoch(0),
            utc_ms: None,
            watches: [
                (Kind::Cluster, Cache::default()),
                (Kind::Pod, Cache::default()),
            ]
            .into(),
            topology: BTreeMap::new(),
            pod_index: BTreeMap::new(),
            samples: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            archive_deltas: BTreeMap::new(),
            pooler_samples: BTreeMap::new(),
            auxiliary: BTreeMap::new(),
            active_auxiliary: BTreeMap::new(),
            active_poolers: BTreeMap::new(),
            active: BTreeMap::new(),
            selected: None,
            rejected: 0,
            revision: 0,
            budgets,
            sample_bytes: 0,
            diagnostic_bytes: 0,
            pooler_bytes: 0,
            auxiliary_bytes: 0,
        }
    }
    pub fn objects(&self, kind: Kind) -> impl Iterator<Item = &Object> {
        self.watches
            .get(&kind)
            .into_iter()
            .flat_map(|c| c.objects.values())
    }
    pub fn clusters(&self) -> impl Iterator<Item = &Cluster> {
        self.objects(Kind::Cluster).filter_map(|o| {
            if let Object::Cluster(v) = o {
                Some(v)
            } else {
                None
            }
        })
    }
    pub fn pods(&self) -> impl Iterator<Item = &Pod> {
        self.objects(Kind::Pod).filter_map(|o| {
            if let Object::Pod(v) = o {
                Some(v)
            } else {
                None
            }
        })
    }
    pub fn pods_for<'a>(&'a self, cluster: &'a Key) -> impl Iterator<Item = &'a Pod> {
        self.pod_index
            .get(cluster)
            .into_iter()
            .flatten()
            .filter_map(|key| self.pod(key))
            .filter(|p| {
                p.cluster_uid
                    .as_ref()
                    .is_none_or(|uid| self.cluster(cluster).is_some_and(|c| &c.id.uid == uid))
            })
    }
    pub fn selected_pods(&self) -> impl Iterator<Item = &Pod> {
        self.selected.iter().flat_map(|key| self.pods_for(key))
    }
    pub fn pod(&self, key: &Key) -> Option<&Pod> {
        self.watches
            .get(&Kind::Pod)?
            .objects
            .get(key)
            .and_then(|o| {
                if let Object::Pod(p) = o {
                    Some(p)
                } else {
                    None
                }
            })
    }
    pub fn cluster(&self, key: &Key) -> Option<&Cluster> {
        self.watches
            .get(&Kind::Cluster)?
            .objects
            .get(key)
            .and_then(|o| {
                if let Object::Cluster(c) = o {
                    Some(c)
                } else {
                    None
                }
            })
    }
    pub fn bytes(&self) -> usize {
        self.sample_bytes
            + self.diagnostic_bytes
            + self.pooler_bytes
            + self.auxiliary_bytes
            + self
                .watches
                .values()
                .map(|c| c.bytes + c.staging_bytes)
                .sum::<usize>()
    }
    pub fn begin_auxiliary(&mut self, ticket: crate::auxiliary::Ticket) -> bool {
        if ticket.scope != self.scope
            || self.active_auxiliary.len() >= 9
            || self.active_auxiliary.contains_key(&ticket.key)
        {
            return false;
        }
        self.active_auxiliary.insert(ticket.key.clone(), ticket);
        true
    }
    pub fn begin_pooler(&mut self, ticket: crate::pooler::Ticket) -> bool {
        if !crate::pooler::valid(self, &ticket)
            || self.active_poolers.len() >= self.budgets.pending_probes
            || self.active_poolers.contains_key(&ticket.pod.key)
        {
            return false;
        }
        self.active_poolers.insert(ticket.pod.key.clone(), ticket);
        true
    }
    pub fn finish_auxiliary(
        &mut self,
        ticket: crate::auxiliary::Ticket,
        result: Result<crate::auxiliary::Sample, ErrorClass>,
        now: Tick,
    ) -> bool {
        if ticket.scope != self.scope || self.active_auxiliary.get(&ticket.key) != Some(&ticket) {
            self.rejected += 1;
            return false;
        }
        self.active_auxiliary.remove(&ticket.key);
        let old = self
            .auxiliary
            .get(&ticket.key)
            .and_then(|o| o.last_good.as_ref())
            .map_or(0, |s| s.value.bytes());
        let entry_bytes = if self.auxiliary.contains_key(&ticket.key) {
            0
        } else {
            1024
        };
        if entry_bytes > 0
            && (self.auxiliary.len() >= 9
                || self.bytes().saturating_add(entry_bytes) > self.budgets.cache_bytes)
        {
            self.rejected += 1;
            return false;
        }
        let result = match result {
            Ok(sample)
                if sample.bytes() > self.budgets.object_bytes
                    || self
                        .bytes()
                        .saturating_sub(old)
                        .saturating_add(sample.bytes())
                        .saturating_add(entry_bytes)
                        > self.budgets.cache_bytes =>
            {
                Err(ErrorClass::Capacity)
            }
            r => r,
        };
        self.auxiliary_bytes += entry_bytes;
        let observation = self.auxiliary.entry(ticket.key.clone()).or_default();
        match result {
            Ok(sample) => {
                self.auxiliary_bytes = self
                    .auxiliary_bytes
                    .saturating_sub(old)
                    .saturating_add(sample.bytes());
                observation.attempt = Attempt::Succeeded(ticket.request);
                observation.last_good = Some(Stamped {
                    ticket,
                    value: sample,
                    at: now,
                });
            }
            Err(error) => observation.attempt = Attempt::Failed(error),
        }
        self.revision += 1;
        true
    }
    pub fn finish_pooler(
        &mut self,
        ticket: crate::pooler::Ticket,
        result: Result<crate::pooler::Sample, ErrorClass>,
        now: Tick,
    ) -> bool {
        if self.active_poolers.get(&ticket.pod.key) != Some(&ticket) {
            self.rejected += 1;
            return false;
        }
        self.active_poolers.remove(&ticket.pod.key);
        if !crate::pooler::valid(self, &ticket) {
            self.rejected += 1;
            return false;
        }
        let key = ticket.pod.key.clone();
        let old = self
            .pooler_samples
            .get(&key)
            .and_then(|o| o.last_good.as_ref())
            .map_or(0, |s| s.value.bytes());
        let entry_bytes = if self.pooler_samples.contains_key(&key) {
            0
        } else {
            1024
        };
        let result = match result {
            Ok(sample)
                if sample.bytes() > self.budgets.object_bytes
                    || self
                        .bytes()
                        .saturating_sub(old)
                        .saturating_add(sample.bytes())
                        .saturating_add(entry_bytes)
                        > self.budgets.cache_bytes =>
            {
                Err(ErrorClass::Capacity)
            }
            result => result,
        };
        if !self.pooler_samples.contains_key(&key) {
            if self.entries().saturating_add(self.pooler_samples.len())
                >= self.budgets.cache_entries
                || self.bytes().saturating_add(1024) > self.budgets.cache_bytes
            {
                return false;
            }
            self.pooler_bytes += 1024;
        }
        let observation =
            self.pooler_samples
                .entry(key)
                .or_insert_with(|| crate::pooler::Observation {
                    latest_ticket: ticket.clone(),
                    last_good: None,
                    attempt: Attempt::Never,
                });
        observation.latest_ticket = ticket.clone();
        match result {
            Ok(sample) => {
                self.pooler_bytes = self
                    .pooler_bytes
                    .saturating_sub(old)
                    .saturating_add(sample.bytes());
                observation.attempt = Attempt::Succeeded(ticket.request);
                observation.last_good = Some(Stamped {
                    value: sample,
                    ticket,
                    at: now,
                });
            }
            Err(error) => observation.attempt = Attempt::Failed(error),
        }
        self.revision += 1;
        true
    }
    pub fn prune_pooler_samples(&mut self) {
        let invalid: Vec<_> = self
            .active_poolers
            .iter()
            .filter(|(_, t)| !crate::pooler::valid(self, t))
            .map(|(k, _)| k.clone())
            .collect();
        for key in invalid {
            self.active_poolers.remove(&key);
        }
        let removed: Vec<_> = self
            .pooler_samples
            .iter()
            .filter(|(_, o)| !crate::pooler::valid_cached(self, &o.latest_ticket))
            .map(|(k, _)| k.clone())
            .collect();
        for key in removed {
            if let Some(observation) = self.pooler_samples.remove(&key) {
                self.pooler_bytes = self
                    .pooler_bytes
                    .saturating_sub(1024 + observation.last_good.map_or(0, |s| s.value.bytes()));
            }
            self.revision += 1;
        }
    }
    pub fn entries(&self) -> usize {
        self.watches
            .values()
            .map(|c| c.objects.len() + c.staging.as_ref().map_or(0, BTreeMap::len))
            .sum()
    }
    pub fn select(&mut self, key: Option<Key>) -> Changes {
        if self.selected == key {
            return Changes::default();
        }
        self.selected = key;
        let cancel = self.active.values().map(|t| t.request).collect();
        for key in self.active.keys() {
            if let Some(o) = self.samples.get_mut(key) {
                o.attempt = Attempt::Failed(ErrorClass::Cancelled);
            }
        }
        for o in self.diagnostics.values_mut() {
            if matches!(o.attempt, Attempt::InFlight(_)) {
                o.attempt = Attempt::Failed(ErrorClass::Cancelled);
            }
        }
        self.active.clear();
        self.revision += 1;
        Changes {
            changed: true,
            cancel,
        }
    }
    pub fn switch_scope(&mut self) -> Changes {
        let next = self.scope.0.checked_add(1).expect("scope epoch exhausted");
        let cancel = self.active.values().map(|t| t.request).collect();
        let revision = self.revision + 1;
        *self = Self::new(self.budgets.clone());
        self.scope = ScopeEpoch(next);
        self.revision = revision;
        Changes {
            changed: true,
            cancel,
        }
    }
    fn reject(&mut self) -> Changes {
        self.rejected += 1;
        Changes::default()
    }
    pub fn describe_sources(&mut self, sources: impl IntoIterator<Item = (Kind, SourceSpec)>) {
        for (kind, source) in sources {
            self.watches.entry(kind).or_default().source = Some(source);
        }
    }
    pub fn evidence_at(&self, kind: Kind, key: &Key) -> Option<&crate::evidence::Evidence> {
        match self.watches.get(&kind)?.objects.get(key)? {
            Object::Evidence(e) => Some(e),
            _ => None,
        }
    }
    pub fn enable_sources(&mut self, kinds: impl IntoIterator<Item = Kind>) {
        for kind in kinds {
            self.watches.entry(kind).or_default();
        }
    }
    pub fn evidence(&self, kind: Kind) -> impl Iterator<Item = &crate::evidence::Evidence> {
        self.objects(kind).filter_map(|o| {
            if let Object::Evidence(e) = o {
                Some(e)
            } else {
                None
            }
        })
    }
    pub fn watch(&mut self, event: WatchEvent) -> Changes {
        self.watch_at(event, Tick(0))
    }
    pub fn watch_at(&mut self, mut event: WatchEvent, now: Tick) -> Changes {
        if let WatchUpdate::Apply(Object::Evidence(e))
        | WatchUpdate::InitApply(Object::Evidence(e)) = &mut event.update
        {
            let previous = self
                .watches
                .get(&event.kind)
                .and_then(|c| c.objects.get(&e.id.key))
                .and_then(|o| {
                    if let Object::Evidence(e) = o {
                        Some(e)
                    } else {
                        None
                    }
                });
            e.observed_at = previous.map_or(now, |old| old.observed_at);
            if previous.is_none_or(|old| old != e) {
                e.observed_at = now;
            }
        }

        if event.scope != self.scope {
            return self.reject();
        }
        let Some(cache) = self.watches.get(&event.kind) else {
            return self.reject();
        };
        if event.watch < cache.epoch {
            return self.reject();
        }
        if event.watch > cache.epoch {
            let cache = self.watches.get_mut(&event.kind).unwrap();
            cache.epoch = event.watch;
            cache.staging = None;
            cache.staging_bytes = 0;
            cache.status = SyncState::Restarting;
        }
        self.watches.get_mut(&event.kind).unwrap().received += 1;
        let mut changes = Changes::default();
        match event.update {
            WatchUpdate::Source(spec) => {
                let c = self.watches.get_mut(&event.kind).unwrap();
                if c.source.as_ref() != Some(&spec) {
                    c.source = Some(spec);
                    changes.changed = true;
                }
            }
            WatchUpdate::Init => {
                let c = self.watches.get_mut(&event.kind).unwrap();
                c.staging = Some(BTreeMap::new());
                c.staging_bytes = 0;
                c.status = SyncState::Syncing;
                changes.changed = true;
            }
            WatchUpdate::InitApply(object) => {
                if object.kind() != event.kind || self.watches[&event.kind].staging.is_none() {
                    return self.reject();
                }
                let key = object.id().key.clone();
                let old = self.watches[&event.kind]
                    .staging
                    .as_ref()
                    .unwrap()
                    .get(&key)
                    .map_or(0, Object::bytes);
                let added = usize::from(old == 0);
                if !self.fits(object.bytes(), old, added) {
                    return self.capacity(event.kind);
                }
                let c = self.watches.get_mut(&event.kind).unwrap();
                c.staging_bytes = c.staging_bytes - old + object.bytes();
                c.staging.as_mut().unwrap().insert(key, object);
            }
            WatchUpdate::InitDone => {
                let c = self.watches.get_mut(&event.kind).unwrap();
                let Some(next) = c.staging.take() else {
                    return self.reject();
                };
                let previous = std::mem::replace(&mut c.objects, next);
                c.bytes = c.staging_bytes;
                c.staging_bytes = 0;
                c.status = SyncState::Synced;
                c.syncs += 1;
                c.last_sync = Some(now);
                c.last_error = None;
                let added: Vec<_> = c
                    .objects
                    .iter()
                    .filter(|(k, _)| !previous.contains_key(*k))
                    .map(|(_, o)| o.clone())
                    .collect();
                let differences: Vec<_> = previous
                    .into_iter()
                    .filter(|(k, old)| c.objects.get(k) != Some(old))
                    .collect();
                for object in added {
                    self.invalidate(None, Some(&object), &mut changes);
                }
                for (key, old) in differences {
                    let new = self.watches[&event.kind].objects.get(&key).cloned();
                    self.invalidate(Some(&old), new.as_ref(), &mut changes);
                }
                changes.changed = true;
            }
            WatchUpdate::Apply(object) => {
                if object.kind() != event.kind || self.watches[&event.kind].staging.is_some() {
                    return self.reject();
                }
                let key = object.id().key.clone();
                let c = &self.watches[&event.kind];
                if c.objects.get(&key) == Some(&object) {
                    return changes;
                }
                let old_bytes = c.objects.get(&key).map_or(0, Object::bytes);
                if !self.fits(object.bytes(), old_bytes, usize::from(old_bytes == 0)) {
                    return self.capacity(event.kind);
                }
                let c = self.watches.get_mut(&event.kind).unwrap();
                c.bytes = c.bytes - old_bytes + object.bytes();
                let old = c.objects.insert(key, object.clone());
                self.invalidate(old.as_ref(), Some(&object), &mut changes);
                changes.changed = true;
            }
            WatchUpdate::Delete(id) => {
                let c = self.watches.get_mut(&event.kind).unwrap();
                if c.objects.get(&id.key).is_none_or(|o| o.id() != &id) {
                    return self.reject();
                }
                let old = c.objects.remove(&id.key).unwrap();
                c.bytes -= old.bytes();
                self.invalidate(Some(&old), None, &mut changes);
                changes.changed = true;
            }
            WatchUpdate::Error(class) => {
                let c = self.watches.get_mut(&event.kind).unwrap();
                c.staging = None;
                c.staging_bytes = 0;
                c.status = SyncState::Failed(class);
                c.errors += 1;
                c.last_error = Some(class);
                changes.changed = true;
            }
        }
        if self
            .selected
            .as_ref()
            .is_none_or(|k| self.cluster(k).is_none())
        {
            let selected = self.clusters().next().map(|c| c.id.key.clone());
            self.selected = selected;
        }
        if changes.changed {
            self.revision += 1;
        }
        changes
    }
    fn fits(&self, new: usize, old: usize, entries: usize) -> bool {
        new <= self.budgets.object_bytes
            && self.bytes().saturating_sub(old).saturating_add(new) <= self.budgets.cache_bytes
            && self.entries() + entries <= self.budgets.cache_entries
    }
    fn capacity(&mut self, kind: Kind) -> Changes {
        let c = self.watches.get_mut(&kind).unwrap();
        c.staging = None;
        c.staging_bytes = 0;
        c.status = SyncState::Failed(ErrorClass::Capacity);
        c.last_error = Some(ErrorClass::Capacity);
        c.errors += 1;
        self.revision += 1;
        Changes {
            changed: true,
            cancel: vec![],
        }
    }
    fn invalidate(&mut self, old: Option<&Object>, new: Option<&Object>, changes: &mut Changes) {
        let old_pod = if let Some(Object::Pod(p)) = old {
            Some(p)
        } else {
            None
        };
        let new_pod = if let Some(Object::Pod(p)) = new {
            Some(p)
        } else {
            None
        };
        if old_pod.map(|p| (&p.id.key, &p.cluster)) != new_pod.map(|p| (&p.id.key, &p.cluster)) {
            if let Some(p) = old_pod
                && let Some(cluster) = &p.cluster
                && let Some(keys) = self.pod_index.get_mut(cluster)
            {
                keys.remove(&p.id.key);
                if keys.is_empty() {
                    self.pod_index.remove(cluster);
                }
            }
            if let Some(p) = new_pod
                && let Some(cluster) = &p.cluster
            {
                self.pod_index
                    .entry(cluster.clone())
                    .or_default()
                    .insert(p.id.key.clone());
            }
        }
        let incarnation = old.is_some_and(|old| new.is_none_or(|new| old.id() != new.id()));
        let role_change = matches!((old,new),(Some(Object::Cluster(a)),Some(Object::Cluster(b))) if a.primary!=b.primary)
            || matches!((old,new),(Some(Object::Pod(a)),Some(Object::Pod(b))) if a.role!=b.role || a.cluster!=b.cluster);
        if !incarnation && !role_change {
            return;
        }
        let cluster = match old {
            Some(Object::Cluster(c)) => Some(c.id.key.clone()),
            Some(Object::Pod(p)) => p.cluster.clone(),
            _ => None,
        };
        if let Some(key) = cluster {
            let epoch = self.topology.entry(key.clone()).or_default();
            epoch.0 = epoch.0.checked_add(1).expect("topology epoch exhausted");
            let active: Vec<_> = self
                .active
                .iter()
                .filter(|(_, t)| t.cluster.key == key)
                .map(|(k, t)| (k.clone(), t.request))
                .collect();
            for (pod, request) in active {
                self.active.remove(&pod);
                changes.cancel.push(request);
                if let Some(o) = self.samples.get_mut(&pod) {
                    o.attempt = Attempt::Failed(ErrorClass::Cancelled);
                }
            }
        }
        if incarnation {
            match old {
                Some(Object::Pod(p)) => self.remove_sample(&p.id.key),
                Some(Object::Cluster(c)) => {
                    let keys: Vec<_> = self
                        .samples
                        .iter()
                        .filter(|(key, o)| {
                            o.last_good
                                .as_ref()
                                .is_some_and(|s| s.ticket.cluster.key == c.id.key)
                                || self
                                    .pod(key)
                                    .is_some_and(|p| p.cluster.as_ref() == Some(&c.id.key))
                        })
                        .map(|(k, _)| k.clone())
                        .collect();
                    for key in keys {
                        self.remove_sample(&key);
                    }
                    let extra: Vec<_> = self
                        .diagnostics
                        .iter()
                        .filter(|((key, _), o)| {
                            o.last_good
                                .as_ref()
                                .is_some_and(|s| s.ticket.cluster.key == c.id.key)
                                || self
                                    .pod(key)
                                    .is_some_and(|p| p.cluster.as_ref() == Some(&c.id.key))
                        })
                        .map(|((k, _), _)| k.clone())
                        .collect();
                    for key in extra {
                        self.remove_sample(&key);
                    }
                    if new.is_none() {
                        self.topology.remove(&c.id.key);
                    }
                }
                _ => {}
            }
        }
    }
    pub fn expire_events(&mut self, now: Tick, wall_ms: i64, retention_ms: u64) {
        let mut changed = false;
        for kind in [Kind::Event, Kind::OperatorEvent] {
            if let Some(cache) = self.watches.get_mut(&kind) {
                cache.objects.retain(|_, o| {
                    let keep = if let Object::Evidence(e) = o {
                        if let crate::evidence::Data::Event { last_seen_ms, .. } = &e.data {
                            now.0.saturating_sub(e.observed_at.0) < retention_ms
                                && last_seen_ms
                                    .is_none_or(|t| wall_ms.saturating_sub(t) < retention_ms as i64)
                        } else {
                            true
                        }
                    } else {
                        true
                    };
                    if !keep {
                        cache.bytes = cache.bytes.saturating_sub(o.bytes());
                        changed = true;
                    }
                    keep
                });
            }
        }
        if changed {
            self.revision += 1;
        }
    }
    fn remove_sample(&mut self, key: &Key) {
        if let Some(o) = self.samples.remove(key) {
            self.sample_bytes = self
                .sample_bytes
                .saturating_sub(o.last_good.as_ref().map_or(512, |s| s.value.bytes() + 512));
        }
        let keys: Vec<_> = self
            .diagnostics
            .keys()
            .filter(|(k, _)| k == key)
            .cloned()
            .collect();
        for key in keys {
            if let Some(o) = self.diagnostics.remove(&key) {
                self.diagnostic_bytes = self
                    .diagnostic_bytes
                    .saturating_sub(o.last_good.as_ref().map_or(512, |s| s.value.bytes() + 512));
            }
        }
        self.archive_deltas.remove(key);
    }
    pub fn valid_ticket(&self, ticket: &ProbeTicket) -> bool {
        ticket.scope == self.scope
            && self.selected.as_ref() == Some(&ticket.cluster.key)
            && self
                .cluster(&ticket.cluster.key)
                .is_some_and(|c| c.id == ticket.cluster)
            && self.pod(&ticket.pod.key).is_some_and(|p| {
                p.id == ticket.pod
                    && p.cluster.as_ref() == Some(&ticket.cluster.key)
                    && p.cluster_uid
                        .as_ref()
                        .is_none_or(|uid| *uid == ticket.cluster.uid)
            })
            && self
                .topology
                .get(&ticket.cluster.key)
                .copied()
                .unwrap_or_default()
                == ticket.topology
    }
    pub fn begin_probe(&mut self, ticket: ProbeTicket) -> bool {
        if !self.valid_ticket(&ticket)
            || self.active.contains_key(&ticket.pod.key)
            || self
                .active
                .values()
                .any(|t| t.cluster.key == ticket.cluster.key)
        {
            return false;
        }
        if ticket.class != Class::Fast {
            let key = (ticket.pod.key.clone(), ticket.class);
            if !self.diagnostics.contains_key(&key) {
                if self.bytes().saturating_add(512) > self.budgets.cache_bytes {
                    return false;
                }
                self.diagnostic_bytes += 512;
            }
            self.diagnostics.entry(key).or_default().attempt = Attempt::InFlight(ticket.request);
            self.active.insert(ticket.pod.key.clone(), ticket);
            self.revision += 1;
            return true;
        }
        if !self.samples.contains_key(&ticket.pod.key) {
            if self.bytes().saturating_add(512) > self.budgets.cache_bytes {
                return false;
            }
            self.sample_bytes += 512;
        }
        self.samples
            .entry(ticket.pod.key.clone())
            .or_default()
            .attempt = Attempt::InFlight(ticket.request);
        self.active.insert(ticket.pod.key.clone(), ticket);
        self.revision += 1;
        true
    }
    pub fn finish_probe(
        &mut self,
        mut ticket: ProbeTicket,
        result: Result<FastSample, ErrorClass>,
        now: Tick,
    ) -> bool {
        if !self.valid_ticket(&ticket) || self.active.get(&ticket.pod.key) != Some(&ticket) {
            self.rejected += 1;
            return false;
        }
        self.active.remove(&ticket.pod.key);
        let role_changed = result.as_ref().is_ok_and(|next| {
            self.samples
                .get(&ticket.pod.key)
                .and_then(|o| o.last_good.as_ref())
                .is_some_and(|old| {
                    old.value.role != next.role
                        || old.value.system_id != next.system_id
                        || old.value.server_version != next.server_version
                        || old.value.timeline != next.timeline
                })
        });
        if role_changed {
            let epoch = self.topology.entry(ticket.cluster.key.clone()).or_default();
            epoch.0 = epoch.0.checked_add(1).expect("topology epoch exhausted");
            ticket.topology = *epoch;
            for observation in self.samples.values_mut() {
                if observation
                    .last_good
                    .as_ref()
                    .is_some_and(|v| v.ticket.cluster.key == ticket.cluster.key)
                {
                    observation.attempt = Attempt::Failed(ErrorClass::Cancelled);
                }
            }
        }
        let old = self
            .samples
            .get(&ticket.pod.key)
            .and_then(|o| o.last_good.as_ref())
            .map_or(512, |s| s.value.bytes() + 512);
        let result = match result {
            Ok(sample)
                if self.bytes().saturating_sub(old) + sample.bytes() + 512
                    > self.budgets.cache_bytes =>
            {
                Err(ErrorClass::Capacity)
            }
            other => other,
        };
        if let Ok(next) = &result {
            let delta = self
                .samples
                .get(&ticket.pod.key)
                .and_then(|o| o.last_good.as_ref())
                .map_or(ArchiveDelta::Unknown, |old| {
                    if old.ticket.pod != ticket.pod
                        || old.ticket.topology != ticket.topology
                        || old.value.system_id != next.system_id
                        || old.value.timeline != next.timeline
                        || old.value.role != Role::Primary
                        || next.role != Role::Primary
                    {
                        return ArchiveDelta::Unknown;
                    }
                    if old.value.archive.stats_reset != next.archive.stats_reset {
                        return ArchiveDelta::Reset;
                    }
                    if next.archive.stats_reset.is_none() {
                        return ArchiveDelta::Unknown;
                    }
                    match (
                        next.archive
                            .archived_count
                            .checked_sub(old.value.archive.archived_count),
                        next.archive
                            .failed_count
                            .checked_sub(old.value.archive.failed_count),
                    ) {
                        (Some(archived), Some(failed)) => ArchiveDelta::Counts {
                            archived,
                            failed,
                            elapsed_ms: now.0.saturating_sub(old.at.0),
                        },
                        _ => ArchiveDelta::Reset,
                    }
                });
            self.archive_deltas.insert(ticket.pod.key.clone(), delta);
        }
        let o = self.samples.entry(ticket.pod.key.clone()).or_default();
        match result {
            Ok(sample) => {
                self.sample_bytes = self.sample_bytes - old + sample.bytes() + 512;
                o.attempt = Attempt::Succeeded(ticket.request);
                o.last_good = Some(Stamped {
                    value: sample,
                    at: now,
                    ticket,
                });
            }
            Err(error) => o.attempt = Attempt::Failed(error),
        }
        self.revision += 1;
        true
    }
    pub fn finish_diagnostic(
        &mut self,
        ticket: ProbeTicket,
        result: Result<Diagnostic, ErrorClass>,
        now: Tick,
    ) -> bool {
        if ticket.class == Class::Fast
            || !self.valid_ticket(&ticket)
            || self.active.get(&ticket.pod.key) != Some(&ticket)
        {
            self.rejected += 1;
            return false;
        }
        self.active.remove(&ticket.pod.key);
        let key = (ticket.pod.key.clone(), ticket.class);
        let old = self
            .diagnostics
            .get(&key)
            .and_then(|o| o.last_good.as_ref())
            .map_or(512, |s| s.value.bytes() + 512);
        let result = match result {
            Ok(value)
                if self
                    .samples
                    .get(&ticket.pod.key)
                    .and_then(|o| o.last_good.as_ref())
                    .is_some_and(|s| {
                        s.ticket.topology == ticket.topology
                            && (s.value.system_id.to_string() != value.stamp().system_id
                                || s.value.server_version != value.stamp().server_version
                                || (s.value.role == Role::Replica) != value.stamp().in_recovery
                                || s.value.timeline != value.stamp().timeline)
                    }) =>
            {
                let epoch = self.topology.entry(ticket.cluster.key.clone()).or_default();
                epoch.0 = epoch.0.checked_add(1).expect("topology epoch exhausted");
                for o in self.samples.values_mut() {
                    if o.last_good
                        .as_ref()
                        .is_some_and(|s| s.ticket.cluster == ticket.cluster)
                    {
                        o.attempt = Attempt::Failed(ErrorClass::Cancelled);
                    }
                }
                Err(ErrorClass::Cancelled)
            }
            Ok(value)
                if self
                    .bytes()
                    .saturating_sub(old)
                    .saturating_add(value.bytes() + 512)
                    > self.budgets.cache_bytes =>
            {
                Err(ErrorClass::Capacity)
            }
            other => other,
        };
        let capability_failure = if ticket.class == Class::Capabilities {
            result.as_ref().err().copied().filter(|e| {
                matches!(
                    e,
                    ErrorClass::Authentication | ErrorClass::Forbidden | ErrorClass::Unsupported
                )
            })
        } else {
            None
        };
        let unavailable = if let Ok(Diagnostic::Capabilities(c)) = &result {
            [Class::Fast, Class::Medium, Class::Deep]
                .into_iter()
                .filter(|class| match class {
                    Class::Fast => c.select_fast().is_none(),
                    Class::Medium => !c.allows(crate::probes::registry::Bundle::Medium),
                    Class::Deep => !c.allows(crate::probes::registry::Bundle::Deep),
                    _ => false,
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let pod = ticket.pod.key.clone();
        let o = self.diagnostics.entry(key).or_default();
        match result {
            Ok(value) => {
                self.diagnostic_bytes = self.diagnostic_bytes - old + value.bytes() + 512;
                o.attempt = Attempt::Succeeded(ticket.request);
                o.last_good = Some(Stamped {
                    value,
                    at: now,
                    ticket,
                });
            }
            Err(e) => o.attempt = Attempt::Failed(e),
        }
        if let Some(error) = capability_failure {
            if let Some(o) = self.samples.get_mut(&pod) {
                o.attempt = Attempt::Failed(error);
            }
            for ((key, class), o) in &mut self.diagnostics {
                if key == &pod && *class != Class::Capabilities {
                    o.attempt = Attempt::Failed(error);
                }
            }
        }
        for class in unavailable {
            if class == Class::Fast {
                if let Some(o) = self.samples.get_mut(&pod) {
                    o.attempt = Attempt::Failed(ErrorClass::Unsupported);
                }
            } else if let Some(o) = self.diagnostics.get_mut(&(pod.clone(), class)) {
                o.attempt = Attempt::Failed(ErrorClass::Unsupported);
            }
        }
        self.revision += 1;
        true
    }
}
