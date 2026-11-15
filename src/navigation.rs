//! Bounded, validated user input and stable-key navigation independent of terminal I/O.
use crate::{
    evidence::Labels,
    model::{InvalidValue, Key},
};
use regex::{Regex, RegexBuilder};
use serde::Serialize;
use std::collections::VecDeque;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    Fleet,
    Instances,
    Health,
    Hints,
    Events,
    Services,
    Storage,
    Poolers,
    Operator,
    Logs,
    Connections,
    Waits,
    Replication,
    Doctor,
    Profile,
    Backups,
    Databases,
    Roles,
    Subscriptions,
    Replicas,
    Plugins,
    Certs,
}
impl View {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "fleet" => Self::Fleet,
            "instances" | "cluster" => Self::Instances,
            "health" => Self::Health,
            "hints" => Self::Hints,
            "events" => Self::Events,
            "services" => Self::Services,
            "storage" => Self::Storage,
            "poolers" => Self::Poolers,
            "operator" => Self::Operator,
            "logs" => Self::Logs,
            "connections" => Self::Connections,
            "waits" => Self::Waits,
            "replication" => Self::Replication,
            "doctor" => Self::Doctor,
            "profile" => Self::Profile,
            "backups" => Self::Backups,
            "databases" => Self::Databases,
            "roles" => Self::Roles,
            "subscriptions" | "publications" => Self::Subscriptions,
            "replicas" => Self::Replicas,
            "plugins" => Self::Plugins,
            "certs" => Self::Certs,
            _ => return None,
        })
    }
}
#[derive(Debug, Clone, Default)]
pub enum Filter {
    #[default]
    All,
    Regex {
        compiled: Regex,
        inverse: bool,
    },
    Label(Vec<(String, String, bool)>),
    Fuzzy(String),
}
impl Filter {
    pub fn parse(input: &str) -> Result<Self, InvalidValue> {
        if input.len() > 512 || input.chars().any(char::is_control) {
            return Err(InvalidValue);
        }
        let value = input.trim().trim_start_matches('/');
        if value.is_empty() {
            return Ok(Self::All);
        }
        if let Some(value) = value.strip_prefix("-l ") {
            let terms = value
                .split(',')
                .map(|v| {
                    let (key, value, inverse) = if let Some((k, v)) = v.split_once("!=") {
                        (k, v, true)
                    } else if let Some((k, v)) = v.split_once('=') {
                        (k, v, false)
                    } else {
                        return Err(InvalidValue);
                    };
                    if key.is_empty() || value.is_empty() {
                        return Err(InvalidValue);
                    }
                    Ok((key.into(), value.into(), inverse))
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(Self::Label(terms));
        }
        if let Some(value) = value.strip_prefix("-f ") {
            return Ok(Self::Fuzzy(value.to_lowercase()));
        }
        let (value, inverse) = value
            .strip_prefix('!')
            .map_or((value, false), |v| (v.trim_start(), true));
        Ok(Self::Regex {
            compiled: RegexBuilder::new(value)
                .size_limit(64 * 1024)
                .dfa_size_limit(64 * 1024)
                .build()
                .map_err(|_| InvalidValue)?,
            inverse,
        })
    }
    pub fn matches(&self, text: &str, labels: &Labels) -> bool {
        match self {
            Self::All => true,
            Self::Regex { compiled, inverse } => compiled.is_match(text) != *inverse,
            Self::Label(terms) => terms
                .iter()
                .all(|(k, v, inverse)| (labels.get(k) == Some(v)) != *inverse),
            Self::Fuzzy(needle) => {
                let hay = text.to_lowercase();
                let mut chars = hay.chars();
                needle.chars().all(|n| chars.by_ref().any(|c| c == n))
            }
        }
    }
}
#[derive(Default)]
pub struct History {
    entries: VecDeque<String>,
    position: Option<usize>,
}
impl History {
    pub fn push(&mut self, value: String) {
        self.position = None;
        if value.is_empty() || value.len() > 512 {
            return;
        }
        if self.entries.back() != Some(&value) {
            self.entries.push_back(value);
        }
        while self.entries.len() > 32 {
            self.entries.pop_front();
        }
    }
    pub fn previous(&mut self) -> Option<&str> {
        let index = self
            .position
            .unwrap_or(self.entries.len())
            .saturating_sub(1);
        self.position = Some(index);
        self.entries.get(index).map(String::as_str)
    }
    pub fn next_entry(&mut self) -> Option<&str> {
        let index = (self.position? + 1).min(self.entries.len());
        self.position = Some(index);
        self.entries.get(index).map(String::as_str)
    }
}
pub fn reconcile_selection(selected: &mut Option<Key>, keys: &[Key]) {
    if selected.as_ref().is_none_or(|k| !keys.contains(k)) {
        *selected = keys.first().cloned();
    }
}

/// A bounded browser history contains views only, never resource payloads or credentials.
#[derive(Default)]
pub struct Trail {
    back: Vec<View>,
    forward: Vec<View>,
}
impl Trail {
    pub fn open(&mut self, current: View, next: View) {
        if current != next {
            self.back.push(current);
            if self.back.len() > 32 {
                self.back.remove(0);
            }
            self.forward.clear();
        }
    }
    pub fn back(&mut self, current: View) -> Option<View> {
        let next = self.back.pop()?;
        self.forward.push(current);
        if self.forward.len() > 32 {
            self.forward.remove(0);
        }
        Some(next)
    }
    pub fn forward(&mut self, current: View) -> Option<View> {
        let next = self.forward.pop()?;
        self.back.push(current);
        if self.back.len() > 32 {
            self.back.remove(0);
        }
        Some(next)
    }
}
pub fn reconcile_rows<'a>(
    selected: &mut Option<String>,
    ids: impl Iterator<Item = &'a str>,
    follow: bool,
) {
    let mut first = None;
    let mut last = None;
    let mut found = false;
    for id in ids {
        first.get_or_insert(id);
        last = Some(id);
        found |= selected.as_deref() == Some(id);
    }
    if follow || !found {
        *selected = if follow { last } else { first }.map(str::to_owned);
    }
}
