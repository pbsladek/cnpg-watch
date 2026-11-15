mod support;
use clap::Parser;
use cnpg_watch::{
    adapters,
    budget::Budgets,
    cli::{Cli, Command, Options, Profile, Setup},
    config::{Config, Paths},
    model::*,
    pooler::{self, Scheduler},
    reducer::{Store, WatchUpdate},
};
use serde_json::json;
use std::time::Duration;
fn seed() -> Store {
    let mut store = support::seed();
    let value = json!({"apiVersion":"postgresql.cnpg.io/v1","kind":"Pooler","metadata":{"namespace":"app-db","name":"payments-pool","uid":"pool-uid"},"spec":{"cluster":{"name":"payments"},"instances":2,"type":"rw","pgbouncer":{"poolMode":"transaction"}},"status":{"phase":"active","image":"pgbouncer:1.25.1"}});
    store.enable_sources([Kind::Pooler]);
    support::watch(
        &mut store,
        Kind::Pooler,
        WatchUpdate::Apply(adapters::pooler(serde_json::from_value(value).unwrap()).unwrap()),
    );
    let deployment = json!({"apiVersion":"apps/v1","kind":"Deployment","metadata":{"namespace":"app-db","name":"payments-pool","uid":"deployment-uid","ownerReferences":[{"apiVersion":"postgresql.cnpg.io/v1","kind":"Pooler","name":"payments-pool","uid":"pool-uid"}]},"spec":{},"status":{}});
    let rs = json!({"apiVersion":"apps/v1","kind":"ReplicaSet","metadata":{"namespace":"app-db","name":"pool-rs","uid":"rs-uid","ownerReferences":[{"apiVersion":"apps/v1","kind":"Deployment","name":"payments-pool","uid":"deployment-uid"}]}});
    for object in [
        adapters::pooler_deployment(serde_json::from_value(deployment).unwrap()).unwrap(),
        adapters::pooler_replicaset(serde_json::from_value(rs).unwrap()).unwrap(),
    ] {
        let kind = object.kind();
        store.enable_sources([kind]);
        support::watch(&mut store, kind, WatchUpdate::Apply(object));
    }
    for i in 0..2 {
        let mut p = store.pods().next().unwrap().clone();
        p.id.key.name = format!("pool-{i}");
        p.id.uid = Uid::new(format!("pool-pod-{i}")).unwrap();
        p.role = None;
        p.ready = Some(true);
        p.pooler = Some(Key::new("app-db", "payments-pool").unwrap());
        p.controller = Some(Controller {
            kind: OwnerKind::ReplicaSet,
            identity: Identity {
                key: Key::new("app-db", "pool-rs").unwrap(),
                uid: Uid::new("rs-uid").unwrap(),
            },
        });
        support::watch(&mut store, Kind::Pod, WatchUpdate::Apply(Object::Pod(p)));
    }
    store
}
fn populated() -> Vec<u8> {
    b"version\nPgBouncer 1.25.1\nuser,cl_waiting,database,sv_idle,cl_active,sv_active,pool_mode,maxwait_us,maxwait,unknown_column\nmonitor,3,\"shop,one\",1,2,1,transaction,500,2,ignored\ndatabase,total_query_count,total_xact_count,avg_query_count,total_wait_time\n\"shop,one\",9,4,1.5,100\n".to_vec()
}
#[test]
fn actual_console_and_column_name_variation_preserve_units_and_unknowns() {
    let actual = pooler::parse(include_bytes!("../fixtures/pgbouncer/1.25.1/empty.csv")).unwrap();
    assert_eq!(actual.version, "1.25.1");
    assert!(actual.pools.is_empty());
    let sample = pooler::parse(&populated()).unwrap();
    assert_eq!(sample.waiting(), 3);
    assert_eq!(sample.pools[0].database, "shop,one");
    assert_eq!(sample.pools[0].max_wait_us, Some(2_000_500));
    assert_eq!(sample.pools[0].servers_login, None);
    assert_eq!(
        sample.stats[0].average_queries_per_second.as_deref(),
        Some("1.5")
    );
    let broken = String::from_utf8(populated())
        .unwrap()
        .replace("1.5", "NaN");
    assert_eq!(
        pooler::parse(broken.as_bytes()),
        Err(ErrorClass::InvalidData)
    );
    let unsupported = String::from_utf8(populated())
        .unwrap()
        .replace("1.25.1", "2.0.0");
    assert_eq!(
        pooler::parse(unsupported.as_bytes()),
        Err(ErrorClass::Unsupported)
    );
    assert_eq!(
        pooler::parse(&vec![b'x'; 256 * 1024 + 1]),
        Err(ErrorClass::OutputLimit)
    );
    let many = format!(
        "version\nPgBouncer 1.25.1\ndatabase,user,cl_active,cl_waiting,sv_active,sv_idle\n{}database,total_query_count,total_xact_count\n",
        "db,user,1,0,1,0\n".repeat(129)
    );
    assert_eq!(pooler::parse(many.as_bytes()), Err(ErrorClass::OutputLimit));
}
#[tokio::test(start_paused = true)]
async fn pooler_admission_is_fair_independent_bounded_and_fenced() {
    let mut store = seed();
    let mut scheduler = Scheduler::default();
    scheduler.reconcile(&store, Tick(0), 128);
    let a = scheduler.admit(Tick(0)).unwrap();
    let b = scheduler.admit(Tick(0)).unwrap();
    assert_ne!(a.pod, b.pod);
    assert!(scheduler.admit(Tick(0)).is_none());
    assert!(store.begin_pooler(a.clone()));
    assert!(store.finish_pooler(a.clone(), Ok(pooler::parse(&populated()).unwrap()), Tick(0)));
    scheduler.finish(&a, Ok(()), Tick(0));
    scheduler.finish(&b, Err(ErrorClass::Forbidden), Tick(0));
    assert!(scheduler.admit(Tick(1)).is_none());
    scheduler.refresh(Tick(2000));
    let first = scheduler.admit(Tick(2000)).unwrap();
    let second = scheduler.admit(Tick(2000)).unwrap();
    assert_ne!(first.pod, second.pod);
    assert!([&first.pod, &second.pod].contains(&&b.pod));
    for ticket in [first, second] {
        let result = if ticket.pod == b.pod {
            Err(ErrorClass::Forbidden)
        } else {
            Ok(())
        };
        scheduler.finish(&ticket, result, Tick(2000));
    }
    let next = scheduler.admit(Tick(10000)).unwrap();
    assert_eq!(next.pod, a.pod);
    assert!(store.begin_pooler(next.clone()));
    assert!(store.finish_pooler(next.clone(), Err(ErrorClass::Overloaded), Tick(10000)));
    let obs = &store.pooler_samples[&a.pod.key];
    assert!(obs.last_good.is_some());
    assert!(matches!(
        obs.freshness(Tick(10001), Duration::from_secs(15)),
        Freshness::Stale(_)
    ));
    assert_eq!(store.samples.len(), 0); // Neither pooler failure writes PostgreSQL evidence nor holds its admission slot.
    let mut old = next.clone();
    old.scope = ScopeEpoch(99);
    assert!(!store.finish_pooler(old, Ok(pooler::parse(&populated()).unwrap()), Tick(10000)));
    let mut p = store.pod(&a.pod.key).unwrap().clone();
    p.id.uid = Uid::new("replacement").unwrap();
    support::watch(&mut store, Kind::Pod, WatchUpdate::Apply(Object::Pod(p)));
    store.prune_pooler_samples();
    assert!(!store.pooler_samples.contains_key(&a.pod.key));
    assert!(!pooler::valid(&store, &a));
    assert!(store.bytes() < Budgets::default().cache_bytes);
}
#[tokio::test]
async fn pooler_setup_is_idempotent_review_only_without_credentials_or_kubeconfig() {
    let cli = Cli::try_parse_from([
        "cnpg-watch",
        "--namespace",
        "app-db",
        "setup",
        "pooler-role",
        "--pooler",
        "payments-pool",
        "--dry-run",
        "--output",
        "json",
    ])
    .unwrap();
    let config = Config::resolve(
        Options {
            profile: Some(Profile::Standard),
            ..Default::default()
        },
        Paths::discover().unwrap(),
    )
    .unwrap();
    let Some(Command::Setup {
        command: Setup::PoolerRole(args),
    }) = cli.command
    else {
        panic!()
    };
    let first = serde_json::to_value(
        cnpg_watch::setup::pooler::plan(&config, &args)
            .await
            .unwrap(),
    )
    .unwrap();
    let second = serde_json::to_value(
        cnpg_watch::setup::pooler::plan(&config, &args)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(first["mutation_supported"], false);
    assert!(
        Cli::try_parse_from([
            "cnpg-watch",
            "setup",
            "pooler-role",
            "--pooler",
            "payments-pool",
            "--apply"
        ])
        .is_err()
    );
}
