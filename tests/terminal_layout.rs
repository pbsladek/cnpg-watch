use cnpg_watch::{
    model::Key,
    navigation::{Trail, View, reconcile_rows},
    screens::{self, Content, Dashboard, ScreenRow, Secondary},
};
use proptest::prelude::*;
use ratatui::{Terminal, backend::TestBackend};
fn content(name: &str) -> Content {
    let (title, headers, values, dashboard) = match name {
        "fleet" => (
            "Fleet",
            vec!["cluster", "phase", "instances ready", "SQL", "health"],
            vec![
                "app-db/payments",
                "Ready",
                "3/3",
                "sampled 0s",
                "no current finding",
            ],
            true,
        ),
        "healthy_cluster" => (
            "Instances",
            vec!["instance", "role", "ready", "LSN", "lag", "sample"],
            vec![
                "payments-1",
                "primary",
                "yes",
                "0/100",
                "not-applicable",
                "fresh 0s",
            ],
            true,
        ),
        "degraded" => (
            "Instances",
            vec!["instance", "role", "ready", "sample"],
            vec!["payments-1", "primary", "yes", "stale 12s; timeout"],
            true,
        ),
        "denied" => (
            "Doctor",
            vec!["feature", "state", "details"],
            vec![
                "Backups",
                "forbidden",
                "Topology and PostgreSQL remain available",
            ],
            false,
        ),
        "overload" => (
            "Connections",
            vec!["instance", "evidence", "details"],
            vec!["payments-1", "pressure", "95%; deep diagnostics deferred"],
            false,
        ),
        "pooler_saturation" => (
            "Poolers",
            vec!["resource", "evidence", "state / details"],
            vec![
                "payments-pool",
                "PgBouncer pool",
                "clients active 1 waiting 5; servers active 1 idle 0; fresh",
            ],
            false,
        ),
        "backup_failure" => (
            "Backups",
            vec!["resource", "evidence", "state / details"],
            vec![
                "backup-1",
                "Backup",
                "failed; unavailable plugin; restore validity unknown",
            ],
            false,
        ),
        "endpoint_mismatch" => (
            "Services",
            vec!["resource", "evidence", "details"],
            vec![
                "payments-rw",
                "persistent mismatch",
                "current primary UID differs from ready endpoint",
            ],
            false,
        ),
        "storage_pending" => (
            "Storage",
            vec!["resource", "role", "state / details"],
            vec!["payments-wal", "WAL", "Pending; capacity unknown"],
            false,
        ),
        "managed_pending" => (
            "Roles",
            vec!["resource", "evidence", "state / details"],
            vec![
                "monitor-role",
                "DatabaseRole",
                "pending generation: desired 2 observed 1",
            ],
            false,
        ),
        "plugin_warning" => (
            "Plugins",
            vec!["resource", "evidence", "state / details"],
            vec![
                "archive-plugin",
                "configured",
                "capabilities/version unknown; opaque payload omitted",
            ],
            false,
        ),
        "certificate_expiring" => (
            "Certs",
            vec!["certificate", "source", "state / details"],
            vec![
                "app-db/server-cert",
                "explicit named GET",
                "expires in 6 days; public metadata only",
            ],
            false,
        ),
        "long_wide" => (
            "Logs",
            vec!["source", "timestamp / level", "correlation / message"],
            vec![
                "app-db/payments-very-long-source-name",
                "2026-11-15T00:00:00Z Info",
                "漢字 🌊 e\u{301} wide text and a deliberately long message that must stay within its cell",
            ],
            false,
        ),
        "syncing" | "empty_after_sync" | "filtered_empty" => {
            ("Fleet", vec!["cluster", "phase", "SQL"], vec![], false)
        }
        _ => panic!("unknown fixture"),
    };
    Content {
        title: title.into(),
        headers,
        rows: if values.is_empty() {
            vec![]
        } else {
            vec![ScreenRow {
                id: "stable-row".into(),
                key: Key::new("app-db", "payments").unwrap(),
                cells: values.into_iter().map(str::to_owned).collect(),
                target: Some(View::Events),
                severity: None,
            }]
        },
        empty: match name {
            "syncing" => "Syncing; retain last complete set",
            "empty_after_sync" => "No clusters after successful synchronization",
            "filtered_empty" => "No matching rows (filter active)",
            _ => "No rows",
        }
        .into(),
        extra: "evidence remains bounded; i inspects selected row".into(),
        dashboard: dashboard.then(|| Dashboard {
            health: if name == "degraded" {
                "Warning: SQL attempt timed out; last-good evidence stale"
            } else {
                "No current findings; direct SQL current"
            }
            .into(),
            poolers: "payments-pool: waiting 0; stats current".into(),
            events: "app-db/payments: Normal: reconciliation completed".into(),
        }),
    }
}
fn render(content: &Content, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame|screens::draw(frame,content,&format!("cnpg-watch | ctx fixture | ns app-db | Incident | reduced 0\napp-db/payments > {} | scope 1 | SQL Available",content.title),"Instances | Enter related | i detail | Esc back | q quit",Some("stable-row"),false)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn reviewed_layouts() {
    for name in ["fleet", "healthy_cluster"] {
        for (w, h) in [(120, 40), (100, 30), (42, 10)] {
            insta::assert_snapshot!(format!("{name}_{w}x{h}"), render(&content(name), w, h));
        }
    }
    for name in [
        "degraded",
        "denied",
        "overload",
        "pooler_saturation",
        "backup_failure",
        "endpoint_mismatch",
        "storage_pending",
        "managed_pending",
        "plugin_warning",
        "certificate_expiring",
        "long_wide",
        "syncing",
        "empty_after_sync",
        "filtered_empty",
    ] {
        insta::assert_snapshot!(name, render(&content(name), 100, 30));
    }
    for (w, h) in [(120, 40), (100, 30)] {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame|screens::render::detail(frame,"cnpg-watch | ctx fixture | ns app-db\napp-db/payments > Backups > backup-1","Backup detail","Snapshot at 1000 ms; scope 1\n\nphase: failed\nmethod: plugin\nRestore validity unknown","Esc back | j/k scroll | q quit",0)).unwrap();
        let buffer = terminal.backend().buffer();
        let text = (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!(format!("detail_{w}x{h}"), text);
    }
}
proptest! {
    #[test]
    fn arbitrary_layouts_and_removed_selection_never_escape_the_buffer(width in 0u16..180,height in 0u16..60,count in 0usize..50,selected in 0usize..70,wide in prop::collection::vec(any::<char>(),0..120)){
        let mut content=content("long_wide");let text: String=wide.into_iter().filter(|c|!c.is_control()).collect();content.rows=(0..count).map(|i|ScreenRow{id:i.to_string(),key:Key::new("app-db",format!("row-{i}")).unwrap(),cells:vec![text.clone(),"state".into(),text.clone()],target:None,severity:None}).collect();
        let mut chosen=Some(selected.to_string());reconcile_rows(&mut chosen,content.rows.iter().map(|r|r.id.as_str()),false);prop_assert!(chosen.is_none()||content.rows.iter().any(|r|Some(&r.id)==chosen.as_ref()));
        let mut terminal=Terminal::new(TestBackend::new(width,height)).unwrap();terminal.draw(|frame|screens::render::draw(frame,&content,&text,"compact | i detail | q quit",chosen.as_deref(),false,Secondary::Poolers)).unwrap();prop_assert_eq!(terminal.backend().buffer().content().len(),usize::from(width)*usize::from(height));
        terminal.draw(|frame|screens::render::detail(frame,&text,"Detail",&text,"Esc back",u16::MAX)).unwrap();
    }
}
#[test]
fn bounded_back_forward_and_stable_row_selection() {
    let mut trail = Trail::default();
    for _ in 0..40 {
        trail.open(View::Fleet, View::Instances);
    }
    let mut count = 0;
    while trail.back(View::Instances).is_some() {
        count += 1;
    }
    assert_eq!(count, 32);
    assert_eq!(trail.forward(View::Fleet), Some(View::Instances));
    trail.open(View::Instances, View::Logs);
    assert_eq!(trail.forward(View::Logs), None);
    let mut selected = Some("b".into());
    reconcile_rows(&mut selected, ["a", "b", "c"].into_iter(), false);
    assert_eq!(selected.as_deref(), Some("b"));
    reconcile_rows(&mut selected, ["a", "c"].into_iter(), false);
    assert_eq!(selected.as_deref(), Some("a"));
    reconcile_rows(&mut selected, ["a", "c"].into_iter(), true);
    assert_eq!(selected.as_deref(), Some("c"));
}
#[test]
fn selected_severity_has_text_and_independent_style() {
    use cnpg_watch::health::Severity;
    use ratatui::style::Color;
    let mut value = content("degraded");
    value.rows[0].cells[0] = "Warning".into();
    value.rows[0].severity = Some(Severity::Warning);
    value.dashboard = None;
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| {
            screens::draw(
                frame,
                &value,
                "context",
                "status",
                Some("stable-row"),
                false,
            )
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let cell = buffer.content().iter().find(|c| c.symbol() == "W").unwrap();
    assert_eq!(cell.fg, Color::Yellow);
    assert_eq!(cell.bg, Color::DarkGray);
}
