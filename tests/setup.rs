use clap::Parser;
use cnpg_watch::{
    capabilities::{self, Feature},
    cli::{Cli, Command, Profile, Setup},
    config::{Config, Paths},
    setup::{rbac, roles::RolePlan},
};

fn input(arguments: &[&str]) -> (Config, Setup) {
    let cli = Cli::try_parse_from(std::iter::once("cnpg-watch").chain(arguments.iter().copied()))
        .unwrap();
    let config = Config::resolve(cli.options, Paths::discover().unwrap()).unwrap();
    let Some(Command::Setup { command }) = cli.command else {
        panic!("setup required")
    };
    (config, command)
}
#[test]
fn profiles_gate_implemented_and_future_work_and_keep_secret_access_off() {
    for profile in [
        Profile::Minimal,
        Profile::Standard,
        Profile::Incident,
        Profile::Secure,
    ] {
        let e = capabilities::envelope(profile);
        assert!(!e.secret_reads && !e.query_text && !e.background_sql);
        assert_eq!(e.sql, profile != Profile::Minimal);
        let caps = capabilities::initial(profile, false);
        let logs = caps
            .iter()
            .find(|c| c.spec.feature == Feature::Logs)
            .unwrap();
        assert_eq!(logs.requested, profile == Profile::Incident);
        if logs.requested {
            assert!(logs.spec.implemented);
            let backups = caps
                .iter()
                .find(|c| c.spec.feature == Feature::Backups)
                .unwrap();
            assert!(backups.spec.implemented);
            assert_eq!(backups.state, "unknown");
        }
    }
}
#[test]
fn rbac_scopes_profiles_and_explicit_subjects_match_reviewed_manifests() {
    for (file, args) in [
        (
            "namespace",
            vec![
                "--namespace",
                "app-db",
                "setup",
                "kubernetes-rbac",
                "--service-account",
                "cnpg-watch",
                "--dry-run",
            ],
        ),
        (
            "operator",
            vec![
                "--namespace",
                "app-db",
                "--operator-namespace",
                "cnpg-system",
                "setup",
                "kubernetes-rbac",
                "--service-account",
                "cnpg-watch",
                "--dry-run",
            ],
        ),
        (
            "cluster",
            vec![
                "--namespace",
                "app-db",
                "setup",
                "kubernetes-rbac",
                "--user",
                "reader@example.test",
                "--include-cluster-resources",
                "--dry-run",
            ],
        ),
    ] {
        let (config, Setup::KubernetesRbac(args)) = input(&args) else {
            unreachable!()
        };
        let manifests = rbac::generate(&config, &args).unwrap();
        let yaml = rbac::yaml(&manifests).unwrap();
        let expected = std::fs::read_to_string(format!("examples/setup/{file}.yaml")).unwrap();
        assert_eq!(
            yaml, expected,
            "review generated output before changing {file}.yaml"
        );
        assert!(
            !yaml.contains("databaserolelists")
                && !yaml.contains("/status")
                && !yaml.contains("secrets")
        );
        for m in &manifests {
            if m.kind == "Role" {
                assert!(!m.rules.as_ref().unwrap().iter().any(|r| {
                    r.resources
                        .as_ref()
                        .unwrap()
                        .iter()
                        .any(|s| s.contains("webhook") || s.contains("customresourcedefinition"))
                }));
            }
        }
    }
    let (config, Setup::KubernetesRbac(args)) = input(&[
        "--namespace",
        "app-db",
        "--profile",
        "minimal",
        "setup",
        "kubernetes-rbac",
        "--group",
        "db-readers",
    ]) else {
        unreachable!()
    };
    assert!(
        !rbac::yaml(&rbac::generate(&config, &args).unwrap())
            .unwrap()
            .contains("pods/exec")
    );
    let (config, Setup::KubernetesRbac(args)) = input(&[
        "--all-namespaces",
        "setup",
        "kubernetes-rbac",
        "--service-account",
        "reader",
        "--service-account-namespace",
        "tools",
    ]) else {
        unreachable!()
    };
    let objects = rbac::generate(&config, &args).unwrap();
    assert_eq!(objects[0].kind, "ClusterRole");
    assert!(
        !rbac::yaml(&objects)
            .unwrap()
            .contains("customresourcedefinitions")
    );
}
#[test]
fn offline_plans_reject_ambiguous_or_privileged_targets() {
    for args in [
        vec!["setup", "kubernetes-rbac"],
        vec!["setup", "kubernetes-rbac", "--user", "a", "--group", "b"],
        vec!["setup", "postgres-role", "--apply", "--print-sql"],
        vec![
            "setup",
            "kubernetes-rbac",
            "--user",
            "a",
            "--apply",
            "--diff",
        ],
    ] {
        assert!(Cli::try_parse_from(std::iter::once("cnpg-watch").chain(args)).is_err());
    }
    for args in [
        vec!["setup", "kubernetes-rbac", "--user", "a"],
        vec![
            "--all-namespaces",
            "setup",
            "kubernetes-rbac",
            "--service-account",
            "reader",
        ],
    ] {
        let (c, Setup::KubernetesRbac(a)) = input(&args) else {
            unreachable!()
        };
        assert!(rbac::generate(&c, &a).is_err());
    }
    for role in [
        "postgres",
        "pg_monitor",
        "cnpg_reserved",
        "bad'role",
        "bad\nrole",
    ] {
        let (c, Setup::PostgresRole(a)) = input(&["setup", "postgres-role", "--role-name", role])
        else {
            unreachable!()
        };
        assert!(RolePlan::new(&c, &a).is_err());
    }
}
#[test]
fn ddl_is_deterministic_scoped_and_password_free() {
    for profile in ["monitor", "monitor-reserved", "actions"] {
        let (c, Setup::PostgresRole(a)) = input(&[
            "setup",
            "postgres-role",
            "--role-profile",
            profile,
            "--print-sql",
        ]) else {
            unreachable!()
        };
        let plan = RolePlan::new(&c, &a).unwrap();
        let sql = plan.sql();
        assert_eq!(
            sql,
            std::fs::read_to_string(format!("examples/setup/{profile}.sql")).unwrap()
        );
        assert!(!sql.contains("PASSWORD '") && !sql.contains("REVOKE"));
        assert!(sql.contains("NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS"));
        if profile == "actions" {
            assert!(sql.contains("NOLOGIN") && !sql.contains("pg_monitor"));
        }
        if profile == "monitor" {
            assert!(
                !sql.contains("pg_signal_backend") && !sql.contains("pg_use_reserved_connections")
            );
        }
    }
    let (c, Setup::PostgresRole(a)) =
        input(&["setup", "postgres-role", "--rotate-password", "--print-sql"])
    else {
        unreachable!()
    };
    assert!(
        RolePlan::new(&c, &a)
            .unwrap()
            .sql()
            .contains("\\getenv cnpg_watch_new_password")
    );
}
#[test]
fn equivalent_rbac_grouping_is_semantically_equal() {
    use k8s_openapi::api::rbac::v1::PolicyRule;
    let grouped = PolicyRule {
        api_groups: Some(vec!["".into()]),
        resources: Some(vec!["pods".into(), "events".into()]),
        verbs: vec!["watch".into(), "get".into()],
        ..Default::default()
    };
    let expanded = rbac::canonical_rules(&[grouped]);
    assert_eq!(expanded.len(), 4);
    assert_eq!(rbac::canonical_rules(&expanded), expanded);
}

