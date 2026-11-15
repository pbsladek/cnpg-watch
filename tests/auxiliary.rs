mod support;
use cnpg_watch::{
    auxiliary::{self, Key as AuxKey, Loader, Metric, Sample, Scheduler},
    budget::Budgets,
    cli::{Options, Profile},
    config::{Config, Paths},
    model::*,
};
use http::{Request, Response};
use kube::{Client, client::Body};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
fn config() -> Config {
    Config::resolve(
        Options {
            profile: Some(Profile::Standard),
            ..Default::default()
        },
        Paths::discover().unwrap(),
    )
    .unwrap()
}
fn client(secret: bool) -> (Client, Arc<Mutex<Vec<String>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let service = tower::service_fn(move |req: Request<Body>| {
        let seen = seen.clone();
        async move {
            seen.lock()
                .unwrap()
                .push(format!("{} {}", req.method(), req.uri().path()));
            let (status, body) = if secret {
                let mut data = std::collections::BTreeMap::new();
                data.insert(
                    "tls.crt".into(),
                    k8s_openapi::ByteString(
                        include_bytes!("../fixtures/cnpg/1.30.0/server-public.crt").to_vec(),
                    ),
                );
                data.insert(
                    "tls.key".into(),
                    k8s_openapi::ByteString(b"PRIVATE_KEY_MARKER".to_vec()),
                );
                let secret = k8s_openapi::api::core::v1::Secret {
                    metadata: kube::api::ObjectMeta {
                        uid: Some("certificate-uid".into()),
                        ..Default::default()
                    },
                    data: Some(data),
                    ..Default::default()
                };
                (200, serde_json::to_vec(&secret).unwrap())
            } else {
                (403,br#"{"apiVersion":"v1","kind":"Status","reason":"Forbidden","status":"Failure","code":403,"message":"SECRET_DENIAL_PAYLOAD"}"#.to_vec())
            };
            Ok::<_, std::convert::Infallible>(
                Response::builder()
                    .status(status)
                    .body(Body::from(body))
                    .unwrap(),
            )
        }
    });
    (Client::new(service, "app-db"), calls)
}
#[test]
fn captured_public_certificate_and_metrics_are_typed_bounded_and_label_free() {
    let cert = auxiliary::parse_certificate(
        include_bytes!("../fixtures/cnpg/1.30.0/server-public.crt"),
        Uid::new("fixture").unwrap(),
    )
    .unwrap();
    assert!(cert.not_before_ms < cert.not_after_ms);
    assert_eq!(cert.fingerprint_sha256.len(), 64);
    assert_eq!(cert.chain_length, 1);
    assert!(
        !serde_json::to_string(&cert)
            .unwrap()
            .contains("CERTIFICATE")
    );
    assert_eq!(
        auxiliary::parse_certificate(
            b"-----BEGIN PRIVATE KEY-----\nprivate\n-----END PRIVATE KEY-----",
            Uid::new("x").unwrap()
        ),
        Err(ErrorClass::InvalidData)
    );
    assert_eq!(
        auxiliary::parse_certificate(&vec![0; 65537], Uid::new("x").unwrap()),
        Err(ErrorClass::OutputLimit)
    );
    let metrics=auxiliary::parse_metrics(b"# HELP ignored\nunknown_metric 999\nworkqueue_depth{name=\"private_label\"} 2\nworkqueue_depth{name=\"other\"} 3\ncontroller_runtime_reconcile_total 42\n").unwrap();
    assert_eq!(metrics[&Metric::QueueDepth].get(), 5.);
    assert_eq!(metrics.len(), 2);
    assert!(
        !serde_json::to_string(&Sample::Metrics(metrics))
            .unwrap()
            .contains("private_label")
    );
    for text in [
        "workqueue_depth NaN",
        "workqueue_depth +Inf",
        "workqueue_depth -1",
        "workqueue_depth{a=\"x\",a=\"y\"} 1",
        "workqueue_depth{a=unquoted} 1",
        "workqueue_depth{a=\"bad\\t\"} 1",
    ] {
        assert_eq!(
            auxiliary::parse_metrics(text.as_bytes()),
            Err(ErrorClass::InvalidData),
            "{text}"
        );
    }
    assert_eq!(
        auxiliary::parse_metrics(b"unknown_metric 3"),
        Err(ErrorClass::Unsupported)
    );
    assert_eq!(
        auxiliary::parse_metrics(&vec![b'x'; 256 * 1024 + 1]),
        Err(ErrorClass::OutputLimit)
    );
}
#[tokio::test]
async fn named_get_only_default_off_minimal_denial_and_private_fields_never_enter_store() {
    let (kube, calls) = client(true);
    let loader = Loader::new(kube, Budgets::default());
    let mut config = config();
    let key = cnpg_watch::model::Key::new("app-db", "public-cert").unwrap();
    let source = AuxKey::Certificate(key.clone());
    let cancel = CancellationToken::new();
    assert_eq!(
        loader.execute(&config, &source, &cancel).await,
        Err(ErrorClass::Forbidden)
    );
    assert!(calls.lock().unwrap().is_empty());
    config.certificate_secrets.push(key);
    config.profile = Profile::Minimal;
    assert_eq!(
        loader.execute(&config, &source, &cancel).await,
        Err(ErrorClass::Unsupported)
    );
    assert!(calls.lock().unwrap().is_empty());
    config.profile = Profile::Standard;
    let sample = loader.execute(&config, &source, &cancel).await.unwrap();
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["GET /api/v1/namespaces/app-db/secrets/public-cert"]
    );
    let mut store = support::seed();
    let mut scheduler = Scheduler::default();
    scheduler.reconcile(&config, Tick(0));
    let ticket = scheduler.admit(store.scope, Tick(0)).unwrap();
    assert!(store.begin_auxiliary(ticket.clone()));
    assert!(store.finish_auxiliary(ticket.clone(), Ok(sample), Tick(0)));
    assert!(!store.finish_auxiliary(ticket.clone(), Err(ErrorClass::InvalidData), Tick(1)));
    let (kube, _) = client(false);
    let denied = Loader::new(kube, Budgets::default())
        .execute(&config, &source, &cancel)
        .await;
    assert_eq!(denied, Err(ErrorClass::Forbidden));
    scheduler.finish(&ticket, Ok(()), Tick(0));
    scheduler.refresh(Tick(2000));
    let next = scheduler.admit(store.scope, Tick(2000)).unwrap();
    assert!(store.begin_auxiliary(next.clone()));
    assert!(store.finish_auxiliary(next, denied, Tick(2000)));
    assert!(store.auxiliary[&source].last_good.is_some());
    assert!(matches!(
        store.auxiliary[&source].freshness(Tick(2001), auxiliary::stale_after(&source)),
        Freshness::Stale(_)
    ));
    let text = serde_json::to_string(&store.auxiliary.values().collect::<Vec<_>>()).unwrap();
    for marker in [
        "PRIVATE_KEY_MARKER",
        "SECRET_DENIAL_PAYLOAD",
        "BEGIN CERTIFICATE",
    ] {
        assert!(!text.contains(marker));
    }
    store.switch_scope();
    assert!(!store.finish_auxiliary(ticket, Err(ErrorClass::Forbidden), Tick(3)));
}
#[tokio::test(start_paused = true)]
async fn scheduler_coalesces_observes_cadence_and_rejects_obsolete_completions() {
    let mut config = config();
    let mut scheduler = Scheduler::default();
    scheduler.reconcile(&config, Tick(0));
    assert!(scheduler.admit(ScopeEpoch(0), Tick(0)).is_none());
    config.operator_metrics_url = Some("http://127.0.0.1:9090/metrics".into());
    scheduler.reconcile(&config, Tick(0));
    let ticket = scheduler.admit(ScopeEpoch(1), Tick(0)).unwrap();
    assert!(scheduler.admit(ScopeEpoch(1), Tick(100000)).is_none());
    let mut obsolete = ticket.clone();
    obsolete.scope = ScopeEpoch(0);
    scheduler.finish(&obsolete, Ok(()), Tick(0));
    assert!(scheduler.accepts(&ticket));
    scheduler.finish(&ticket, Ok(()), Tick(1000));
    assert!(scheduler.admit(ScopeEpoch(1), Tick(30999)).is_none());
    assert!(scheduler.admit(ScopeEpoch(1), Tick(31000)).is_some());
}
#[tokio::test]
async fn metrics_transport_has_no_kubernetes_credentials_and_bounds_timeout_cancel_and_redirect() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    for (response, expected) in [
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 18\r\nConnection: close\r\n\r\nworkqueue_depth 3\n",
            None,
        ),
        (
            "HTTP/1.1 302 Found\r\nLocation: http://example.invalid/\r\nContent-Length: 0\r\n\r\n",
            Some(ErrorClass::Unavailable),
        ),
        (
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n",
            Some(ErrorClass::Forbidden),
        ),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let n = socket.read(&mut request).await.unwrap();
            let headers = String::from_utf8_lossy(&request[..n]).to_ascii_lowercase();
            assert!(!headers.contains("authorization") && !headers.contains("bearer"));
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let mut kube_config = kube::Config::new("http://127.0.0.1:1".parse().unwrap());
        kube_config.auth_info.token = Some(secrecy::SecretString::from(
            "DO_NOT_FORWARD_KUBERNETES_TOKEN",
        ));
        let kube = Client::try_from(kube_config).unwrap();
        let mut config = config();
        config.operator_metrics_url = Some(format!("http://{address}/metrics"));
        let loader = Loader::new(kube, Budgets::default());
        let result = loader
            .execute(&config, &AuxKey::Metrics, &CancellationToken::new())
            .await;
        if let Some(expected) = expected {
            assert_eq!(result, Err(expected));
        } else {
            assert!(matches!(result, Ok(Sample::Metrics(_))));
        }
        server.await.unwrap();
    }
    let (kube, _) = client(false);
    let loader = Loader::new(kube, Budgets::default());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert_eq!(
        loader.execute(&config(), &AuxKey::Metrics, &cancel).await,
        Err(ErrorClass::Cancelled)
    );
}
#[test]
fn certificate_rotation_expiry_and_quiet_clock_changes_invalidate_health_cache() {
    use cnpg_watch::health::Engine;
    use cnpg_watch::reducer::WatchUpdate;
    let mut store = support::seed();
    let key = cnpg_watch::model::Key::new("app-db", "server-cert").unwrap();
    let mut cluster = store.clusters().next().unwrap().clone();
    cluster.operational.certificate_secrets = vec![key.name.clone()];
    support::watch(
        &mut store,
        Kind::Cluster,
        WatchUpdate::Apply(Object::Cluster(cluster)),
    );
    let original = auxiliary::parse_certificate(
        include_bytes!("../fixtures/cnpg/1.30.0/server-public.crt"),
        Uid::new("original").unwrap(),
    )
    .unwrap();
    let source = AuxKey::Certificate(key);
    let ticket = auxiliary::Ticket {
        scope: store.scope,
        request: RequestId(1),
        key: source.clone(),
    };
    assert!(store.begin_auxiliary(ticket.clone()));
    assert!(store.finish_auxiliary(ticket, Ok(Sample::Certificate(original.clone())), Tick(0)));
    let mut cached = Engine::default();
    let mut full = Engine::default();
    let budgets = Budgets::default();
    for (utc, expired) in [
        (original.not_after_ms - 8 * 86400000, false),
        (original.not_after_ms + 1, true),
    ] {
        store.utc_ms = Some(utc);
        let reasons = cached.update(&store, Tick(1), &budgets);
        assert_eq!(reasons, full.recompute(&store, Tick(1), &budgets));
        assert_eq!(
            reasons
                .iter()
                .any(|r| r.id.starts_with("certificate.expired")),
            expired
        );
    }
    // A rotation replaces public metadata atomically, under the same configured Secret key.
    let rotated = auxiliary::Certificate {
        uid: original.uid.clone(),
        not_after_ms: original.not_after_ms + 30 * 86400000,
        fingerprint_sha256: "a".repeat(64),
        ..original
    };
    let ticket = auxiliary::Ticket {
        scope: store.scope,
        request: RequestId(2),
        key: source,
    };
    assert!(store.begin_auxiliary(ticket.clone()));
    assert!(store.finish_auxiliary(ticket, Ok(Sample::Certificate(rotated)), Tick(2)));
    assert!(
        !cached
            .update(&store, Tick(2), &budgets)
            .iter()
            .any(|r| r.id.starts_with("certificate.expired"))
    );
    assert_eq!(
        cached.update(&store, Tick(2), &budgets),
        full.recompute(&store, Tick(2), &budgets)
    );
    assert!(
        !cached
            .update(&store, Tick(180003), &budgets)
            .iter()
            .any(|r| r.id.starts_with("certificate.expired"))
    );
}
#[tokio::test]
async fn hanging_metrics_obey_deadline_and_cancel() {
    use tokio::{io::AsyncReadExt, net::TcpListener};
    for cancel_early in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 4096];
            let _ = socket.read(&mut bytes).await;
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        });
        let (kube, _) = client(false);
        let mut config = config();
        config.operator_metrics_url = Some(format!("http://{address}/metrics"));
        let budgets = Budgets {
            diagnostic_timeout_ms: 100,
            ..Default::default()
        };
        let loader = Loader::new(kube, budgets);
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let cancelling = tokio::spawn(async move {
            if cancel_early {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                token.cancel();
            }
        });
        let result = loader.execute(&config, &AuxKey::Metrics, &cancel).await;
        assert_eq!(
            result,
            Err(if cancel_early {
                ErrorClass::Cancelled
            } else {
                ErrorClass::Timeout
            })
        );
        cancelling.await.unwrap();
        server.abort();
        let _ = server.await;
    }
}
