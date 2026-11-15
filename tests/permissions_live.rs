//! Isolated Phase 2 qualification with actual bearer-token identities and production setup.
use clap::Parser;
use cnpg_watch::{
    cli::{Cli, Command, Profile, Setup},
    config::{Config, Paths},
    doctor::{self, State},
    kubernetes,
    model::Key,
    probes::{self, PodExec},
    setup::rbac,
};
use k8s_openapi::{
    api::{
        authentication::v1::{TokenRequest, TokenRequestSpec},
        core::v1::{Pod, ServiceAccount},
        rbac::v1::{PolicyRule, Role, RoleBinding, RoleRef, Subject},
    },
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use kube::{
    Api, Client,
    api::{AttachParams, ListParams, LogParams, PostParams, WatchParams},
    config::{AuthInfo, KubeConfigOptions},
    core::{ApiResource, GroupVersionKind},
};
use secrecy::SecretString;
use serde_json::{Value, json};
use std::{process::Command as Process, time::Duration};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
fn fixture() -> Config {
    assert_eq!(std::env::var("CNPG_WATCH_E2E").as_deref(), Ok("1"));
    let ctx = std::env::var("E2E_CONTEXT").unwrap();
    assert!(ctx.starts_with("k3d-cnpg-watch-"));
    assert!(std::env::var_os("KUBECONFIG").is_some());
    Config::resolve(
        cnpg_watch::cli::Options {
            config: Some("tests/e2e/watch.toml".into()),
            kube_context: Some(ctx),
            password_file: Some(std::env::var("E2E_PASSWORD_FILE").unwrap().into()),
            ..Default::default()
        },
        Paths::discover().unwrap(),
    )
    .unwrap()
}
fn rbac_args(config: &Config, name: &str, operator: Option<&str>) -> Vec<rbac::Manifest> {
    let mut args = vec![
        "cnpg-watch",
        "--namespace",
        "cnpg-watch-e2e",
        "setup",
        "kubernetes-rbac",
        "--service-account",
        name,
        "--name",
        name,
    ];
    if let Some(ns) = operator {
        args.extend(["--operator-namespace", ns]);
    }
    let cli = Cli::try_parse_from(args).unwrap();
    let Some(Command::Setup {
        command: Setup::KubernetesRbac(args),
    }) = cli.command
    else {
        unreachable!()
    };
    let mut config = config.clone();
    config.operator_namespace = operator.map(Into::into);
    rbac::generate(&config, &args).unwrap()
}
async fn token_client(admin: Client, config: &Config, name: &str) -> Client {
    let accounts: Api<ServiceAccount> = Api::namespaced(admin, "cnpg-watch-e2e");
    if accounts.get_opt(name).await.unwrap().is_none() {
        accounts
            .create(
                &PostParams::default(),
                &ServiceAccount {
                    metadata: ObjectMeta {
                        name: Some(name.into()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }
    let issued = accounts
        .create_token_request(
            name,
            &PostParams::default(),
            &TokenRequest {
                spec: TokenRequestSpec {
                    audiences: vec![],
                    expiration_seconds: Some(600),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let mut raw = kube::Config::from_kubeconfig(&KubeConfigOptions {
        context: config.context.clone(),
        ..Default::default()
    })
    .await
    .unwrap();
    raw.auth_info = AuthInfo {
        token: Some(SecretString::from(issued.status.unwrap().token)),
        ..Default::default()
    };
    Client::try_from(raw).unwrap()
}
async fn primary(admin: Client, config: &Config) -> Key {
    kubernetes::primary(admin, "cnpg-watch-e2e", config.cluster.as_deref().unwrap())
        .await
        .unwrap()
}
async fn admin_sql(client: Client, pod: &Key, sql: &str) -> String {
    let pods: Api<Pod> = Api::namespaced(client, &pod.namespace);
    let mut p = pods
        .exec(
            &pod.name,
            [
                "psql",
                "-XAtqw",
                "-d",
                "postgres",
                "-v",
                "ON_ERROR_STOP=1",
                "-c",
                sql,
            ],
            &AttachParams::default()
                .container("postgres")
                .stdout(true)
                .stderr(true),
        )
        .await
        .unwrap();
    let mut out = String::new();
    let mut err = String::new();
    let mut stdout = p.stdout().unwrap();
    let mut stderr = p.stderr().unwrap();
    let status = p.take_status().unwrap();
    let (_, _, status) = tokio::join!(
        stdout.read_to_string(&mut out),
        stderr.read_to_string(&mut err),
        status
    );
    assert!(
        status.unwrap().status.as_deref() == Some("Success"),
        "test admin observer failed; contents suppressed"
    );
    p.join().await.unwrap();
    out.trim().into()
}
fn role_command(config: &Config, role: &str, extra: &[&str]) -> std::process::Output {
    let out = Process::new(env!("CARGO_BIN_EXE_cnpg-watch"))
        .args([
            "--config",
            "tests/e2e/watch.toml",
            "--kube-context",
            config.context.as_deref().unwrap(),
            "--password-file",
            config.password_file.as_ref().unwrap().to_str().unwrap(),
            "setup",
            "postgres-role",
            "--role-name",
            role,
            "--admin-role",
            "postgres",
            "--admin-password-file",
            &std::env::var("E2E_ADMIN_PASSWORD_FILE").unwrap(),
            "--output",
            "json",
        ])
        .args(extra)
        .output()
        .unwrap();
    let password = std::fs::read_to_string(config.password_file.as_ref().unwrap()).unwrap();
    let admin = std::fs::read_to_string(std::env::var("E2E_ADMIN_PASSWORD_FILE").unwrap()).unwrap();
    for bytes in [&out.stdout, &out.stderr] {
        let text = String::from_utf8_lossy(bytes);
        assert!(
            !text.contains(password.trim()) && !text.contains(admin.trim()),
            "credential leaked"
        );
    }
    out
}
#[tokio::test]
#[ignore = "requires the explicitly provisioned permissions fixture"]
async fn generated_rbac_and_exec_verb_matrix_match_real_requests() {
    let config = fixture();
    let (admin, ns) = kubernetes::client(&config, &config.budgets).await.unwrap();
    let pod = primary(admin.clone(), &config).await;
    let password = probes::password_file(config.password_file.as_ref().unwrap())
        .await
        .unwrap();
    let reader = token_client(admin.clone(), &config, "phase2-reader").await;
    let desired = rbac_args(&config, "phase2-reader", Some("cnpg-system"));
    let diff = rbac::reconcile(admin.clone(), desired.clone(), false, false)
        .await
        .unwrap();
    assert!(diff.missing.len() == 4 || diff.missing.is_empty());
    rbac::reconcile(admin.clone(), desired.clone(), true, true)
        .await
        .unwrap();
    let second = rbac::reconcile(admin.clone(), desired, true, false)
        .await
        .unwrap();
    assert!(!second.would_change);
    let pods: Api<Pod> = Api::namespaced(reader.clone(), &ns);
    assert_eq!(
        pods.list(
            &ListParams::default().labels("cnpg.io/cluster=payments,cnpg.io/podRole=instance")
        )
        .await
        .unwrap()
        .items
        .len(),
        2
    );
    let _watch = pods
        .watch(&WatchParams::default().timeout(1), "0")
        .await
        .unwrap();
    let r = ApiResource::from_gvk(&GroupVersionKind::gvk(
        "postgresql.cnpg.io",
        "v1",
        "Cluster",
    ));
    let clusters: Api<kube::api::DynamicObject> = Api::namespaced_with(reader.clone(), &ns, &r);
    assert_eq!(
        clusters
            .list(&ListParams::default())
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(
        PodExec::new(
            reader.clone(),
            password.clone(),
            config.db_role.clone(),
            config.budgets.clone()
        )
        .execute_sql(&pod, probes::FAST_SQL, &CancellationToken::new())
        .await
        .is_ok()
    );
    for (group, kind) in [
        ("apiextensions.k8s.io", "CustomResourceDefinition"),
        (
            "admissionregistration.k8s.io",
            "ValidatingWebhookConfiguration",
        ),
        ("postgresql.cnpg.io", "ClusterImageCatalog"),
    ] {
        let r = ApiResource::from_gvk(&GroupVersionKind::gvk(group, "v1", kind));
        let a: Api<kube::api::DynamicObject> = Api::all_with(reader.clone(), &r);
        assert_eq!(
            probes::api_error(&a.list(&ListParams::default().limit(1)).await.unwrap_err()),
            cnpg_watch::model::ErrorClass::Forbidden
        );
    }
    let secrets: Api<k8s_openapi::api::core::v1::Secret> = Api::namespaced(reader.clone(), &ns);
    assert!(secrets.list(&ListParams::default()).await.is_err());
    let mut cfg = config.clone();
    cfg.operator_namespace = Some("cnpg-system".into());
    let report = doctor::inspect_client(&cfg, reader.clone(), ns.clone(), false).await;
    assert_eq!(report.cnpg_api, State::Available);
    assert_eq!(report.postgres_role, State::Available);
    assert_eq!(report.operator.state, State::Available);
    assert!(
        report
            .capabilities
            .iter()
            .filter(|c| !c.spec.implemented && c.requested)
            .all(|c| c.state == "unfinished")
    );
    let operator: Api<k8s_openapi::api::apps::v1::Deployment> =
        Api::namespaced(reader.clone(), "cnpg-system");
    assert_eq!(
        operator
            .list(&ListParams::default())
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let mut same = config.clone();
    same.operator_namespace = Some(ns.clone());
    let same_report = doctor::inspect_client(&same, reader.clone(), ns.clone(), false).await;
    // Pooler ownership requires namespaced Deployment reads; an empty database namespace is now observable.
    assert_eq!(same_report.operator.state, State::Empty);
    let matrix: serde_json::Value = serde_json::from_str(include_str!("e2e/matrix.json")).unwrap();
    let cell = std::env::var("E2E_MATRIX_CELL").unwrap_or_else(|_| "k8s135".into());
    let expected = matrix["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == cell)
        .unwrap();
    let mut exec_results = serde_json::Map::new();
    for (name, verbs, variant) in [
        ("phase2-exec-get", vec!["get"], "get_only"),
        ("phase2-exec-create", vec!["create"], "create_only"),
        ("phase2-exec-both", vec!["get", "create"], "both"),
        ("phase2-exec-neither", vec![], "neither"),
    ] {
        let expected = expected["exec_allowed"][variant].as_bool().unwrap();
        let client = token_client(admin.clone(), &config, name).await;
        let roles: Api<Role> = Api::namespaced(admin.clone(), &ns);
        let bindings: Api<RoleBinding> = Api::namespaced(admin.clone(), &ns);
        if roles.get_opt(name).await.unwrap().is_none() {
            roles
                .create(
                    &PostParams::default(),
                    &Role {
                        metadata: ObjectMeta {
                            name: Some(name.into()),
                            ..Default::default()
                        },
                        rules: Some(if verbs.is_empty() {
                            vec![]
                        } else {
                            vec![PolicyRule {
                                api_groups: Some(vec!["".into()]),
                                resources: Some(vec!["pods/exec".into()]),
                                verbs: verbs.iter().map(|v| (*v).into()).collect(),
                                ..Default::default()
                            }]
                        }),
                    },
                )
                .await
                .unwrap();
        }
        if bindings.get_opt(name).await.unwrap().is_none() {
            bindings
                .create(
                    &PostParams::default(),
                    &RoleBinding {
                        metadata: ObjectMeta {
                            name: Some(name.into()),
                            ..Default::default()
                        },
                        role_ref: RoleRef {
                            api_group: "rbac.authorization.k8s.io".into(),
                            kind: "Role".into(),
                            name: name.into(),
                        },
                        subjects: Some(vec![Subject {
                            kind: "ServiceAccount".into(),
                            name: name.into(),
                            namespace: Some(ns.clone()),
                            ..Default::default()
                        }]),
                    },
                )
                .await
                .unwrap();
        }
        let outcome = PodExec::new(
            client,
            password.clone(),
            config.db_role.clone(),
            config.budgets.clone(),
        )
        .execute_sql(&pod, probes::FAST_SQL, &CancellationToken::new())
        .await;
        println!(
            "exec verbs {verbs:?}: {}",
            if outcome.is_ok() { "allowed" } else { "denied" }
        );
        exec_results.insert(
            variant.into(),
            serde_json::json!({"allowed":outcome.is_ok(),"error":outcome.as_ref().err()}),
        );
        assert_eq!(
            outcome.is_ok(),
            expected,
            "pinned exec permissions changed; update catalog only after investigation"
        );
    }
    std::fs::create_dir_all("artifacts/phase7").unwrap();
    std::fs::write(
        format!("artifacts/phase7/exec-{cell}.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"schema_version":1,"cell":cell,"actual":exec_results}),
        )
        .unwrap(),
    )
    .unwrap();
    // Incident generates actual log access; standard does not grant it preemptively.
    assert!(
        pods.logs(
            &pod.name,
            &LogParams {
                container: Some("postgres".into()),
                tail_lines: Some(1),
                limit_bytes: Some(1024),
                ..Default::default()
            }
        )
        .await
        .is_err()
    );
    let mut incident = config.clone();
    incident.profile = Profile::Incident;
    rbac::reconcile(
        admin.clone(),
        rbac_args(&incident, "phase2-reader", Some("cnpg-system")),
        true,
        false,
    )
    .await
    .unwrap();
    assert!(
        pods.logs(
            &pod.name,
            &LogParams {
                container: Some("postgres".into()),
                tail_lines: Some(1),
                limit_bytes: Some(1024),
                ..Default::default()
            }
        )
        .await
        .is_ok()
    );
    // No operator/global metadata grant is necessary for namespaced observation.
    let db_only = token_client(admin.clone(), &config, "phase2-db-only").await;
    rbac::reconcile(
        admin.clone(),
        rbac_args(&config, "phase2-db-only", None),
        true,
        false,
    )
    .await
    .unwrap();
    let db_report = doctor::inspect_client(&config, db_only.clone(), ns.clone(), false).await;
    assert_eq!(db_report.cnpg_api, State::Available);
    assert_eq!(db_report.postgres_role, State::Available);
    assert_eq!(db_report.operator.state, State::Forbidden);
    // Deny API discovery for this isolated fixture, while retaining ordinary resource access.
    let bindings: Api<k8s_openapi::api::rbac::v1::ClusterRoleBinding> = Api::all(admin.clone());
    let discovery = bindings.get("system:discovery").await.unwrap();
    bindings
        .patch(
            "system:discovery",
            &kube::api::PatchParams::default(),
            &kube::api::Patch::Merge(json!({"subjects":[]})),
        )
        .await
        .unwrap();
    let discovery_result = db_only
        .request::<Value>(
            http::Request::get("/apis/postgresql.cnpg.io/v1")
                .body(vec![])
                .unwrap(),
        )
        .await;
    // Restore before asserting so a failed contract cannot leave discovery disabled.
    let direct = doctor::inspect_client(&config, db_only.clone(), ns.clone(), false).await;
    bindings
        .patch(
            "system:discovery",
            &kube::api::PatchParams::default(),
            &kube::api::Patch::Merge(json!({"subjects":discovery.subjects})),
        )
        .await
        .unwrap();
    assert!(
        discovery_result.is_err(),
        "fixture must actually deny API discovery"
    );
    assert_eq!(direct.cnpg_api, State::Available);
    assert_eq!(direct.postgres_role, State::Available);
    let same_desired = rbac_args(&config, "phase2-db-only", Some(&ns));
    rbac::reconcile(admin.clone(), same_desired, true, false)
        .await
        .unwrap();
    let mut same_config = config.clone();
    same_config.operator_namespace = Some(ns.clone());
    let same_report = doctor::inspect_client(&same_config, db_only, ns.clone(), false).await;
    assert_eq!(same_report.operator.state, State::Empty);
    // Two visible operator candidates must remain ambiguous, with no inferred namespace.
    let deployments: Api<k8s_openapi::api::apps::v1::Deployment> =
        Api::namespaced(admin.clone(), "cnpg-system");
    let candidate: k8s_openapi::api::apps::v1::Deployment=serde_json::from_value(json!({"metadata":{"name":"phase2-operator-candidate","labels":{"app.kubernetes.io/name":"cloudnative-pg"}},"spec":{"replicas":0,"selector":{"matchLabels":{"app":"phase2-unused"}},"template":{"metadata":{"labels":{"app":"phase2-unused"}},"spec":{"containers":[{"name":"unused","image":"registry.k8s.io/pause:3.10"}]}}}})).unwrap();
    deployments
        .create(&PostParams::default(), &candidate)
        .await
        .unwrap();
    let ambiguous = doctor::inspect_client(&config, reader.clone(), ns.clone(), false).await;
    // reader lacks all-namespace deployments, so explicitly test the configured namespace.
    assert_eq!(ambiguous.operator.state, State::Forbidden);
    let mut configured = config.clone();
    configured.operator_namespace = Some("cnpg-system".into());
    let ambiguous = doctor::inspect_client(&configured, reader.clone(), ns.clone(), false).await;
    assert_eq!(ambiguous.operator.state, State::Ambiguous);
    deployments
        .delete(
            "phase2-operator-candidate",
            &kube::api::DeleteParams::default(),
        )
        .await
        .unwrap();
    // The assembled runtime fences profile transitions before cancellation.
    let mut session = cnpg_watch::app::Session::start(config.clone())
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
    while session.completed < 2 {
        session.tick();
        assert!(
            tokio::time::Instant::now() < deadline,
            "profile startup: SQL {:?}, sources {:?}",
            session.sql,
            session
                .store
                .watches
                .iter()
                .map(|(k, w)| (k, w.status))
                .collect::<Vec<_>>()
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let old_scope = session.store.scope;
    session.set_profile(Profile::Minimal).await;
    assert!(session.store.scope.0 > old_scope.0);
    assert!(session.store.samples.is_empty());
    assert_eq!(
        session.sql,
        cnpg_watch::model::Availability::DisabledByProfile
    );
    let completed = session.completed;
    let until = tokio::time::Instant::now() + Duration::from_secs(1);
    while tokio::time::Instant::now() < until {
        session.tick();
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(session.completed, completed);
    assert!(session.store.samples.is_empty());
    session.set_profile(Profile::Secure).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
    while session.completed == completed {
        session.tick();
        assert!(
            tokio::time::Instant::now() < deadline,
            "profile startup: SQL {:?}, sources {:?}",
            session.sql,
            session
                .store
                .watches
                .iter()
                .map(|(k, w)| (k, w.status))
                .collect::<Vec<_>>()
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(session.shutdown().await, 0);
    println!(
        "generated namespaced RBAC, separate operator scope, forbidden optional resources, and live logs passed"
    );
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL bootstrap admin credential"]
async fn role_setup_applies_twice_preserves_rotates_and_validates_real_monitor() {
    let config = fixture();
    let (admin, _) = kubernetes::client(&config, &config.budgets).await.unwrap();
    let pod = primary(admin.clone(), &config).await;
    // Wait for the fixture's explicitly enabled superuser TCP authentication.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let first = role_command(
        &config,
        "phase2_monitor",
        &["--apply", "--rotate-password", "--validate"],
    );
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let value: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(value["validated"], true);
    let hash = admin_sql(
        admin.clone(),
        &pod,
        "SELECT rolpassword FROM pg_authid WHERE rolname='phase2_monitor'",
    )
    .await;
    let second = role_command(
        &config,
        "phase2_monitor",
        &["--apply", "--preserve-password"],
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&second.stdout).unwrap()["would_change"],
        false
    );
    assert!(
        hash == admin_sql(
            admin.clone(),
            &pod,
            "SELECT rolpassword FROM pg_authid WHERE rolname='phase2_monitor'"
        )
        .await,
        "preserve changed the password"
    );
    let diff = role_command(&config, "phase2_monitor", &["--diff"]);
    assert!(diff.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&diff.stdout).unwrap()["would_change"],
        false
    );
    let password = probes::password_file(config.password_file.as_ref().unwrap())
        .await
        .unwrap();
    let monitor = PodExec::new(
        admin.clone(),
        password,
        "phase2_monitor".into(),
        config.budgets.clone(),
    );
    let raw = monitor
        .execute_sql(&pod, probes::FAST_SQL, &CancellationToken::new())
        .await
        .unwrap();
    assert!(probes::parse(&raw, "phase2_monitor").is_ok());
    for (role, profile, grant) in [
        (
            "phase2_reserved",
            "monitor-reserved",
            "pg_use_reserved_connections",
        ),
        ("phase2_actions", "actions", "pg_signal_backend"),
    ] {
        let out = role_command(&config, role, &["--role-profile", profile, "--apply"]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            admin_sql(
                admin.clone(),
                &pod,
                &format!("SELECT pg_has_role('{role}','{grant}','MEMBER')")
            )
            .await,
            "t"
        );
    }
    assert_eq!(
        admin_sql(
            admin.clone(),
            &pod,
            "SELECT rolcanlogin FROM pg_roles WHERE rolname='phase2_actions'"
        )
        .await,
        "f"
    );
    assert_eq!(admin_sql(admin.clone(),&pod,"SELECT pg_has_role('phase2_monitor','pg_signal_backend','MEMBER') OR pg_has_role('phase2_monitor','pg_use_reserved_connections','MEMBER')").await,"f");
    admin_sql(
        admin.clone(),
        &pod,
        "GRANT pg_read_all_data TO phase2_monitor",
    )
    .await;
    let kept = role_command(&config, "phase2_monitor", &["--apply"]);
    assert!(kept.status.success());
    assert!(
        !serde_json::from_slice::<Value>(&kept.stdout).unwrap()["extra"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let pruned = role_command(&config, "phase2_monitor", &["--apply", "--prune"]);
    assert!(
        pruned.status.success(),
        "{}",
        String::from_utf8_lossy(&pruned.stderr)
    );
    assert_eq!(
        admin_sql(
            admin.clone(),
            &pod,
            "SELECT pg_has_role('phase2_monitor','pg_read_all_data','MEMBER')"
        )
        .await,
        "f"
    );
    admin_sql(
        admin.clone(),
        &pod,
        "GRANT pg_monitor TO phase2_monitor WITH INHERIT FALSE",
    )
    .await;
    let broken = role_command(&config, "phase2_monitor", &["--diff"]);
    assert!(broken.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&broken.stdout).unwrap()["would_change"],
        true
    );
    let repaired = role_command(&config, "phase2_monitor", &["--apply", "--validate"]);
    assert!(
        repaired.status.success(),
        "{}",
        String::from_utf8_lossy(&repaired.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&repaired.stdout).unwrap()["role_authenticated"],
        true
    );
    let rotated = role_command(&config, "phase2_monitor", &["--apply", "--rotate-password"]);
    assert!(rotated.status.success());
    assert!(
        hash != admin_sql(
            admin,
            &pod,
            "SELECT rolpassword FROM pg_authid WHERE rolname='phase2_monitor'"
        )
        .await,
        "explicit rotation did not change verifier"
    );
    println!(
        "role creation, repeat apply, preservation, explicit rotation/pruning, reserved/action separation, and actual production bundle passed"
    );
}
