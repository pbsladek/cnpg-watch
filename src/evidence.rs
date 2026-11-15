//! Compact Kubernetes evidence. API DTOs and unbounded messages stay in adapters.
use crate::model::{Identity, Key, Kind, Tick, Uid};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Labels = BTreeMap<String, String>;
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Association {
    Label(Key),
    Owner { key: Key, uid: Uid },
    Conflict,
    Unrelated,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    pub target: Option<Identity>,
    pub ready: Option<bool>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Data {
    Operational(Box<crate::operational::Resource>),
    Event {
        warning: bool,
        reason: String,
        message: String,
        involved: Option<Identity>,
        #[serde(default)]
        involved_kind: Option<Kind>,
        last_seen_ms: Option<i64>,
        count: u32,
    },
    Pvc {
        phase: Option<String>,
        requested: Option<String>,
        capacity: Option<String>,
        reported_state: Option<String>,
    },
    Service {
        selector: Labels,
        read_write: bool,
    },
    Endpoints {
        service: Key,
        endpoints: Vec<Endpoint>,
    },
    Pooler {
        desired: Option<u32>,
        ready: Option<u32>,
        mode: Option<String>,
        unavailable: Option<u32>,
        phase: Option<String>,
        type_: Option<String>,
        image: Option<String>,
    },
    ReplicaSet {
        deployment: Option<Identity>,
    },
    Deployment {
        desired: Option<u32>,
        ready: Option<u32>,
        images: Vec<String>,
        watch_namespaces: Option<String>,
        reconciliation: crate::operational::Reconciliation,
        pooler_owner: Option<Identity>,
    },
    OperatorPod {
        ready: Option<bool>,
        containers: Vec<String>,
    },
    Lease {
        holder: Option<String>,
        renewed_ms: Option<i64>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Evidence {
    pub id: Identity,
    pub kind: Kind,
    pub labels: Labels,
    pub association: Association,
    pub data: Data,
    #[serde(default)]
    pub observed_at: Tick,
}
impl Evidence {
    pub fn cluster_hint(&self) -> Option<&Key> {
        match &self.association {
            Association::Label(k) | Association::Owner { key: k, .. } => Some(k),
            _ => None,
        }
    }
    pub fn bytes(&self) -> usize {
        let strings = match &self.data {
            Data::Operational(value) => value.bytes(),
            Data::Event {
                reason, message, ..
            } => reason.capacity() + message.capacity(),
            Data::Pvc {
                phase,
                requested,
                capacity,
                reported_state,
            } => [phase, requested, capacity, reported_state]
                .into_iter()
                .flatten()
                .map(String::capacity)
                .sum(),
            Data::Service { selector, .. } => selector
                .iter()
                .map(|(k, v)| k.capacity() + v.capacity() + 96)
                .sum(),
            Data::Endpoints { service, endpoints } => {
                service.namespace.capacity()
                    + service.name.capacity()
                    + endpoints.capacity() * std::mem::size_of::<Endpoint>()
                    + endpoints
                        .iter()
                        .filter_map(|e| e.target.as_ref())
                        .map(|i| i.key.name.capacity() + i.key.namespace.capacity() + 256)
                        .sum::<usize>()
            }
            Data::Pooler {
                mode,
                phase,
                type_,
                image,
                ..
            } => [mode, phase, type_, image]
                .into_iter()
                .flatten()
                .map(String::capacity)
                .sum(),
            Data::ReplicaSet { .. } => 512,
            Data::Deployment {
                images,
                watch_namespaces,
                reconciliation,
                ..
            } => {
                reconciliation.bytes()
                    + images.iter().map(String::capacity).sum::<usize>()
                    + watch_namespaces.as_ref().map_or(0, String::capacity)
            }
            Data::OperatorPod { containers, .. } => containers.iter().map(String::capacity).sum(),
            Data::Lease { holder, .. } => holder.as_ref().map_or(0, String::capacity),
        };
        std::mem::size_of::<Self>()
            + 512
            + self.id.key.namespace.capacity()
            + self.id.key.name.capacity()
            + self
                .labels
                .iter()
                .map(|(k, v)| k.capacity() + v.capacity() + 96)
                .sum::<usize>()
            + strings
    }
}
