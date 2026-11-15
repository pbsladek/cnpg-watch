//! Bounded metadata indexes over redacted canonical records. Message text never drives correlation.
use super::{Book, Channel, Context, Line, Source};
use crate::{evidence::Data, model::*, reducer::Store};
use serde::Serialize;
use std::collections::BTreeMap;
pub const MAX_ROWS: usize = 512;
pub const MAX_INDEX_BYTES: usize = 512 * 1024;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Scope {
    Selected,
    Cluster,
    Backup(Key),
    Operator,
    Unmatched,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Sources {
    pub instances: bool,
    pub poolers: bool,
    pub backups: bool,
    pub operator: bool,
    pub events: bool,
}
impl Default for Sources {
    fn default() -> Self {
        Self {
            instances: true,
            poolers: true,
            backups: true,
            operator: true,
            events: true,
        }
    }
}
impl Sources {
    pub fn contains(self, channel: Channel) -> bool {
        match channel {
            Channel::Postgres => self.instances,
            Channel::Pooler => self.poolers,
            Channel::Backup => self.backups,
            Channel::Operator => self.operator,
            Channel::Events => self.events,
            Channel::OperatorEvents => self.events && self.operator,
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Window {
    #[default]
    Follow,
    Paused(u64),
    Oldest(u64),
}
impl Window {
    pub fn cutoff(self) -> Option<u64> {
        match self {
            Self::Follow => None,
            Self::Paused(n) | Self::Oldest(n) => Some(n),
        }
    }
    pub fn following(self) -> bool {
        self == Self::Follow
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Correlation<'a> {
    pub cluster: &'a Key,
    pub backup: Option<&'a Key>,
    pub confidence: &'static str,
}
#[derive(Clone, Copy)]
struct Anchor<'a> {
    correlation: Correlation<'a>,
    timestamp: i64,
    ambiguous: bool,
}
#[derive(Clone, Copy)]
struct EventAnchor<'a> {
    identity: &'a Identity,
    kind: Kind,
    cluster: &'a Key,
    timestamp: i64,
}
pub struct Index<'a> {
    store: &'a Store,
    events: BTreeMap<&'a Key, Vec<EventAnchor<'a>>>,
    reconciles: BTreeMap<(&'a Source, [u8; 16]), Anchor<'a>>,
    pub bytes: usize,
    pub omitted: usize,
}
impl<'a> Index<'a> {
    pub fn new(store: &'a Store) -> Self {
        let mut index = Self {
            store,
            events: BTreeMap::new(),
            reconciles: BTreeMap::new(),
            bytes: 0,
            omitted: 0,
        };
        for e in store
            .evidence(Kind::Event)
            .chain(store.evidence(Kind::OperatorEvent))
        {
            let Data::Event {
                involved: Some(id),
                involved_kind: Some(kind),
                last_seen_ms: Some(timestamp),
                ..
            } = &e.data
            else {
                continue;
            };
            let Some(correlation) = target(store, &id.key, Some(*kind), Some(&id.uid)) else {
                continue;
            };
            if index.bytes + 192 > MAX_INDEX_BYTES / 2 {
                index.omitted += 1;
                continue;
            }
            index.events.entry(&id.key).or_default().push(EventAnchor {
                identity: id,
                kind: *kind,
                cluster: correlation.cluster,
                timestamp: *timestamp,
            });
            index.bytes += 192;
        }
        index
    }
    pub fn add_anchor(&mut self, source: &'a Source, line: &'a Line) {
        if !source.channel.operator() || !source_current(self.store, source) {
            return;
        }
        let Some(context) = &line.context else { return };
        let Some(id) = context.reconcile else { return };
        let Some(timestamp) = timestamp(line) else {
            return;
        };
        let Some(correlation) = self.direct_context(context, timestamp) else {
            return;
        };
        if let Some(previous) = self.reconciles.get_mut(&(source, id)) {
            if previous.correlation.cluster != correlation.cluster {
                previous.ambiguous = true;
            }
            previous.timestamp = previous.timestamp.max(timestamp);
        } else if self.reconciles.len() < 512 && self.bytes + 192 <= MAX_INDEX_BYTES {
            self.reconciles.insert(
                (source, id),
                Anchor {
                    correlation,
                    timestamp,
                    ambiguous: false,
                },
            );
            self.bytes += 192;
        } else {
            self.omitted += 1;
        }
    }
    fn direct_context(&self, context: &Context, timestamp: i64) -> Option<Correlation<'a>> {
        let key = context.target.as_ref()?;
        let correlation = target(self.store, key, context.kind, context.uid.as_ref());
        if let Some(c) = correlation {
            if self
                .store
                .cluster(c.cluster)
                .and_then(|c| c.operational.created_ms)
                .is_some_and(|created| timestamp < created)
            {
                return None;
            }
            return Some(c);
        }
        let mut matches = self.events.get(key)?.iter().filter(|event| {
            context.kind.is_none_or(|k| k == event.kind)
                && context
                    .uid
                    .as_ref()
                    .is_none_or(|uid| uid == &event.identity.uid)
                && timestamp.abs_diff(event.timestamp) <= 30_000
        });
        let first = matches.next()?;
        if matches.any(|e| e.cluster != first.cluster) {
            return None;
        }
        Some(Correlation {
            cluster: first.cluster,
            backup: if first.kind == Kind::Backup {
                Some(&first.identity.key)
            } else {
                None
            },
            confidence: "event UID and 30s time window",
        })
    }
    pub fn correlate(&self, source: &Source, line: &Line) -> Option<Correlation<'a>> {
        if source.scope != self.store.scope || !source_current(self.store, source) {
            return None;
        }
        match source.channel {
            Channel::Events => {
                return self.store.cluster(&source.key).map(|c| {
                    let backup = line
                        .context
                        .as_ref()
                        .and_then(|ctx| ctx.target.as_ref())
                        .and_then(|key| self.store.evidence_at(Kind::Backup, key))
                        .map(|e| &e.id.key);
                    Correlation {
                        cluster: &c.id.key,
                        backup,
                        confidence: "canonical Kubernetes event",
                    }
                });
            }
            Channel::Postgres | Channel::Pooler | Channel::Backup => {
                let pod = self.store.pod(&source.key)?;
                let cluster = pod.cluster.as_ref()?;
                if !self.store.pods_for(cluster).any(|p| p.id == pod.id) {
                    return None;
                }
                let backup = pod
                    .backup
                    .as_ref()
                    .and_then(|key| self.store.evidence_at(Kind::Backup, key))
                    .filter(|e| pod.backup_uid.as_ref().is_none_or(|uid| uid == &e.id.uid))
                    .map(|e| &e.id.key);
                return Some(Correlation {
                    cluster,
                    backup,
                    confidence: if pod.cluster_uid.is_some() {
                        "pod and Cluster UID"
                    } else {
                        "pod UID; labelled cluster association"
                    },
                });
            }
            _ => {}
        }
        let context = line.context.as_ref()?;
        let time = timestamp(line)?;
        if let Some(c) = self.direct_context(context, time) {
            return Some(c);
        }
        if context.target.is_some() {
            return None;
        }
        let id = context.reconcile?;
        let anchor = self.reconciles.get(&(source, id))?;
        if anchor.ambiguous || time.abs_diff(anchor.timestamp) > 30_000 {
            return None;
        }
        Some(Correlation {
            confidence: "hashed reconcile ID and 30s time window",
            ..anchor.correlation
        })
    }
}
fn target<'a>(
    store: &'a Store,
    key: &Key,
    kind: Option<Kind>,
    uid: Option<&Uid>,
) -> Option<Correlation<'a>> {
    let kinds = [
        Kind::Cluster,
        Kind::Pod,
        Kind::Pooler,
        Kind::Backup,
        Kind::ScheduledBackup,
        Kind::Database,
        Kind::DatabaseRole,
        Kind::Publication,
        Kind::Subscription,
        Kind::Pvc,
        Kind::Service,
    ];
    let mut found = None;
    for kind in kinds
        .into_iter()
        .filter(|k| kind.is_none_or(|selected| *k == selected))
    {
        let candidate = match kind {
            Kind::Cluster => store
                .cluster(key)
                .filter(|c| uid.is_none_or(|u| u == &c.id.uid))
                .map(|c| Correlation {
                    cluster: &c.id.key,
                    backup: None,
                    confidence: if uid.is_some() {
                        "structured resource UID"
                    } else {
                        "structured namespace/name; UID unknown"
                    },
                }),
            Kind::Pod => store
                .pod(key)
                .filter(|p| uid.is_none_or(|u| u == &p.id.uid))
                .and_then(|p| {
                    p.cluster
                        .as_ref()
                        .filter(|cluster| store.pods_for(cluster).any(|current| current.id == p.id))
                })
                .map(|cluster| Correlation {
                    cluster,
                    backup: None,
                    confidence: "structured pod key",
                }),
            _ => store
                .evidence_at(kind, key)
                .filter(|e| uid.is_none_or(|u| u == &e.id.uid))
                .and_then(|e| {
                    crate::health::cluster_key(store, e).map(|cluster| Correlation {
                        cluster,
                        backup: if kind == Kind::Backup {
                            Some(&e.id.key)
                        } else {
                            None
                        },
                        confidence: if uid.is_some() {
                            "structured resource UID"
                        } else {
                            "structured namespace/name; UID unknown"
                        },
                    })
                }),
        };
        if let Some(candidate) = candidate {
            if found.is_some() {
                return None;
            }
            found = Some(candidate);
        }
    }
    found
}
pub fn timestamp(line: &Line) -> Option<i64> {
    line.timestamp
        .as_ref()
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.timestamp_millis())
}
pub fn source_current(store: &Store, source: &Source) -> bool {
    match source.channel {
        Channel::Events => store
            .cluster(&source.key)
            .is_some_and(|c| c.id.uid == source.uid),
        Channel::OperatorEvents => store
            .evidence_at(Kind::Deployment, &source.key)
            .is_some_and(|e| e.id.uid == source.uid),
        Channel::Operator => store
            .evidence_at(Kind::OperatorPod, &source.key)
            .is_some_and(|e| e.id.uid == source.uid),
        _ => store
            .pod(&source.key)
            .is_some_and(|p| p.id.uid == source.uid),
    }
}
pub struct Entry<'a> {
    pub source: &'a Source,
    pub line: &'a Line,
    pub correlation: Option<Correlation<'a>>,
    pub timestamp_ms: Option<i64>,
}
pub struct Projection<'a> {
    pub entries: Vec<Entry<'a>>,
    pub index_bytes: usize,
    pub omitted: usize,
}
pub fn project<'a>(
    store: &'a Store,
    book: &'a Book,
    scope: &Scope,
    sources: Sources,
    window: Window,
) -> Projection<'a> {
    let mut candidates = Vec::new();
    for (source, retained) in &book.sources {
        if source.scope != store.scope
            || !sources.contains(source.channel)
            || (*scope == Scope::Selected && !retained.active)
        {
            continue;
        }
        let eligible = retained
            .lines
            .iter()
            .filter(|line| window.cutoff().is_none_or(|n| line.sequence <= n));
        if matches!(window, Window::Oldest(_)) {
            candidates.extend(eligible.take(MAX_ROWS).map(|line| (source, line)));
        } else {
            candidates.extend(eligible.rev().take(MAX_ROWS).map(|line| (source, line)));
        }
    }
    let mut index = Index::new(store);
    for (source, line) in &candidates {
        index.add_anchor(source, line);
    }
    let mut entries: Vec<_> = candidates
        .into_iter()
        .filter_map(|(source, line)| {
            let correlation = index.correlate(source, line);
            let selected = match scope {
                Scope::Selected => true,
                Scope::Cluster => correlation
                    .as_ref()
                    .is_some_and(|c| store.selected.as_ref() == Some(c.cluster)),
                Scope::Backup(key) => correlation.as_ref().is_some_and(|c| {
                    c.backup == Some(key) && store.selected.as_ref() == Some(c.cluster)
                }),
                Scope::Operator => source.channel.operator(),
                Scope::Unmatched => source.channel.operator() && correlation.is_none(),
            };
            selected.then_some(Entry {
                source,
                line,
                correlation,
                timestamp_ms: timestamp(line),
            })
        })
        .collect();
    entries.sort_by_key(|entry| {
        (
            entry.timestamp_ms.map_or((true, 0), |t| (false, t)),
            entry.source,
            entry.line.sequence,
        )
    });
    let omitted = index.omitted + entries.len().saturating_sub(MAX_ROWS);
    if entries.len() > MAX_ROWS {
        if matches!(window, Window::Oldest(_)) {
            entries.truncate(MAX_ROWS);
        } else {
            entries.drain(..entries.len() - MAX_ROWS);
        }
    }
    Projection {
        entries,
        index_bytes: index.bytes,
        omitted,
    }
}
