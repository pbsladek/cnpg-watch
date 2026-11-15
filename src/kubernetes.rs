//! Shared client and rate admission for HTTP request establishment, not stream frames.
use crate::{budget::Budgets, config::Config, model::ErrorClass};
use kube::{Client, client::ClientBuilder, config::KubeConfigOptions};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{sync::Mutex, time::Instant};
use tower::{Layer, Service};

struct Tokens {
    at: Instant,
    available: f64,
}
#[derive(Clone)]
pub struct RateGate {
    state: Arc<Mutex<Tokens>>,
    qps: f64,
    burst: f64,
}
impl RateGate {
    pub fn new(qps: u32, burst: u32) -> Self {
        Self {
            state: Arc::new(Mutex::new(Tokens {
                at: Instant::now(),
                available: f64::from(burst),
            })),
            qps: qps.into(),
            burst: burst.into(),
        }
    }
    pub async fn acquire(&self) {
        loop {
            let wait = {
                let mut state = self.state.lock().await;
                let now = Instant::now();
                state.available = (state.available
                    + now.duration_since(state.at).as_secs_f64() * self.qps)
                    .min(self.burst);
                state.at = now;
                if state.available >= 1.0 {
                    state.available -= 1.0;
                    return;
                }
                Duration::from_secs_f64((1.0 - state.available) / self.qps)
            };
            tokio::time::sleep(wait).await;
        }
    }
}
#[derive(Clone)]
struct RateLayer(RateGate);
impl<S> Layer<S> for RateLayer {
    type Service = RateService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        RateService {
            inner,
            gate: self.0.clone(),
        }
    }
}
struct RateService<S> {
    inner: S,
    gate: RateGate,
}
impl<S, R> Service<R> for RateService<S>
where
    S: Service<R>,
    S::Future: Send + 'static,
    S::Response: 'static,
    S::Error: 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, request: R) -> Self::Future {
        let future = self.inner.call(request);
        let gate = self.gate.clone();
        Box::pin(async move {
            gate.acquire().await;
            future.await
        })
    }
}

pub async fn client(config: &Config, budgets: &Budgets) -> Result<(Client, String), ErrorClass> {
    let options = KubeConfigOptions {
        context: config.context.clone(),
        ..Default::default()
    };
    let raw = if config.context.is_some() {
        kube::Config::from_kubeconfig(&options)
            .await
            .map_err(|_| ErrorClass::Authentication)?
    } else {
        kube::Config::infer()
            .await
            .map_err(|_| ErrorClass::Authentication)?
    };
    let namespace = config
        .namespace
        .clone()
        .unwrap_or_else(|| raw.default_namespace.clone());
    Ok((configured_client(raw, budgets)?, namespace))
}
/// Build one shared limited client, including explicit live-test/runtime identities.
pub fn configured_client(mut raw: kube::Config, budgets: &Budgets) -> Result<Client, ErrorClass> {
    raw.connect_timeout = Some(Duration::from_secs(2));
    raw.read_timeout = Some(Duration::from_secs(35));
    raw.write_timeout = Some(Duration::from_secs(2));
    Ok(ClientBuilder::try_from(raw)
        .map_err(|_| ErrorClass::Transport)?
        .with_layer(&RateLayer(RateGate::new(
            budgets.api_qps,
            budgets.api_burst,
        )))
        .build())
}

/// Resolve an explicitly named primary without discovery or CRD-object access.
pub async fn primary(
    client: Client,
    namespace: &str,
    cluster: &str,
) -> Result<crate::model::Key, ErrorClass> {
    use kube::{
        Api,
        api::DynamicObject,
        core::{ApiResource, GroupVersionKind},
    };
    let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(
        "postgresql.cnpg.io",
        "v1",
        "Cluster",
    ));
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), namespace, &resource);
    let object = api
        .get(cluster)
        .await
        .map_err(|e| crate::probes::api_error(&e))?;
    let name = object
        .data
        .pointer("/status/currentPrimary")
        .and_then(|v| v.as_str())
        .ok_or(ErrorClass::Unavailable)?;
    let key = crate::model::Key::new(namespace, name).map_err(|_| ErrorClass::InvalidData)?;
    let pods: Api<k8s_openapi::api::core::v1::Pod> = Api::namespaced(client, namespace);
    let pod = pods
        .get(name)
        .await
        .map_err(|e| crate::probes::api_error(&e))?;
    if pod
        .metadata
        .labels
        .as_ref()
        .and_then(|l| l.get("cnpg.io/cluster"))
        .map(String::as_str)
        != Some(cluster)
    {
        return Err(ErrorClass::InvalidData);
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn bucket_bounds_establishment_but_not_stream_messages() {
        let gate = RateGate::new(5, 10);
        for _ in 0..10 {
            gate.acquire().await;
        }
        let start = Instant::now();
        gate.acquire().await;
        assert!(start.elapsed() >= Duration::from_millis(200));
    }
}
