#[path = "support/live.rs"]
mod live;
use clap::Parser;
use cnpg_watch::{
    app::Session,
    auxiliary::{Key as AuxKey, Sample},
    cli::{Cli, Command, Profile, Setup},
    diagnostics,
    evidence::Data,
    logs::{
        self,
        timeline::{Scope, Sources, Window},
    },
    model::*,
    navigation::{Filter, View},
    operational::{
        self,
        health::{Routing, webhook_routing},
    },
    screens,
    setup::rbac,
};
use kube::{
    Api,
    api::{DynamicObject, ListParams},
    core::{ApiResource, GroupVersionKind},
};
#[tokio::test]
#[ignore = "requires isolated operational CRs, explicit named certificate and forwarded operator metrics"]
async fn operational_resources_auxiliary_views_logs_and_redacted_dump() {
    let mut config = live::config(false, Profile::Incident);
    config.cluster = Some("payments".into());
    config.cluster_resources = true;
    config.pooler_stats = true;
    config.certificate_secrets = vec![Key::new("cnpg-watch-e2e", "payments-server").unwrap()];
    config.operator_metrics_url =
        Some(std::env::var("E2E_METRICS_URL").expect("explicit fixture metrics forward required"));
    let mut session = Session::start(config.clone()).await.unwrap();
    live::wait(&mut session,90,|s|s.store.watches.values().all(|w|w.status==SyncState::Synced)&&s.store.evidence(Kind::DatabaseRole).any(|e|matches!(&e.data,Data::Operational(r) if matches!(r.as_ref(),operational::Resource::Managed{reconciliation,..} if reconciliation.current() && reconciliation.applied==Some(true))))&&s.store.auxiliary.len()==2&&s.store.auxiliary.values().all(|o|o.last_good.is_some())&&s.completed>=2).await;
    for kind in [
        Kind::Backup,
        Kind::ScheduledBackup,
        Kind::Database,
        Kind::DatabaseRole,
        Kind::Publication,
        Kind::ImageCatalog,
        Kind::ClusterImageCatalog,
        Kind::ValidatingWebhook,
        Kind::MutatingWebhook,
    ] {
        assert!(session.store.evidence(kind).count() > 0, "{kind:?}");
    }
    for kind in [Kind::DatabaseRole, Kind::Subscription, Kind::FailoverQuorum] {
        let source = session.store.watches[&kind].source.as_ref().unwrap();
        assert_eq!(source.api_version, "postgresql.cnpg.io/v1");
        assert_eq!(
            source.resource,
            match kind {
                Kind::DatabaseRole => "databaseroles",
                Kind::Subscription => "subscriptions",
                _ => "failoverquorums",
            }
        );
    }
    assert!(session.store.evidence(Kind::Backup).any(|e|matches!(&e.data,Data::Operational(r) if matches!(r.as_ref(),operational::Resource::Backup{phase,..} if phase.as_deref()==Some("failed")))));
    assert!(
        session
            .health
            .reasons()
            .iter()
            .any(|r| r.id.starts_with("backup.failed"))
    );
    assert_eq!(
        webhook_routing(
            &session.store,
            &Key::new("cnpg-system", "cnpg-webhook-service").unwrap()
        ),
        Routing::Ready
    );
    assert!(matches!(
        session.store.auxiliary[&AuxKey::Metrics]
            .last_good
            .as_ref()
            .unwrap()
            .value,
        Sample::Metrics(_)
    ));
    let metrics_view =
        screens::content(&session, View::Operator, &Filter::All, Window::Follow, None);
    let metrics_row = metrics_view
        .rows
        .iter()
        .find(|r| r.id == "operator-metrics")
        .unwrap();
    assert!(
        metrics_row
            .cells
            .join(" ")
            .contains("controller_runtime_reconcile_total=")
    );
    assert!(!metrics_row.cells.join(" ").contains("Value("));
    for view in [
        View::Backups,
        View::Roles,
        View::Databases,
        View::Plugins,
        View::Certs,
        View::Replicas,
        View::Services,
        View::Storage,
    ] {
        let content = screens::content(&session, view, &Filter::All, Window::Follow, None);
        assert!(!content.rows.is_empty(), "{view:?}: {}", content.extra);
    }
    session.set_view(View::Logs);
    session.set_log_scope(Scope::Cluster);
    let operator = session
        .store
        .evidence(Kind::OperatorPod)
        .next()
        .unwrap()
        .clone();
    let source = logs::Source {
        channel: logs::Channel::Operator,
        scope: session.store.scope,
        key: operator.id.key,
        uid: operator.id.uid,
        container: "manager".into(),
    };
    session.set_log_sources(vec![source.clone()], 900);
    live::wait(&mut session, 20, |s| {
        s.logs
            .sources
            .get(&source)
            .is_some_and(|s| !s.lines.is_empty())
    })
    .await;
    session.sync_log_events();
    let cluster = logs::timeline::project(
        &session.store,
        &session.logs,
        &Scope::Cluster,
        Sources::default(),
        Window::Follow,
    );
    assert!(cluster.entries.len() <= logs::timeline::MAX_ROWS);
    assert!(cluster.entries.iter().all(|e| {
        e.correlation
            .as_ref()
            .is_some_and(|c| Some(c.cluster) == session.store.selected.as_ref())
    }));
    let full = logs::timeline::project(
        &session.store,
        &session.logs,
        &Scope::Operator,
        Sources::default(),
        Window::Follow,
    );
    assert!(!full.entries.is_empty());
    assert!(
        full.entries
            .iter()
            .all(|e| e.source.key.namespace == "cnpg-system")
    );
    let dump = diagnostics::snapshot(&session, "safe fixture screen");
    let text = serde_json::to_string(&dump).unwrap();
    for field in [
        "BEGIN CERTIFICATE",
        "PRIVATE KEY",
        "tls.key",
        "conninfo",
        "reconcileID",
        "watch_fixture",
    ] {
        assert!(!text.contains(field), "dump leaked {field}");
    }
    assert_eq!(dump["operational"]["schema_version"], 1);
    assert_eq!(dump["logs"]["timeline_schema_version"], 1);
    std::fs::create_dir_all("artifacts/phase56").unwrap();
    std::fs::write(
        "artifacts/phase56/operational-live.json",
        serde_json::to_vec_pretty(&dump).unwrap(),
    )
    .unwrap();
    // Capture only consumed public operational resources, never a Secret object or connection strings.
    let mut captures = serde_json::Map::new();
    for (kind, plural, global) in [
        ("Backup", "backups", false),
        ("ScheduledBackup", "scheduledbackups", false),
        ("DatabaseRole", "databaseroles", false),
        ("Database", "databases", false),
        ("Publication", "publications", false),
        ("Subscription", "subscriptions", false),
        ("FailoverQuorum", "failoverquorums", false),
        ("ImageCatalog", "imagecatalogs", false),
        ("ClusterImageCatalog", "clusterimagecatalogs", true),
    ] {
        let mut resource =
            ApiResource::from_gvk(&GroupVersionKind::gvk("postgresql.cnpg.io", "v1", kind));
        resource.plural = plural.into();
        let api: Api<DynamicObject> = if global {
            Api::all_with(session.client(), &resource)
        } else {
            Api::namespaced_with(session.client(), "cnpg-watch-e2e", &resource)
        };
        let items = api.list(&ListParams::default()).await.unwrap().items;
        captures.insert(plural.into(), serde_json::to_value(items).unwrap());
    }
    std::fs::write(
        "artifacts/phase56/operational-resources.json",
        serde_json::to_vec_pretty(&captures).unwrap(),
    )
    .unwrap();
    session.set_profile(Profile::Minimal).await;
    live::pump(&mut session, 2).await;
    assert!(session.store.auxiliary.is_empty());
    assert!(session.store.pooler_samples.is_empty());
    assert_eq!(session.active_log_streams(), 0);
    assert_eq!(session.shutdown().await, 0);
}
#[tokio::test]
#[ignore = "requires isolated runtime ServiceAccount and operational fixture"]
async fn denied_stats_named_secret_managed_webhook_and_operator_logs_stay_local() {
    let mut config = live::config(false, Profile::Incident);
    config.cluster = Some("payments".into());
    config.cluster_resources = true;
    config.pooler_stats = true;
    config.certificate_secrets = vec![Key::new("cnpg-watch-e2e", "payments-server").unwrap()];
    let (admin, namespace) = cnpg_watch::kubernetes::client(&config, &config.budgets)
        .await
        .unwrap();
    let client = live::identity(admin.clone(), &config, "phase56-reduced").await;
    let cli = Cli::try_parse_from([
        "cnpg-watch",
        "setup",
        "kubernetes-rbac",
        "--name",
        "phase56-reduced",
        "--service-account",
        "phase56-reduced",
    ])
    .unwrap();
    let Some(Command::Setup {
        command: Setup::KubernetesRbac(args),
    }) = cli.command
    else {
        panic!()
    };
    let mut grants = rbac::generate(&config, &args).unwrap();
    grants.retain(|m| !m.kind.starts_with("Cluster"));
    for grant in &mut grants {
        if let Some(rules) = &mut grant.rules {
            rules.retain(|r| {
                !r.resources
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(|s| ["secrets", "databaseroles", "pods/log"].contains(&s.as_str()))
            });
            for rule in rules {
                if rule
                    .resources
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(|s| s == "pods/exec")
                {
                    rule.resource_names = Some(vec!["payments-1".into(), "payments-2".into()]);
                }
            }
        }
    }
    rbac::reconcile(admin, grants, true, true).await.unwrap();
    let mut session = Session::with_client(config, client, namespace)
        .await
        .unwrap();
    session.set_view(View::Poolers);
    live::wait(&mut session, 90, |s| {
        s.store.watches[&Kind::DatabaseRole].status == SyncState::Failed(ErrorClass::Forbidden)
            && s.store.watches[&Kind::ValidatingWebhook].status
                == SyncState::Failed(ErrorClass::Forbidden)
            && s.store
                .auxiliary
                .values()
                .any(|o| o.attempt == Attempt::Failed(ErrorClass::Forbidden))
            && s.store
                .pooler_samples
                .values()
                .any(|o| o.attempt == Attempt::Failed(ErrorClass::Forbidden))
            && s.store
                .samples
                .values()
                .filter(|o| o.last_good.is_some())
                .count()
                == 2
    })
    .await;
    assert_eq!(
        session.store.watches[&Kind::Pooler].status,
        SyncState::Synced
    );
    assert_eq!(
        session.store.watches[&Kind::Backup].status,
        SyncState::Synced
    );
    session.set_view(View::Logs);
    let operator = session
        .store
        .evidence(Kind::OperatorPod)
        .next()
        .unwrap()
        .clone();
    let source = logs::Source {
        channel: logs::Channel::Operator,
        scope: session.store.scope,
        key: operator.id.key,
        uid: operator.id.uid,
        container: "manager".into(),
    };
    session.set_log_scope(Scope::Operator);
    session.set_log_sources(vec![source.clone()], 60);
    live::wait(&mut session, 20, |s| {
        s.logs
            .sources
            .get(&source)
            .is_some_and(|s| s.state == SyncState::Failed(ErrorClass::Forbidden))
    })
    .await;
    assert_eq!(
        session.store.watches[&Kind::Cluster].status,
        SyncState::Synced
    );
    assert_eq!(session.shutdown().await, 0);
}
