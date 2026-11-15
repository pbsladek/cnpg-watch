//! Coalesced class scheduling with seeded jitter, fair admission, and overload shedding.
use crate::{
    budget::Budgets,
    cli::Profile,
    model::*,
    probes::registry::{Bundle, Class, Diagnostic},
    reducer::Store,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
#[derive(Debug, Clone)]
struct Target {
    pod: Identity,
    cluster: Identity,
    topology: TopologyEpoch,
    next: Tick,
    failures: u32,
    suspended: bool,
    last_start: Option<Tick>,
}
#[derive(Debug, Default, Clone, Serialize)]
pub struct ClassStats {
    pub completed: u64,
    pub failed: u64,
    pub last_duration_ms: u64,
    pub last_sweep_ms: Option<u64>,
    #[serde(skip)]
    sweep_start: Option<Tick>,
    #[serde(skip)]
    seen: BTreeSet<Key>,
}
pub struct Scheduler {
    targets: BTreeMap<(Key, Class), Target>,
    cursor: Option<Key>,
    request: u64,
    seed: u64,
    budgets: Budgets,
    interval: Duration,
    pub omitted: usize,
    last_recheck: Tick,
    registry: bool,
    profile: Profile,
    deep_active: bool,
    show_query: bool,
    turn: usize,
    pressure_until: Tick,
    credential_block: Option<Key>,
    selected: Option<Identity>,
    pub pressure_percent: Option<u32>,
    pub stats: BTreeMap<Class, ClassStats>,
}
impl Scheduler {
    pub fn new(budgets: Budgets, interval: Duration, seed: u64) -> Self {
        Self {
            targets: BTreeMap::new(),
            cursor: None,
            request: 0,
            seed,
            budgets,
            interval,
            omitted: 0,
            last_recheck: Tick(0),
            registry: false,
            profile: Profile::Standard,
            deep_active: false,
            show_query: false,
            turn: 0,
            pressure_until: Tick(0),
            credential_block: None,
            selected: None,
            pressure_percent: None,
            stats: BTreeMap::new(),
        }
    }
    pub fn with_registry(
        budgets: Budgets,
        interval: Duration,
        seed: u64,
        profile: Profile,
        show_query: bool,
    ) -> Self {
        let mut s = Self::new(budgets, interval, seed);
        s.registry = true;
        s.profile = profile;
        s.show_query = show_query;
        s
    }
    pub fn set_diagnostic_view(&mut self, active: bool) {
        self.deep_active = active;
    }
    fn interval(&self, class: Class) -> u64 {
        match class {
            Class::Capabilities => 60_000,
            Class::Fast => self.interval.as_millis().try_into().unwrap_or(u64::MAX),
            Class::Medium => self.budgets.medium_interval_ms,
            Class::Deep => self.budgets.deep_interval_ms,
        }
    }
    pub fn capability<'a>(
        &self,
        store: &'a Store,
        key: &Key,
        now: Tick,
    ) -> Option<&'a crate::probes::registry::Capabilities> {
        let o = store.diagnostics.get(&(key.clone(), Class::Capabilities))?;
        let s = o.last_good.as_ref()?;
        if matches!(o.attempt, Attempt::Failed(_))
            || !store.valid_ticket(&s.ticket)
            || now.0.saturating_sub(s.at.0) >= 90_000
        {
            return None;
        }
        if let Diagnostic::Capabilities(c) = &s.value {
            Some(c)
        } else {
            None
        }
    }
    pub fn bundle(&self, store: &Store, ticket: &ProbeTicket, now: Tick) -> Option<Bundle> {
        if !self.registry {
            return Some(Bundle::Fast);
        }
        if ticket.class == Class::Capabilities {
            return Some(Bundle::Capabilities);
        }
        let c = self.capability(store, &ticket.pod.key, now)?;
        match ticket.class {
            Class::Fast => c.select_fast(),
            Class::Medium => c.allows(Bundle::Medium).then_some(Bundle::Medium),
            Class::Deep => {
                let b = if self.show_query
                    && self.profile == Profile::Incident
                    && c.allows(Bundle::DeepText)
                {
                    Bundle::DeepText
                } else {
                    Bundle::Deep
                };
                c.allows(b).then_some(b)
            }
            Class::Capabilities => Some(Bundle::Capabilities),
        }
    }
    pub fn reconcile(&mut self, store: &Store, now: Tick) {
        let Some(cluster) = store.selected.as_ref().and_then(|k| store.cluster(k)) else {
            self.targets.clear();
            return;
        };
        if self.selected.as_ref() != Some(&cluster.id) {
            self.selected = Some(cluster.id.clone());
            self.pressure_until = Tick(0);
            self.pressure_percent = None;
            self.credential_block = None;
            for stats in self.stats.values_mut() {
                stats.seen.clear();
                stats.sweep_start = None;
            }
        }
        if self
            .credential_block
            .as_ref()
            .is_some_and(|key| key != &cluster.id.key)
        {
            self.credential_block = None;
        }
        let topology = store
            .topology
            .get(&cluster.id.key)
            .copied()
            .unwrap_or_default();
        let pods: Vec<_> = store
            .pods_for(&cluster.id.key)
            .filter(|p| {
                p.cluster.as_ref() == Some(&cluster.id.key)
                    && p.ready == Some(true)
                    && p.role.is_some()
            })
            .collect();
        self.targets.retain(|(_, class), t| {
            t.cluster == cluster.id
                && t.topology == topology
                && pods.iter().any(|p| p.id == t.pod)
                && (*class != Class::Deep || self.deep_active && self.profile == Profile::Incident)
        });
        let mut omitted = 0;
        for pod in pods {
            let classes: Vec<_> = if !self.registry {
                vec![Class::Fast]
            } else if let Some(cap) = self.capability(store, &pod.id.key, now) {
                let mut classes = vec![Class::Capabilities];
                if cap.select_fast().is_some() {
                    classes.push(Class::Fast);
                }
                if cap.allows(Bundle::Medium) {
                    classes.push(Class::Medium);
                }
                if self.deep_active && self.profile == Profile::Incident && cap.allows(Bundle::Deep)
                {
                    classes.push(Class::Deep);
                }
                classes
            } else {
                vec![Class::Capabilities]
            };
            // An expired capability blocks dependent starts without deleting their failure state.
            for class in classes {
                let key = (pod.id.key.clone(), class);
                if !self.targets.contains_key(&key) {
                    if self.targets.len() >= self.budgets.pending_probes {
                        omitted += 1;
                        continue;
                    }
                    self.targets.insert(
                        key,
                        Target {
                            pod: pod.id.clone(),
                            cluster: cluster.id.clone(),
                            topology,
                            next: now,
                            failures: 0,
                            suspended: false,
                            last_start: None,
                        },
                    );
                }
            }
        }
        self.omitted = omitted;
    }
    pub fn admit(&mut self, store: &Store, now: Tick) -> Option<ProbeTicket> {
        if !store.active.is_empty()
            || self.credential_block.as_ref() == store.selected.as_ref()
                && self.credential_block.is_some()
        {
            return None;
        }
        let order = [
            Class::Capabilities,
            Class::Fast,
            Class::Fast,
            Class::Medium,
            Class::Fast,
            Class::Deep,
        ];
        for offset in 0..order.len() {
            let index = (self.turn + offset) % order.len();
            let class = order[index];
            if class == Class::Deep
                && (now < self.pressure_until || self.pressure_percent.is_some_and(|p| p >= 85))
            {
                continue;
            }
            let keys: Vec<_> = self
                .targets
                .iter()
                .filter(|((_, c), t)| *c == class && !t.suspended && t.next <= now)
                .map(|((k, _), _)| k.clone())
                .collect();
            for key in keys
                .iter()
                .filter(|k| self.cursor.as_ref().is_none_or(|cursor| *k > cursor))
                .chain(
                    keys.iter()
                        .filter(|k| self.cursor.as_ref().is_some_and(|cursor| *k <= cursor)),
                )
            {
                let t = &self.targets[&(key.clone(), class)];
                let spacing = match class {
                    Class::Medium if self.shedding(now) => self.interval(class).saturating_mul(4),
                    Class::Deep if self.pressure_percent.is_some_and(|p| p >= 70) => {
                        self.interval(class).saturating_mul(3)
                    }
                    _ => 0,
                };
                if t.last_start
                    .is_some_and(|last| now.0.saturating_sub(last.0) < spacing)
                {
                    continue;
                }
                let ticket = ProbeTicket {
                    scope: store.scope,
                    class,
                    topology: t.topology,
                    request: RequestId(self.request.checked_add(1).expect("request ID exhausted")),
                    cluster: t.cluster.clone(),
                    pod: t.pod.clone(),
                };
                if self.bundle(store, &ticket, now).is_none() {
                    continue;
                }
                self.request = ticket.request.0;
                self.targets
                    .get_mut(&(key.clone(), class))
                    .unwrap()
                    .last_start = Some(now);
                self.cursor = Some(key.clone());
                self.turn = (index + 1) % order.len();
                return Some(ticket);
            }
        }
        None
    }
    fn jitter(&mut self, base: u64) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed % (base / 10 + 1)
    }
    pub fn pressure(&mut self, percent: Option<u32>) {
        self.pressure_percent = percent;
    }
    pub fn finish(&mut self, ticket: &ProbeTicket, result: Result<(), ErrorClass>, now: Tick) {
        let key = (ticket.pod.key.clone(), ticket.class);
        let Some(t) = self.targets.get(&key) else {
            return;
        };
        if t.pod != ticket.pod || t.topology != ticket.topology {
            return;
        }
        if matches!(result, Err(ErrorClass::Authentication)) {
            self.credential_block = Some(ticket.cluster.key.clone());
        }
        let failures = if result.is_ok() {
            0
        } else {
            t.failures.saturating_add(1)
        };
        let suspended = matches!(
            result,
            Err(ErrorClass::Authentication | ErrorClass::Forbidden | ErrorClass::Unsupported)
        );
        let duration = t.last_start.map_or(0, |s| now.0.saturating_sub(s.0));
        if matches!(result, Err(ErrorClass::Overloaded | ErrorClass::Timeout)) {
            self.pressure_until = now.after(Duration::from_secs(30));
        }
        let base = if failures == 0 {
            let normal = self.interval(ticket.class);
            match ticket.class {
                Class::Medium
                    if now < self.pressure_until
                        || self.pressure_percent.is_some_and(|p| p >= 85) =>
                {
                    normal.saturating_mul(4)
                }
                Class::Deep if self.pressure_percent.is_some_and(|p| p >= 70) => {
                    normal.saturating_mul(3)
                }
                _ => normal,
            }
        } else {
            self.budgets
                .backoff_floor_ms
                .saturating_mul(1u64.checked_shl((failures - 1).min(63)).unwrap_or(u64::MAX))
                .min(self.budgets.backoff_ceiling_ms)
        };
        let delay = base.saturating_add(self.jitter(base));
        let delay = if failures > 0 {
            delay.min(self.budgets.backoff_ceiling_ms)
        } else {
            delay
        };
        let t = self.targets.get_mut(&key).unwrap();
        t.failures = failures;
        t.suspended = suspended;
        t.next = now.after(Duration::from_millis(delay));
        let target_count = self
            .targets
            .keys()
            .filter(|(_, class)| *class == ticket.class)
            .count();
        let stats = self.stats.entry(ticket.class).or_default();
        stats.completed += 1;
        stats.failed += u64::from(result.is_err());
        stats.last_duration_ms = duration;
        stats.sweep_start.get_or_insert(now);
        stats.seen.insert(ticket.pod.key.clone());
        if stats.seen.len() >= target_count {
            stats.last_sweep_ms = stats
                .sweep_start
                .map(|start| now.0.saturating_sub(start.0) + duration);
            stats.sweep_start = None;
            stats.seen.clear();
        }
    }
    pub fn refresh(&mut self, now: Tick) {
        let recheck = now.0.saturating_sub(self.last_recheck.0) >= self.budgets.backoff_floor_ms;
        if recheck {
            self.credential_block = None;
            self.last_recheck = now;
        }
        let intervals: BTreeMap<_, _> =
            [Class::Capabilities, Class::Fast, Class::Medium, Class::Deep]
                .into_iter()
                .map(|c| {
                    (
                        c,
                        if c == Class::Capabilities {
                            self.budgets.backoff_floor_ms
                        } else {
                            self.interval(c)
                        },
                    )
                })
                .collect();
        for ((_, class), t) in &mut self.targets {
            if t.suspended && recheck {
                t.suspended = false;
                t.next = t.last_start.map_or(now, |s| {
                    s.after(Duration::from_millis(intervals[class])).max(now)
                });
            } else if t.failures == 0 {
                t.next = t.last_start.map_or(now, |s| {
                    s.after(Duration::from_millis(intervals[class])).max(now)
                });
            }
        }
    }
    pub fn next(&self) -> Option<Tick> {
        self.targets
            .values()
            .filter(|t| !t.suspended)
            .map(|t| t.next)
            .min()
    }
    pub fn len(&self) -> usize {
        self.targets.len()
    }
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
    pub fn shedding(&self, now: Tick) -> bool {
        now < self.pressure_until || self.pressure_percent.is_some_and(|p| p >= 85)
    }
}
