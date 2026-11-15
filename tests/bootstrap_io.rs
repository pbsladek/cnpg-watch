use clap::Parser;
use cnpg_watch::{
    cli::{Cli, Command, Setup},
    config::{Config, Paths},
    setup::rbac,
};
use http::{Request, Response};
use kube::{Client, client::Body};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
#[derive(Default)]
struct Server {
    objects: BTreeMap<String, Value>,
    requests: Vec<(String, String)>,
}
fn server() -> (Client, Arc<Mutex<Server>>) {
    let state = Arc::new(Mutex::new(Server::default()));
    let captured = state.clone();
    let service = tower::service_fn(move |request: Request<Body>| {
        let state = captured.clone();
        async move {
            let method = request.method().as_str().to_string();
            let path = request.uri().path().to_string();
            let bytes = request.into_body().collect_bytes().await.unwrap();
            let mut s = state.lock().unwrap();
            s.requests.push((method.clone(), path.clone()));
            let (code,value)=match method.as_str() {
                "GET"=>s.objects.get(&path).cloned().map(|v|(200,v)).unwrap_or((404,json!({"apiVersion":"v1","kind":"Status","status":"Failure","reason":"NotFound","code":404,"message":"missing"}))),
                "POST"=>{let mut v:Value=serde_json::from_slice(&bytes).unwrap();v["metadata"]["resourceVersion"]="1".into();s.objects.insert(format!("{path}/{}",v["metadata"]["name"].as_str().unwrap()),v.clone());(201,v)},
                "PATCH"=>{let patch:Value=serde_json::from_slice(&bytes).unwrap();let live=s.objects.get_mut(&path).unwrap();for (key,value) in patch.as_object().unwrap(){if key=="metadata"{for (k,v) in value.as_object().unwrap(){live["metadata"][k]=v.clone();}}else{live[key]=value.clone();}}(200,live.clone())},
                _=>panic!("unexpected mutation {method}"),
            };
            Ok::<_, std::convert::Infallible>(
                Response::builder()
                    .status(code)
                    .body(Body::from(serde_json::to_vec(&value).unwrap()))
                    .unwrap(),
            )
        }
    });
    (Client::new(service, "app-db"), state)
}
fn desired() -> Vec<rbac::Manifest> {
    let cli = Cli::try_parse_from([
        "cnpg-watch",
        "--namespace",
        "app-db",
        "setup",
        "kubernetes-rbac",
        "--user",
        "reader",
    ])
    .unwrap();
    let config = Config::resolve(cli.options, Paths::discover().unwrap()).unwrap();
    let Some(Command::Setup {
        command: Setup::KubernetesRbac(args),
    }) = cli.command
    else {
        unreachable!()
    };
    rbac::generate(&config, &args).unwrap()
}
#[tokio::test]
async fn diff_is_read_only_apply_is_idempotent_and_extras_require_explicit_prune() {
    let (client, server) = server();
    let desired = desired();
    let before = rbac::reconcile(client.clone(), desired.clone(), false, false)
        .await
        .unwrap();
    assert_eq!(before.missing.len(), 2);
    assert!(
        server
            .lock()
            .unwrap()
            .requests
            .iter()
            .all(|(m, _)| m == "GET")
    );
    rbac::reconcile(client.clone(), desired.clone(), true, false)
        .await
        .unwrap();
    server.lock().unwrap().requests.clear();
    let second = rbac::reconcile(client.clone(), desired.clone(), true, false)
        .await
        .unwrap();
    assert!(!second.would_change);
    assert_eq!(second.matching.len(), 2);
    assert!(
        server
            .lock()
            .unwrap()
            .requests
            .iter()
            .all(|(m, _)| m == "GET")
    );
    let role_path = {
        let s = server.lock().unwrap();
        s.objects
            .keys()
            .find(|p| p.contains("/roles/"))
            .unwrap()
            .clone()
    };
    server.lock().unwrap().objects.get_mut(&role_path).unwrap()["rules"]
        .as_array_mut()
        .unwrap()
        .push(json!({"apiGroups":[""],"resources":["configmaps"],"verbs":["get"]}));
    let kept = rbac::reconcile(client.clone(), desired.clone(), true, false)
        .await
        .unwrap();
    assert!(!kept.would_change);
    assert_eq!(kept.extra_managed_rules.len(), 1);
    let pruned = rbac::reconcile(client.clone(), desired.clone(), true, true)
        .await
        .unwrap();
    assert!(pruned.would_change);
    let final_diff = rbac::reconcile(client, desired, false, false)
        .await
        .unwrap();
    assert!(!final_diff.would_change && final_diff.extra_managed_rules.is_empty());
}
#[tokio::test]
async fn unmanaged_name_collision_fails_before_any_write() {
    let (client, server) = server();
    let desired = desired();
    let mut binding = serde_json::to_value(&desired[1]).unwrap();
    binding["metadata"]["labels"] = json!({"owner":"someone-else"});
    server.lock().unwrap().objects.insert(
        "/apis/rbac.authorization.k8s.io/v1/namespaces/app-db/rolebindings/cnpg-watch".into(),
        binding,
    );
    assert!(rbac::reconcile(client, desired, true, false).await.is_err());
    assert!(
        server
            .lock()
            .unwrap()
            .requests
            .iter()
            .all(|(m, _)| m == "GET")
    );
}

#[tokio::test]
async fn apiserver_subject_defaulting_does_not_trigger_an_update() {
    let (client, server) = server();
    let mut desired = desired();
    desired[1].subjects.as_mut().unwrap()[0] = k8s_openapi::api::rbac::v1::Subject {
        kind: "ServiceAccount".into(),
        name: "reader".into(),
        namespace: Some("app-db".into()),
        api_group: Some("".into()),
    };
    for object in &desired {
        let mut value = serde_json::to_value(object).unwrap();
        if object.kind == "RoleBinding" {
            value["subjects"][0]
                .as_object_mut()
                .unwrap()
                .remove("apiGroup");
        }
        let plural = if object.kind == "Role" {
            "roles"
        } else {
            "rolebindings"
        };
        server.lock().unwrap().objects.insert(
            format!("/apis/rbac.authorization.k8s.io/v1/namespaces/app-db/{plural}/cnpg-watch"),
            value,
        );
    }
    let report = rbac::reconcile(client, desired, false, false)
        .await
        .unwrap();
    assert!(!report.would_change);
    assert!(report.changed.is_empty());
}