#[test]
fn profile_defaults_are_conservative_and_explicit_refresh_is_preserved() {
    for (profile, seconds) in [
        ("minimal", 5),
        ("standard", 2),
        ("incident", 1),
        ("secure", 3),
    ] {
        let cli = Cli::try_parse_from(["cnpg-watch", "--profile", profile, "info"]).unwrap();
        let config = Config::resolve(cli.options, Paths::discover().unwrap()).unwrap();
        assert_eq!(config.refresh.as_secs(), seconds);
        assert!(!config.refresh_explicit);
        let cli = Cli::try_parse_from([
            "cnpg-watch",
            "--profile",
            profile,
            "--refresh",
            "7s",
            "info",
        ])
        .unwrap();
        let config = Config::resolve(cli.options, Paths::discover().unwrap()).unwrap();
        assert_eq!(config.refresh.as_secs(), 7);
        assert!(config.refresh_explicit);
    }
}

#[test]
fn explicit_pruning_can_be_reviewed_offline_before_apply() {
    let (config, Setup::PostgresRole(args)) =
        input(&["setup", "postgres-role", "--prune", "--print-sql"])
    else {
        unreachable!()
    };
    assert!(
        RolePlan::new(&config, &args)
            .unwrap()
            .sql()
            .contains("REVOKE %I FROM %I")
    );
}

#[test]
fn all_namespace_database_access_keeps_configured_operator_access_namespaced() {
    let (config, Setup::KubernetesRbac(args)) = input(&[
        "--all-namespaces",
        "--operator-namespace",
        "cnpg-system",
        "setup",
        "kubernetes-rbac",
        "--user",
        "reader",
    ]) else {
        unreachable!()
    };
    let objects = rbac::generate(&config, &args).unwrap();
    assert_eq!(objects.len(), 4);
    let fleet = objects.iter().find(|o| o.kind == "ClusterRole").unwrap();
    assert!(
        fleet
            .rules
            .as_ref()
            .unwrap()
            .iter()
            .filter(|r| r.api_groups.as_ref().unwrap().contains(&"apps".to_string()))
            .all(|r| r.resources.as_ref().unwrap().iter().all(|name| [
                "deployments",
                "replicasets"
            ]
            .contains(&name.as_str())))
    );
    let operator = objects.iter().find(|o| o.kind == "Role").unwrap();
    assert_eq!(operator.metadata.namespace.as_deref(), Some("cnpg-system"));
}

#[test]
fn explicit_certificate_grants_are_named_get_only_in_each_namespace() {
    let (config, Setup::KubernetesRbac(args)) = input(&[
        "--namespace",
        "app-db",
        "--certificate-secret",
        "app-db/server-cert",
        "--certificate-secret",
        "cnpg-system/webhook-cert",
        "setup",
        "kubernetes-rbac",
        "--user",
        "reader",
        "--dry-run",
    ]) else {
        panic!()
    };
    let manifests = rbac::generate(&config, &args).unwrap();
    let grants: Vec<_> = manifests
        .iter()
        .filter(|m| m.kind == "Role")
        .flat_map(|m| {
            m.rules
                .as_ref()
                .unwrap()
                .iter()
                .map(move |r| (m.metadata.namespace.as_deref(), r))
        })
        .filter(|(_, r)| r.resources.as_ref().unwrap().iter().any(|s| s == "secrets"))
        .collect();
    assert_eq!(grants.len(), 2);
    for (namespace, rule) in grants {
        assert_eq!(rule.verbs, vec!["get"]);
        assert_eq!(rule.resource_names.as_ref().unwrap().len(), 1);
        assert_eq!(
            rule.resource_names.as_ref().unwrap()[0],
            if namespace == Some("app-db") {
                "server-cert"
            } else {
                "webhook-cert"
            }
        );
    }
}
