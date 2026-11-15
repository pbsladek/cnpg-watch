#[path = "../tests/support/mod.rs"]
mod support;
use cnpg_watch::{
    budget::Budgets,
    model::*,
    reducer::{WatchEvent, WatchUpdate},
    ui,
};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ratatui::{Terminal, backend::TestBackend};
use std::hint::black_box;
fn pipeline(c: &mut Criterion) {
    let budgets = Budgets::default();
    let mut group = c.benchmark_group("pipeline");
    for count in [10, 100, 1000] {
        let mut store = support::fleet(count);
        let object = store.objects(Kind::Pod).next().unwrap().clone();
        group.bench_with_input(
            BenchmarkId::new("unchanged_reduce", count),
            &count,
            |b, _| {
                b.iter(|| {
                    store.watch(WatchEvent {
                        scope: store.scope,
                        watch: WatchEpoch(0),
                        kind: Kind::Pod,
                        update: WatchUpdate::Apply(black_box(object.clone())),
                    })
                })
            },
        );
        group.bench_with_input(BenchmarkId::new("view", count), &count, |b, _| {
            b.iter(|| ui::derive(black_box(&store), Tick(0), &budgets))
        });
        group.bench_with_input(BenchmarkId::new("capabilities", count), &count, |b, _| {
            b.iter(|| {
                cnpg_watch::capabilities::observed(
                    black_box(&store),
                    cnpg_watch::cli::Profile::Standard,
                    Availability::Available,
                )
            })
        });
        let mut health = cnpg_watch::health::Engine::default();
        group.bench_with_input(BenchmarkId::new("health_full", count), &count, |b, _| {
            b.iter(|| {
                black_box(health.recompute(&store, Tick(0), &budgets));
            })
        });
        health.update(&store, Tick(0), &budgets);
        group.bench_with_input(BenchmarkId::new("health_cached", count), &count, |b, _| {
            b.iter(|| {
                black_box(health.update(&store, Tick(0), &budgets));
            })
        });
        let view = ui::derive(&store, Tick(0), &budgets);
        for (width, height) in [(120, 40), (100, 30)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            group.bench_function(
                BenchmarkId::new(format!("render_{width}x{height}"), count),
                |b| {
                    b.iter(|| {
                        terminal
                            .draw(|f| {
                                ui::draw(f, black_box(&view), "benchmark", "synthetic", None, false)
                            })
                            .unwrap();
                    })
                },
            );
        }
    }
    group.finish();
    c.bench_function("capability_parse", |b| {
        b.iter(|| {
            cnpg_watch::probes::registry::parse(
                cnpg_watch::probes::registry::Bundle::Capabilities,
                black_box(include_bytes!("../fixtures/postgres/18/capabilities.json")),
                "watch_monitor",
                cnpg_watch::cli::Profile::Incident,
            )
            .unwrap()
        })
    });
    c.bench_function("log_frame_parse_redact", |b| {
        b.iter(|| {
            let mut framer = cnpg_watch::logs::Framer::new(16384);
            framer.push(
                black_box(b"2026-11-15T00:00:00Z password='private' connection closed\n"),
                |line, truncated| {
                    black_box(cnpg_watch::logs::parse(
                        line,
                        truncated,
                        cnpg_watch::cli::Profile::Secure,
                        1,
                    ));
                },
            );
        })
    });
    c.bench_function("class_admission", |b| {
        b.iter_batched(
            || {
                let store = support::seed();
                let mut scheduler = cnpg_watch::scheduler::Scheduler::with_registry(
                    Budgets::default(),
                    std::time::Duration::from_secs(2),
                    7,
                    cnpg_watch::cli::Profile::Incident,
                    false,
                );
                scheduler.reconcile(&store, Tick(0));
                (store, scheduler)
            },
            |(store, mut scheduler)| black_box(scheduler.admit(&store, Tick(0))),
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("pooler_csv", |b| {
        b.iter(|| {
            cnpg_watch::pooler::parse(black_box(include_bytes!(
                "../fixtures/pgbouncer/1.25.1/saturated.csv"
            )))
            .unwrap()
        })
    });
    let operational = support::operational_fleet(100);
    let mut operational_health = cnpg_watch::health::Engine::default();
    c.bench_function("operational_health_100", |b| {
        b.iter(|| {
            black_box(operational_health.recompute(&operational, Tick(0), &budgets));
        })
    });
    let mut dashboard =
        cnpg_watch::screens::instances(&ui::derive(&operational, Tick(0), &budgets));
    operational_health.update(&operational, Tick(0), &budgets);
    dashboard.dashboard = Some(cnpg_watch::screens::dashboard(
        &operational,
        operational_health.reasons(),
        Tick(0),
        cnpg_watch::cli::Profile::Incident,
        true,
        false,
    ));
    for (width, height) in [(120, 40), (100, 30)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        c.bench_function(&format!("dashboard_render_{width}x{height}"), |b| {
            b.iter(|| {
                terminal
                    .draw(|frame| {
                        cnpg_watch::screens::draw(
                            frame,
                            black_box(&dashboard),
                            "context / namespace / profile",
                            "q quit | i detail",
                            None,
                            false,
                        )
                    })
                    .unwrap();
            })
        });
    }
    let mut book = cnpg_watch::logs::Book::default();
    for pod in operational.pods().take(8) {
        let source = cnpg_watch::logs::Source {
            channel: cnpg_watch::logs::Channel::Postgres,
            scope: operational.scope,
            key: pod.id.key.clone(),
            uid: pod.id.uid.clone(),
            container: "postgres".into(),
        };
        book.activate(source.clone(), 0, Tick(0));
        let lines = (0..512)
            .map(|i| {
                cnpg_watch::logs::parse(
                    br#"{"ts":"2026-11-15T00:00:00Z","msg":"safe"}"#,
                    false,
                    cnpg_watch::cli::Profile::Secure,
                    i,
                )
            })
            .collect();
        book.accept(
            cnpg_watch::logs::Message {
                source,
                generation: 0,
                update: cnpg_watch::logs::Update::Lines(lines),
            },
            Tick(0),
            &budgets,
        );
    }
    c.bench_function("timeline_8x512", |b| {
        b.iter(|| {
            cnpg_watch::logs::timeline::project(
                black_box(&operational),
                black_box(&book),
                &cnpg_watch::logs::timeline::Scope::Cluster,
                Default::default(),
                Default::default(),
            )
        })
    });
    c.bench_function("cluster_adapter", |b| {
        b.iter(|| {
            cnpg_watch::adapters::cluster(black_box(
                serde_json::from_str(include_str!("../fixtures/cnpg/1.30.0/cluster.json")).unwrap(),
            ))
            .unwrap()
        })
    });
}
criterion_group!(benches, pipeline);
criterion_main!(benches);
