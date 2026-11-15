#[path = "support/live.rs"]
mod live;
#[path = "support/pty.rs"]
mod pty;
use cnpg_watch::{cli::Profile, navigation::View};
use std::time::{Duration, Instant};
fn command() -> portable_pty::CommandBuilder {
    let binary = std::env::var("CNPG_WATCH_BINARY")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_cnpg-watch").into());
    let mut c = portable_pty::CommandBuilder::new(binary);
    c.args([
        "--config",
        "tests/e2e/watch.toml",
        "--kube-context",
        &std::env::var("E2E_CONTEXT").unwrap(),
        "--password-file",
        &std::env::var("E2E_PASSWORD_FILE").unwrap(),
        "--log-file",
        &std::env::var("E2E_LOG_FILE").unwrap(),
        "--operator-namespace",
        "cnpg-system",
        "--profile",
        "incident",
        "--pooler-stats",
        "--cluster-resources",
    ]);
    if let Ok(url) = std::env::var("E2E_METRICS_URL") {
        c.args([
            "--operator-metrics-url",
            &url,
            "--certificate-secret",
            "cnpg-watch-e2e/payments-server",
        ]);
    }
    c.env("TERM", "xterm-256color");
    c
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires disposable operational fixture; real portable-pty/vt100 product and flood gate"]
async fn real_terminal_navigation_resize_details_optional_views_and_flood() {
    let config = live::config(false, Profile::Incident);
    let (client, _) = cnpg_watch::kubernetes::client(&config, &config.budgets)
        .await
        .unwrap();
    let primary = live::primary(client.clone(), "cnpg-watch-e2e", "payments").await;
    let mut child = pty::Pty::spawn(command(), 120, 40);
    child.wait("payments-1", 60);
    child.wait("fresh", 60);
    child.wait("Health / evidence", 15);
    child.wait("Recent events", 15);
    for _ in 0..8 {
        child.send(b"d");
        child.wait("Doctor / accepted evidence", 15);
        child.resize(100, 30);
        child.send(b"\x1b");
        child.wait("> Instances |", 15);
        child.send(b"p");
        child.wait("Change profile", 15);
        child.resize(120, 40);
        child.send(b"\x1b");
        child.wait("> Instances |", 15);
    }
    child.send(b"i");
    child.wait("Snapshot at", 10);
    child.send(b"\x1b");
    child.wait("> Instances |", 10);
    child.command(":fleet", "Fleet");
    child.send(b"\x1b");
    child.wait("> Instances |", 10);
    child.send(b"\x1b[C");
    child.wait("> Fleet |", 10);
    child.command(":instances", "Instances");
    child.send(b"/no-such-instance\r");
    child.wait("No matching", 10);
    child.send(b":clear\r");
    child.wait("payments-1", 10);
    child.resize(100, 30);
    child.send(b":pane poolers\r");
    child.wait("Poolers", 10);
    child.resize(42, 10);
    child.wait("Compact", 10);
    child.resize(1, 1);
    child.read(Duration::from_millis(100));
    child.resize(120, 40);
    child.wait("Health / evidence", 15);
    for view in [
        View::Backups,
        View::Roles,
        View::Databases,
        View::Subscriptions,
        View::Replicas,
        View::Plugins,
        View::Certs,
        View::Poolers,
        View::Services,
        View::Storage,
        View::Connections,
        View::Waits,
    ] {
        let name = format!("{view:?}");
        child.command(&format!(":{}", name.to_lowercase()), &name);
    }
    if std::env::var_os("E2E_METRICS_URL").is_some() {
        child.command(":certs", "Certs");
        child.wait("explicit named", 60);
    }
    child.command(":logs", "Logs");
    child.send(b":logscope cluster\r");
    let flood = tokio::spawn(async move {
        for _ in 0..4 {
            tokio::time::timeout(Duration::from_secs(10),live::admin(client.clone(),&primary,"DO $$ BEGIN FOR i IN 1..2000 LOOP RAISE LOG 'cnpg-watch-pty-flood-% 漢字', i; END LOOP; END $$;")).await.unwrap();
        }
    });
    child.wait("cnpg-watch-pty-flood", 30);
    let mut feedback = Vec::new();
    for _ in 0..6 {
        let start = Instant::now();
        child.send(b"?");
        child.wait("Observe-only", 5);
        feedback.push(start.elapsed().as_micros() as u64);
        child.send(b"?");
        child.wait_until(Duration::from_secs(5), |screen| {
            screen.contains("> Logs |") && !screen.contains("Observe-only")
        });
    }
    child.send(b"i");
    child.wait("Snapshot at", 10);
    child.send(b"\x1b");
    child.wait("> Logs |", 10);
    child.command(":instances", "Instances");
    child.send(b"q");
    assert_eq!(child.exit(5), 0);
    assert!(child.restored());
    tokio::time::timeout(Duration::from_secs(15), flood)
        .await
        .unwrap()
        .unwrap();
    feedback.sort_unstable();
    let p95 = feedback[(feedback.len() - 1) * 95 / 100];
    assert!(
        p95 <= 250_000,
        "real terminal flood feedback exceeded 250ms: {p95}us"
    );
    std::fs::create_dir_all("artifacts/phase7").unwrap();
    std::fs::write("artifacts/phase7/terminal-live.json",serde_json::to_vec_pretty(&serde_json::json!({"schema_version":1,"driver":"portable-pty 0.9 / vt100 0.18","real_navigation_resize_cycles":8,"flood_lines":8000,"feedback_us":feedback,"feedback_p95_us":p95,"normal_exit":0,"terminal_restored":true})).unwrap()).unwrap();
    let mut interrupted = pty::Pty::spawn(command(), 100, 30);
    interrupted.wait("payments-1", 60);
    interrupted.send(&[3]);
    assert_eq!(interrupted.exit(5), 0);
    assert!(interrupted.restored());
}
