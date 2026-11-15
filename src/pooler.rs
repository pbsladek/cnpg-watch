//! Fixed read-only PgBouncer console operations and independent admission state.
use crate::{budget::Budgets, model::*, probes, reducer::Store};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};
use tokio_util::sync::CancellationToken;

pub const INTERVAL_MS: u64 = 5_000;
pub const STALE_MS: u64 = 15_000;
pub const MAX_ROWS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pool {
    pub database: String,
    pub user: String,
    pub clients_active: u64,
    pub clients_waiting: u64,
    pub servers_active: u64,
    pub servers_idle: u64,
    pub servers_login: Option<u64>,
    pub max_wait_us: Option<u64>,
    pub mode: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stats {
    pub database: String,
    pub total_transactions: u64,
    pub total_queries: u64,
    pub total_wait_us: Option<u64>,
    /// PgBouncer versions vary between integral and decimal rates.
    pub average_queries_per_second: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sample {
    pub version: String,
    pub pools: Vec<Pool>,
    pub stats: Vec<Stats>,
}
impl Sample {
    pub fn waiting(&self) -> u64 {
        self.pools
            .iter()
            .map(|p| p.clients_waiting)
            .fold(0, u64::saturating_add)
    }
    pub fn bytes(&self) -> usize {
        // Include allocation/tree overhead conservatively in the shared cache budget.
        serde_json::to_vec(self).map_or(usize::MAX, |v| {
            v.len().saturating_mul(3).saturating_add(1024)
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ticket {
    pub scope: ScopeEpoch,
    pub request: RequestId,
    pub cluster: Identity,
    pub pooler: Identity,
    pub pod: Identity,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Observation {
    pub latest_ticket: Ticket,
    pub last_good: Option<Stamped<Sample, Ticket>>,
    pub attempt: Attempt,
}
impl Observation {
    pub fn freshness(&self, now: Tick, stale: Duration) -> Freshness {
        match &self.last_good {
            None => Freshness::Never,
            Some(s) if matches!(self.attempt, Attempt::Failed(_)) || now.age(s.at) >= stale => {
                Freshness::Stale(now.age(s.at))
            }
            Some(s) => Freshness::Fresh(now.age(s.at)),
        }
    }
}

/// Column names, not positions, determine meaning. Unknown columns remain ignored.
pub fn parse(bytes: &[u8]) -> Result<Sample, ErrorClass> {
    if bytes.len() > 256 * 1024 {
        return Err(ErrorClass::OutputLimit);
    }
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(bytes);
    let mut section = 0;
    let mut header = BTreeMap::new();
    let mut sample = Sample {
        version: String::new(),
        pools: Vec::new(),
        stats: Vec::new(),
    };
    for row in reader.records() {
        let row = row.map_err(|_| ErrorClass::InvalidData)?;
        if row.is_empty() {
            continue;
        }
        if row.len() > 128
            || row
                .iter()
                .any(|v| v.len() > 1024 || v.chars().any(char::is_control))
        {
            return Err(ErrorClass::InvalidData);
        }
        if row.len() == 1 && row.get(0) == Some("version") {
            if section != 0 {
                return Err(ErrorClass::InvalidData);
            }
            section = 1;
            continue;
        }
        if section == 1 {
            if row.len() != 1 {
                return Err(ErrorClass::InvalidData);
            }
            let version = row.get(0).unwrap();
            let number = version
                .strip_prefix("PgBouncer ")
                .ok_or(ErrorClass::Unsupported)?;
            let mut parts = number.split('.');
            if parts.next() != Some("1")
                || !parts
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    .is_some_and(|v| (19..=25).contains(&v))
            {
                return Err(ErrorClass::Unsupported);
            }
            sample.version = number.to_owned();
            section = 2;
            continue;
        }
        if row.iter().any(|v| v == "database")
            && (row.iter().any(|v| v == "cl_waiting")
                || row.iter().any(|v| v == "total_query_count"))
        {
            let next = if row.iter().any(|v| v == "cl_waiting") {
                3
            } else if row.iter().any(|v| v == "total_query_count") {
                4
            } else {
                return Err(ErrorClass::Unsupported);
            };
            if next != section + 1 {
                return Err(ErrorClass::InvalidData);
            }
            header.clear();
            for (i, name) in row.iter().enumerate() {
                if header.insert(name.to_owned(), i).is_some() {
                    return Err(ErrorClass::InvalidData);
                }
            }
            section = next;
            continue;
        }
        let get = |name: &str| header.get(name).and_then(|i| row.get(*i));
        let number = |name: &str| -> Result<Option<u64>, ErrorClass> {
            get(name)
                .map(|v| v.parse().map_err(|_| ErrorClass::InvalidData))
                .transpose()
        };
        let required = |name: &str| -> Result<u64, ErrorClass> {
            number(name)?.ok_or(ErrorClass::Unsupported)
        };
        let name = |field: &str| -> Result<String, ErrorClass> {
            let v = get(field).ok_or(ErrorClass::Unsupported)?;
            if v.is_empty() || v.len() > 253 {
                return Err(ErrorClass::InvalidData);
            }
            Ok(v.to_owned())
        };
        if row.len() != header.len() {
            return Err(ErrorClass::InvalidData);
        }
        match section {
            3 => {
                if sample.pools.len() >= MAX_ROWS {
                    return Err(ErrorClass::OutputLimit);
                }
                let seconds = number("maxwait")?;
                let micros = number("maxwait_us")?;
                if micros.is_some_and(|v| v >= 1_000_000) {
                    return Err(ErrorClass::InvalidData);
                }
                let wait = seconds
                    .map(|s| {
                        s.checked_mul(1_000_000)
                            .and_then(|s| s.checked_add(micros.unwrap_or(0)))
                            .ok_or(ErrorClass::InvalidData)
                    })
                    .transpose()?;
                let mode = get("pool_mode").map(str::to_owned);
                if mode
                    .as_ref()
                    .is_some_and(|v| !["session", "transaction", "statement"].contains(&v.as_str()))
                {
                    return Err(ErrorClass::Unsupported);
                }
                sample.pools.push(Pool {
                    database: name("database")?,
                    user: name("user")?,
                    clients_active: required("cl_active")?,
                    clients_waiting: required("cl_waiting")?,
                    servers_active: required("sv_active")?,
                    servers_idle: required("sv_idle")?,
                    servers_login: number("sv_login")?,
                    max_wait_us: wait,
                    mode,
                });
            }
            4 => {
                if sample.stats.len() >= MAX_ROWS {
                    return Err(ErrorClass::OutputLimit);
                }
                let rate = get("avg_query_count").map(str::to_owned);
                if rate.as_ref().is_some_and(|v| {
                    v.len() > 64 || !v.parse::<f64>().is_ok_and(|v| v.is_finite() && v >= 0.)
                }) {
                    return Err(ErrorClass::InvalidData);
                }
                sample.stats.push(Stats {
                    database: name("database")?,
                    total_transactions: required("total_xact_count")?,
                    total_queries: required("total_query_count")?,
                    total_wait_us: number("total_wait_time")?,
                    average_queries_per_second: rate,
                });
            }
            _ => return Err(ErrorClass::InvalidData),
        }
    }
    if section != 4 || sample.version.is_empty() {
        return Err(ErrorClass::InvalidData);
    }
    // The console connection itself is not application pressure.
    sample.pools.retain(|p| p.database != "pgbouncer");
    sample.stats.retain(|s| s.database != "pgbouncer");
    Ok(sample)
}

pub async fn execute(
    client: kube::Client,
    pod: &Key,
    socket: &str,
    budgets: &Budgets,
    cancel: &CancellationToken,
) -> Result<Sample, ErrorClass> {
    let command = [
        "env",
        "PGAPPNAME=cnpg-watch",
        "PGCONNECT_TIMEOUT=2",
        "PGSSLMODE=disable",
        "PGOPTIONS=",
        "psql",
        "-X",
        "--csv",
        "-P",
        "footer=off",
        "-w",
        "-v",
        "ON_ERROR_STOP=1",
        "-h",
        socket,
        "-p",
        "5432",
        "-U",
        "pgbouncer",
        "-d",
        "pgbouncer",
        "-c",
        "SHOW VERSION",
        "-c",
        "SHOW POOLS",
        "-c",
        "SHOW STATS",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    probes::exec_input_in(client, pod, "pgbouncer", command, b"", budgets, cancel)
        .await
        .and_then(|b| parse(&b))
}

#[derive(Clone)]
struct Target {
    ticket: Ticket,
    next: Tick,
    failures: u32,
    suspended: bool,
    active: bool,
    last_start: Option<Tick>,
}
#[derive(Default)]
pub struct Scheduler {
    targets: BTreeMap<Key, Target>,
    request: u64,
    cursor: Option<Key>,
    last_recheck: Tick,
    pub omitted: usize,
}
impl Scheduler {
    pub fn reconcile(&mut self, store: &Store, now: Tick, limit: usize) {
        let mut desired = BTreeMap::new();
        if let Some(cluster) = store.selected.as_ref().and_then(|k| store.cluster(k)) {
            for pod in store
                .pods_for(&cluster.id.key)
                .filter(|p| p.pooler.is_some() && p.ready == Some(true))
            {
                let Some(pooler) = pod
                    .pooler
                    .as_ref()
                    .and_then(|key| store.evidence_at(Kind::Pooler, key))
                    .filter(|p| crate::health::cluster_key(store, p) == Some(&cluster.id.key))
                else {
                    continue;
                };
                if !owns_pod(store, &pooler.id, pod) {
                    continue;
                }
                desired.insert(
                    pod.id.key.clone(),
                    Ticket {
                        scope: store.scope,
                        request: RequestId(0),
                        cluster: cluster.id.clone(),
                        pooler: pooler.id.clone(),
                        pod: pod.id.clone(),
                    },
                );
            }
        }
        self.targets.retain(|key, t| {
            desired
                .get(key)
                .is_some_and(|d| same_identity(&t.ticket, d))
        });
        self.omitted = desired.len().saturating_sub(limit);
        for (key, ticket) in desired.into_iter().take(limit) {
            self.targets.entry(key).or_insert(Target {
                ticket,
                next: now,
                failures: 0,
                suspended: false,
                active: false,
                last_start: None,
            });
        }
    }
    pub fn admit(&mut self, now: Tick) -> Option<Ticket> {
        let key = self
            .targets
            .iter()
            .filter(|(_, t)| !t.active && !t.suspended && t.next <= now)
            .map(|(k, _)| k)
            .find(|k| self.cursor.as_ref().is_none_or(|c| *k > c))
            .cloned()
            .or_else(|| {
                self.targets
                    .iter()
                    .find(|(_, t)| !t.active && !t.suspended && t.next <= now)
                    .map(|(k, _)| k.clone())
            })?;
        self.request = self
            .request
            .checked_add(1)
            .expect("pooler request ID exhausted");
        let t = self.targets.get_mut(&key).unwrap();
        t.ticket.request = RequestId(self.request);
        t.active = true;
        t.last_start = Some(now);
        self.cursor = Some(key);
        Some(t.ticket.clone())
    }
    pub fn accepts(&self, ticket: &Ticket) -> bool {
        self.targets
            .get(&ticket.pod.key)
            .is_some_and(|t| t.active && t.ticket == *ticket)
    }
    pub fn finish(&mut self, ticket: &Ticket, result: Result<(), ErrorClass>, now: Tick) {
        if !self.accepts(ticket) {
            return;
        }
        let t = self.targets.get_mut(&ticket.pod.key).unwrap();
        t.active = false;
        t.failures = if result.is_ok() {
            0
        } else {
            t.failures.saturating_add(1)
        };
        t.suspended = matches!(
            result,
            Err(ErrorClass::Forbidden
                | ErrorClass::Authentication
                | ErrorClass::Unsupported
                | ErrorClass::Unavailable)
        );
        let delay = if t.failures == 0 {
            INTERVAL_MS
        } else {
            2000_u64
                .saturating_mul(1_u64.checked_shl(t.failures.min(5) - 1).unwrap_or(u64::MAX))
                .min(30_000)
        };
        let jitter = ticket.request.0.wrapping_mul(6364136223846793005) % (delay / 10 + 1);
        t.next = now.after(Duration::from_millis(
            (delay + jitter).min(if t.failures == 0 { u64::MAX } else { 30_000 }),
        ));
    }
    pub fn refresh(&mut self, now: Tick) {
        if now.age(self.last_recheck) < Duration::from_secs(2) {
            return;
        }
        self.last_recheck = now;
        for t in self.targets.values_mut() {
            t.suspended = false;
            let floor = t.last_start.unwrap_or(now).after(Duration::from_secs(2));
            t.next = Tick(t.next.0.min(now.0.max(floor.0)));
        }
    }
}
fn same_identity(a: &Ticket, b: &Ticket) -> bool {
    a.scope == b.scope && a.cluster == b.cluster && a.pooler == b.pooler && a.pod == b.pod
}

pub fn valid(store: &Store, ticket: &Ticket) -> bool {
    store.selected.as_ref() == Some(&ticket.cluster.key) && valid_cached(store, ticket)
}
pub fn valid_cached(store: &Store, ticket: &Ticket) -> bool {
    ticket.scope == store.scope
        && store
            .cluster(&ticket.cluster.key)
            .is_some_and(|c| c.id == ticket.cluster)
        && store.pod(&ticket.pod.key).is_some_and(|p| {
            p.id == ticket.pod
                && p.pooler.as_ref() == Some(&ticket.pooler.key)
                && owns_pod(store, &ticket.pooler, p)
        })
        && store
            .evidence_at(Kind::Pooler, &ticket.pooler.key)
            .is_some_and(|p| {
                p.id == ticket.pooler
                    && crate::health::cluster_key(store, p) == Some(&ticket.cluster.key)
            })
}

pub fn summary(
    store: &Store,
    pooler: &Key,
    now: Tick,
    enabled: bool,
    profile: crate::cli::Profile,
) -> String {
    if !enabled || profile == crate::cli::Profile::Minimal {
        return "stats disabled by config/profile".into();
    }
    let samples: Vec<_> = store
        .pooler_samples
        .values()
        .filter(|o| {
            o.last_good
                .as_ref()
                .is_some_and(|s| &s.ticket.pooler.key == pooler)
        })
        .collect();
    if samples.is_empty() {
        let error = store
            .pods()
            .filter(|p| p.pooler.as_ref() == Some(pooler))
            .filter_map(|p| store.pooler_samples.get(&p.id.key))
            .find_map(|o| {
                if let Attempt::Failed(e) = o.attempt {
                    Some(e)
                } else {
                    None
                }
            });
        return format!(
            "stats never sampled; latest error {error:?}; local-peer console identity pgbouncer"
        );
    }
    samples
        .iter()
        .map(|o| {
            let s = o.last_good.as_ref().unwrap();
            format!(
                "{} PGbouncer {} waiting {} pools {} {:?}; {:?}",
                s.ticket.pod.key,
                s.value.version,
                s.value.waiting(),
                s.value.pools.len(),
                o.freshness(now, Duration::from_millis(STALE_MS)),
                o.attempt
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn owns_pod(store: &Store, pooler: &Identity, pod: &Pod) -> bool {
    let Some(controller) = &pod.controller else {
        return false;
    };
    let deployment = match controller.kind {
        OwnerKind::Deployment => &controller.identity,
        OwnerKind::ReplicaSet => {
            let Some(rs) = store
                .evidence_at(Kind::PoolerReplicaSet, &controller.identity.key)
                .filter(|rs| rs.id == controller.identity)
            else {
                return false;
            };
            let crate::evidence::Data::ReplicaSet {
                deployment: Some(deployment),
            } = &rs.data
            else {
                return false;
            };
            deployment
        }
        _ => return false,
    };
    let Some(deployment) = store
        .evidence_at(Kind::PoolerDeployment, &deployment.key)
        .filter(|e| e.id == *deployment)
    else {
        return false;
    };
    matches!(&deployment.data,crate::evidence::Data::Deployment{pooler_owner:Some(owner),..} if owner==pooler)
}

/// Doctor uses the same canonical ownership predicate as runtime admission.
/// A label match alone is insufficient to select an exec target.
pub async fn verify_owner(
    client: kube::Client,
    raw: k8s_openapi::api::core::v1::Pod,
    cluster: Cluster,
    budgets: &Budgets,
) -> Result<Key, ErrorClass> {
    use kube::{
        Api,
        api::DynamicObject,
        core::{ApiResource, GroupVersionKind},
    };
    async fn get(
        client: kube::Client,
        key: &Key,
        group: &str,
        kind: &str,
    ) -> Result<DynamicObject, ErrorClass> {
        let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(group, "v1", kind));
        Api::<DynamicObject>::namespaced_with(client, &key.namespace, &resource)
            .get(&key.name)
            .await
            .map_err(|e| crate::probes::api_error(&e))
    }
    let operation = async {
        let Object::Pod(pod) = crate::adapters::pod(raw).map_err(|_| ErrorClass::InvalidData)?
        else {
            return Err(ErrorClass::InvalidData);
        };
        let key = pod.pooler.as_ref().ok_or(ErrorClass::InvalidData)?;
        let pooler = crate::adapters::pooler(
            get(client.clone(), key, "postgresql.cnpg.io", "Pooler").await?,
        )
        .map_err(|_| ErrorClass::InvalidData)?;
        let Object::Evidence(pooler) = pooler else {
            return Err(ErrorClass::InvalidData);
        };
        let mut store = Store::new(budgets.clone());
        let mut objects = vec![
            Object::Cluster(cluster),
            Object::Pod(pod.clone()),
            Object::Evidence(pooler.clone()),
        ];
        let controller = pod.controller.as_ref().ok_or(ErrorClass::Unavailable)?;
        let deployment = match controller.kind {
            OwnerKind::Deployment => controller.identity.key.clone(),
            OwnerKind::ReplicaSet => {
                let rs = crate::adapters::pooler_replicaset(
                    get(
                        client.clone(),
                        &controller.identity.key,
                        "apps",
                        "ReplicaSet",
                    )
                    .await?,
                )
                .map_err(|_| ErrorClass::InvalidData)?;
                let Object::Evidence(e) = &rs else {
                    return Err(ErrorClass::InvalidData);
                };
                let crate::evidence::Data::ReplicaSet {
                    deployment: Some(deployment),
                } = &e.data
                else {
                    return Err(ErrorClass::Unavailable);
                };
                let key = deployment.key.clone();
                objects.push(rs);
                key
            }
            _ => return Err(ErrorClass::Unavailable),
        };
        objects.push(
            crate::adapters::pooler_deployment(
                get(client, &deployment, "apps", "Deployment").await?,
            )
            .map_err(|_| ErrorClass::InvalidData)?,
        );
        for object in objects {
            let kind = object.kind();
            store.enable_sources([kind]);
            store.watch(crate::reducer::WatchEvent {
                scope: store.scope,
                watch: WatchEpoch(0),
                kind,
                update: crate::reducer::WatchUpdate::Apply(object),
            });
        }
        if crate::health::cluster_key(&store, &pooler) != store.selected.as_ref()
            || !owns_pod(&store, &pooler.id, &pod)
        {
            return Err(ErrorClass::Unavailable);
        }
        Ok(pod.id.key)
    };
    tokio::time::timeout(Duration::from_secs(5), operation)
        .await
        .unwrap_or(Err(ErrorClass::Timeout))
}
