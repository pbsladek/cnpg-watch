//! Validated identities and observations. No API objects or I/O live here.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, net::IpAddr, str::FromStr, time::Duration};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Kind {
    Cluster,
    Pod,
    Event,
    Pvc,
    Service,
    EndpointSlice,
    Pooler,
    Deployment,
    OperatorPod,
    OperatorEvent,
    Lease,
    Backup,
    ScheduledBackup,
    Database,
    DatabaseRole,
    Publication,
    Subscription,
    FailoverQuorum,
    ImageCatalog,
    ClusterImageCatalog,
    ValidatingWebhook,
    MutatingWebhook,
    OperatorService,
    OperatorEndpointSlice,
    PoolerDeployment,
    PoolerReplicaSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Key {
    pub namespace: String,
    pub name: String,
}
impl Key {
    pub fn new(
        namespace: impl Into<String>,
        name: impl Into<String>,
    ) -> Result<Self, InvalidValue> {
        let key = Self {
            namespace: namespace.into(),
            name: name.into(),
        };
        if key.namespace.contains('.') {
            return Err(InvalidValue);
        }
        for (value, max) in [(&key.namespace, 63), (&key.name, 253)] {
            if value.is_empty()
                || value.len() > max
                || !value
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'.')
                || !value.as_bytes()[0].is_ascii_alphanumeric()
                || !value.as_bytes()[value.len() - 1].is_ascii_alphanumeric()
            {
                return Err(InvalidValue);
            }
        }
        Ok(key)
    }
}
impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.namespace, self.name)
    }
}

#[derive(Debug, Error, Clone, Copy)]
#[error("invalid external value")]
pub struct InvalidValue;

