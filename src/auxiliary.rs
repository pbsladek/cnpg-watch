//! Explicit named-certificate reads and isolated optional metrics scrapes.
use crate::{budget::Budgets, config::Config, model::*};
use futures::StreamExt;
use k8s_openapi::api::core::v1::Secret;
use kube::{Api, Client};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Key {
    Certificate(crate::model::Key),
    Metrics,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ticket {
    pub scope: ScopeEpoch,
    pub request: RequestId,
    pub key: Key,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Certificate {
    pub uid: Uid,
    pub not_before_ms: i64,
    pub not_after_ms: i64,
    pub fingerprint_sha256: String,
    pub chain_length: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Metric {
    Reconciles,
    ReconcileErrors,
    ActiveWorkers,
    QueueDepth,
    Retries,
}
impl Metric {
    pub fn name(self) -> &'static str {
        match self {
            Self::Reconciles => "controller_runtime_reconcile_total",
            Self::ReconcileErrors => "controller_runtime_reconcile_errors_total",
            Self::ActiveWorkers => "controller_runtime_active_workers",
            Self::QueueDepth => "workqueue_depth",
            Self::Retries => "workqueue_retries_total",
        }
    }
    fn from_name(name: &str) -> Option<Self> {
        [
            Self::Reconciles,
            Self::ReconcileErrors,
            Self::ActiveWorkers,
            Self::QueueDepth,
            Self::Retries,
        ]
        .into_iter()
        .find(|m| m.name() == name)
    }
}
/// Only finite, nonnegative monitoring values can enter the domain or its hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Value(u64);
impl Value {
    pub fn new(value: f64) -> Result<Self, ErrorClass> {
        if !value.is_finite() || value < 0. {
            return Err(ErrorClass::InvalidData);
        }
        Ok(Self(if value == 0. {
            0.0_f64.to_bits()
        } else {
            value.to_bits()
        }))
    }
    pub fn get(self) -> f64 {
        f64::from_bits(self.0)
    }
}
impl Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(self.get())
    }
}
impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = f64::deserialize(d)?;
        Self::new(value)
            .map_err(|_| serde::de::Error::custom("non-finite or negative monitoring value"))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sample {
    Certificate(Certificate),
    Metrics(BTreeMap<Metric, Value>),
}
impl Sample {
    pub fn bytes(&self) -> usize {
        1024 + match self {
            Self::Certificate(c) => c.fingerprint_sha256.capacity(),
            Self::Metrics(m) => m.len() * 128,
        }
    }
}

