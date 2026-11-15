use std::process::Command;

fn binary() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cnpg-watch"));
    for key in [
        "CNPG_WATCH_LOG_FILE",
        "CNPG_WATCH_CONFIG",
        "CNPG_WATCH_CONTEXT",
        "CNPG_WATCH_NAMESPACE",
        "CNPG_WATCH_ALL_NAMESPACES",
        "CNPG_WATCH_PROFILE",
        "CNPG_WATCH_REFRESH",
        "CNPG_WATCH_PASSWORD_FILE",
        "CNPG_WATCH_DB_ROLE",
    ] {
        command.env_remove(key);
    }
    command
}

#[test]
fn json_info_has_clean_stdout_and_unknown_live_values() {
    let output = binary()
        .args(["info", "--output", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert!(value["detected_context"].is_null());
    assert!(output.stderr.is_empty());
}

#[test]
fn setup_apply_requires_explicit_target_and_admin_credentials() {
    let output = binary()
        .args(["setup", "postgres-role", "--apply"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("requires explicit --namespace and --cluster")
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn flags_override_environment_and_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "refresh = '4s'\nprofile = 'minimal'\n").unwrap();
    let output = binary()
        .env("CNPG_WATCH_REFRESH", "3s")
        .args([
            "--config",
            path.to_str().unwrap(),
            "--refresh",
            "1s",
            "info",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["refresh_ms"], 1000);
    assert_eq!(value["profile"], "minimal");
}

#[test]
fn invalid_flags_and_noninteractive_watch_fail() {
    for args in [
        vec!["--refresh", "0s", "info"],
        vec!["--namespace", "db", "--all-namespaces", "info"],
        vec!["watch"],
    ] {
        assert!(!binary().args(args).output().unwrap().status.success());
    }
    let output = binary().arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("read-only"));
}

#[test]
fn contradictory_or_extreme_budgets_are_rejected_without_panicking_or_echoing_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    for contents in [
        "[budgets]\nexec_concurrency=0",
        "[budgets]\nstdout_bytes=18446744073709551615\nstderr_bytes=18446744073709551615",
        "unrecognized_password='do-not-echo-this'",
        "[budgets]\nobject_bytes=8388608",
    ] {
        std::fs::write(&path, contents).unwrap();
        let output = binary()
            .args([
                "--config",
                path.to_str().unwrap(),
                "info",
                "--output",
                "json",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let text = String::from_utf8_lossy(&output.stderr);
        assert!(!text.contains("do-not-echo-this"));
        assert!(!text.contains("panicked"));
    }
}

#[test]
fn explicit_all_namespace_scope_overrides_the_file_namespace() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "namespace='file-db'").unwrap();
    let output = binary()
        .args([
            "--config",
            path.to_str().unwrap(),
            "--all-namespaces",
            "info",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["scope"]["all_namespaces"], true);
    assert!(value["scope"]["namespace"].is_null());
}

#[test]
fn dry_run_commands_need_no_kubeconfig_and_never_echo_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let password = dir.path().join("credential");
    std::fs::write(&password, "do-not-print-this-password").unwrap();
    for args in [
        vec![
            "--namespace",
            "app-db",
            "setup",
            "kubernetes-rbac",
            "--user",
            "reader",
            "--dry-run",
        ],
        vec!["setup", "postgres-role", "--rotate-password", "--print-sql"],
    ] {
        let output = binary()
            .env("KUBECONFIG", "/does/not/exist")
            .args(["--password-file", password.to_str().unwrap()])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("do-not-print-this-password"));
        assert!(output.stderr.is_empty());
    }
    let output = binary()
        .env("KUBECONFIG", "/does/not/exist")
        .args(["doctor", "--output", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert!(!value["findings"].as_array().unwrap().is_empty());
    assert!(output.stderr.is_empty());
}
#[test]
fn secure_info_hides_paths_and_reports_actual_feature_envelope() {
    let output = binary()
        .args([
            "--profile",
            "secure",
            "--log-file",
            "/credential/location",
            "info",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["log_file"], "<redacted>");
    assert_eq!(value["profile_envelope"]["query_text"], false);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("/credential/location"));
}
#[test]
fn operational_opt_ins_validate_without_echoing_sensitive_values() {
    for (flag, value) in [
        ("--certificate-secret", "bad/reference/SECRET_MARKER"),
        ("--certificate-secret", "*/SECRET_MARKER"),
        (
            "--operator-metrics-url",
            "https://user:SECRET_MARKER@example.test/metrics",
        ),
        (
            "--operator-metrics-url",
            "https://example.test/metrics?token=SECRET_MARKER",
        ),
        ("--operator-metrics-url", "file:///SECRET_MARKER"),
        ("--pooler-socket", "relative/SECRET_MARKER"),
    ] {
        let output = binary()
            .args([flag, value, "info", "--output", "json"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("SECRET_MARKER"));
    }
    let output = binary()
        .args(["info", "--output", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["pooler_stats"]["enabled"], false);
    assert_eq!(value["operational"]["cluster_scoped_metadata"], false);
    assert_eq!(value["operational"]["explicit_certificate_reads"], 0);
    assert_eq!(value["operational"]["operator_metrics_configured"], false);
}
