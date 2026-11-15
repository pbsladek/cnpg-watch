//! Explicitly selected, isolated-cluster contracts. Never use a default context.
use cnpg_watch::{
    adapters,
    app::Session,
    cli::Options,
    config::{Config, Paths},
    kubernetes,
    model::*,
    probes::PodExec,
    reducer::{Store, WatchEvent, WatchUpdate},
};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api,
    api::{AttachParams, DeleteParams, DynamicObject, ListParams},
    core::{ApiResource, GroupVersionKind},
};
use secrecy::SecretString;
use std::{path::PathBuf, time::Duration};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

fn config() -> Config {
    assert_eq!(
        std::env::var("CNPG_WATCH_E2E").as_deref(),
        Ok("1"),
        "use the isolated e2e wrapper"
    );
    let context = std::env::var("E2E_CONTEXT").expect("E2E_CONTEXT");
    assert!(context.starts_with("k3d-cnpg-watch-"));
    let password_file =
        PathBuf::from(std::env::var("E2E_PASSWORD_FILE").expect("E2E_PASSWORD_FILE"));
    assert!(
        std::env::var_os("KUBECONFIG").is_some(),
        "explicit fixture kubeconfig required"
    );
    Config::resolve(
        Options {
            config: Some("tests/e2e/watch.toml".into()),
            kube_context: Some(context),
            password_file: Some(password_file),
            ..Options::default()
        },
        Paths::discover().unwrap(),
    )
    .unwrap()
}
async fn setup() -> (Config, kube::Client, ProbeTicket) {
    let config = config();
    let (client, namespace) = kubernetes::client(&config, &config.budgets).await.unwrap();
    let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(
        "postgresql.cnpg.io",
        "v1",
        "Cluster",
    ));
    let clusters: Api<DynamicObject> = Api::namespaced_with(client.clone(), &namespace, &resource);
    let c = adapters::cluster(clusters.get("payments").await.unwrap()).unwrap();
    let primary = if let Object::Cluster(c) = &c {
        c.primary.as_ref().unwrap().clone()
    } else {
        unreachable!()
    };
    let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
    let p = adapters::pod(pods.get(&primary).await.unwrap()).unwrap();
    let mut store = Store::new(config.budgets.clone());
    for object in [c, p] {
        store.watch(WatchEvent {
            scope: store.scope,
            watch: WatchEpoch(0),
            kind: object.kind(),
            update: WatchUpdate::Apply(object),
        });
    }
    let pod = store.pods().next().unwrap();
    let cluster = store.cluster(pod.cluster.as_ref().unwrap()).unwrap();
    let ticket = ProbeTicket {
        scope: store.scope,
        class: Default::default(),
        topology: TopologyEpoch(0),
        request: RequestId(1),
        cluster: cluster.id.clone(),
        pod: pod.id.clone(),
    };
    (config, client, ticket)
}
async fn admin(client: kube::Client, pod: &Key, sql: &str) -> String {
    let api: Api<Pod> = Api::namespaced(client, &pod.namespace);
    let mut p = api
        .exec(
            &pod.name,
            [
                "psql",
                "-XAtw",
                "-d",
                "postgres",
                "-v",
                "ON_ERROR_STOP=1",
                "-c",
                sql,
            ],
            &AttachParams::default()
                .container("postgres")
                .stdout(true)
                .stderr(true),
        )
        .await
        .unwrap();
    let mut out = String::new();
    let mut err = String::new();
    let mut stdout = p.stdout().unwrap();
    let mut stderr = p.stderr().unwrap();
    let status = p.take_status().unwrap();
    let (_, _, status) = tokio::join!(
        stdout.read_to_string(&mut out),
        stderr.read_to_string(&mut err),
        status
    );
    assert_eq!(
        status.unwrap().status.as_deref(),
        Some("Success"),
        "admin observer failed (contents suppressed)"
    );
    p.join().await.unwrap();
    out.trim().into()
}
async fn no_remote_probe(client: kube::Client, pod: &Key) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let count=admin(client.clone(),pod,"SELECT count(*) FROM pg_stat_activity WHERE application_name='cnpg-watch' AND pid <> pg_backend_pid()").await;
        if count == "0" {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "remote probe survived its statement/connect bound"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
#[ignore = "requires explicitly provisioned disposable k3d fixture"]
async fn actual_monitor_transport_authentication_timeout_status_caps_and_cancel() {
    let (config, client, ticket) = setup().await;
    let password = cnpg_watch::probes::password_file(config.password_file.as_ref().unwrap())
        .await
        .unwrap();
    let executor = PodExec::new(
        client.clone(),
        password,
        config.db_role.clone(),
        config.budgets.clone(),
    );
    let cancel = CancellationToken::new();
    let start = tokio::time::Instant::now();
    let sample = executor
        .execute(&ticket, &cancel)
        .await
        .expect("real monitor fast bundle");
    println!(
        "fast bundle {:?}; version {}; role {:?}; current {:?}",
        start.elapsed(),
        sample.server_version,
        sample.role,
        sample.current
    );
    if std::env::var_os("E2E_CAPTURE_FIXTURES").is_some() {
        let raw = executor
            .execute_sql(&ticket.pod.key, cnpg_watch::probes::FAST_SQL, &cancel)
            .await
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        value["system_id"] = "42".into();
        value["server_time"] = "2026-11-14T00:00:00Z".into();
        value["archive"]["stats_reset"] = "2026-11-14T00:00:00Z".into();
        for name in ["last_archived_time", "last_failed_time"] {
            value["archive"][name] = serde_json::Value::Null;
        }
        for peer in value["peers"].as_array_mut().unwrap() {
            peer["address"] = "192.0.2.11".into();
        }
        std::fs::write(
            "artifacts/phase1/primary-fast.json",
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(sample.role, Role::Primary);
    assert!(sample.current.is_some());
    assert_eq!(admin(client.clone(),&ticket.pod.key,"SELECT rolsuper OR rolcreaterole OR rolcreatedb OR rolreplication OR rolbypassrls FROM pg_roles WHERE rolname='watch_monitor'").await,"f");
    let bad = PodExec::new(
        client.clone(),
        SecretString::from("deliberately-invalid"),
        config.db_role.clone(),
        config.budgets.clone(),
    );
    assert_eq!(
        bad.execute(&ticket, &cancel).await,
        Err(ErrorClass::Authentication)
    );
    assert_eq!(
        executor
            .execute_sql(&ticket.pod.key, "SELECT pg_sleep(2)", &cancel)
            .await,
        Err(ErrorClass::Timeout)
    );
    no_remote_probe(client.clone(), &ticket.pod.key).await;
    assert!(
        executor
            .execute_sql(&ticket.pod.key, "SELECT '{}'::json; SELECT 1/0;", &cancel)
            .await
            .is_err(),
        "valid stdout must not override failed remote status"
    );
    assert_eq!(
        executor
            .execute_sql(&ticket.pod.key, "SELECT repeat('x',1048576)", &cancel)
            .await,
        Err(ErrorClass::OutputLimit)
    );
    let token = CancellationToken::new();
    let stop = token.clone();
    let (result, ()) = tokio::join!(
        executor.execute_sql(&ticket.pod.key, "SELECT pg_sleep(2)", &token),
        async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            stop.cancel();
        }
    );
    assert_eq!(result, Err(ErrorClass::Cancelled));
    no_remote_probe(client, &ticket.pod.key).await;
}

#[tokio::test]
#[ignore = "requires explicitly provisioned disposable k3d fixture"]
async fn assembled_session_samples_replaces_reconnects_and_degrades_without_sql() {
    let config = config();
    let mut session = Session::start(config.clone()).await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
    while session
        .store
        .samples
        .values()
        .filter(|o| o.last_good.is_some())
        .count()
        < 2
    {
        session.tick();
        assert!(
            tokio::time::Instant::now() < deadline,
            "no assembled samples: {:?}",
            session.store.watches
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(session.store.pods().filter(|p| p.role.is_some()).count(), 2);
    assert!(
        session
            .store
            .samples
            .values()
            .all(|o| o.last_good.is_some()),
        "both instances should be sampled"
    );
    let replica = session
        .store
        .pods()
        .find(|p| p.role == Some(Role::Replica))
        .unwrap()
        .id
        .clone();
    let (client, namespace) = kubernetes::client(&config, &config.budgets).await.unwrap();
    let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
    pods.delete(&replica.key.name, &DeleteParams::default())
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(150);
    loop {
        session.tick();
        if session
            .store
            .pod(&replica.key)
            .is_some_and(|p| p.id.uid != replica.uid && p.ready == Some(true))
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "replacement not observed"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if let Some(value) = session
        .store
        .samples
        .get(&replica.key)
        .and_then(|o| o.last_good.as_ref())
    {
        assert_ne!(value.ticket.pod.uid, replica.uid);
    }
    // Watches use a 30s server timeout; continued source health proves reconnect.
    let until = tokio::time::Instant::now() + Duration::from_secs(32);
    while tokio::time::Instant::now() < until {
        session.tick();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        session
            .store
            .watches
            .values()
            .all(|w| w.status == SyncState::Synced)
    );
    println!(
        "assembled samples {}; cache {} B; max queued {} B; tasks {}",
        session.completed,
        session.store.bytes(),
        session.max_queue_bytes,
        session.task_count()
    );
    assert_eq!(session.shutdown().await, 0);
    let mut disabled = config;
    disabled.password_file = None;
    let mut session = Session::start(disabled).await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while session.store.pods().filter(|p| p.role.is_some()).count() < 2 {
        session.tick();
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(matches!(session.sql, Availability::Unavailable(_)));
    assert_eq!(session.completed, 0);
    assert_eq!(session.shutdown().await, 0);
    assert_eq!(
        pods.list(&ListParams::default().labels("cnpg.io/podRole=instance"))
            .await
            .unwrap()
            .items
            .len(),
        2
    );
}