pub fn parse_certificate(bytes: &[u8], uid: Uid) -> Result<Certificate, ErrorClass> {
    if bytes.len() > 64 * 1024 {
        return Err(ErrorClass::OutputLimit);
    }
    let mut chain = Vec::new();
    if bytes.starts_with(b"-----BEGIN") {
        for pem in x509_parser::pem::Pem::iter_from_buffer(bytes) {
            let pem = pem.map_err(|_| ErrorClass::InvalidData)?;
            if pem.label != "CERTIFICATE" || chain.len() >= 16 {
                return Err(ErrorClass::InvalidData);
            }
            chain.push(pem.contents);
        }
    } else {
        chain.push(bytes.to_vec());
    }
    let mut leaf = None;
    for der in &chain {
        let (remaining, cert) =
            x509_parser::parse_x509_certificate(der).map_err(|_| ErrorClass::InvalidData)?;
        if !remaining.is_empty() {
            return Err(ErrorClass::InvalidData);
        }
        if leaf.is_none() {
            leaf = Some((
                cert.validity().not_before.timestamp(),
                cert.validity().not_after.timestamp(),
                format!("{:x}", Sha256::digest(der)),
            ));
        }
    }
    let (before, after, fingerprint) = leaf.ok_or(ErrorClass::InvalidData)?;
    if before > after {
        return Err(ErrorClass::InvalidData);
    }
    Ok(Certificate {
        uid,
        not_before_ms: before.checked_mul(1000).ok_or(ErrorClass::InvalidData)?,
        not_after_ms: after.checked_mul(1000).ok_or(ErrorClass::InvalidData)?,
        fingerprint_sha256: fingerprint,
        chain_length: chain.len(),
    })
}
fn labels_valid(labels: &str) -> bool {
    let bytes = labels.as_bytes();
    let mut i = 0;
    let mut count = 0;
    let mut keys = std::collections::BTreeSet::new();
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i == bytes.len() {
            return true;
        }
        count += 1;
        if count > 32 {
            return false;
        }
        if !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
            return false;
        }
        let start = i;
        i += 1;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        if !keys.insert(&labels[start..i]) {
            return false;
        }
        if bytes.get(i) != Some(&b'=') {
            return false;
        }
        i += 1;
        if bytes.get(i) != Some(&b'"') {
            return false;
        }
        i += 1;
        loop {
            let Some(b) = bytes.get(i) else { return false };
            i += 1;
            match *b {
                b'"' => break,
                b'\\' => {
                    if !bytes
                        .get(i)
                        .is_some_and(|b| [b'\\', b'"', b'n'].contains(b))
                    {
                        return false;
                    }
                    i += 1;
                }
                b if b.is_ascii_control() => return false,
                _ => {}
            }
        }
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i == bytes.len() {
            return true;
        }
        if bytes[i] != b',' {
            return false;
        }
        i += 1;
    }
    true
}
pub fn parse_metrics(bytes: &[u8]) -> Result<BTreeMap<Metric, Value>, ErrorClass> {
    if bytes.len() > 256 * 1024 {
        return Err(ErrorClass::OutputLimit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ErrorClass::InvalidData)?;
    let mut values = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if index >= 4096 || line.len() > 16 * 1024 {
            return Err(ErrorClass::OutputLimit);
        }
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let end = line
            .find(|c: char| c == '{' || c.is_ascii_whitespace())
            .unwrap_or(line.len());
        let Some(metric) = Metric::from_name(&line[..end]) else {
            continue;
        };
        let rest = if line[end..].starts_with('{') {
            let close = line.rfind('}').ok_or(ErrorClass::InvalidData)?;
            if close < end || !labels_valid(&line[end + 1..close]) {
                return Err(ErrorClass::InvalidData);
            }
            &line[close + 1..]
        } else {
            &line[end..]
        };
        let mut fields = rest.split_ascii_whitespace();
        let value = Value::new(
            fields
                .next()
                .ok_or(ErrorClass::InvalidData)?
                .parse()
                .map_err(|_| ErrorClass::InvalidData)?,
        )?;
        if let Some(timestamp) = fields.next() {
            timestamp
                .parse::<i64>()
                .map_err(|_| ErrorClass::InvalidData)?;
        }
        if fields.next().is_some() {
            return Err(ErrorClass::InvalidData);
        }
        let previous = values.get(&metric).map_or(0., |v: &Value| v.get());
        values.insert(metric, Value::new(previous + value.get())?);
    }
    if values.is_empty() {
        return Err(ErrorClass::Unsupported);
    }
    Ok(values)
}

