mod support;
use cnpg_watch::{budget::Budgets, model::*, reducer::WatchUpdate, ui};
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn idle_primary_replica_age_is_not_a_lag_alarm() {
    let mut store = support::seed();
    let ticket = support::ticket(&store, 1);
    store.begin_probe(ticket.clone());
    store.finish_probe(ticket, Ok(support::sample()), Tick(0));
    let mut replica = support::pod();
    if let Object::Pod(p) = &mut replica {
        p.id.key.name = "payments-2".into();
        p.id.uid = Uid::new("pod-2").unwrap();
        p.role = Some(Role::Replica);
    }
    let id = replica.id().clone();
    support::watch(&mut store, Kind::Pod, WatchUpdate::Apply(replica));
    let mut ticket = support::ticket(&store, 2);
    ticket.pod = id.clone();
    store.begin_probe(ticket.clone());
    let mut sample = support::sample();
    sample.role = Role::Replica;
    sample.current = None;
    sample.replay = Some(Lsn(0x100));
    sample.last_replayed_transaction_age_ms = Some(86_400_000);
    store.finish_probe(ticket, Ok(sample), Tick(0));
    assert_eq!(
        ui::byte_lag(
            &store,
            store.pod(&id.key).unwrap(),
            Tick(0),
            &Budgets::default()
        ),
        Some(0)
    );
    assert_eq!(
        ui::byte_lag(
            &store,
            store.pod(&id.key).unwrap(),
            Tick(10001),
            &Budgets::default()
        ),
        None
    );
}

#[test]
fn actual_buffers_show_rows_and_stale_state_at_supported_sizes() {
    let mut store = support::seed();
    let ticket = support::ticket(&store, 1);
    store.begin_probe(ticket.clone());
    store.finish_probe(ticket, Ok(support::sample()), Tick(0));
    let view = ui::derive(&store, Tick(12000), &Budgets::default());
    for (width, height) in [(120, 40), (100, 30), (40, 10), (1, 1)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| ui::draw(frame, &view, "context test", "SQL available", None, false))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        if width >= 100 {
            assert!(text.contains("payments-1"));
            assert!(text.contains("stale 12s"));
        }
    }
}

#[test]
fn capability_panel_reports_local_gaps_without_discarding_good_sql() {
    use cnpg_watch::{
        cli::{Options, Profile},
        config::{Config, Paths},
        doctor,
    };
    let mut store = support::seed();
    let ticket = support::ticket(&store, 1);
    store.begin_probe(ticket.clone());
    store.finish_probe(ticket, Ok(support::sample()), Tick(0));
    let ticket = support::ticket(&store, 2);
    store.begin_probe(ticket.clone());
    store.finish_probe(ticket, Err(ErrorClass::Authentication), Tick(1));
    let config = Config::resolve(
        Options {
            profile: Some(Profile::Secure),
            no_operator_watch: true,
            ..Default::default()
        },
        Paths::discover().unwrap(),
    )
    .unwrap();
    let details = doctor::live_text(&store, &config, Availability::Available);
    assert!(details.contains("FastSql: unavailable (Unavailable(Authentication))"));
    assert!(details.contains("Operator: disabled_by_config"));
    assert!(store.samples.values().all(|s| s.last_good.is_some()));
    for (width, height) in [(120, 40), (100, 30), (40, 10), (1, 1)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| ui::draw_details(f, "Doctor / accepted evidence", &details))
            .unwrap();
        if width >= 100 {
            let text = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(text.contains("Profile Secure"));
            assert!(text.contains("FastSql: unavailable"));
            assert!(text.contains("Operator: disabled_by_config"));
        }
    }
}
