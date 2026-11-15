//! Kubernetes events enter the same redacted, byte-bounded canonical log store.
use super::{Book, Channel, Context, Level, Line, Message, Source, Update};
use crate::{budget::Budgets, evidence::Data, model::*, reducer::Store};
use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{Hash, Hasher},
};
#[derive(Clone, Copy)]
pub struct Selection {
    pub cluster: bool,
    pub operator: bool,
}
#[derive(Default)]
pub struct Events {
    seen: BTreeMap<(Source, Kind, Key), (Uid, u64)>,
    active: BTreeSet<Source>,
    generation: u64,
    pub bytes: usize,
    pub omitted: usize,
}
impl Events {
    pub fn active(&self) -> impl Iterator<Item = &Source> {
        self.active.iter()
    }
    pub fn close(&mut self) {
        self.seen.clear();
        self.active.clear();
        self.bytes = 0;
    }
    pub fn sync(
        &mut self,
        store: &Store,
        book: &mut Book,
        budgets: &Budgets,
        selection: Selection,
        profile: crate::cli::Profile,
        now: Tick,
    ) {
        let mut desired = BTreeSet::new();
        if selection.cluster
            && let Some(c) = store.selected.as_ref().and_then(|k| store.cluster(k))
        {
            desired.insert(Source {
                scope: store.scope,
                key: c.id.key.clone(),
                uid: c.id.uid.clone(),
                container: "events".into(),
                channel: Channel::Events,
            });
        }
        if selection.operator {
            for deployment in store.evidence(Kind::Deployment).take(1) {
                desired.insert(Source {
                    scope: store.scope,
                    key: deployment.id.key.clone(),
                    uid: deployment.id.uid.clone(),
                    container: "events".into(),
                    channel: Channel::OperatorEvents,
                });
            }
        }
        for source in &desired {
            if !self.active.contains(source) || !book.sources.contains_key(source) {
                self.generation = self
                    .generation
                    .checked_add(1)
                    .expect("event generation exhausted");
                self.seen.retain(|(s, _, _), _| s != source);
                book.activate(source.clone(), self.generation, now);
            }
        }
        self.active = desired;
        self.seen.retain(|(source, kind, key), (uid, _)| {
            self.active.contains(source)
                && store
                    .evidence_at(*kind, key)
                    .is_some_and(|e| &e.id.uid == uid)
        });
        self.bytes = self
            .seen
            .keys()
            .map(|(source, _, key)| cost(source, key))
            .sum();
        self.omitted = 0;
        for source in &self.active {
            let kind = if source.channel == Channel::Events {
                Kind::Event
            } else {
                Kind::OperatorEvent
            };
            let mut candidates: Vec<_> = store
                .evidence(kind)
                .filter(|e| {
                    if source.channel == Channel::Events {
                        crate::health::cluster_key(store, e) == Some(&source.key)
                    } else {
                        e.id.key.namespace == source.key.namespace
                    }
                })
                .take(4096)
                .collect();
            candidates.sort_by_key(|e| {
                std::cmp::Reverse(match &e.data {
                    Data::Event { last_seen_ms, .. } => *last_seen_ms,
                    _ => None,
                })
            });
            // Replay initial snapshots oldest-first; subsequent updates retain their arrival sequence.
            candidates.truncate(1024);
            candidates.reverse();
            for event in candidates {
                let Data::Event {
                    warning,
                    reason,
                    message,
                    involved,
                    involved_kind,
                    last_seen_ms,
                    count,
                } = &event.data
                else {
                    continue;
                };
                let text = super::redact(&format!("{reason}: {message} (count {count})"), profile);
                let timestamp = last_seen_ms
                    .and_then(chrono::DateTime::from_timestamp_millis)
                    .map(|t| t.to_rfc3339());
                let context = involved.as_ref().map(|id| {
                    Box::new(Context {
                        target: Some(id.key.clone()),
                        kind: *involved_kind,
                        uid: Some(id.uid.clone()),
                        reconcile: None,
                    })
                });
                let mut hash = std::hash::DefaultHasher::new();
                text.hash(&mut hash);
                timestamp.hash(&mut hash);
                context.hash(&mut hash);
                let signature = (event.id.uid.clone(), hash.finish());
                let key = (source.clone(), kind, event.id.key.clone());
                if self.seen.get(&key) == Some(&signature) {
                    continue;
                }
                if !self.seen.contains_key(&key) {
                    let bytes = cost(source, &event.id.key);
                    if self.bytes + bytes > super::timeline::MAX_INDEX_BYTES
                        || self.seen.len() >= 4096
                    {
                        self.omitted += 1;
                        continue;
                    }
                    self.bytes += bytes;
                }
                self.seen.insert(key, signature);
                let generation = book.sources[source].generation;
                book.accept(
                    Message {
                        source: source.clone(),
                        generation,
                        update: Update::Lines(vec![Line {
                            sequence: 0,
                            timestamp,
                            level: if *warning {
                                Level::Warning
                            } else {
                                Level::Info
                            },
                            text,
                            truncated: false,
                            context,
                        }]),
                    },
                    now,
                    budgets,
                );
            }
        }
    }
}
fn cost(source: &Source, key: &Key) -> usize {
    1024 + source.key.name.capacity()
        + source.key.namespace.capacity()
        + key.name.capacity()
        + key.namespace.capacity()
}
