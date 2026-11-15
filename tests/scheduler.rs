mod support;
use cnpg_watch::{budget::Budgets, model::*, scheduler::Scheduler, time::Clock};
use std::time::Duration;

#[tokio::test(start_paused = true)]
async fn monotonic_clock_and_backoff_do_not_wait_wall_seconds() {
    let clock = Clock::default();
    let mut store = support::seed();
    let mut scheduler = Scheduler::new(Budgets::default(), Duration::from_secs(2), 42);
    scheduler.reconcile(&store, clock.now());
    let ticket = scheduler.admit(&store, clock.now()).unwrap();
    assert!(store.begin_probe(ticket.clone()));
    assert!(scheduler.admit(&store, clock.now()).is_none());
    store.finish_probe(ticket.clone(), Err(ErrorClass::Timeout), clock.now());
    scheduler.finish(&ticket, Err(ErrorClass::Timeout), clock.now());
    let eligible = scheduler.next().unwrap();
    assert!((2000..=2200).contains(&eligible.0));
    scheduler.refresh(clock.now());
    assert!(scheduler.admit(&store, Tick(eligible.0 - 1)).is_none());
    tokio::time::advance(Duration::from_millis(eligible.0)).await;
    assert!(scheduler.admit(&store, clock.now()).is_some());
}

#[test]
fn authentication_suspends_and_manual_rechecks_are_rate_limited() {
    let mut store = support::seed();
    let mut scheduler = Scheduler::new(Budgets::default(), Duration::from_secs(2), 1);
    scheduler.reconcile(&store, Tick(0));
    let ticket = scheduler.admit(&store, Tick(0)).unwrap();
    store.begin_probe(ticket.clone());
    store.finish_probe(ticket.clone(), Err(ErrorClass::Authentication), Tick(0));
    scheduler.finish(&ticket, Err(ErrorClass::Authentication), Tick(0));
    assert!(scheduler.admit(&store, Tick(60000)).is_none());
    scheduler.refresh(Tick(0));
    assert!(scheduler.admit(&store, Tick(1)).is_none());
    scheduler.refresh(Tick(2000));
    assert!(scheduler.admit(&store, Tick(2000)).is_some());
}

#[test]
fn repeated_failures_cap_at_thirty_seconds_and_runtime_stalls_do_not_burst() {
    let mut store = support::seed();
    let mut scheduler = Scheduler::new(Budgets::default(), Duration::from_secs(2), 5);
    let mut now = Tick(0);
    scheduler.reconcile(&store, now);
    for _ in 0..20 {
        let ticket = scheduler.admit(&store, now).unwrap();
        store.begin_probe(ticket.clone());
        store.finish_probe(ticket.clone(), Err(ErrorClass::Timeout), now);
        scheduler.finish(&ticket, Err(ErrorClass::Timeout), now);
        let next = scheduler.next().unwrap();
        assert!(next.0 - now.0 <= 30_000);
        now = next;
    }
    let ticket = scheduler.admit(&store, Tick(now.0 + 1_000_000)).unwrap();
    store.begin_probe(ticket);
    assert!(scheduler.admit(&store, Tick(now.0 + 1_000_000)).is_none());
}

#[test]
fn manual_refresh_cannot_bypass_minimum_spacing_or_queue_duplicate_work() {
    let mut store = support::seed();
    let mut scheduler = Scheduler::new(Budgets::default(), Duration::from_secs(2), 42);
    scheduler.reconcile(&store, Tick(0));
    let ticket = scheduler.admit(&store, Tick(0)).unwrap();
    store.begin_probe(ticket.clone());
    store.finish_probe(ticket.clone(), Ok(support::sample()), Tick(20));
    scheduler.finish(&ticket, Ok(()), Tick(20));
    for tick in 30..2000 {
        scheduler.refresh(Tick(tick));
        assert!(scheduler.admit(&store, Tick(tick)).is_none());
    }
    assert_eq!(scheduler.len(), 1);
    assert!(scheduler.admit(&store, Tick(2000)).is_some());
}