#[derive(Clone)]
pub struct Loader {
    kubernetes: Client,
    http: std::sync::Arc<std::sync::OnceLock<Result<reqwest::Client, ErrorClass>>>,
    budgets: Budgets,
}
impl Loader {
    pub fn new(kubernetes: Client, budgets: Budgets) -> Self {
        Self {
            kubernetes,
            http: Default::default(),
            budgets,
        }
    }
    pub async fn execute(
        &self,
        config: &Config,
        key: &Key,
        cancel: &CancellationToken,
    ) -> Result<Sample, ErrorClass> {
        if cancel.is_cancelled() {
            return Err(ErrorClass::Cancelled);
        }
        if config.profile == crate::cli::Profile::Minimal {
            return Err(ErrorClass::Unsupported);
        }
        if let Key::Certificate(key) = key
            && !config.certificate_secrets.contains(key)
        {
            return Err(ErrorClass::Forbidden);
        }
        if *key == Key::Metrics && !config.operator_watch {
            return Err(ErrorClass::Unavailable);
        }
        let operation = async {
            match key {
                Key::Certificate(key) => {
                    let api: Api<Secret> = Api::namespaced(self.kubernetes.clone(), &key.namespace);
                    let mut raw = api
                        .get(&key.name)
                        .await
                        .map_err(|e| crate::probes::api_error(&e))?;
                    let uid = Uid::new(raw.metadata.uid.take().ok_or(ErrorClass::InvalidData)?)
                        .map_err(|_| ErrorClass::InvalidData)?;
                    let certificate = raw
                        .data
                        .as_mut()
                        .and_then(|data| data.remove("tls.crt").or_else(|| data.remove("ca.crt")))
                        .ok_or(ErrorClass::InvalidData)?;
                    drop(raw); // Discard every other field, including any private key, before parsing.
                    parse_certificate(&certificate.0, uid).map(Sample::Certificate)
                }
                Key::Metrics => {
                    let url = config
                        .operator_metrics_url
                        .as_deref()
                        .ok_or(ErrorClass::Unavailable)?;
                    let http = self
                        .http
                        .get_or_init(|| {
                            reqwest::Client::builder()
                                .no_proxy()
                                .redirect(reqwest::redirect::Policy::none())
                                .connect_timeout(Duration::from_millis(750))
                                .timeout(Duration::from_millis(self.budgets.diagnostic_timeout_ms))
                                .build()
                                .map_err(|_| ErrorClass::Transport)
                        })
                        .as_ref()
                        .map_err(|e| *e)?;
                    let response = http
                        .get(url)
                        .header(reqwest::header::ACCEPT, "text/plain; version=0.0.4")
                        .send()
                        .await
                        .map_err(|e| {
                            if e.is_timeout() {
                                ErrorClass::Timeout
                            } else {
                                ErrorClass::Transport
                            }
                        })?;
                    match response.status().as_u16() {
                        200 => {}
                        401 => return Err(ErrorClass::Authentication),
                        403 => return Err(ErrorClass::Forbidden),
                        429 => return Err(ErrorClass::Overloaded),
                        _ => return Err(ErrorClass::Unavailable),
                    }
                    let limit = self.budgets.stdout_bytes.min(256 * 1024);
                    if response.content_length().is_some_and(|n| n > limit as u64) {
                        return Err(ErrorClass::OutputLimit);
                    }
                    let mut stream = response.bytes_stream();
                    let mut body = Vec::new();
                    while let Some(chunk) = stream.next().await {
                        let chunk = chunk.map_err(|_| ErrorClass::Transport)?;
                        if body.len().saturating_add(chunk.len()) > limit {
                            return Err(ErrorClass::OutputLimit);
                        }
                        body.extend_from_slice(&chunk);
                    }
                    parse_metrics(&body).map(Sample::Metrics)
                }
            }
        };
        tokio::select! {
            _=cancel.cancelled()=>Err(ErrorClass::Cancelled),
            result=tokio::time::timeout(Duration::from_millis(self.budgets.diagnostic_timeout_ms),operation)=>result.unwrap_or(Err(ErrorClass::Timeout)),
        }
    }
}
#[derive(Default)]
pub struct Scheduler {
    next: BTreeMap<Key, Tick>,
    request: u64,
    pub active: Option<Ticket>,
    last_refresh: Tick,
}
impl Scheduler {
    pub fn reconcile(&mut self, config: &Config, now: Tick) {
        let mut desired = Vec::new();
        if config.profile != crate::cli::Profile::Minimal {
            desired.extend(
                config
                    .certificate_secrets
                    .iter()
                    .cloned()
                    .map(Key::Certificate),
            );
            if config.operator_watch && config.operator_metrics_url.is_some() {
                desired.push(Key::Metrics);
            }
        }
        self.next.retain(|key, _| desired.contains(key));
        for key in desired {
            self.next.entry(key).or_insert(now);
        }
    }
    pub fn admit(&mut self, scope: ScopeEpoch, now: Tick) -> Option<Ticket> {
        if self.active.is_some() {
            return None;
        }
        let key = self
            .next
            .iter()
            .filter(|(_, at)| **at <= now)
            .min_by_key(|(key, at)| (**at, *key))
            .map(|(key, _)| key.clone())?;
        self.request = self
            .request
            .checked_add(1)
            .expect("auxiliary request ID exhausted");
        let ticket = Ticket {
            scope,
            request: RequestId(self.request),
            key,
        };
        self.active = Some(ticket.clone());
        Some(ticket)
    }
    pub fn accepts(&self, ticket: &Ticket) -> bool {
        self.active.as_ref() == Some(ticket) && self.next.contains_key(&ticket.key)
    }
    pub fn finish(&mut self, ticket: &Ticket, result: Result<(), ErrorClass>, now: Tick) {
        if self.active.as_ref() != Some(ticket) {
            return;
        }
        self.active = None;
        let interval = if result.is_err() {
            30_000
        } else {
            match ticket.key {
                Key::Metrics => 30_000,
                Key::Certificate(_) => 60_000,
            }
        };
        if let Some(next) = self.next.get_mut(&ticket.key) {
            *next = now.after(Duration::from_millis(interval));
        }
    }
    pub fn refresh(&mut self, now: Tick) {
        if now.age(self.last_refresh) < Duration::from_secs(2) {
            return;
        }
        self.last_refresh = now;
        for time in self.next.values_mut() {
            *time = now;
        }
    }
}
pub fn stale_after(key: &Key) -> Duration {
    Duration::from_secs(match key {
        Key::Metrics => 90,
        Key::Certificate(_) => 180,
    })
}
