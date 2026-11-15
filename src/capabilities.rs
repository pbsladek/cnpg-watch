//! One permission/profile catalog for setup, preflight, and live presentation.
use crate::{
    cli::Profile,
    model::{Availability, ErrorClass, Kind, SyncState},
    reducer::Store,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    Topology,
    FastSql,
    Logs,
    Operator,
    Poolers,
    Backups,
    Services,
    Storage,
    Events,
    Certificates,
    ManagedObjects,
    ClusterPosture,
    DeepSql,
    MediumSql,
    PoolerStats,
    ReplicaTopology,
    Plugins,
    OperatorMetrics,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Database,
    Operator,
    Cluster,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Requirement {
    pub group: &'static str,
    pub version: &'static str,
    pub kind: &'static str,
    pub resource: &'static str,
    pub verbs: &'static [&'static str],
    pub scope: Scope,
}
const READ: &[&str] = &["get", "list", "watch"];
const fn req(
    group: &'static str,
    version: &'static str,
    kind: &'static str,
    resource: &'static str,
    scope: Scope,
) -> Requirement {
    Requirement {
        group,
        version,
        kind,
        resource,
        verbs: READ,
        scope,
    }
}
use Scope::{Cluster as Global, Database as Db, Operator as Op};
const POD: Requirement = req("", "v1", "Pod", "pods", Db);
const EXEC: Requirement = Requirement {
    resource: "pods/exec",
    verbs: &["get", "create"],
    ..POD
};
const LOG: Requirement = Requirement {
    resource: "pods/log",
    verbs: &["get"],
    ..POD
};
#[derive(Debug, Serialize)]
pub struct FeatureSpec {
    pub feature: Feature,
    pub name: &'static str,
    pub implemented: bool,
    pub kubernetes: &'static [Requirement],
    pub postgres_role: Option<&'static str>,
    pub reduced_mode: &'static str,
}
pub const CATALOG: &[FeatureSpec] = &[
    FeatureSpec {
        feature: Feature::Topology,
        name: "Cluster and instance topology",
        implemented: true,
        kubernetes: &[
            req("postgresql.cnpg.io", "v1", "Cluster", "clusters", Db),
            POD,
        ],
        postgres_role: None,
        reduced_mode: "Cluster access is needed; missing pod access degrades instance rows",
    },
    FeatureSpec {
        feature: Feature::FastSql,
        name: "Fast PostgreSQL probes",
        implemented: true,
        kubernetes: &[EXEC],
        postgres_role: Some("pg_monitor"),
        reduced_mode: "SQL unavailable; Kubernetes observation continues",
    },
    FeatureSpec {
        feature: Feature::Logs,
        name: "Selected logs",
        implemented: true,
        kubernetes: &[LOG, Requirement { scope: Op, ..LOG }],
        postgres_role: None,
        reduced_mode: "Selected logs require incident profile and an active log view",
    },
    FeatureSpec {
        feature: Feature::Operator,
        name: "Operator evidence",
        implemented: true,
        kubernetes: &[
            req("apps", "v1", "Deployment", "deployments", Op),
            Requirement { scope: Op, ..POD },
            req("", "v1", "Event", "events", Op),
            req("coordination.k8s.io", "v1", "Lease", "leases", Op),
            req("", "v1", "Service", "services", Op),
            req(
                "discovery.k8s.io",
                "v1",
                "EndpointSlice",
                "endpointslices",
                Op,
            ),
        ],
        postgres_role: None,
        reduced_mode: "Operator evidence unavailable; database observation is independent",
    },
    FeatureSpec {
        feature: Feature::Poolers,
        name: "Pooler availability",
        implemented: true,
        kubernetes: &[
            req("postgresql.cnpg.io", "v1", "Pooler", "poolers", Db),
            req("apps", "v1", "Deployment", "deployments", Db),
            req("apps", "v1", "ReplicaSet", "replicasets", Db),
        ],
        postgres_role: None,
        reduced_mode: "Pooler availability is observed; statistics are independently configured",
    },
    FeatureSpec {
        feature: Feature::Backups,
        name: "Backups",
        implemented: true,
        kubernetes: &[
            req("postgresql.cnpg.io", "v1", "Backup", "backups", Db),
            req(
                "postgresql.cnpg.io",
                "v1",
                "ScheduledBackup",
                "scheduledbackups",
                Db,
            ),
        ],
        postgres_role: None,
        reduced_mode: "Backup view arrives in Phase 6",
    },
    FeatureSpec {
        feature: Feature::Services,
        name: "Services and endpoints",
        implemented: true,
        kubernetes: &[
            req("", "v1", "Service", "services", Db),
            req(
                "discovery.k8s.io",
                "v1",
                "EndpointSlice",
                "endpointslices",
                Db,
            ),
        ],
        postgres_role: None,
        reduced_mode: "Connectivity evidence requires synchronized services and EndpointSlices",
    },
    FeatureSpec {
        feature: Feature::Storage,
        name: "Storage",
        implemented: true,
        kubernetes: &[req(
            "",
            "v1",
            "PersistentVolumeClaim",
            "persistentvolumeclaims",
            Db,
        )],
        postgres_role: None,
        reduced_mode: "Storage evidence unavailable",
    },
    FeatureSpec {
        feature: Feature::Events,
        name: "Events",
        implemented: true,
        kubernetes: &[req("", "v1", "Event", "events", Db)],
        postgres_role: None,
        reduced_mode: "Event evidence unavailable",
    },
    FeatureSpec {
        feature: Feature::Certificates,
        name: "Certificates",
        implemented: true,
        kubernetes: &[],
        postgres_role: None,
        reduced_mode: "Only explicit named tls.crt reads are permitted; no Secret list/watch",
    },
    FeatureSpec {
        feature: Feature::ManagedObjects,
        name: "Managed objects",
        implemented: true,
        kubernetes: &[
            req("postgresql.cnpg.io", "v1", "Database", "databases", Db),
            req(
                "postgresql.cnpg.io",
                "v1",
                "DatabaseRole",
                "databaseroles",
                Db,
            ),
            req(
                "postgresql.cnpg.io",
                "v1",
                "Publication",
                "publications",
                Db,
            ),
            req(
                "postgresql.cnpg.io",
                "v1",
                "Subscription",
                "subscriptions",
                Db,
            ),
        ],
        postgres_role: None,
        reduced_mode: "Reconciliation is per resource and observed generation; missing endpoints/permissions stay local",
    },
    FeatureSpec {
        feature: Feature::ClusterPosture,
        name: "Optional cluster posture",
        implemented: true,
        kubernetes: &[
            req(
                "postgresql.cnpg.io",
                "v1",
                "ClusterImageCatalog",
                "clusterimagecatalogs",
                Global,
            ),
            req(
                "admissionregistration.k8s.io",
                "v1",
                "ValidatingWebhookConfiguration",
                "validatingwebhookconfigurations",
                Global,
            ),
            req(
                "admissionregistration.k8s.io",
                "v1",
                "MutatingWebhookConfiguration",
                "mutatingwebhookconfigurations",
                Global,
            ),
        ],
        postgres_role: None,
        reduced_mode: "Optional cluster-scoped metadata; never needed for namespaced observation",
    },
    FeatureSpec {
        feature: Feature::DeepSql,
        name: "Incident diagnostics",
        implemented: true,
        kubernetes: &[EXEC],
        postgres_role: Some("pg_monitor"),
        reduced_mode: "Deep SQL requires incident profile, capabilities, and an active diagnostic view",
    },
    FeatureSpec {
        feature: Feature::MediumSql,
        name: "Replication/settings probes",
        implemented: true,
        kubernetes: &[EXEC],
        postgres_role: Some("pg_monitor"),
        reduced_mode: "Medium SQL unavailable; fast SQL remains independent",
    },
    FeatureSpec {
        feature: Feature::PoolerStats,
        name: "PgBouncer statistics",
        implemented: true,
        kubernetes: &[EXEC],
        postgres_role: None,
        reduced_mode: "Explicit --pooler-stats uses the built-in local peer console identity; readiness is independent",
    },
    FeatureSpec {
        feature: Feature::ReplicaTopology,
        name: "Replica topology",
        implemented: true,
        kubernetes: &[req(
            "postgresql.cnpg.io",
            "v1",
            "FailoverQuorum",
            "failoverquorums",
            Db,
        )],
        postgres_role: None,
        reduced_mode: "Missing quorum evidence never implies safe or unsafe promotion",
    },
    FeatureSpec {
        feature: Feature::Plugins,
        name: "Plugin and image posture",
        implemented: true,
        kubernetes: &[req(
            "postgresql.cnpg.io",
            "v1",
            "ImageCatalog",
            "imagecatalogs",
            Db,
        )],
        postgres_role: None,
        reduced_mode: "Compatibility stays unknown without reported version/capability evidence",
    },
    FeatureSpec {
        feature: Feature::OperatorMetrics,
        name: "Optional operator metrics",
        implemented: true,
        kubernetes: &[],
        postgres_role: None,
        reduced_mode: "Metrics require explicit URL configuration and never gate operator or database availability",
    },
];

