#[path = "support/live.rs"]
mod live;
use clap::Parser;
use cnpg_watch::{
    app::Session,
    cli::{Cli, Command, Profile, Setup},
    collectors, diagnostics, logs,
    model::*,
    navigation::{Filter, View},
    screens,
    setup::rbac,
};
use kube::{
    Api,
    api::{DynamicObject, ListParams, Patch, PatchParams},
    core::{ApiResource, GroupVersionKind},
};
use std::path::PathBuf;
#[tokio::test]
#[ignore = "requires isolated two-namespace fleet fixture"]
async fn fleet_scope_replacement_operator_pooler_logs_and_redacted_dump() {
    let mut config = live::config(true, Profile::Incident);
    config.paths.dumps = PathBuf::from(std::env::var("E2E_LOG_FILE").unwrap())
        .parent()
        .unwrap()
        .join("dumps");
    let mut session = Session::start(config.clone()).await.unwrap();
    live::wait(&mut session, 45, |s| {
        s.store.clusters().count() == 2
            && s.store.evidence(Kind::Pooler).count() == 1
            && s.completed >= 2
            && s.store
                .watches
                .values()
                .all(|w| w.status == SyncState::Synced)
    })
    .await;
    assert_eq!(session.view, View::Fleet);
    assert_eq!(
        screens::content(
            &session,
            View::Fleet,
            &Filter::All,
            cnpg_watch::logs::timeline::Window::Follow,
            None
        )
        .rows
        .len(),
        2
    );
    let payments = Key::new("cnpg-watch-e2e", "payments").unwrap();
    let inventory = Key::new("cnpg-watch-e2e-other", "inventory").unwrap();
    assert_eq!(session.store.selected.as_ref(), Some(&payments));
    assert!(
        session
            .store
            .samples
            .keys()
            .all(|k| k.namespace == payments.namespace),
        "unselected fleet SQL started"
    );
    let pooler = session
        .store
        .pods_for(&payments)
        .find(|p| p.pooler.is_some())
        .unwrap()
        .clone();
    assert!(pooler.ready == Some(true));
    assert!(session.store.evidence(Kind::Deployment).count() >= 1);
    let primary = session
        .store
        .pods_for(&payments)
        .find(|p| p.role == Some(Role::Primary))
        .unwrap()
        .clone();
    let source = logs::Source {
        channel: cnpg_watch::logs::Channel::Postgres,
        scope: session.store.scope,
        key: primary.id.key.clone(),
        uid: primary.id.uid.clone(),
        container: "postgres".into(),
    };
    live::admin(
        session.client(),
        &primary.id.key,
        "DO $$ BEGIN RAISE LOG 'cnpg-watch-fixture-safe-log'; END $$",
    )
    .await;
    session.set_view(View::Logs);
    session.set_log_sources(vec![source], 900);
    live::wait(&mut session, 15, |s| {
        s.logs.sources.values().any(|v| !v.lines.is_empty())
    })
    .await;
    assert_eq!(session.active_log_streams(), 1);
    assert!(session.logs.bytes <= session.budgets.log_total_bytes);
    session.set_view(View::Instances);
    live::pump(&mut session, 1).await;
    assert_eq!(session.active_log_streams(), 0);
    session.set_view(View::Logs);
    session.set_log_sources(
        vec![logs::Source {
            channel: cnpg_watch::logs::Channel::Postgres,
            scope: session.store.scope,
            key: pooler.id.key,
            uid: pooler.id.uid,
            container: "pgbouncer".into(),
        }],
        300,
    );
    live::wait(&mut session, 15, |s| {
        s.logs
            .sources
            .values()
            .any(|v| v.active && v.state == SyncState::Synced)
    })
    .await;
    session.set_view(View::Instances);
    let dump = diagnostics::write(&session, "bounded topology screen").unwrap();
    let text = std::fs::read_to_string(&dump).unwrap();
    assert!(
        !text.contains(
            std::fs::read_to_string(config.password_file.as_ref().unwrap())
                .unwrap()
                .trim()
        )
    );
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert!(
        value["redacted_fields"]
            .as_array()
            .unwrap()
            .contains(&"query_text".into())
    );
    let old = session.store.scope;
    let mut other = config.clone();
    other.all_namespaces = false;
    other.namespace = Some(inventory.namespace.clone());
    session.replace_scope(other.clone());
    assert!(session.store.scope.0 > old.0);
    assert!(session.store.clusters().next().is_none());
    live::wait(&mut session, 40, |s| {
        s.store.cluster(&inventory).is_some()
            && s.store
                .samples
                .keys()
                .any(|k| k.namespace == inventory.namespace)
    })
    .await;
    assert_eq!(session.store.clusters().count(), 1);
    assert!(
        session
            .store
            .pods()
            .all(|p| p.id.key.namespace == inventory.namespace)
    );
    let old = session.store.scope;
    let mut invalid = other.clone();
    invalid.context = Some("explicitly-missing-fixture-context".into());
    session.replace_scope(invalid);
    live::wait(&mut session, 15, |s| s.scope_error.is_some()).await;
    assert!(session.store.scope.0 > old.0);
    assert!(session.store.clusters().next().is_none());
    session.replace_scope(config.clone());
    live::wait(&mut session, 40, |s| {
        s.store.clusters().count() == 2 && s.completed >= 3
    })
    .await;
    // Complete empty selector means an empty synchronized result, not an authorization gap.
    let mut empty = config;
    empty.selector = Some("cnpg-watch-fixture=does-not-exist".into());
    session.replace_scope(empty);
    live::wait(&mut session, 30, |s| {
        s.store
            .watches
            .get(&Kind::Cluster)
            .is_some_and(|w| w.status == SyncState::Synced)
    })
    .await;
    assert_eq!(session.store.clusters().count(), 0);
    println!(
        "two namespaces, independent operator/pooler evidence, selected-only SQL/logs, context errors/recovery, empty scope, and redacted dump passed"
    );
    assert_eq!(session.shutdown().await, 0);
}
#[tokio::test]
#[ignore = "requires isolated fixture and real runtime ServiceAccount"]
async fn reduced_runtime_permissions_remain_local_and_revocation_preserves_samples() {
    let mut config = live::config(false, Profile::Incident);
    config.namespace = Some("cnpg-watch-e2e".into());
    config.operator_watch = false;
    config.cluster = Some("payments".into());
    let (admin, ns) = cnpg_watch::kubernetes::client(&config, &config.budgets)
        .await
        .unwrap();
    let client = live::identity(admin.clone(), &config, "phase3-runtime").await;
    let cli = Cli::try_parse_from([
        "cnpg-watch",
        "--namespace",
        "cnpg-watch-e2e",
        "--profile",
        "incident",
        "--no-operator-watch",
        "setup",
        "kubernetes-rbac",
        "--name",
        "phase3-runtime",
        "--service-account",
        "phase3-runtime",
    ])
    .unwrap();
    let Some(Command::Setup {
        command: Setup::KubernetesRbac(args),
    }) = cli.command
    else {
        unreachable!()
    };
    let desired = rbac::generate(&config, &args).unwrap();
    rbac::reconcile(admin.clone(), desired, true, true)
        .await
        .unwrap();
    let mut session = Session::with_client(config.clone(), client, ns.clone())
        .await
        .unwrap();
    live::wait(&mut session, 45, |s| {
        s.store
            .samples
            .values()
            .filter(|o| o.last_good.is_some())
            .count()
            == 2
            && s.store
                .watches
                .values()
                .all(|w| w.status == SyncState::Synced)
    })
    .await;
    assert!(session.operator.is_none());
    let good = session
        .store
        .samples
        .values()
        .filter(|o| o.last_good.is_some())
        .count();
    assert_eq!(good, 2);
    let roles: Api<k8s_openapi::api::rbac::v1::Role> = Api::namespaced(admin, &ns);
    let mut role = roles.get("phase3-runtime").await.unwrap();
    role.rules.as_mut().unwrap().retain(|r| {
        !r.resources
            .as_ref()
            .unwrap()
            .iter()
            .any(|r| r == "pods/log" || r == "pods/exec" || r == "services")
    });
    roles.patch("phase3-runtime",&PatchParams::default(),&Patch::Merge(serde_json::json!({"metadata":{"resourceVersion":role.metadata.resource_version},"rules":role.rules}))).await.unwrap();
    let pod = session
        .store
        .selected_pods()
        .find(|p| p.role == Some(Role::Primary))
        .unwrap()
        .clone();
    session.set_view(View::Logs);
    session.set_log_sources(
        vec![logs::Source {
            channel: cnpg_watch::logs::Channel::Postgres,
            scope: session.store.scope,
            key: pod.id.key,
            uid: pod.id.uid,
            container: "postgres".into(),
        }],
        300,
    );
    session.refresh().await;
    live::wait(&mut session, 20, |s| {
        s.logs
            .sources
            .values()
            .any(|v| v.state == SyncState::Failed(ErrorClass::Forbidden))
            && s.store
                .samples
                .values()
                .any(|o| matches!(o.attempt, Attempt::Failed(ErrorClass::Forbidden)))
    })
    .await;
    assert!(
        session
            .store
            .samples
            .values()
            .all(|o| o.last_good.is_some())
    );
    assert_eq!(
        session.store.watches[&Kind::Cluster].status,
        SyncState::Synced
    );
    assert_eq!(session.store.watches[&Kind::Pod].status, SyncState::Synced);
    assert_eq!(session.shutdown().await, 0);
    // Restarting under the reduced identity establishes an actual service-watch denial.
    let (admin, _) = cnpg_watch::kubernetes::client(&config, &config.budgets)
        .await
        .unwrap();
    let client = live::identity(admin, &config, "phase3-runtime").await;
    let mut session = Session::with_client(config, client, ns).await.unwrap();
    live::wait(&mut session, 30, |s| {
        s.store.watches[&Kind::Service].status == SyncState::Failed(ErrorClass::Forbidden)
            && s.store.clusters().count() == 1
    })
    .await;
    assert_eq!(
        session.store.watches[&Kind::Cluster].status,
        SyncState::Synced
    );
    assert!(session.store.pods().count() >= 2);
    assert_eq!(session.shutdown().await, 0);
    println!(
        "actual least-privilege watches, forbidden logs/exec/services, retained SQL, and independent topology passed"
    );
}
#[tokio::test]
#[ignore = "captures consumed source fields from the isolated fleet fixture"]
async fn capture_compact_schema_fixtures() {
    if std::env::var_os("E2E_CAPTURE_FIXTURES").is_none() {
        return;
    }
    let config = live::config(true, Profile::Standard);
    let (client, ns) = cnpg_watch::kubernetes::client(&config, &config.budgets)
        .await
        .unwrap();
    let dir = std::path::Path::new("artifacts/phase34/captures");
    std::fs::create_dir_all(dir).unwrap();
    for plan in collectors::plans(&config, &ns) {
        let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(
            plan.group,
            plan.version,
            plan.resource_kind,
        ));
        let api: Api<DynamicObject> = if let Some(ns) = &plan.namespace {
            Api::namespaced_with(client.clone(), ns, &resource)
        } else {
            Api::all_with(client.clone(), &resource)
        };
        let params = plan
            .labels
            .as_ref()
            .map_or(ListParams::default(), |l| ListParams::default().labels(l));
        let list = api.list(&params).await.unwrap();
        let objects: Vec<_> = list
            .items
            .into_iter()
            .take(8)
            .map(|o| {
                let mut value = serde_json::to_value(o).unwrap();
                value["metadata"].as_object_mut().unwrap().retain(|k, _| {
                    [
                        "namespace",
                        "name",
                        "uid",
                        "labels",
                        "ownerReferences",
                        "creationTimestamp",
                    ]
                    .contains(&k.as_str())
                });
                if let Some(status) = value.get_mut("status").and_then(|v| v.as_object_mut()) {
                    status.remove("secrets");
                }
                if plan.kind == Kind::Deployment
                    && let Some(containers) = value
                        .pointer_mut("/spec/template/spec/containers")
                        .and_then(|v| v.as_array_mut())
                {
                    for container in containers {
                        container
                            .as_object_mut()
                            .unwrap()
                            .retain(|k, _| k == "name" || k == "image");
                    }
                }
                value
            })
            .collect();
        std::fs::write(
            dir.join(format!("{:?}.json", plan.kind)),
            serde_json::to_vec_pretty(&objects).unwrap(),
        )
        .unwrap();
    }
}