macro_rules! counter {
    ($($name:ident),+) => { $(
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(pub u64);
    )+ };
}
counter!(ScopeEpoch, WatchEpoch, TopologyEpoch, RequestId, Tick);
impl Tick {
    pub fn after(self, duration: Duration) -> Self {
        Self(
            self.0
                .saturating_add(duration.as_millis().try_into().unwrap_or(u64::MAX)),
        )
    }
    pub fn age(self, earlier: Self) -> Duration {
        Duration::from_millis(self.0.saturating_sub(earlier.0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Uid(String);
impl Uid {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidValue> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(InvalidValue);
        }
        Ok(Self(value))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Identity {
    pub key: Key,
    pub uid: Uid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Lsn(pub u64);
impl FromStr for Lsn {
    type Err = InvalidValue;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (a, b) = value.split_once('/').ok_or(InvalidValue)?;
        if a.is_empty()
            || b.is_empty()
            || a.len() > 8
            || b.len() > 8
            || !a.bytes().chain(b.bytes()).all(|c| c.is_ascii_hexdigit())
        {
            return Err(InvalidValue);
        }
        Ok(Self(
            (u64::from(u32::from_str_radix(a, 16).map_err(|_| InvalidValue)?) << 32)
                | u64::from(u32::from_str_radix(b, 16).map_err(|_| InvalidValue)?),
        ))
    }
}
impl fmt::Display for Lsn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:X}/{:X}", self.0 >> 32, self.0 as u32)
    }
}
impl Lsn {
    pub fn checked_lag(self, replay: Self) -> Option<u64> {
        self.0.checked_sub(replay.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Primary,
    Replica,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cluster {
    pub id: Identity,
    pub primary: Option<String>,
    pub phase: Option<String>,
    #[serde(default)]
    pub operational: Box<crate::operational::ClusterContext>,
    #[serde(default)]
    pub labels: crate::evidence::Labels,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OwnerKind {
    ReplicaSet,
    Deployment,
    Cluster,
    Backup,
    Job,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Controller {
    pub kind: OwnerKind,
    pub identity: Identity,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pod {
    pub id: Identity,
    pub cluster: Option<Key>,
    pub role: Option<Role>,
    pub ready: Option<bool>,
    pub restarts: u32,
    pub address: Option<IpAddr>,
    #[serde(default)]
    pub labels: crate::evidence::Labels,
    #[serde(default)]
    pub pooler: Option<Key>,
    #[serde(default)]
    pub backup: Option<Key>,
    #[serde(default)]
    pub backup_uid: Option<Uid>,
    #[serde(default)]
    pub controller: Option<Controller>,
    #[serde(default)]
    pub containers: Vec<String>,
    #[serde(default)]
    pub cluster_uid: Option<Uid>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Object {
    Cluster(Cluster),
    Pod(Pod),
    Evidence(crate::evidence::Evidence),
}
impl Object {
    pub fn id(&self) -> &Identity {
        match self {
            Self::Cluster(v) => &v.id,
            Self::Pod(v) => &v.id,
            Self::Evidence(v) => &v.id,
        }
    }
    pub fn kind(&self) -> Kind {
        match self {
            Self::Cluster(_) => Kind::Cluster,
            Self::Pod(_) => Kind::Pod,
            Self::Evidence(v) => v.kind,
        }
    }
    /// Conservative accounting includes owned strings and estimated map-node overhead.
    pub fn bytes(&self) -> usize {
        if let Self::Evidence(e) = self {
            return e.bytes();
        }
        let id = self.id();
        std::mem::size_of::<Self>()
            + 128
            + id.key.namespace.capacity()
            + id.key.name.capacity()
            + id.uid.0.capacity()
            + self
                .labels()
                .iter()
                .map(|(k, v)| k.capacity() + v.capacity() + 96)
                .sum::<usize>()
            + match self {
                Self::Cluster(c) => {
                    c.operational.bytes()
                        + c.primary.as_ref().map_or(0, String::capacity)
                        + c.phase.as_ref().map_or(0, String::capacity)
                }
                Self::Pod(p) => {
                    p.cluster
                        .as_ref()
                        .map_or(0, |k| k.name.capacity() + k.namespace.capacity())
                        + p.backup
                            .as_ref()
                            .map_or(0, |k| k.name.capacity() + k.namespace.capacity() + 256)
                }
                Self::Evidence(_) => unreachable!(),
            }
    }
    pub fn labels(&self) -> &crate::evidence::Labels {
        match self {
            Self::Cluster(c) => &c.labels,
            Self::Pod(p) => &p.labels,
            Self::Evidence(e) => &e.labels,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorClass {
    Forbidden,
    Unavailable,
    Authentication,
    Overloaded,
    Timeout,
    Transport,
    OutputLimit,
    InvalidData,
    Capacity,
    Unsupported,
    Cancelled,
}
impl fmt::Display for ErrorClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Availability {
    Available,
    #[default]
    Unknown,
    DisabledByProfile,
    DisabledByConfig,
    Unavailable(ErrorClass),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Attempt {
    Never,
    InFlight(RequestId),
    Succeeded(RequestId),
    Failed(ErrorClass),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stamped<T, Ticket = ProbeTicket> {
    pub value: T,
    pub at: Tick,
    pub ticket: Ticket,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Observation<T, Ticket = ProbeTicket> {
    pub last_good: Option<Stamped<T, Ticket>>,
    pub attempt: Attempt,
}
impl<T, Ticket> Default for Observation<T, Ticket> {
    fn default() -> Self {
        Self {
            last_good: None,
            attempt: Attempt::Never,
        }
    }
}
impl<T, Ticket> Observation<T, Ticket> {
    pub fn freshness(&self, now: Tick, stale_after: Duration) -> Freshness {
        match &self.last_good {
            None => Freshness::Never,
            Some(v)
                if matches!(self.attempt, Attempt::Failed(_)) || now.age(v.at) >= stale_after =>
            {
                Freshness::Stale(now.age(v.at))
            }
            Some(v) => Freshness::Fresh(now.age(v.at)),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Freshness {
    Never,
    Fresh(Duration),
    Stale(Duration),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProbeTicket {
    pub scope: ScopeEpoch,
    #[serde(default)]
    pub class: crate::probes::registry::Class,
    pub topology: TopologyEpoch,
    pub request: RequestId,
    pub cluster: Identity,
    pub pod: Identity,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Peer {
    pub application: String,
    pub address: Option<IpAddr>,
    pub replay: Option<Lsn>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Archive {
    pub archived_count: u64,
    pub failed_count: u64,
    pub stats_reset: Option<String>,
    pub last_archived_time: Option<String>,
    pub last_failed_time: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FastSample {
    pub server_version: u32,
    pub role: Role,
    pub system_id: u64,
    pub timeline: u32,
    pub current: Option<Lsn>,
    pub received: Option<Lsn>,
    pub replay: Option<Lsn>,
    pub server_time: String,
    pub last_replayed_transaction_age_ms: Option<u64>,
    pub peers: Vec<Peer>,
    pub archive: Archive,
    #[serde(default)]
    pub connections: Option<crate::probes::registry::Connections>,
}
impl FastSample {
    pub fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.server_time.capacity()
            + self
                .peers
                .iter()
                .map(|p| std::mem::size_of::<Peer>() + p.application.capacity())
                .sum::<usize>()
            + [
                &self.archive.stats_reset,
                &self.archive.last_archived_time,
                &self.archive.last_failed_time,
            ]
            .into_iter()
            .flatten()
            .map(String::capacity)
            .sum::<usize>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyncState {
    Starting,
    Syncing,
    Synced,
    Restarting,
    Failed(ErrorClass),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WatchScope {
    AllNamespaces,
    Namespace(String),
    Cluster,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceSpec {
    pub scope: WatchScope,
    pub selector: Option<String>,
    pub api_version: String,
    pub resource: String,
}
impl SourceSpec {
    pub fn covers_namespace(&self, namespace: &str) -> bool {
        matches!(&self.scope, WatchScope::AllNamespaces)
            || matches!(&self.scope,WatchScope::Namespace(ns) if ns==namespace)
    }
}
#[derive(Debug, Clone)]
pub struct Cache {
    pub source: Option<SourceSpec>,
    pub epoch: WatchEpoch,
    pub status: SyncState,
    pub objects: BTreeMap<Key, Object>,
    pub staging: Option<BTreeMap<Key, Object>>,
    pub bytes: usize,
    pub staging_bytes: usize,
    pub received: u64,
    pub syncs: u64,
    pub errors: u64,
    pub last_sync: Option<Tick>,
    pub last_error: Option<ErrorClass>,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            source: None,
            epoch: WatchEpoch(0),
            status: SyncState::Starting,
            objects: BTreeMap::new(),
            staging: None,
            bytes: 0,
            staging_bytes: 0,
            received: 0,
            syncs: 0,
            errors: 0,
            last_sync: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lag {
    NotApplicable,
    Bytes(u64),
    Unknown(LagUnknown),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LagUnknown {
    WatchUnsynced,
    NeverSampled,
    Stale,
    MissingPrimary,
    IdentityChanged,
    TimelineMismatch,
    SamplingSkew,
    MissingLsn,
    MissingPeer,
    AmbiguousPeer,
    NegativeDelta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveDelta {
    Unknown,
    Reset,
    Counts {
        archived: u64,
        failed: u64,
        elapsed_ms: u64,
    },
}
