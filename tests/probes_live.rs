#[path = "support/live.rs"]
mod live;
use cnpg_watch::{
    app::Session,
    cli::Profile,
    model::*,
    navigation::View,
    probes::{
        self, PodExec,
        registry::{self, Bundle, Class},
    },
};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api,
    api::{AttachParams, DynamicObject, Patch, PatchParams},
    core::{ApiResource, GroupVersionKind},
};
use tokio::{
    io::AsyncWriteExt,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
#[tokio::test]
#[ignore = "runs all PostgreSQL variants as the monitor role on the isolated primary/replica"]
async fn every_bundle_runs_as_monitor_and_records_server_and_transport_cost() {
    let config = live::config(true, Profile::Incident);
    let (client, _) = cnpg_watch::kubernetes::client(&config, &config.budgets)
        .await
        .unwrap();
    let password = live::password(&config).await;
    let executor = PodExec::new(
        client.clone(),
        password,
        config.db_role.clone(),
        config.budgets.clone(),
    );
    let cancel = CancellationToken::new();
    let mut costs = Vec::new();
    for (namespace, cluster) in [
        ("cnpg-watch-e2e", "payments"),
        ("cnpg-watch-e2e-other", "inventory"),
    ] {
        let primary = live::primary(client.clone(), namespace, cluster).await;
        let mut targets = vec![primary.clone()];
        if cluster == "payments" {
            let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
            let list = pods
                .list(
                    &kube::api::ListParams::default()
                        .labels("cnpg.io/instanceRole=replica,cnpg.io/podRole=instance"),
                )
                .await
                .unwrap();
            targets.extend(
                list.items
                    .into_iter()
                    .map(|p| Key::new(namespace, p.metadata.name.unwrap()).unwrap()),
            );
        }
        for pod in targets {
            for bundle in [
                Bundle::Capabilities,
                Bundle::Fast,
                Bundle::FastBasic,
                Bundle::Medium,
                Bundle::Deep,
                Bundle::DeepText,
            ] {
                let start = Instant::now();
                let bytes = executor
                    .execute_bundle(&pod, bundle, &cancel)
                    .await
                    .expect("monitor bundle; remote contents suppressed");
                let elapsed = start.elapsed();
                if matches!(bundle, Bundle::Fast | Bundle::FastBasic) {
                    probes::parse(&bytes, "watch_monitor").unwrap();
                } else {
                    registry::parse(bundle, &bytes, "watch_monitor", Profile::Incident).unwrap();
                }
                let explain: &'static str = Box::leak(
                    format!(
                        "EXPLAIN (ANALYZE, FORMAT JSON, TIMING FALSE) {}",
                        bundle.sql()
                    )
                    .into_boxed_str(),
                );
                // Cost measurement is test-only and executes the same known read-only statement.
                let plan = executor.execute_sql(&pod, explain, &cancel).await.unwrap();
                let plan: serde_json::Value = serde_json::from_slice(&plan).unwrap();
                costs.push(serde_json::json!({"pod":pod.to_string(),"bundle":bundle,"full_round_trip_ms":elapsed.as_secs_f64()*1000.,"db_execution_ms":plan[0]["Execution Time"],"db_planning_ms":plan[0]["Planning Time"],"bytes":bytes.len()}));
                if std::env::var_os("E2E_CAPTURE_FIXTURES").is_some()
                    && namespace == "cnpg-watch-e2e"
                    && pod == primary
                    && !matches!(bundle, Bundle::Fast | Bundle::FastBasic | Bundle::DeepText)
                {
                    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    value["system_id"] = "42".into();
                    value["timeline"] = 1.into();
                    value["server_time"] = "2026-11-15T00:00:00Z".into();
                    if bundle == Bundle::Deep {
                        for tx in value["long_transactions"].as_array_mut().unwrap() {
                            tx["query_text"] = serde_json::Value::Null;
                        }
                    }
                    std::fs::create_dir_all("artifacts/phase34").unwrap();
                    std::fs::write(
                        format!("artifacts/phase34/{bundle:?}.json"),
                        serde_json::to_vec_pretty(&value).unwrap(),
                    )
                    .unwrap();
                }
            }
        }
        let startup = Instant::now();
        executor
            .execute_sql(&primary, "SELECT 1", &cancel)
            .await
            .unwrap();
        costs.push(serde_json::json!({"pod":primary.to_string(),"psql_startup_connect_select1_round_trip_ms":startup.elapsed().as_secs_f64()*1000.}));
        let start = Instant::now();
        probes::exec_input(
            client.clone(),
            &primary,
            vec!["true".into()],
            b"",
            &config.budgets,
            &cancel,
        )
        .await
        .unwrap();
        costs.push(serde_json::json!({"pod":primary.to_string(),"exec_only_baseline_ms":start.elapsed().as_secs_f64()*1000.}));
    }
    std::fs::create_dir_all("artifacts/phase34").unwrap();
    std::fs::write(
        "artifacts/phase34/probe-costs.json",
        serde_json::to_vec_pretty(&costs).unwrap(),
    )
    .unwrap();
    println!(
        "all six production variants passed on both primaries and replica; isolated DB EXPLAIN and exec-only baselines recorded"
    );
}
#[tokio::test]
#[ignore = "injects saturation and promotion only in the explicitly provisioned fixture"]
async fn overload_sheds_deep_preserves_sql_and_keeps_topology_and_logs_alive() {
    let mut config = live::config(false, Profile::Incident);
    config.cluster = Some("payments".into());
    let mut session = Session::start(config.clone()).await.unwrap();
    session.set_view(View::Waits);
    live::wait(&mut session, 45, |s| {
        s.store.samples.len() >= 2
            && s.store
                .diagnostics
                .iter()
                .any(|((_, c), o)| *c == Class::Deep && o.last_good.is_some())
    })
    .await;
    println!("initial fast/medium/deep samples accepted");
    let primary = session
        .store
        .selected_pods()
        .find(|p| p.role == Some(Role::Primary))
        .unwrap()
        .clone();
    let client = session.client();
    session.set_view(View::Logs);
    session.set_log_sources(
        vec![cnpg_watch::logs::Source {
            channel: cnpg_watch::logs::Channel::Postgres,
            scope: session.store.scope,
            key: primary.id.key.clone(),
            uid: primary.id.uid.clone(),
            container: "postgres".into(),
        }],
        300,
    );
    live::wait(&mut session, 10, |s| {
        s.logs.sources.values().any(|s| !s.lines.is_empty())
    })
    .await;
    let pods: Api<Pod> = Api::namespaced(client.clone(), &primary.id.key.namespace);
    let script = "IFS= read -r PGPASSWORD || exit 1; export PGPASSWORD PGSSLMODE=require PGAPPNAME=cnpg-watch-load PGOPTIONS='-c statement_timeout=35000'; i=0; while [ $i -lt 28 ]; do psql -XAtqw -h 127.0.0.1 -U watch_monitor -d postgres -c 'SELECT pg_sleep(30)' >/dev/null 2>&1 & i=$((i+1)); done; wait";
    let mut pressure = pods
        .exec(
            &primary.id.key.name,
            ["sh", "-c", script],
            &AttachParams::default().container("postgres").stdin(true),
        )
        .await
        .unwrap();
    let mut stdin = pressure.stdin().unwrap();
    use secrecy::ExposeSecret;
    let password = live::password(&config).await;
    stdin
        .write_all(password.expose_secret().as_bytes())
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.shutdown().await.unwrap();
    println!("saturation exec established");
    let deadline = Instant::now() + Duration::from_secs(8);
    let active = loop {
        let active = live::admin(
            client.clone(),
            &primary.id.key,
            "SELECT count(*) FROM pg_stat_activity WHERE application_name='cnpg-watch-load'",
        )
        .await
        .parse::<u32>()
        .unwrap();
        if active >= 20 || Instant::now() >= deadline {
            break active;
        }
        live::pump(&mut session, 1).await;
    };
    println!("held load backends {active}");
    assert!(
        active >= 20,
        "fixture failed to create actual connection pressure"
    );
    let retained = session
        .store
        .samples
        .values()
        .filter(|o| o.last_good.is_some())
        .count();
    let sources_before = session.store.watches[&Kind::Pod].received;
    live::wait(&mut session, 15, |s| {
        s.scheduler.shedding(s.clock.now())
            || s.store
                .samples
                .values()
                .any(|o| matches!(o.attempt, Attempt::Failed(ErrorClass::Overloaded)))
    })
    .await;
    println!("overload shedding observed");
    assert!(
        session
            .store
            .samples
            .values()
            .filter(|o| o.last_good.is_some())
            .count()
            >= retained
    );
    assert_eq!(
        session.store.watches[&Kind::Cluster].status,
        SyncState::Synced
    );
    assert_eq!(session.active_log_streams(), 1);
    live::admin(client.clone(),&primary.id.key,"SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='cnpg-watch-load' AND pid<>pg_backend_pid()").await;
    pressure.abort();
    let _ = tokio::time::timeout(Duration::from_secs(2), pressure.join()).await;
    live::pump(&mut session, 4).await;
    assert!(session.store.watches[&Kind::Pod].received >= sources_before);
    println!("load cleanup finished; requesting promotion");
    // Controlled CNPG switchover through its status target, never a runtime action path.
    session.set_view(View::Instances);
    let replica = session
        .store
        .selected_pods()
        .find(|p| p.role == Some(Role::Replica))
        .unwrap()
        .id
        .key
        .name
        .clone();
    let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(
        "postgresql.cnpg.io",
        "v1",
        "Cluster",
    ));
    let clusters: Api<DynamicObject> =
        Api::namespaced_with(client.clone(), "cnpg-watch-e2e", &resource);
    clusters
        .patch_status(
            "payments",
            &PatchParams::default(),
            &Patch::Merge(serde_json::json!({"status":{"targetPrimary":replica}})),
        )
        .await
        .unwrap();
    live::wait(&mut session, 120, |s| {
        s.store
            .cluster(&Key::new("cnpg-watch-e2e", "payments").unwrap())
            .is_some_and(|c| c.primary.as_deref() == Some(replica.as_str()))
    })
    .await;
    session.refresh().await;
    live::wait(&mut session, 90, |s| {
        s.store
            .samples
            .values()
            .filter_map(|o| o.last_good.as_ref())
            .any(|sample| {
                sample.ticket.pod.key.name == replica
                    && sample.value.role == Role::Primary
                    && Some(sample.ticket.topology)
                        == s.store.topology.get(&sample.ticket.cluster.key).copied()
            })
    })
    .await;
    let stats = serde_json::to_value(&session.scheduler.stats).unwrap();
    let cleanup = Instant::now();
    assert_eq!(session.shutdown().await, 0);
    std::fs::create_dir_all("artifacts/phase34").unwrap();
    std::fs::write("artifacts/phase34/scheduler-live.json", serde_json::to_vec_pretty(&serde_json::json!({
        "scenario":"selected incident cluster, actual saturation, independent logs/topology and promotion recovery",
        "held_load_backends":active,"classes":stats,"shutdown_ms":cleanup.elapsed().as_secs_f64()*1000.,"cleanup_errors":0
    })).unwrap()).unwrap();
    println!(
        "saturation, retained samples, independent topology/logs, bounded cleanup, and actual promotion passed"
    );
}
