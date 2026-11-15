//! Release-build bounded synthetic pipeline load/soak; explicitly selected.
mod support;
use cnpg_watch::{
    budget::Budgets,
    cli::Profile,
    health::Engine,
    logs::{self, Book, Framer, Message, Source, Update},
    model::*,
    queue,
    reducer::{WatchEvent, WatchUpdate},
    ui::ViewCache,
};
use ratatui::{Terminal, backend::TestBackend};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

fn rss_kib() -> Option<u64> {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}
fn percentile(values: &mut [u64], fraction: usize) -> u64 {
    values.sort_unstable();
    values[(values.len().saturating_sub(1) * fraction) / 100]
}

#[tokio::test]
#[ignore = "dedicated release load/soak job; default is 60 wall-clock seconds"]
async fn bounded_pipeline_load_and_lifecycle_soak() {
    let seconds: u64 = std::env::var("CNPG_WATCH_LOAD_SECONDS")
        .unwrap_or_else(|_| "60".into())
        .parse()
        .unwrap();
    assert!(seconds >= 1);
    let budgets = Budgets::default();
    let mut store = support::operational_fleet(100);
    support::add_pooler(&mut store);
    let mut poolers = cnpg_watch::pooler::Scheduler::default();
    let (pool_tx, mut pool_rx) = queue::channel::<(
        cnpg_watch::pooler::Ticket,
        cnpg_watch::pooler::Sample,
    )>(budgets.result_messages, budgets.result_bytes);
    let mut pooler_completions = 0u64;
    let objects: Vec<_> = store.pods().cloned().collect();
    let clusters: Vec<_> = store.clusters().cloned().collect();
    let operational: Vec<_> = [
        Kind::Backup,
        Kind::DatabaseRole,
        Kind::Pooler,
        Kind::PoolerDeployment,
        Kind::PoolerReplicaSet,
    ]
    .into_iter()
    .flat_map(|k| store.objects(k).cloned())
    .collect();
    let (top_tx, mut top_rx) = queue::channel(budgets.topology_messages, budgets.topology_bytes);
    let (log_tx, mut log_rx) =
        queue::channel::<Message>(budgets.log_messages, budgets.log_queue_bytes);
    let (control_tx, mut control_rx) =
        queue::channel(budgets.control_messages, budgets.control_bytes);
    let epoch = Arc::new(AtomicU64::new(0));
    let drops = Arc::new(AtomicU64::new(0));
    let stop = CancellationToken::new();
    let top_task = tokio::spawn({
        let stop = stop.clone();
        let epoch = epoch.clone();
        let objects = objects.clone();
        async move {
            let mut turn = 0u64;
            let mut timer = tokio::time::interval(Duration::from_millis(100));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {_=stop.cancelled()=>break,_=timer.tick()=>{}}
                for p in objects.iter().take(100) {
                    let mut p = p.clone();
                    p.restarts = (turn % 100) as u32;
                    let event = WatchEvent {
                        scope: ScopeEpoch(epoch.load(Ordering::Relaxed)),
                        watch: WatchEpoch(0),
                        kind: Kind::Pod,
                        update: WatchUpdate::Apply(Object::Pod(p)),
                    };
                    if top_tx.send(event, 1024, &stop).await.is_err() {
                        return;
                    }
                }
                turn += 1;
            }
        }
    });
    let log_task = tokio::spawn({
        let stop = stop.clone();
        let drops = drops.clone();
        let epoch = epoch.clone();
        async move {
            let mut framers: Vec<_> = (0..8).map(|_| Framer::new(16 * 1024)).collect();
            let mut sequence = 0;
            let mut timer = tokio::time::interval(Duration::from_millis(100));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {_=stop.cancelled()=>break,_=timer.tick()=>{}}
                // 8 * 64 lines / 100ms = 5,120 lines/s across eight sources.
                for (source, framer) in framers.iter_mut().enumerate() {
                    let raw = format!(
                        "{{\"ts\":\"2026-11-15T00:00:00Z\",\"level\":\"info\",\"cluster\":{{\"namespace\":\"benchmark\",\"name\":\"cluster-0000\"}},\"msg\":\"source {source} {}\"}}\n",
                        "safe message ".repeat(14)
                    );
                    let mut lines = Vec::with_capacity(64);
                    for _ in 0..64 {
                        framer.push(raw.as_bytes(), |line, truncated| {
                            sequence += 1;
                            lines.push(logs::parse(line, truncated, Profile::Incident, sequence));
                        });
                    }
                    let source = Source {
                        channel: cnpg_watch::logs::Channel::Postgres,
                        scope: ScopeEpoch(epoch.load(Ordering::Relaxed)),
                        key: Key::new(
                            "benchmark",
                            format!("cluster-{:04}-{}", source / 3, source % 3 + 1),
                        )
                        .unwrap(),
                        uid: Uid::new(format!("p-{}-{}", source / 3, source % 3 + 1)).unwrap(),
                        container: "postgres".into(),
                    };
                    let bytes = lines.iter().map(logs::Line::bytes).sum::<usize>() + 512;
                    if log_tx
                        .try_send(
                            Message {
                                source,
                                generation: 0,
                                update: Update::Lines(lines),
                            },
                            bytes,
                        )
                        .is_err()
                    {
                        drops.fetch_add(64, Ordering::Relaxed);
                    }
                }
            }
        }
    });
    let control_task = tokio::spawn({
        let stop = stop.clone();
        async move {
            let mut timer = tokio::time::interval(Duration::from_millis(47));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {_=stop.cancelled()=>break,_=timer.tick()=>{}}
                let _ = control_tx.try_send(Instant::now(), 64);
            }
        }
    });
    let mut book = Book::default();
    let mut health = Engine::default();
    let mut high_logs = 0;
    let mut cache = ViewCache::default();
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let mut latencies = Vec::new();
    let mut frames = Vec::new();
    let mut high_queue = 0;
    let mut high_cache = store.bytes();
    let mut max_rss = 0;
    let mut rss_samples = Vec::new();
    let mut accepted_lines = 0u64;
    let start = Instant::now();
    let mut timer = tokio::time::interval(Duration::from_millis(10));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut next_draw = Instant::now();
    let mut last_churn = Instant::now();
    let mut last_rss = Instant::now() - Duration::from_secs(10);
    let mut churns = 0;
    let mut pending_input = Vec::new();
    while start.elapsed() < Duration::from_secs(seconds) {
        timer.tick().await;
        while let Some(packet) = control_rx.try_recv() {
            pending_input.push(packet.value);
        }
        high_queue = high_queue
            .max(top_rx.queued_bytes() + log_rx.queued_bytes() + control_rx.queued_bytes());
        let work = Instant::now();
        for _ in 0..budgets.batch_events {
            let mut any = false;
            if let Some(packet) = top_rx.try_recv() {
                store.watch(packet.value.clone());
                any = true;
            }
            if let Some(packet) = log_rx.try_recv() {
                let message = packet.value;
                if message.source.scope == store.scope {
                    let now = Tick(start.elapsed().as_millis() as u64);
                    if !book.sources.contains_key(&message.source) {
                        book.activate(message.source.clone(), message.generation, now);
                    }
                    if let Update::Lines(lines) = &message.update {
                        accepted_lines += lines.len() as u64;
                    }
                    book.accept(message, now, &budgets);
                }
                any = true;
            }
            if !any || work.elapsed() >= Duration::from_millis(budgets.batch_ms) {
                break;
            }
        }
        if last_churn.elapsed() >= Duration::from_secs(10) {
            let _ = store.switch_scope();
            epoch.store(store.scope.0, Ordering::Relaxed);
            for kind in [Kind::Cluster, Kind::Pod] {
                support::watch(&mut store, kind, WatchUpdate::Init);
            }
            for c in &clusters {
                support::watch(
                    &mut store,
                    Kind::Cluster,
                    WatchUpdate::InitApply(Object::Cluster(c.clone())),
                );
            }
            for p in &objects {
                support::watch(
                    &mut store,
                    Kind::Pod,
                    WatchUpdate::InitApply(Object::Pod(p.clone())),
                );
            }
            for kind in [Kind::Cluster, Kind::Pod] {
                support::watch(&mut store, kind, WatchUpdate::InitDone);
            }
            for kind in [
                Kind::Backup,
                Kind::DatabaseRole,
                Kind::Pooler,
                Kind::PoolerDeployment,
                Kind::PoolerReplicaSet,
            ] {
                store.enable_sources([kind]);
                support::watch(&mut store, kind, WatchUpdate::Init);
                for object in operational.iter().filter(|o| o.kind() == kind) {
                    support::watch(&mut store, kind, WatchUpdate::InitApply(object.clone()));
                }
                support::watch(&mut store, kind, WatchUpdate::InitDone);
            }
            // Drop/recreate source retention during navigation churn.
            book.clear();
            while pool_rx.try_recv().is_some() {}
            poolers = Default::default();
            churns += 1;
            last_churn = Instant::now();
        }
        if Instant::now() >= next_draw {
            let now = Tick(start.elapsed().as_millis() as u64);
            let frame_start = Instant::now();
            health.update(&store, now, &budgets);
            let timeline = logs::timeline::project(
                &store,
                &book,
                &logs::timeline::Scope::Cluster,
                logs::timeline::Sources::default(),
                logs::timeline::Window::Follow,
            );
            assert!(timeline.entries.len() <= logs::timeline::MAX_ROWS);
            assert!(timeline.index_bytes <= logs::timeline::MAX_INDEX_BYTES);
            while let Some(packet) = pool_rx.try_recv() {
                let (ticket, sample) = packet.value;
                if store.finish_pooler(ticket.clone(), Ok(sample), now) {
                    pooler_completions += 1;
                }
                poolers.finish(&ticket, Ok(()), now);
            }
            poolers.reconcile(&store, now, budgets.pending_probes);
            if let Some(ticket) = poolers.admit(now) {
                assert!(store.begin_pooler(ticket.clone()));
                let sample = cnpg_watch::pooler::parse(include_bytes!(
                    "../fixtures/pgbouncer/1.25.1/saturated.csv"
                ))
                .unwrap();
                let bytes = sample.bytes() + 512;
                assert!(pool_tx.try_send((ticket, sample), bytes).is_ok());
            }
            let view = cache.get(&store, now, &budgets);
            let mut content = cnpg_watch::screens::instances(view);
            content.dashboard = Some(cnpg_watch::screens::dashboard(
                &store,
                health.reasons(),
                now,
                Profile::Incident,
                true,
                false,
            ));
            terminal
                .draw(|frame| {
                    cnpg_watch::screens::draw(frame,&content,"cnpg-watch | synthetic release workload\nbenchmark/cluster-0000 > Instances | SQL synthetic","Instances | q quit | i detail",None,false)
                })
                .unwrap();
            frames.push(frame_start.elapsed().as_micros() as u64);
            for input in pending_input.drain(..) {
                latencies.push(input.elapsed().as_micros() as u64);
            }
            let overdue = Instant::now()
                .saturating_duration_since(next_draw)
                .as_millis() as u64;
            next_draw +=
                Duration::from_millis((overdue / budgets.render_ms + 1) * budgets.render_ms);
        }
        high_cache = high_cache.max(store.bytes());
        assert!(store.bytes() <= budgets.cache_bytes);
        assert!(store.entries() <= budgets.cache_entries);
        assert!(top_rx.queued_bytes() <= budgets.topology_bytes);
        assert!(log_rx.queued_bytes() <= budgets.log_queue_bytes);
        high_logs = high_logs.max(book.bytes);
        assert!(book.bytes <= budgets.log_total_bytes);
        if last_rss.elapsed() >= Duration::from_secs(5) {
            if let Some(rss) = rss_kib() {
                max_rss = max_rss.max(rss);
                rss_samples.push(rss);
            }
            last_rss = Instant::now();
        }
    }
    stop.cancel();
    for task in [top_task, log_task, control_task] {
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
    }
    assert!(!latencies.is_empty());
    assert!(!frames.is_empty());
    let p95 = percentile(&mut latencies, 95);
    let p99 = percentile(&mut latencies, 99);
    let frame_p95 = percentile(&mut frames, 95);
    // At 10fps worst-case screen feedback includes a full 100ms frame period.
    assert!(
        p95 <= 100_000 && p99 <= 250_000,
        "input responsiveness regression: p95={p95}, p99={p99} us"
    );
    assert!(
        frame_p95 <= 16_000,
        "view/render regression: {frame_p95} us"
    );
    assert!(
        max_rss > 0,
        "RSS observation unavailable; run with host process-stat permissions"
    );
    assert!(max_rss <= 128 * 1024, "RSS target exceeded: {max_rss} KiB");
    assert!(
        pooler_completions >= seconds / 5,
        "pooler completion traffic stopped after source churn"
    );
    let report = serde_json::json!({"scenario":"100 clusters/300 instances/2 pooler pods, 1000 topology updates/s, 5120 log lines/s using production framing/redaction/retention, bounded timeline projection, operational health and queued/admitted/reduced PgBouncer samples, production responsive dashboard, TestBackend 120x40", "seconds":seconds,"pooler_completions":pooler_completions,"input_p95_us":p95,"input_p99_us":p99,"view_render_p95_us":frame_p95,"rss_max_kib":if max_rss>0{Some(max_rss)}else{None},"rss_samples_kib":rss_samples,"cache_high_bytes":high_cache,"log_high_bytes":high_logs,"queued_high_bytes":high_queue,"accepted_log_lines":accepted_lines,"dropped_log_lines":drops.load(Ordering::Relaxed),"scope_churns":churns,"render_count":frames.len(),"live_tasks_after_shutdown":0,"os":std::env::consts::OS,"arch":std::env::consts::ARCH});
    println!("{report}");
    if let Ok(path) = std::env::var("CNPG_WATCH_LOAD_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}
