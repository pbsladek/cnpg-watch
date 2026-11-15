use crate::{budget::Budgets, model::*, reducer::Store};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::{Block, Paragraph, Row, Table},
};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceRow {
    pub key: Key,
    pub role: String,
    pub ready: String,
    pub lsn: String,
    pub lag: String,
    pub state: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewModel {
    pub cluster: String,
    pub sources: String,
    pub rows: Vec<InstanceRow>,
    pub empty: String,
    pub archive: String,
}

pub fn derive(store: &Store, now: Tick, budgets: &Budgets) -> ViewModel {
    let cluster = store
        .selected
        .as_ref()
        .map_or_else(|| "none".into(), ToString::to_string);
    let sources = store
        .watches
        .iter()
        .map(|(k, c)| format!("{k:?}:{:?}", c.status))
        .collect::<Vec<_>>()
        .join(" | ");
    let rows = store
        .selected_pods()
        .filter(|p| {
            p.cluster.as_ref() == store.selected.as_ref() && p.cluster.is_some() && p.role.is_some()
        })
        .map(|pod| {
            let observation = store.samples.get(&pod.id.key);
            let last = observation.and_then(|o| o.last_good.as_ref());
            let role = pod
                .role
                .map_or_else(|| "unknown".into(), |r| format!("{r:?}").to_lowercase());
            let ready = pod
                .ready
                .map_or("unknown", |r| if r { "yes" } else { "no" })
                .into();
            let lsn = last
                .and_then(|s| s.value.current.or(s.value.replay))
                .map_or_else(|| "unknown".into(), |v| v.to_string());
            let state = observation.map_or_else(
                || "never sampled".into(),
                |o| {
                    let freshness = match o.freshness(now, budgets.stale_after()) {
                        Freshness::Never => "never sampled".into(),
                        Freshness::Fresh(age) => format!("fresh {}s", age.as_secs()),
                        Freshness::Stale(age) => format!("stale {}s", age.as_secs()),
                    };
                    match o.attempt {
                        Attempt::Failed(error) => format!("{freshness} ({error})"),
                        Attempt::InFlight(_) => format!("{freshness}; sampling"),
                        _ => freshness,
                    }
                },
            );
            let lag = match lag(store, pod, now, budgets) {
                Lag::NotApplicable => "not-applicable".into(),
                Lag::Bytes(value) => format!("{value} B"),
                Lag::Unknown(_) => "unknown".into(),
            };
            InstanceRow {
                key: pod.id.key.clone(),
                role,
                ready,
                lsn,
                lag,
                state,
            }
        })
        .collect();
    let empty = if store
        .watches
        .values()
        .any(|c| !matches!(c.status, SyncState::Synced))
    {
        "Waiting for synchronized source evidence"
    } else if store.selected.is_none() {
        "No CNPG clusters in the selected scope"
    } else {
        "No instance pods in the selected cluster"
    }
    .into();
    let archive = store
        .selected_pods()
        .filter_map(|p| store.samples.get(&p.id.key)?.last_good.as_ref())
        .find(|s| s.value.role == Role::Primary)
        .map_or_else(
            || "Archive: unknown".into(),
            |s| {
                format!(
                    "Archive: archived {} | failed {} | sample {}s old",
                    s.value.archive.archived_count,
                    s.value.archive.failed_count,
                    now.age(s.at).as_secs()
                )
            },
        );
    ViewModel {
        cluster,
        sources,
        rows,
        empty,
        archive,
    }
}

/// No alarm uses transaction age. Comparisons require compatible, fresh evidence.
pub fn lag(store: &Store, pod: &Pod, now: Tick, budgets: &Budgets) -> Lag {
    match validated_lag(store, pod, now, budgets) {
        Ok(value) => value,
        Err(reason) => Lag::Unknown(reason),
    }
}

pub fn byte_lag(store: &Store, pod: &Pod, now: Tick, budgets: &Budgets) -> Option<u64> {
    match lag(store, pod, now, budgets) {
        Lag::Bytes(value) => Some(value),
        _ => None,
    }
}

fn validated_lag(
    store: &Store,
    pod: &Pod,
    now: Tick,
    budgets: &Budgets,
) -> Result<Lag, LagUnknown> {
    use LagUnknown::*;
    if store
        .watches
        .iter()
        .filter(|(kind, _)| matches!(kind, Kind::Cluster | Kind::Pod))
        .any(|(_, watch)| watch.status != SyncState::Synced)
    {
        return Err(WatchUnsynced);
    }
    let replica = store.samples.get(&pod.id.key).ok_or(NeverSampled)?;
    if !matches!(
        replica.freshness(now, budgets.stale_after()),
        Freshness::Fresh(_)
    ) {
        return Err(Stale);
    }
    let r = replica.last_good.as_ref().ok_or(NeverSampled)?;
    let c = store
        .cluster(pod.cluster.as_ref().ok_or(MissingPrimary)?)
        .ok_or(MissingPrimary)?;
    let epoch = store.topology.get(&c.id.key).copied().unwrap_or_default();
    if r.ticket.pod != pod.id || r.ticket.topology != epoch {
        return Err(IdentityChanged);
    }
    if r.value.role == Role::Primary {
        return Ok(Lag::NotApplicable);
    }
    let primary_key = Key::new(
        c.id.key.namespace.clone(),
        c.primary.as_ref().ok_or(MissingPrimary)?.clone(),
    )
    .map_err(|_| MissingPrimary)?;
    let primary = store.samples.get(&primary_key).ok_or(MissingPrimary)?;
    if !matches!(
        primary.freshness(now, budgets.stale_after()),
        Freshness::Fresh(_)
    ) {
        return Err(Stale);
    }
    let p = primary.last_good.as_ref().ok_or(MissingPrimary)?;
    if p.value.role != Role::Primary
        || p.value.system_id != r.value.system_id
        || p.ticket.topology != epoch
    {
        return Err(IdentityChanged);
    }
    if p.value.timeline != r.value.timeline {
        return Err(TimelineMismatch);
    }
    if p.at.0.abs_diff(r.at.0) > 2000 {
        return Err(SamplingSkew);
    }
    let p_time =
        chrono::DateTime::parse_from_rfc3339(&p.value.server_time).map_err(|_| SamplingSkew)?;
    let r_time =
        chrono::DateTime::parse_from_rfc3339(&r.value.server_time).map_err(|_| SamplingSkew)?;
    if (p_time - r_time).num_milliseconds().unsigned_abs() > 2000 {
        return Err(SamplingSkew);
    }
    let peers: Vec<_> = p
        .value
        .peers
        .iter()
        .filter(|peer| {
            peer.application == pod.id.key.name
                || (peer.address.is_some() && peer.address == pod.address)
        })
        .collect();
    if peers.is_empty() {
        return Err(MissingPeer);
    }
    if peers.len() != 1 {
        return Err(AmbiguousPeer);
    }
    let current = p.value.current.ok_or(MissingLsn)?;
    let replay = r.value.replay.ok_or(MissingLsn)?;
    current
        .checked_lag(replay)
        .map(Lag::Bytes)
        .ok_or(NegativeDelta)
}

#[derive(Default)]
pub struct ViewCache {
    stamp: Option<(u64, u64)>,
    view: Option<ViewModel>,
    pub builds: u64,
}
impl ViewCache {
    pub fn get<'a>(&'a mut self, store: &Store, now: Tick, budgets: &Budgets) -> &'a ViewModel {
        let stamp = (store.revision, now.0 / 1000);
        if self.stamp != Some(stamp) {
            self.view = Some(derive(store, now, budgets));
            self.stamp = Some(stamp);
            self.builds += 1;
        }
        self.view.as_ref().unwrap()
    }
}

