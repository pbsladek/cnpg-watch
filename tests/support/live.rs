#![allow(dead_code)]
use cnpg_watch::{
    app::Session,
    cli::{Options, Profile},
    config::{Config, Paths},
    kubernetes,
    model::*,
    probes,
};
use k8s_openapi::api::{
    authentication::v1::{TokenRequest, TokenRequestSpec},
    core::v1::{Pod, ServiceAccount},
};
use kube::{
    Api, Client,
    api::{AttachParams, PostParams},
    config::{AuthInfo, KubeConfigOptions},
};
use secrecy::SecretString;
use std::time::Duration;
use tokio::io::AsyncReadExt;
pub fn config(all: bool, profile: Profile) -> Config {
    assert_eq!(std::env::var("CNPG_WATCH_E2E").as_deref(), Ok("1"));
    assert!(std::env::var_os("KUBECONFIG").is_some());
    let context = std::env::var("E2E_CONTEXT").unwrap();
    assert!(context.starts_with("k3d-cnpg-watch-"));
    let mut config = Config::resolve(
        Options {
            config: Some("tests/e2e/watch.toml".into()),
            kube_context: Some(context),
            all_namespaces: Some(all),
            operator_namespace: Some("cnpg-system".into()),
            profile: Some(profile),
            password_file: Some(std::env::var("E2E_PASSWORD_FILE").unwrap().into()),
            ..Default::default()
        },
        Paths::discover().unwrap(),
    )
    .unwrap();
    config.cluster = None;
    config
}
pub async fn wait(session: &mut Session, seconds: u64, ready: impl Fn(&Session) -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        session.tick();
        if ready(session) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "live deadline: sources {:?}, completed {}, diagnostics {}, pending {}, logs {:?}",
            session
                .store
                .watches
                .iter()
                .map(|(k, w)| (k, w.status, w.objects.len()))
                .collect::<Vec<_>>(),
            session.completed,
            session.diagnostic_completed,
            session.pending_targets(),
            session
                .logs
                .sources
                .iter()
                .map(|(k, s)| (&k.key, s.active, s.state, s.lines.len()))
                .collect::<Vec<_>>()
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
pub async fn pump(session: &mut Session, seconds: u64) {
    let until = tokio::time::Instant::now() + Duration::from_secs(seconds);
    while tokio::time::Instant::now() < until {
        session.tick();
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
pub async fn primary(client: Client, namespace: &str, cluster: &str) -> Key {
    kubernetes::primary(client, namespace, cluster)
        .await
        .unwrap()
}
pub async fn admin(client: Client, pod: &Key, sql: &str) -> String {
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
    let mut stdout = p.stdout().unwrap();
    let mut stderr = p.stderr().unwrap();
    let status = p.take_status().unwrap();
    let mut out = String::new();
    let mut err = String::new();
    let (a, b, status) = tokio::join!(
        stdout.read_to_string(&mut out),
        stderr.read_to_string(&mut err),
        status
    );
    assert!(
        a.is_ok() && b.is_ok() && status.is_some_and(|s| s.status.as_deref() == Some("Success")),
        "test-only admin observer failed; contents suppressed"
    );
    p.join().await.unwrap();
    out.trim().into()
}
pub async fn identity(admin: Client, config: &Config, name: &str) -> Client {
    let accounts: Api<ServiceAccount> = Api::namespaced(admin, "cnpg-watch-e2e");
    if accounts.get_opt(name).await.unwrap().is_none() {
        accounts
            .create(
                &PostParams::default(),
                &ServiceAccount {
                    metadata: kube::api::ObjectMeta {
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
                    expiration_seconds: Some(3600),
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
    // The same production rate-limiting builder is used for injected runtime identities.
    cnpg_watch::kubernetes::configured_client(raw, &config.budgets).unwrap()
}
pub async fn password(config: &Config) -> SecretString {
    probes::password_file(config.password_file.as_ref().unwrap())
        .await
        .unwrap()
}
