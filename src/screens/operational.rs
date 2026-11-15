//! Pure projection of accepted evidence into rows.
use super::*;
pub(super) fn content(
    session: &Session,
    view: View,
    filter: &Filter,
    _log_window: crate::logs::timeline::Window,
    _severity: Option<crate::logs::Level>,
) -> Content {
    let store = &session.store;
    let now = session.clock.now();
    let mut out = Content {
        title: format!("{view:?}"),
        ..Default::default()
    };
    let mut push = |row: ScreenRow, labels: &Labels| {
        if filter.matches(&row.cells.join(" "), labels) {
            out.rows.push(row);
        }
    };
    match view {
        View::Backups | View::Databases | View::Roles | View::Subscriptions | View::Plugins => {
            out.headers = vec!["resource", "evidence", "state / details"];
            let kinds: &[Kind] = match view {
                View::Backups => &[Kind::Backup, Kind::ScheduledBackup],
                View::Databases => &[Kind::Database],
                View::Roles => &[Kind::DatabaseRole],
                View::Subscriptions => &[Kind::Publication, Kind::Subscription],
                _ => &[Kind::ImageCatalog, Kind::ClusterImageCatalog],
            };
            for kind in kinds {
                for e in store.evidence(*kind) {
                    let belongs = crate::health::cluster_key(store, e) == store.selected.as_ref()
                        && store.selected.is_some();
                    let catalog = matches!(kind, Kind::ImageCatalog | Kind::ClusterImageCatalog)
                        && (e.id.key.namespace == session.namespace
                            || *kind == Kind::ClusterImageCatalog
                            || session.config().all_namespaces);
                    if !belongs && !catalog {
                        continue;
                    }
                    if let Data::Operational(resource) = &e.data {
                        push(
                            row(
                                format!("{kind:?}:{}", e.id.key),
                                &e.id.key,
                                vec![
                                    e.id.key.to_string(),
                                    format!("{kind:?}"),
                                    crate::operational::summary(
                                        resource,
                                        session.config().profile == crate::cli::Profile::Secure,
                                    ),
                                ],
                                Some(View::Events),
                            ),
                            &e.labels,
                        );
                    }
                }
            }
            if let Some(cluster) = store.selected.as_ref().and_then(|k| store.cluster(k)) {
                let context = &cluster.operational;
                out.extra = if view == View::Backups {
                    format!(
                        "Reported latest success {:?}, failure {:?}, recovery point {:?}; retention {:?}. Backup status does not prove restore/PITR validity.",
                        context.latest_success_ms,
                        context.latest_failure_ms,
                        context.recovery_point_ms,
                        context.retention
                    )
                } else if view == View::Plugins {
                    for plugin in &context.plugins {
                        push(
                            row(
                                format!("plugin:{}:{}", cluster.id.key, plugin.name),
                                &cluster.id.key,
                                vec![
                                    cluster.id.key.to_string(),
                                    "plugin".into(),
                                    format!(
                                        "{} configured {} enabled {:?}; version {:?}; archiver {:?}; reported capabilities {:?}; opaque status omitted",
                                        plugin.name,
                                        plugin.configured,
                                        plugin.enabled,
                                        plugin.version,
                                        plugin.archiver,
                                        plugin.capabilities
                                    ),
                                ],
                                None,
                            ),
                            &cluster.labels,
                        );
                    }
                    format!(
                        "Image {:?}; operator version {:?} hash {:?}; catalog {:?}. Unreported version compatibility stays unknown.",
                        context.image,
                        context.operator_version,
                        context.operator_hash,
                        context.catalog
                    )
                } else {
                    "Application status is authoritative only for the reported generation; controller message contents are omitted.".into()
                };
            }
            out.empty = format!(
                "No matching operational rows; sources {:?}. Missing/forbidden evidence is local to this view.",
                kinds
                    .iter()
                    .map(|k| (k, store.watches.get(k).map(|w| w.status)))
                    .collect::<Vec<_>>()
            );
        }
        View::Certs => {
            out.headers = vec!["certificate", "source", "state / details"];
            out.empty="Certificate reads require explicit --certificate-secret namespace/name; no Secret list/watch is performed".into();
            for key in &session.config().certificate_secrets {
                let source = crate::auxiliary::Key::Certificate(key.clone());
                let observation = store.auxiliary.get(&source);
                let state = observation.map_or_else(
                    || "not yet read or disabled by profile".into(),
                    |o| {
                        format!(
                            "{:?}; {:?}",
                            o.attempt,
                            o.freshness(now, crate::auxiliary::stale_after(&source))
                        )
                    },
                );
                let metadata = observation
                    .and_then(|o| o.last_good.as_ref())
                    .and_then(|s| {
                        if let crate::auxiliary::Sample::Certificate(c) = &s.value {
                            Some(c)
                        } else {
                            None
                        }
                    });
                let summary=metadata.map_or(state.clone(),|c|format!("{state}; not before {} not after {}; SHA256 {}; {} chain certificates; metadata only, no TLS trust verification",c.not_before_ms,c.not_after_ms,c.fingerprint_sha256,c.chain_length));
                push(
                    row(
                        format!("certificate:{key}"),
                        key,
                        vec![
                            key.to_string(),
                            "explicit named public-certificate read".into(),
                            summary,
                        ],
                        None,
                    ),
                    &Labels::new(),
                );
            }
        }
        _ => unreachable!("operational projection"),
    }
    out
}
