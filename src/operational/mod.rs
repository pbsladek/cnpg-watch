//! Compact operational domain models; API decoding and health rules remain separate.
use crate::model::Key;
use serde::{Deserialize, Serialize};
mod adapters;
pub use adapters::{adapt, cluster, reconciliation};
pub mod health;
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Condition {
    pub type_: String,
    pub status: String,
    pub reason: Option<String>,
    pub observed_generation: Option<i64>,
    pub changed_ms: Option<i64>,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Reconciliation {
    pub generation: Option<i64>,
    pub observed_generation: Option<i64>,
    pub applied: Option<bool>,
    pub conditions: Vec<Condition>,
    pub message_present: bool,
}
impl Reconciliation {
    pub fn current(&self) -> bool {
        self.generation.is_some() && self.generation == self.observed_generation
    }
    pub fn state(&self) -> &'static str {
        if !self.current() {
            if self
                .generation
                .zip(self.observed_generation)
                .is_some_and(|(g, o)| o < g)
            {
                "pending generation"
            } else {
                "generation unknown"
            }
        } else {
            match self.applied {
                Some(true) => "applied",
                Some(false) => "failed",
                None => "current generation; application unknown",
            }
        }
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub version: Option<String>,
    pub configured: bool,
    pub enabled: Option<bool>,
    pub archiver: Option<bool>,
    pub capabilities: Vec<String>,
    pub reported_status: bool,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Replica {
    pub enabled: Option<bool>,
    pub source: Option<String>,
    pub delay: Option<String>,
    pub external_names: Vec<String>,
    pub sync_method: Option<String>,
    pub sync_number: Option<u32>,
    pub durability: Option<String>,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CatalogRef {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub major: Option<u32>,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClusterContext {
    pub availability: crate::model::Availability,
    pub generation: Option<i64>,
    pub created_ms: Option<i64>,
    pub conditions: Vec<Condition>,
    pub target_primary: Option<String>,
    pub image: Option<String>,
    pub operator_version: Option<String>,
    pub operator_hash: Option<String>,
    pub ready: Option<u32>,
    pub desired: Option<u32>,
    pub retention: Option<String>,
    pub latest_success_ms: Option<i64>,
    pub latest_failure_ms: Option<i64>,
    pub recovery_point_ms: Option<i64>,
    pub dangling: Vec<String>,
    pub unusable: Vec<String>,
    pub resizing: Vec<String>,
    pub initializing: Vec<String>,
    pub healthy: Vec<String>,
    pub replica: Replica,
    pub plugins: Vec<Plugin>,
    pub catalog: CatalogRef,
    pub certificate_secrets: Vec<String>,
}
impl ClusterContext {
    pub fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + 512
            + options(&[
                &self.target_primary,
                &self.image,
                &self.operator_version,
                &self.operator_hash,
                &self.retention,
            ])
            + conditions_bytes(&self.conditions)
            + [
                &self.dangling,
                &self.unusable,
                &self.resizing,
                &self.initializing,
                &self.healthy,
                &self.certificate_secrets,
            ]
            .into_iter()
            .map(strings)
            .sum::<usize>()
            + options(&[
                &self.replica.source,
                &self.replica.delay,
                &self.replica.sync_method,
                &self.replica.durability,
            ])
            + strings(&self.replica.external_names)
            + options(&[&self.catalog.name, &self.catalog.kind])
            + self.plugins.capacity() * std::mem::size_of::<Plugin>()
            + self
                .plugins
                .iter()
                .map(|p| p.name.capacity() + options(&[&p.version]) + strings(&p.capabilities))
                .sum::<usize>()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Resource {
    Backup {
        phase: Option<String>,
        method: Option<String>,
        started_ms: Option<i64>,
        stopped_ms: Option<i64>,
        instance: Option<String>,
        failure_reported: bool,
    },
    Schedule {
        schedule: Option<String>,
        suspended: Option<bool>,
        method: Option<String>,
        last_check_ms: Option<i64>,
        last_run_ms: Option<i64>,
        next_run_ms: Option<i64>,
        failure_reported: bool,
    },
    Managed {
        name: Option<String>,
        database: Option<String>,
        reconciliation: Reconciliation,
    },
    Quorum {
        primary: Option<String>,
        method: Option<String>,
        standby_names: Vec<String>,
        standby_number: Option<u32>,
    },
    Catalog {
        images: Vec<(u32, String)>,
        component_entries: usize,
        global: bool,
    },
    Webhook {
        rules: usize,
        services: Vec<Key>,
        failure_policies: Vec<String>,
        ca_present: bool,
        external_clients: usize,
    },
}
impl Resource {
    pub fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + 512
            + match self {
                Self::Backup {
                    phase,
                    method,
                    instance,
                    ..
                } => options(&[phase, method, instance]),
                Self::Schedule {
                    schedule, method, ..
                } => options(&[schedule, method]),
                Self::Managed {
                    name,
                    database,
                    reconciliation,
                } => options(&[name, database]) + reconciliation.bytes(),
                Self::Quorum {
                    primary,
                    method,
                    standby_names,
                    ..
                } => options(&[primary, method]) + strings(standby_names),
                Self::Catalog { images, .. } => {
                    images.capacity() * std::mem::size_of::<(u32, String)>()
                        + images
                            .iter()
                            .map(|(_, image)| image.capacity())
                            .sum::<usize>()
                }
                Self::Webhook {
                    services,
                    failure_policies,
                    ..
                } => {
                    services.capacity() * std::mem::size_of::<Key>()
                        + services
                            .iter()
                            .map(|k| k.name.capacity() + k.namespace.capacity())
                            .sum::<usize>()
                        + strings(failure_policies)
                }
            }
    }
}

pub fn summary(resource: &Resource, secure: bool) -> String {
    let display = |value: &Option<String>| {
        if secure && value.is_some() {
            "[redacted]".into()
        } else {
            value.clone().unwrap_or_else(|| "unknown".into())
        }
    };
    match resource {
        Resource::Backup {
            phase,
            method,
            started_ms,
            stopped_ms,
            instance,
            failure_reported,
        } => format!(
            "phase {} method {} started {started_ms:?} stopped {stopped_ms:?} instance {}; controller error present {failure_reported}",
            display(phase),
            display(method),
            display(instance)
        ),
        Resource::Schedule {
            schedule,
            suspended,
            method,
            last_check_ms,
            last_run_ms,
            next_run_ms,
            failure_reported,
        } => format!(
            "schedule {} suspended {suspended:?} method {}; reported check {last_check_ms:?}, last {last_run_ms:?}, next {next_run_ms:?}; error {failure_reported}; unreported next run stays unknown",
            display(schedule),
            display(method)
        ),
        Resource::Managed {
            name,
            database,
            reconciliation: r,
        } => format!(
            "name {} database {}; {}; generation {:?}/{:?}; controller message present {}",
            display(name),
            display(database),
            r.state(),
            r.observed_generation,
            r.generation,
            r.message_present
        ),
        Resource::Quorum {
            primary,
            method,
            standby_names,
            standby_number,
        } => format!(
            "latest reported primary {} method {} standby number {standby_number:?} names {}; not a data-safety guarantee",
            display(primary),
            display(method),
            if secure {
                "[redacted]".into()
            } else {
                standby_names.join(",")
            }
        ),
        Resource::Catalog {
            images,
            component_entries,
            global,
        } => format!(
            "{} catalog; {} image entries {:?}; {component_entries} component entries; compatibility not inferred",
            if *global {
                "cluster-scoped"
            } else {
                "namespaced"
            },
            images.len(),
            images
        ),
        Resource::Webhook {
            services,
            failure_policies,
            rules,
            ca_present,
            external_clients,
        } => format!(
            "CNPG-applicable admission routing ({rules} hooks) services {services:?}; failure policies {failure_policies:?}; CA bundle present {ca_present}; external endpoints {external_clients} (URLs omitted)"
        ),
    }
}

fn options(values: &[&Option<String>]) -> usize {
    values
        .iter()
        .filter_map(|s| s.as_ref())
        .map(String::capacity)
        .sum()
}
fn strings(values: &Vec<String>) -> usize {
    values.capacity() * std::mem::size_of::<String>()
        + values.iter().map(String::capacity).sum::<usize>()
}
fn conditions_bytes(values: &Vec<Condition>) -> usize {
    values.capacity() * std::mem::size_of::<Condition>()
        + values
            .iter()
            .map(|c| c.type_.capacity() + c.status.capacity() + options(&[&c.reason]))
            .sum::<usize>()
}
impl Reconciliation {
    pub fn bytes(&self) -> usize {
        std::mem::size_of::<Self>() + conditions_bytes(&self.conditions)
    }
}
