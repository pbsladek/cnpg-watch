#[path = "support/live.rs"]
mod live;
use cnpg_watch::{app::Session, cli::Profile, model::*, navigation::View, pooler};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api,
    api::{AttachParams, DynamicObject, Patch, PatchParams},
    core::{ApiResource, GroupVersionKind},
};
use secrecy::ExposeSecret;
use tokio::{
    io::AsyncWriteExt,
    time::{Duration, Instant},
};

#[tokio::test]
#[ignore = "requires the explicitly provisioned Pooler fixture; injects isolated application pressure"]
async fn stock_peer_stats_saturation_failure_and_shared_admission() {
    let mut config = live::config(false, Profile::Standard);
    config.cluster = Some("payments".into());
    config.operator_watch = false;
    config.pooler_stats = true;
    let (client, _) = cnpg_watch::kubernetes::client(&config, &config.budgets)
        .await
        .unwrap();
    let resource =
        ApiResource::from_gvk(&GroupVersionKind::gvk("postgresql.cnpg.io", "v1", "Pooler"));
    let poolers: Api<DynamicObject> =
        Api::namespaced_with(client.clone(), "cnpg-watch-e2e", &resource);
    poolers.patch("payments-pool",&PatchParams::default(),&Patch::Merge(serde_json::json!({"spec":{"pgbouncer":{"parameters":{"default_pool_size":"1","reserve_pool_size":"0","max_client_conn":"100"}}}}))).await.unwrap();
    // Poll the production collector through configuration reload; successful zero application pools is valid.
    let mut session = Session::start(config.clone()).await.unwrap();
    session.set_view(View::Poolers);
    live::wait(&mut session, 45, |s| {
        s.store
            .pooler_samples
            .values()
            .any(|o| o.last_good.is_some())
            && s.store
                .samples
                .values()
                .filter(|o| o.last_good.is_some())
                .count()
                == 2
    })
    .await;
    let pooler_pod = session
        .store
        .pooler_samples
        .values()
        .next()
        .unwrap()
        .latest_ticket
        .pod
        .clone();
    assert_eq!(
        session.store.pooler_samples[&pooler_pod.key]
            .last_good
            .as_ref()
            .unwrap()
            .value
            .version,
        "1.25.1"
    );
    let primary = live::primary(client.clone(), "cnpg-watch-e2e", "payments").await;
    let pods: Api<Pod> = Api::namespaced(client.clone(), "cnpg-watch-e2e");
    let script = "IFS= read -r PGPASSWORD || exit 1; export PGPASSWORD PGSSLMODE=require PGAPPNAME=cnpg-watch-pooler-load PGOPTIONS='-c statement_timeout=20000'; i=0; while [ $i -lt 6 ]; do psql -XAtqw -h payments-pool -p 5432 -U watch_monitor -d postgres -c 'SELECT pg_sleep(15)' >/dev/null 2>&1 & i=$((i+1)); done; wait";
    let mut process = pods
        .exec(
            &primary.name,
            ["sh", "-c", script],
            &AttachParams::default().container("postgres").stdin(true),
        )
        .await
        .unwrap();
    let password = live::password(&config).await;
    let mut stdin = process.stdin().unwrap();
    stdin
        .write_all(password.expose_secret().as_bytes())
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.shutdown().await.unwrap();
    live::wait(&mut session, 15, |s| {
        s.store
            .pooler_samples
            .values()
            .filter_map(|o| o.last_good.as_ref())
            .any(|s| s.value.waiting() > 0)
    })
    .await;
    let sample = session.store.pooler_samples[&pooler_pod.key]
        .last_good
        .as_ref()
        .unwrap();
    assert!(sample.value.waiting() > 0);
    std::fs::create_dir_all("artifacts/phase56").unwrap();
    std::fs::write(
        "artifacts/phase56/pooler-saturated.json",
        serde_json::to_vec_pretty(&sample.value).unwrap(),
    )
    .unwrap();
    // Captured production framing/output, rather than a hand-written table fixture.
    let bytes = cnpg_watch::probes::exec_input_in(
        client.clone(),
        &pooler_pod.key,
        "pgbouncer",
        [
            "psql",
            "-X",
            "--csv",
            "-w",
            "-h",
            "/controller/run",
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
        .collect(),
        b"",
        &config.budgets,
        &tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    pooler::parse(&bytes).unwrap();
    std::fs::write("artifacts/phase56/pooler-saturated.csv", bytes).unwrap();
    live::admin(client.clone(),&primary,"SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='cnpg-watch-pooler-load' AND pid<>pg_backend_pid()").await;
    process.abort();
    let _ = tokio::time::timeout(Duration::from_secs(2), process.join()).await;
    // A bad pooler transport stays local while PostgreSQL samples and topology continue.
    let completed = session.completed;
    session.set_view(View::Instances);
    live::pump(&mut session, 1).await;
    assert_eq!(session.shutdown().await, 0);
    let mut failed = config.clone();
    failed.pooler_socket = "/nonexistent-cnpg-watch-socket".into();
    let mut session = Session::start(failed).await.unwrap();
    session.set_view(View::Poolers);
    live::wait(&mut session, 30, |s| {
        s.store
            .pooler_samples
            .values()
            .any(|o| matches!(o.attempt, Attempt::Failed(_)))
            && s.completed >= 2
    })
    .await;
    assert_eq!(
        session.store.watches[&Kind::Cluster].status,
        SyncState::Synced
    );
    assert_eq!(session.store.watches[&Kind::Pod].status, SyncState::Synced);
    let start = Instant::now();
    assert_eq!(session.shutdown().await, 0);
    println!(
        "actual PgBouncer peer role, SHOW output, waiting clients, independent direct SQL/topology and cleanup passed; earlier SQL completions {completed}; shutdown {:?}",
        start.elapsed()
    );
    // No PostgreSQL password is needed for the independent local peer transport.
    config.password_file = None;
    let mut session = Session::start(config).await.unwrap();
    session.set_view(View::Poolers);
    live::wait(&mut session, 30, |s| {
        s.store
            .pooler_samples
            .values()
            .any(|o| o.last_good.is_some())
    })
    .await;
    assert_eq!(session.completed, 0);
    assert_eq!(session.shutdown().await, 0);
}