pub fn enabled(feature: Feature, profile: Profile) -> bool {
    match feature {
        Feature::Topology
        | Feature::Services
        | Feature::Storage
        | Feature::Events
        | Feature::Operator => true,
        Feature::Logs | Feature::DeepSql => profile == Profile::Incident,
        Feature::Certificates => false,
        _ => profile != Profile::Minimal,
    }
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Redaction {
    Standard,
    Strict,
}
#[derive(Debug, Serialize)]
pub struct Envelope {
    pub profile: Profile,
    pub redaction: Redaction,
    pub secret_reads: bool,
    pub query_text: bool,
    pub background_sql: bool,
    pub sql: bool,
    pub logs_requested: bool,
    pub minimum_refresh_ms: u64,
    pub default_refresh_ms: u64,
}
pub fn envelope(profile: Profile) -> Envelope {
    Envelope {
        profile,
        redaction: if profile == Profile::Secure {
            Redaction::Strict
        } else {
            Redaction::Standard
        },
        secret_reads: false,
        query_text: false,
        background_sql: false,
        sql: enabled(Feature::FastSql, profile),
        logs_requested: enabled(Feature::Logs, profile),
        minimum_refresh_ms: 100,
        default_refresh_ms: match profile {
            Profile::Minimal => 5000,
            Profile::Standard => 2000,
            Profile::Incident => 1000,
            Profile::Secure => 3000,
        },
    }
}
#[derive(Debug, Serialize)]
pub struct Capability {
    #[serde(flatten)]
    pub spec: &'static FeatureSpec,
    pub requested: bool,
    pub availability: Availability,
    pub state: &'static str,
}
pub fn initial(profile: Profile, configured: bool) -> Vec<Capability> {
    CATALOG
        .iter()
        .map(|spec| {
            let requested = enabled(spec.feature, profile);
            let (availability, state) = if !requested {
                (Availability::DisabledByProfile, "disabled_by_profile")
            } else if !spec.implemented {
                (
                    Availability::Unavailable(ErrorClass::Unsupported),
                    "unfinished",
                )
            } else if spec.postgres_role.is_some() && !configured {
                (
                    Availability::Unavailable(ErrorClass::Authentication),
                    "credentials_missing",
                )
            } else {
                (Availability::Unknown, "unknown")
            };
            Capability {
                spec,
                requested,
                availability,
                state,
            }
        })
        .collect()
}
pub fn observed(store: &Store, profile: Profile, sql: Availability) -> Vec<Capability> {
    let mut caps = initial(profile, true);
    let topology = &mut caps[0];
    topology.availability = match store.watches.get(&Kind::Cluster).map(|w| w.status) {
        Some(SyncState::Synced) => Availability::Available,
        Some(SyncState::Failed(e)) => Availability::Unavailable(e),
        _ => Availability::Unknown,
    };
    topology.state = match store.watches.get(&Kind::Cluster).map(|w| w.status) {
        Some(SyncState::Synced) if store.clusters().count() == 0 => "empty",
        Some(SyncState::Synced) => "available",
        Some(SyncState::Failed(ErrorClass::Forbidden)) => "forbidden",
        Some(SyncState::Failed(_)) => "unavailable",
        _ => "syncing",
    };
    if let Some(SyncState::Failed(e)) = store.watches.get(&Kind::Pod).map(|w| w.status) {
        topology.availability = Availability::Unavailable(e);
        topology.state = if e == ErrorClass::Forbidden {
            "forbidden"
        } else {
            "unavailable"
        };
    }
    let sql_cap = &mut caps[1];
    let selected_samples = || {
        store
            .selected_pods()
            .filter_map(|p| store.samples.get(&p.id.key))
    };
    let latest = selected_samples()
        .filter_map(|o| match o.attempt {
            crate::model::Attempt::Failed(e) => Some(e),
            _ => None,
        })
        .next();
    sql_cap.availability = if !sql_cap.requested {
        Availability::DisabledByProfile
    } else if let Some(error) = latest {
        Availability::Unavailable(error)
    } else {
        sql
    };
    sql_cap.state = match sql_cap.availability {
        Availability::DisabledByProfile => "disabled_by_profile",
        Availability::Unavailable(ErrorClass::Forbidden) => "forbidden",
        Availability::Unavailable(_) => "unavailable",
        Availability::Available if selected_samples().any(|s| s.last_good.is_some()) => "available",
        _ => "unknown",
    };
    for cap in caps.iter_mut().filter(|c| {
        c.requested
            && c.spec.implemented
            && !matches!(c.spec.feature, Feature::Topology | Feature::FastSql)
    }) {
        let kinds: &[Kind] = match cap.spec.feature {
            Feature::Services => &[Kind::Service, Kind::EndpointSlice],
            Feature::Storage => &[Kind::Pvc],
            Feature::Events => &[Kind::Event],
            Feature::Poolers => &[Kind::Pooler],
            Feature::Backups => &[Kind::Backup, Kind::ScheduledBackup],
            Feature::ManagedObjects => &[
                Kind::Database,
                Kind::DatabaseRole,
                Kind::Publication,
                Kind::Subscription,
            ],
            Feature::ReplicaTopology => &[Kind::FailoverQuorum],
            Feature::Plugins => &[Kind::ImageCatalog],
            Feature::ClusterPosture => &[
                Kind::ClusterImageCatalog,
                Kind::ValidatingWebhook,
                Kind::MutatingWebhook,
            ],
            Feature::Operator => &[Kind::Deployment, Kind::OperatorPod],
            _ => &[],
        };
        if !kinds.is_empty() {
            if let Some(error) = kinds
                .iter()
                .filter_map(|k| store.watches.get(k))
                .find_map(|w| {
                    if let SyncState::Failed(e) = w.status {
                        Some(e)
                    } else {
                        None
                    }
                })
            {
                cap.availability = Availability::Unavailable(error);
                cap.state = if error == ErrorClass::Forbidden {
                    "forbidden"
                } else if error == ErrorClass::Unavailable {
                    "absent"
                } else {
                    "unavailable"
                };
            } else if kinds.iter().all(|k| {
                store
                    .watches
                    .get(k)
                    .is_some_and(|w| w.status == SyncState::Synced)
            }) {
                cap.availability = Availability::Available;
                cap.state = if kinds.iter().all(|k| store.objects(*k).next().is_none()) {
                    "empty"
                } else {
                    "available"
                };
            } else {
                cap.availability = Availability::Unknown;
                cap.state = "syncing";
            }
        } else if matches!(cap.spec.feature, Feature::MediumSql | Feature::DeepSql) {
            let class = if cap.spec.feature == Feature::MediumSql {
                crate::probes::registry::Class::Medium
            } else {
                crate::probes::registry::Class::Deep
            };
            let observations: Vec<_> = store
                .diagnostics
                .iter()
                .filter(|((key, c), _)| {
                    *c == class
                        && store
                            .pod(key)
                            .is_some_and(|p| p.cluster.as_ref() == store.selected.as_ref())
                })
                .map(|(_, o)| o)
                .collect();
            if let Some(e) = observations.iter().find_map(|o| {
                if let crate::model::Attempt::Failed(e) = o.attempt {
                    Some(e)
                } else {
                    None
                }
            }) {
                cap.availability = Availability::Unavailable(e);
                cap.state = if e == ErrorClass::Forbidden {
                    "forbidden"
                } else {
                    "unavailable"
                };
            } else if observations.iter().any(|o| o.last_good.is_some()) {
                cap.availability = Availability::Available;
                cap.state = "available";
            } else {
                cap.availability = Availability::Unknown;
                cap.state = if class == crate::probes::registry::Class::Deep {
                    "view_inactive"
                } else {
                    "unknown"
                };
            }
        } else if cap.spec.feature == Feature::PoolerStats {
            let observations: Vec<_> = store
                .pooler_samples
                .values()
                .filter(|o| store.selected.as_ref() == Some(&o.latest_ticket.cluster.key))
                .collect();
            if let Some(e) = observations.iter().find_map(|o| {
                if let crate::model::Attempt::Failed(e) = o.attempt {
                    Some(e)
                } else {
                    None
                }
            }) {
                cap.availability = Availability::Unavailable(e);
                cap.state = if e == ErrorClass::Forbidden {
                    "forbidden"
                } else {
                    "unavailable"
                };
            } else if observations.iter().any(|o| o.last_good.is_some()) {
                cap.availability = Availability::Available;
                cap.state = "available";
            } else {
                cap.state = "view_inactive";
            }
        } else if cap.spec.feature == Feature::Logs {
            cap.availability = Availability::Unknown;
            cap.state = "view_inactive";
        }
    }
    caps
}

fn apply_config(caps: &mut [Capability], config: &crate::config::Config) {
    for (feature, enabled) in [
        (Feature::ClusterPosture, config.cluster_resources),
        (
            Feature::Certificates,
            !config.certificate_secrets.is_empty(),
        ),
        (
            Feature::OperatorMetrics,
            config.operator_watch && config.operator_metrics_url.is_some(),
        ),
    ] {
        if let Some(c) = caps.iter_mut().find(|c| c.spec.feature == feature) {
            let requested = enabled && config.profile != Profile::Minimal;
            c.requested = requested;
            if !requested {
                c.availability = if config.profile == Profile::Minimal {
                    Availability::DisabledByProfile
                } else {
                    Availability::DisabledByConfig
                };
                c.state = if config.profile == Profile::Minimal {
                    "disabled_by_profile"
                } else {
                    "disabled_by_config"
                };
            } else if matches!(
                c.availability,
                Availability::DisabledByProfile | Availability::DisabledByConfig
            ) {
                c.availability = Availability::Unknown;
                c.state = "unknown";
            }
        }
    }

    if !config.pooler_stats
        && let Some(c) = caps
            .iter_mut()
            .find(|c| c.spec.feature == Feature::PoolerStats)
    {
        c.requested = false;
        c.availability = Availability::DisabledByConfig;
        c.state = "disabled_by_config";
    }
    if !config.operator_watch
        && let Some(cap) = caps
            .iter_mut()
            .find(|c| c.spec.feature == Feature::Operator)
    {
        cap.requested = false;
        cap.availability = Availability::DisabledByConfig;
        cap.state = "disabled_by_config";
    }
}

pub fn initial_for(config: &crate::config::Config) -> Vec<Capability> {
    let mut caps = initial(config.profile, config.password_file.is_some());
    apply_config(&mut caps, config);
    caps
}
pub fn observed_for(
    store: &Store,
    config: &crate::config::Config,
    sql: Availability,
) -> Vec<Capability> {
    let mut caps = observed(store, config.profile, sql);
    for (feature, metrics) in [
        (Feature::Certificates, false),
        (Feature::OperatorMetrics, true),
    ] {
        if let Some(cap) = caps.iter_mut().find(|c| c.spec.feature == feature) {
            let observations: Vec<_> = store
                .auxiliary
                .iter()
                .filter(|(k, _)| matches!(k, crate::auxiliary::Key::Metrics) == metrics)
                .map(|(_, o)| o)
                .collect();
            if let Some(error) = observations.iter().find_map(|o| {
                if let crate::model::Attempt::Failed(e) = o.attempt {
                    Some(e)
                } else {
                    None
                }
            }) {
                cap.availability = Availability::Unavailable(error);
                cap.state = if error == ErrorClass::Forbidden {
                    "forbidden"
                } else {
                    "unavailable"
                };
            } else if observations.iter().any(|o| o.last_good.is_some()) {
                cap.availability = Availability::Available;
                cap.state = "available";
            }
        }
    }
    apply_config(&mut caps, config);
    caps
}

pub fn log_state(caps: &mut [Capability], book: &crate::logs::Book) {
    if let Some(cap) = caps
        .iter_mut()
        .find(|c| c.spec.feature == Feature::Logs && c.requested)
    {
        let states: Vec<_> = book
            .sources
            .values()
            .filter(|s| s.active)
            .map(|s| s.state)
            .collect();
        if let Some(e) = states.iter().find_map(|s| {
            if let SyncState::Failed(e) = s {
                Some(*e)
            } else {
                None
            }
        }) {
            cap.availability = Availability::Unavailable(e);
            cap.state = if e == ErrorClass::Forbidden {
                "forbidden"
            } else {
                "unavailable"
            };
        } else if !states.is_empty() && states.iter().all(|s| *s == SyncState::Synced) {
            cap.availability = Availability::Available;
            cap.state = "available";
        }
    }
}
