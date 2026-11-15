//! PostgreSQL 18 registry. All shipped variants have bounded, typed schemas.
use crate::{cli::Profile, model::*, probes};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Capabilities,
    #[default]
    Fast,
    Medium,
    Deep,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bundle {
    Capabilities,
    Fast,
    FastBasic,
    Medium,
    Deep,
    DeepText,
}
#[derive(Serialize)]
pub struct Spec {
    pub id: &'static str,
    pub class: Class,
    pub min_server: u32,
    pub max_server_exclusive: u32,
    pub schema_version: u32,
    pub timeout_ms: u64,
    pub max_rows: usize,
    pub required_columns: &'static [(&'static str, &'static [&'static str])],
    pub required_functions: &'static [&'static str],
    pub role: &'static str,
    pub redaction: &'static str,
    pub fallback: Option<Bundle>,
}
const COMMON_COLUMNS: &[(&str, &[&str])] = &[("pg_roles", &["rolname", "rolsuper"])];
const FAST_COLUMNS: &[(&str, &[&str])] = &[
    COMMON_COLUMNS[0],
    (
        "pg_stat_replication",
        &["application_name", "client_addr", "replay_lsn"],
    ),
    (
        "pg_stat_archiver",
        &[
            "archived_count",
            "failed_count",
            "stats_reset",
            "last_archived_time",
            "last_failed_time",
        ],
    ),
];
const FAST_PRESSURE_COLUMNS: &[(&str, &[&str])] = &[
    FAST_COLUMNS[0],
    FAST_COLUMNS[1],
    FAST_COLUMNS[2],
    ("pg_stat_activity", &["backend_type", "state", "pid"]),
];
const FAST_FUNCTIONS: &[&str] = &[
    "pg_control_system()",
    "pg_control_checkpoint()",
    "pg_is_in_recovery()",
    "pg_current_wal_lsn()",
    "pg_last_wal_receive_lsn()",
    "pg_last_wal_replay_lsn()",
    "pg_last_xact_replay_timestamp()",
];
const MEDIUM_FUNCTIONS: &[&str] = &[
    "pg_control_system()",
    "pg_control_checkpoint()",
    "pg_is_in_recovery()",
    "pg_current_wal_lsn()",
    "pg_wal_lsn_diff(pg_lsn,pg_lsn)",
];
const DEEP_FUNCTIONS: &[&str] = &[
    "pg_control_system()",
    "pg_control_checkpoint()",
    "pg_is_in_recovery()",
    "pg_blocking_pids(integer)",
];
const MEDIUM_COLUMNS: &[(&str, &[&str])] = &[
    COMMON_COLUMNS[0],
    (
        "pg_replication_slots",
        &[
            "slot_name",
            "slot_type",
            "active",
            "restart_lsn",
            "confirmed_flush_lsn",
            "wal_status",
            "safe_wal_size",
        ],
    ),
    (
        "pg_stat_replication",
        &[
            "application_name",
            "state",
            "sync_state",
            "write_lsn",
            "flush_lsn",
            "replay_lsn",
        ],
    ),
    ("pg_settings", &["name", "setting"]),
];
const DEEP_COLUMNS: &[(&str, &[&str])] = &[
    COMMON_COLUMNS[0],
    (
        "pg_stat_activity",
        &[
            "pid",
            "backend_type",
            "state",
            "wait_event_type",
            "wait_event",
            "xact_start",
            "query_start",
        ],
    ),
];
const DEEP_TEXT_COLUMNS: &[(&str, &[&str])] = &[
    DEEP_COLUMNS[0],
    DEEP_COLUMNS[1],
    ("pg_stat_activity", &["query"]),
];
impl Bundle {
    pub fn sql(self) -> &'static str {
        match self {
            Self::Capabilities => include_str!("capabilities.sql"),
            Self::Fast => probes::FAST_SQL,
            Self::FastBasic => include_str!("fast-basic.sql"),
            Self::Medium => include_str!("medium.sql"),
            Self::Deep => include_str!("deep.sql"),
            Self::DeepText => include_str!("deep-text.sql"),
        }
    }
    pub fn spec(self) -> Spec {
        let (id, class, columns, functions, rows) = match self {
            Self::Capabilities => (
                "postgres18.capabilities.v1",
                Class::Capabilities,
                COMMON_COLUMNS,
                &FAST_FUNCTIONS[..3],
                6,
            ),
            Self::Fast | Self::FastBasic => (
                if self == Self::Fast {
                    "postgres18.fast-pressure.v1"
                } else {
                    "postgres18.fast-basic.v1"
                },
                Class::Fast,
                if self == Self::Fast {
                    FAST_PRESSURE_COLUMNS
                } else {
                    FAST_COLUMNS
                },
                FAST_FUNCTIONS,
                128,
            ),
            Self::Medium => (
                "postgres18.medium.v1",
                Class::Medium,
                MEDIUM_COLUMNS,
                MEDIUM_FUNCTIONS,
                128,
            ),
            Self::Deep | Self::DeepText => (
                if self == Self::Deep {
                    "postgres18.deep.v1"
                } else {
                    "postgres18.deep-text.v1"
                },
                Class::Deep,
                if self == Self::DeepText {
                    DEEP_TEXT_COLUMNS
                } else {
                    DEEP_COLUMNS
                },
                DEEP_FUNCTIONS,
                64,
            ),
        };
        Spec {
            id,
            class,
            min_server: 180000,
            max_server_exclusive: 190000,
            schema_version: 1,
            timeout_ms: if matches!(class, Class::Medium | Class::Deep) {
                2000
            } else {
                750
            },
            max_rows: rows,
            required_columns: columns,
            required_functions: functions,
            role: "pg_monitor",
            redaction: "query text omitted unless incident opt-in; always omitted from dumps",
            fallback: if self == Self::Fast {
                Some(Self::FastBasic)
            } else {
                None
            },
        }
    }
}
pub fn registry() -> Vec<Spec> {
    [
        Bundle::Capabilities,
        Bundle::Fast,
        Bundle::FastBasic,
        Bundle::Medium,
        Bundle::Deep,
        Bundle::DeepText,
    ]
    .into_iter()
    .map(Bundle::spec)
    .collect()
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stamp {
    pub schema_version: u32,
    pub server_version: u32,
    pub current_user: String,
    pub session_user: String,
    pub superuser: bool,
    pub monitor: bool,
    pub system_id: String,
    pub timeline: u32,
    pub in_recovery: bool,
    pub server_time: String,
}
impl Stamp {
    fn validate(&self, role: &str) -> Result<(), ErrorClass> {
        if self.schema_version != 1
            || self.system_id.parse::<u64>().is_err()
            || self.timeline == 0
            || chrono::DateTime::parse_from_rfc3339(&self.server_time).is_err()
        {
            return Err(ErrorClass::InvalidData);
        }
        if !(180000..190000).contains(&self.server_version) {
            return Err(ErrorClass::Unsupported);
        }
        if self.superuser || !self.monitor || self.current_user != role || self.session_user != role
        {
            return Err(ErrorClass::Forbidden);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ViewCapability {
    pub name: String,
    pub readable: bool,
    pub columns: BTreeSet<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunctionCapability {
    pub name: String,
    pub allowed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(flatten)]
    pub stamp: Stamp,
    pub views: Vec<ViewCapability>,
    pub functions: Vec<FunctionCapability>,
}
impl Capabilities {
    pub fn allows(&self, bundle: Bundle) -> bool {
        let spec = bundle.spec();
        spec.required_columns.iter().all(|(name, cols)| {
            self.views.iter().any(|v| {
                v.name == *name && v.readable && cols.iter().all(|c| v.columns.contains(*c))
            })
        }) && spec
            .required_functions
            .iter()
            .all(|name| self.functions.iter().any(|f| f.name == *name && f.allowed))
    }
    pub fn select_fast(&self) -> Option<Bundle> {
        [Bundle::Fast, Bundle::FastBasic]
            .into_iter()
            .find(|b| self.allows(*b))
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Connections {
    pub clients: u32,
    pub active: u32,
    pub idle_in_transaction: u32,
    pub max_connections: u32,
    pub reserved: u32,
    pub super_reserved: u32,
}
impl Connections {
    pub fn pressure_percent(&self) -> Option<u32> {
        let usable = self
            .max_connections
            .checked_sub(self.reserved)?
            .checked_sub(self.super_reserved)?;
        if usable == 0 {
            return None;
        }
        Some((u64::from(self.clients) * 100 / u64::from(usable)).min(u64::from(u32::MAX)) as u32)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Slot {
    pub slot_name: String,
    pub slot_type: String,
    pub active: bool,
    pub restart_lsn: Option<String>,
    pub confirmed_flush_lsn: Option<String>,
    pub wal_status: Option<String>,
    pub safe_wal_size: Option<i64>,
    pub retained_bytes: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Replication {
    pub application_name: String,
    pub state: String,
    pub sync_state: String,
    pub write_lsn: Option<String>,
    pub flush_lsn: Option<String>,
    pub replay_lsn: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Medium {
    #[serde(flatten)]
    pub stamp: Stamp,
    pub slots: Vec<Slot>,
    pub slot_count: u32,
    pub replication: Vec<Replication>,
    pub settings: BTreeMap<String, String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Wait {
    pub wait_event_type: Option<String>,
    pub wait_event: Option<String>,
    pub count: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Blocker {
    pub pid: u32,
    pub blocking_pids: Vec<u32>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Transaction {
    pub pid: u32,
    pub backend_type: Option<String>,
    pub state: Option<String>,
    pub wait_event_type: Option<String>,
    pub wait_event: Option<String>,
    pub xact_age_ms: Option<u64>,
    pub query_age_ms: Option<u64>,
    pub query_text: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Deep {
    #[serde(flatten)]
    pub stamp: Stamp,
    pub connections: Connections,
    pub waits: Vec<Wait>,
    pub blockers: Vec<Blocker>,
    pub long_transactions: Vec<Transaction>,
    pub autovacuum_workers: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Diagnostic {
    Capabilities(Capabilities),
    Medium(Medium),
    Deep(Deep),
}
impl Diagnostic {
    pub fn bytes(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |b| {
            b.len().saturating_mul(3).saturating_add(512)
        })
    }
    pub fn stamp(&self) -> &Stamp {
        match self {
            Self::Capabilities(c) => &c.stamp,
            Self::Medium(c) => &c.stamp,
            Self::Deep(c) => &c.stamp,
        }
    }
}
pub fn parse(
    bundle: Bundle,
    bytes: &[u8],
    role: &str,
    profile: Profile,
) -> Result<Diagnostic, ErrorClass> {
    if bytes.len() > 256 * 1024 {
        return Err(ErrorClass::OutputLimit);
    }
    let invalid = |_| ErrorClass::InvalidData;
    let value = match bundle {
        Bundle::Capabilities => {
            let c: Capabilities = serde_json::from_slice(bytes).map_err(invalid)?;
            if c.views.len() > 16
                || c.functions.len() > 32
                || c.views.iter().any(|v| {
                    v.name.len() > 63
                        || v.columns.len() > 128
                        || v.columns.iter().any(|c| c.len() > 63)
                })
                || c.functions.iter().any(|f| f.name.len() > 128)
            {
                return Err(ErrorClass::InvalidData);
            }
            Diagnostic::Capabilities(c)
        }
        Bundle::Medium => {
            let m: Medium = serde_json::from_slice(bytes).map_err(invalid)?;
            if m.slots.len() > 64
                || m.replication.len() > 128
                || m.settings.len() > 16
                || m.slots.iter().any(|s| {
                    s.slot_name.len() > 63
                        || s.slot_type.len() > 32
                        || s.wal_status.as_ref().is_some_and(|s| s.len() > 32)
                        || s.retained_bytes
                            .as_ref()
                            .is_some_and(|s| s.parse::<u64>().is_err())
                })
                || m.replication.iter().any(|r| {
                    r.application_name.len() > 253 || r.state.len() > 64 || r.sync_state.len() > 64
                })
                || m.settings
                    .iter()
                    .any(|(k, v)| k.len() > 63 || v.len() > 1024)
            {
                return Err(ErrorClass::InvalidData);
            }
            for s in &m.slots {
                for lsn in [&s.restart_lsn, &s.confirmed_flush_lsn]
                    .into_iter()
                    .flatten()
                {
                    lsn.parse::<Lsn>().map_err(|_| ErrorClass::InvalidData)?;
                }
            }
            for r in &m.replication {
                for lsn in [&r.write_lsn, &r.flush_lsn, &r.replay_lsn]
                    .into_iter()
                    .flatten()
                {
                    lsn.parse::<Lsn>().map_err(|_| ErrorClass::InvalidData)?;
                }
            }
            Diagnostic::Medium(m)
        }
        Bundle::Deep | Bundle::DeepText => {
            let mut d: Deep = serde_json::from_slice(bytes).map_err(invalid)?;
            if d.waits.len() > 64
                || d.blockers.len() > 20
                || d.long_transactions.len() > 20
                || d.blockers.iter().any(|b| b.blocking_pids.len() > 16)
                || d.waits.iter().any(|w| {
                    w.wait_event.as_ref().is_some_and(|s| s.len() > 128)
                        || w.wait_event_type.as_ref().is_some_and(|s| s.len() > 128)
                })
            {
                return Err(ErrorClass::InvalidData);
            }
            for t in &mut d.long_transactions {
                if [&t.backend_type, &t.state, &t.wait_event_type, &t.wait_event]
                    .into_iter()
                    .flatten()
                    .any(|s| s.len() > 128 || s.chars().any(char::is_control))
                    || t.query_text.as_ref().is_some_and(|q| q.len() > 2048)
                {
                    return Err(ErrorClass::InvalidData);
                }
                if bundle != Bundle::DeepText || profile != Profile::Incident {
                    t.query_text = None;
                } else if let Some(q) = &mut t.query_text {
                    *q = crate::logs::redact(q, profile);
                }
            }
            Diagnostic::Deep(d)
        }
        _ => return Err(ErrorClass::InvalidData),
    };
    value.stamp().validate(role)?;
    Ok(value)
}
