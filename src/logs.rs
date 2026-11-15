//! Selected streams use capped framing before parsing, redaction, and retention.
use crate::{budget::Budgets, cli::Profile, model::*, queue};
use futures::AsyncReadExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client, api::LogParams};
use regex::Regex;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;

pub mod events;
pub mod timeline;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum Channel {
    Postgres,
    Pooler,
    Backup,
    Operator,
    Events,
    OperatorEvents,
}
impl Channel {
    pub fn stream(self) -> bool {
        !matches!(self, Self::Events | Self::OperatorEvents)
    }
    pub fn operator(self) -> bool {
        matches!(self, Self::Operator | Self::OperatorEvents)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Context {
    pub target: Option<Key>,
    pub kind: Option<Kind>,
    pub uid: Option<Uid>,
    pub reconcile: Option<[u8; 16]>,
}
impl Context {
    fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + 256
            + self
                .target
                .as_ref()
                .map_or(0, |k| k.name.capacity() + k.namespace.capacity())
    }
}
fn context(json: &serde_json::Value) -> Option<Box<Context>> {
    let cluster = json.get("cluster").filter(|v| v.is_object());
    let namespace = cluster
        .and_then(|v| v.get("namespace"))
        .or_else(|| json.get("namespace"))
        .and_then(|v| v.as_str());
    let name = cluster
        .and_then(|v| v.get("name"))
        .or_else(|| json.get("name"))
        .and_then(|v| v.as_str());
    let target = namespace
        .zip(name)
        .and_then(|(ns, name)| Key::new(ns, name).ok());
    let kind = if cluster.is_some() {
        Some(Kind::Cluster)
    } else {
        json.get("controllerKind")
            .or_else(|| json.get("kind"))
            .and_then(|v| v.as_str())
            .and_then(|v| match v {
                "Cluster" => Some(Kind::Cluster),
                "Pod" => Some(Kind::Pod),
                "Backup" => Some(Kind::Backup),
                "Pooler" => Some(Kind::Pooler),
                "ScheduledBackup" => Some(Kind::ScheduledBackup),
                "Database" => Some(Kind::Database),
                "DatabaseRole" => Some(Kind::DatabaseRole),
                "Publication" => Some(Kind::Publication),
                "Subscription" => Some(Kind::Subscription),
                _ => None,
            })
    };
    let uid = cluster
        .and_then(|v| v.get("uid"))
        .or_else(|| json.get("uid"))
        .and_then(|v| v.as_str())
        .and_then(|v| Uid::new(v).ok());
    let reconcile = json
        .get("reconcileID")
        .and_then(|v| v.as_str())
        .filter(|v| v.len() <= 128 && !v.chars().any(char::is_control))
        .map(|v| {
            use sha2::Digest;
            let digest = sha2::Sha256::digest(v.as_bytes());
            let mut key = [0; 16];
            key.copy_from_slice(&digest[..16]);
            key
        });
    if target.is_none() && reconcile.is_none() {
        None
    } else {
        Some(Box::new(Context {
            target,
            kind,
            uid,
            reconcile,
        }))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Source {
    pub channel: Channel,
    pub scope: ScopeEpoch,
    pub key: Key,
    pub uid: Uid,
    pub container: String,
}
impl Source {
    pub fn identity(&self) -> Identity {
        Identity {
            key: self.key.clone(),
            uid: self.uid.clone(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Level {
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
    Unknown,
}
#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub sequence: u64,
    pub timestamp: Option<String>,
    pub level: Level,
    pub text: String,
    pub truncated: bool,
    pub context: Option<Box<Context>>,
}
impl Line {
    pub fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + 128
            + self.context.as_ref().map_or(0, |c| c.bytes())
            + self.text.capacity()
            + self.timestamp.as_ref().map_or(0, String::capacity)
    }
}
#[derive(Debug, Clone)]
pub enum Update {
    Lines(Vec<Line>),
    State(SyncState),
}
#[derive(Debug, Clone)]
pub struct Message {
    pub source: Source,
    pub generation: u64,
    pub update: Update,
}
pub struct Framer {
    prefix: Vec<u8>,
    limit: usize,
    overflow: bool,
}
impl Framer {
    pub fn new(limit: usize) -> Self {
        Self {
            prefix: Vec::with_capacity(limit.min(4096)),
            limit,
            overflow: false,
        }
    }
    pub fn push(&mut self, bytes: &[u8], mut line: impl FnMut(&[u8], bool)) {
        for &byte in bytes {
            if byte == b'\n' {
                line(&self.prefix, self.overflow);
                self.prefix.clear();
                self.overflow = false;
            } else if self.prefix.len() < self.limit {
                self.prefix.push(byte);
            } else {
                self.overflow = true;
            }
        }
    }
    pub fn finish(&mut self, mut line: impl FnMut(&[u8], bool)) {
        if !self.prefix.is_empty() || self.overflow {
            line(&self.prefix, self.overflow);
            self.prefix.clear();
            self.overflow = false;
        }
    }
    pub fn retained(&self) -> usize {
        self.prefix.len()
    }
}
fn patterns() -> &'static [Regex] {
    static REGEX: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEX.get_or_init(||[
        r#"(?i)(?:postgres(?:ql)?|https?)://[^\s"<>]+"#,
        r"(?i)bearer\s+[^\s,;]+",
        r#"(?i)(?:password|passwd|token|secret|api_key|authorization)["']?\s*[:=]\s*(?:\"[^\"]*\"|'[^']*'|[^\s,;]+)"#,
        r"(?i)\bpassword\s+'(?:''|[^'])*'",
        r"\b(?:\d{1,3}\.){3}\d{1,3}(?::\d+)?\b",
        r"(?i)(?:\b(?:[0-9a-f]{1,4}:){2,}[0-9a-f:]+|::[0-9a-f:]+)",
        r#"(?i)(?:statement|query|parameters|detail|execute[^:]*?)["']?\s*[:=].*"#,
    ].iter().map(|p|Regex::new(p).expect("static redaction pattern")).collect())
}
pub fn redact(value: &str, profile: Profile) -> String {
    let mut text: String = value
        .chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .take(16 * 1024)
        .collect();
    for regex in patterns() {
        if regex.is_match(&text) {
            text = regex.replace_all(&text, "[REDACTED]").into_owned();
        }
    }
    if profile == Profile::Secure {
        static STRICT: OnceLock<Regex> = OnceLock::new();
        text=STRICT.get_or_init(||Regex::new(r#"(?i)(?:user(?:name)?|database|application(?:_name)?|client|path)["']?\s*[:=]\s*(?:\"[^\"]*\"|'[^']*'|[^\s,;]+)"#).unwrap()).replace_all(&text,"[REDACTED]").into_owned();
    }
    text
}
pub fn parse(bytes: &[u8], truncated: bool, profile: Profile, sequence: u64) -> Line {
    if truncated {
        return Line {
            sequence,
            timestamp: None,
            level: Level::Unknown,
            text: "[oversized log line omitted]".into(),
            truncated: true,
            context: None,
        };
    }
    let raw = String::from_utf8_lossy(bytes);
    let (timestamp, body) = raw
        .split_once(' ')
        .filter(|(t, _)| chrono::DateTime::parse_from_rfc3339(t).is_ok())
        .map_or((None, raw.as_ref()), |(t, b)| (Some(t.to_owned()), b));
    // Only allowlisted JSON fields enter the canonical record. Query/detail/user/address fields do not.
    let json = serde_json::from_str::<serde_json::Value>(body).ok();
    let level = json
        .as_ref()
        .and_then(|j| {
            j.pointer("/record/error_severity")
                .or_else(|| j.get("level"))
        })
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_ascii_lowercase();
    let level = match level.as_str() {
        "debug" | "debug1" | "debug2" => Level::Debug,
        "info" | "log" | "notice" => Level::Info,
        "warn" | "warning" => Level::Warning,
        "error" => Level::Error,
        "fatal" | "panic" => Level::Fatal,
        _ => Level::Unknown,
    };
    let message = json
        .as_ref()
        .and_then(|j| {
            j.pointer("/record/message")
                .or_else(|| j.get("msg"))
                .or_else(|| j.get("message"))
        })
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            if json.is_some() {
                "[structured fields omitted]"
            } else {
                body
            }
        });
    let timestamp = timestamp.or_else(|| {
        json.as_ref()
            .and_then(|j| j.get("ts").or_else(|| j.get("timestamp")))
            .and_then(|v| v.as_str())
            .filter(|t| t.len() <= 64 && chrono::DateTime::parse_from_rfc3339(t).is_ok())
            .map(str::to_owned)
    });
    Line {
        sequence,
        timestamp,
        level,
        text: redact(message, profile),
        truncated,
        context: json.as_ref().and_then(context),
    }
}
#[derive(Debug)]
pub struct Retained {
    pub generation: u64,
    pub active: bool,
    pub last_active: Tick,
    pub state: SyncState,
    pub lines: VecDeque<Line>,
    pub bytes: usize,
    pub evicted: u64,
}
#[derive(Default)]
pub struct Book {
    pub sources: BTreeMap<Source, Retained>,
    pub rejected: u64,
    pub truncated: u64,
    pub bytes: usize,
    pub high_bytes: usize,
    pub revision: u64,
    sequence: u64,
}
impl Book {
    pub fn activate(&mut self, source: Source, generation: u64, now: Tick) {
        let slot = self.sources.entry(source).or_insert(Retained {
            generation,
            active: true,
            last_active: now,
            state: SyncState::Starting,
            lines: VecDeque::new(),
            bytes: 0,
            evicted: 0,
        });
        if slot.generation != generation {
            self.bytes = self.bytes.saturating_sub(slot.bytes);
            slot.lines.clear();
            slot.bytes = 0;
            slot.state = SyncState::Starting;
        }
        slot.generation = generation;
        slot.active = true;
        slot.last_active = now;
        self.revision += 1;
    }
    pub fn retain_active(&mut self, active: &BTreeSet<Source>, now: Tick) {
        for (key, s) in &mut self.sources {
            if s.active && !active.contains(key) {
                s.active = false;
                s.last_active = now;
            }
        }
    }
    pub fn accept(&mut self, message: Message, now: Tick, budgets: &Budgets) {
        let Some(slot) = self
            .sources
            .get_mut(&message.source)
            .filter(|s| s.active && s.generation == message.generation)
        else {
            self.rejected += 1;
            return;
        };
        slot.last_active = now;
        match message.update {
            Update::State(state) => slot.state = state,
            Update::Lines(lines) => {
                slot.state = SyncState::Synced;
                for mut line in lines {
                    self.sequence = self
                        .sequence
                        .checked_add(1)
                        .expect("canonical log sequence exhausted");
                    line.sequence = self.sequence;
                    if line.truncated {
                        self.truncated += 1;
                    }
                    let bytes = line.bytes();
                    if bytes > budgets.log_source_bytes {
                        slot.evicted += 1;
                        continue;
                    }
                    slot.bytes += bytes;
                    self.bytes += bytes;
                    slot.lines.push_back(line);
                    while slot.bytes > budgets.log_source_bytes {
                        let line = slot.lines.pop_front().unwrap();
                        slot.bytes -= line.bytes();
                        self.bytes -= line.bytes();
                        slot.evicted += 1;
                    }
                }
            }
        }
        self.revision += 1;
        self.enforce(now, budgets);
    }
    pub fn enforce(&mut self, now: Tick, budgets: &Budgets) {
        let expired: Vec<_> = self
            .sources
            .iter()
            .filter(|(_, s)| {
                !s.active && now.0.saturating_sub(s.last_active.0) >= budgets.log_idle_ms
            })
            .map(|(k, _)| k.clone())
            .collect();
        for key in expired {
            self.bytes = self
                .bytes
                .saturating_sub(self.sources.remove(&key).unwrap().bytes);
            self.revision += 1;
        }
        while self.sources.len() > budgets.log_streams * 2 {
            let key = self
                .sources
                .iter()
                .filter(|(_, s)| !s.active)
                .min_by_key(|(_, s)| s.last_active)
                .map(|(k, _)| k.clone());
            if let Some(key) = key {
                self.bytes = self
                    .bytes
                    .saturating_sub(self.sources.remove(&key).unwrap().bytes);
            } else {
                break;
            }
        }
        while self.bytes > budgets.log_total_bytes {
            let oldest = self
                .sources
                .iter()
                .filter_map(|(k, s)| s.lines.front().map(|line| (line.sequence, k.clone())))
                .min();
            let Some((_, key)) = oldest else {
                break;
            };
            let s = self.sources.get_mut(&key).unwrap();
            let line = s.lines.pop_front().unwrap();
            s.bytes -= line.bytes();
            self.bytes -= line.bytes();
            s.evicted += 1;
        }
        self.high_bytes = self.high_bytes.max(self.bytes);
    }
    pub fn last_sequence(&self) -> u64 {
        self.sequence
    }
    pub fn clear(&mut self) {
        let revision = self.revision + 1;
        let sequence = self.sequence;
        *self = Self::default();
        self.revision = revision;
        self.sequence = sequence;
    }
}
#[derive(Clone)]
pub struct StreamRequest {
    pub source: Source,
    pub generation: u64,
    pub since_seconds: i64,
    pub profile: Profile,
    pub budgets: Budgets,
}
pub async fn follow(
    client: Client,
    request: StreamRequest,
    tx: queue::Sender<Message>,
    dropped: Arc<AtomicU64>,
    cancel: CancellationToken,
) {
    if !request.source.channel.stream() {
        return;
    }
    let api: Api<Pod> = Api::namespaced(client, &request.source.key.namespace);
    let mut sequence = 0;
    loop {
        let params = LogParams {
            container: Some(request.source.container.clone()),
            follow: true,
            tail_lines: Some(200),
            since_seconds: Some(request.since_seconds),
            timestamps: true,
            ..Default::default()
        };
        let opened = tokio::select! {_=cancel.cancelled()=>return,r=tokio::time::timeout(Duration::from_secs(5),api.log_stream(&request.source.key.name,&params))=>r};
        let mut stream = match opened {
            Ok(Ok(s)) => s,
            other => {
                let error = match other {
                    Ok(Err(e)) => crate::probes::api_error(&e),
                    _ => ErrorClass::Timeout,
                };
                let _ = tx.try_send(
                    Message {
                        source: request.source.clone(),
                        generation: request.generation,
                        update: Update::State(SyncState::Failed(error)),
                    },
                    512,
                );
                if matches!(error, ErrorClass::Forbidden | ErrorClass::Unavailable) {
                    return;
                }
                tokio::select! {_=cancel.cancelled()=>return,_=tokio::time::sleep(Duration::from_secs(2))=>continue,}
            }
        };
        let _ = tx.try_send(
            Message {
                source: request.source.clone(),
                generation: request.generation,
                update: Update::State(SyncState::Synced),
            },
            512,
        );
        let mut framer = Framer::new(request.budgets.log_line_bytes);
        let mut chunk = [0u8; 4096];
        loop {
            let read = tokio::select! {_=cancel.cancelled()=>return,result=stream.read(&mut chunk)=>result};
            let read = match read {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let mut lines = Vec::new();
            let mut bytes = 0;
            framer.push(&chunk[..read], |line, truncated| {
                sequence += 1;
                let line = parse(line, truncated, request.profile, sequence);
                bytes += line.bytes();
                lines.push(line);
                if bytes
                    >= request
                        .budgets
                        .log_batch_bytes
                        .saturating_sub(request.budgets.log_line_bytes + 512)
                {
                    send_batch(&request, &tx, &dropped, &mut lines, &mut bytes);
                }
            });
            send_batch(&request, &tx, &dropped, &mut lines, &mut bytes);
        }
        let _ = tx.try_send(
            Message {
                source: request.source.clone(),
                generation: request.generation,
                update: Update::State(SyncState::Restarting),
            },
            512,
        );
        tokio::select! {_=cancel.cancelled()=>return,_=tokio::time::sleep(Duration::from_secs(2))=>{}}
    }
}
fn send_batch(
    request: &StreamRequest,
    tx: &queue::Sender<Message>,
    dropped: &AtomicU64,
    lines: &mut Vec<Line>,
    bytes: &mut usize,
) {
    if lines.is_empty() {
        return;
    }
    let count = lines.len();
    if tx
        .try_send(
            Message {
                source: request.source.clone(),
                generation: request.generation,
                update: Update::Lines(std::mem::take(lines)),
            },
            *bytes + 512,
        )
        .is_err()
    {
        dropped.fetch_add(count as u64, Ordering::Relaxed);
    }
    *bytes = 0;
}