pub fn draw(
    frame: &mut Frame<'_>,
    view: &ViewModel,
    header: &str,
    sql: &str,
    selected: Option<&Key>,
    help: bool,
) {
    let areas = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
        Constraint::Length(2),
    ])
    .split(frame.area());
    frame.render_widget(
        Paragraph::new(format!(
            "{header} | cluster {}\n{}",
            view.cluster, view.sources
        )),
        areas[0],
    );
    if help {
        frame.render_widget(Paragraph::new("q / Ctrl-C: quit\nj / Down, k / Up: select instance\nTab: next cluster\nr: coalesced refresh (respects retry limits)\nd: capability details; p: profile (1/2/3/4)\n?: close help\nObserve-only: no actions or setup from the TUI"),areas[1]);
        return;
    }
    if view.rows.is_empty() {
        frame.render_widget(
            Paragraph::new(view.empty.as_str()).block(Block::bordered().title("Instances")),
            areas[1],
        );
    } else {
        let count = usize::from(areas[1].height.saturating_sub(3)).max(1);
        let position = view
            .rows
            .iter()
            .position(|r| Some(&r.key) == selected)
            .unwrap_or(0);
        let start = position.saturating_sub(count - 1);
        let rows = view.rows.iter().skip(start).take(count).map(|r| {
            let style = if Some(&r.key) == selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            Row::new([
                r.key.name.clone(),
                r.role.clone(),
                r.ready.clone(),
                r.lsn.clone(),
                r.lag.clone(),
                r.state.clone(),
            ])
            .style(style)
        });
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(20),
                Constraint::Length(8),
                Constraint::Length(7),
                Constraint::Length(18),
                Constraint::Length(14),
                Constraint::Min(8),
            ],
        )
        .header(
            Row::new(["instance", "role", "ready", "LSN", "lag", "sample"])
                .style(Style::default().fg(Color::Cyan)),
        )
        .block(Block::bordered().title("Instances"));
        frame.render_widget(table, areas[1]);
    }
    frame.render_widget(Paragraph::new(format!("{}\n{sql}", view.archive)), areas[2]);
    frame.render_widget(Paragraph::new("q quit | j/k select | Tab cluster | r refresh | ? help\nd capabilities | p profile | observe-only; setup is a separate command"),areas[3]);
}

pub fn sample_age_interval() -> Duration {
    Duration::from_secs(1)
}

pub fn draw_details(frame: &mut Frame<'_>, title: &str, details: &str) {
    frame.render_widget(
        Paragraph::new(details)
            .wrap(ratatui::widgets::Wrap { trim: false })
            .block(Block::bordered().title(title)),
        frame.area(),
    );
}
