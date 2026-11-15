//! Terminal input/navigation; runtime ownership is isolated in `runtime`.
pub use crate::runtime::Session;
use crate::{cli::Profile, config::Config, logs, model::*, navigation::View, queue};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;

use crate::{
    diagnostics,
    navigation::{Filter, History},
    screens::{self, Content},
};

use std::time::Duration;
use tokio::time::MissedTickBehavior;
struct Navigation {
    selected: Option<String>,
    filter: Filter,
    filter_text: String,
    input: Option<String>,
    history: History,
    trail: crate::navigation::Trail,
    detail: Option<(String, String)>,
    detail_scroll: u16,
    pane: screens::Secondary,
    help: bool,
    auto_mode: bool,
    status: String,
    log_window: logs::timeline::Window,
    scope: ScopeEpoch,
    log_group: u8,
    since: i64,
    severity: Option<logs::Level>,
    focus: Option<Key>,
    log_focus: Option<Key>,
    last_screen: String,
}
impl Default for Navigation {
    fn default() -> Self {
        Self {
            selected: None,
            filter: Filter::All,
            filter_text: String::new(),
            input: None,
            history: History::default(),
            trail: Default::default(),
            detail: None,
            detail_scroll: 0,
            pane: Default::default(),
            help: false,
            auto_mode: true,
            status: String::new(),
            log_window: Default::default(),
            scope: ScopeEpoch(0),
            log_group: 0,
            since: 900,
            severity: None,
            focus: None,
            log_focus: None,
            last_screen: String::new(),
        }
    }
}
fn open(session: &mut Session, nav: &mut Navigation, view: View) {
    if view == View::Logs {
        nav.log_group = 0;
        session.set_log_scope(logs::timeline::Scope::Selected);
        nav.log_window = logs::timeline::Window::Follow;
        nav.log_focus = nav.focus.as_ref().and_then(|key| {
            if session.store.pod(key).is_some()
                || session
                    .store
                    .evidence(Kind::Pooler)
                    .any(|e| e.id.key == *key)
                || session.store.evidence_at(Kind::Backup, key).is_some()
            {
                Some(key.clone())
            } else {
                session
                    .store
                    .evidence(Kind::Event)
                    .find(|e| e.id.key == *key)
                    .and_then(|e| {
                        if let crate::evidence::Data::Event {
                            involved: Some(id), ..
                        } = &e.data
                        {
                            session
                                .store
                                .pod(&id.key)
                                .filter(|p| p.id == *id)
                                .map(|p| p.id.key.clone())
                        } else {
                            None
                        }
                    })
            }
        });
    }
    if view == View::Logs
        && let Some(key) = &nav.log_focus
        && session.store.evidence_at(Kind::Backup, key).is_some()
    {
        session.set_log_scope(logs::timeline::Scope::Backup(key.clone()));
    }
    if session.view != view {
        nav.trail.open(session.view, view);
        nav.detail = None;
        nav.detail_scroll = 0;
        session.set_view(view);
        nav.selected = None;
        nav.auto_mode = false;
        nav.help = false;
    }
}
fn selected_key<'a>(content: &'a Content, nav: &Navigation) -> Option<&'a Key> {
    content
        .rows
        .iter()
        .find(|r| Some(&r.id) == nav.selected.as_ref())
        .map(|r| &r.key)
}
fn desired_logs(session: &Session, nav: &Navigation) -> (Vec<logs::Source>, usize) {
    use logs::{Channel, timeline::Scope};
    let mut sources = Vec::new();
    let mut candidates = 0usize;
    let mut add = |source: logs::Source| {
        candidates += 1;
        if sources.len() < session.budgets.log_streams {
            sources.push(source);
        }
    };
    let selected = nav.log_focus.as_ref();
    let operator = match &session.log_scope {
        Scope::Selected => nav.log_group == 3,
        _ => session.log_sources.operator,
    };
    if operator && session.log_sources.operator {
        for e in session.store.evidence(Kind::OperatorPod) {
            if let crate::evidence::Data::OperatorPod { containers, .. } = &e.data
                && let Some(container) = containers.first()
            {
                add(logs::Source {
                    channel: Channel::Operator,
                    scope: session.store.scope,
                    key: e.id.key.clone(),
                    uid: e.id.uid.clone(),
                    container: container.clone(),
                });
            }
        }
    }
    if !matches!(session.log_scope, Scope::Operator | Scope::Unmatched) {
        let mut pods: Vec<_> = session.store.selected_pods().collect();
        pods.sort_by_key(|p| (p.role != Some(Role::Primary), p.id.key.clone()));
        for p in pods {
            let channel = if p.pooler.is_some() {
                Channel::Pooler
            } else if p.backup.is_some() {
                Channel::Backup
            } else {
                Channel::Postgres
            };
            if !session.log_sources.contains(channel) {
                continue;
            }
            let accept = match &session.log_scope {
                Scope::Selected => match nav.log_group {
                    0 => selected.map_or(p.role == Some(Role::Primary), |key| {
                        key == &p.id.key
                            || p.pooler.as_ref() == Some(key)
                            || p.backup.as_ref() == Some(key)
                    }),
                    1 => p.role == Some(Role::Replica),
                    2 => p.pooler.is_some(),
                    _ => false,
                },
                Scope::Cluster => {
                    p.role.is_some()
                        || p.pooler.is_some()
                        || p.backup.is_some()
                        || p.labels
                            .get("cnpg.io/podRole")
                            .is_some_and(|v| v == "instance")
                }
                Scope::Backup(key) => p.backup.as_ref() == Some(key),
                _ => false,
            };
            if accept {
                let default = if channel == Channel::Pooler {
                    "pgbouncer"
                } else {
                    "postgres"
                };
                let container = p
                    .containers
                    .iter()
                    .find(|c| c.as_str() == default)
                    .or_else(|| p.containers.first())
                    .map_or(default, String::as_str);
                add(logs::Source {
                    channel,
                    scope: session.store.scope,
                    key: p.id.key.clone(),
                    uid: p.id.uid.clone(),
                    container: container.into(),
                });
                if session.log_scope == Scope::Selected && nav.log_group == 0 {
                    break;
                }
            }
        }
    }
    let omitted = candidates.saturating_sub(sources.len());
    (sources, omitted)
}
async fn command(session: &mut Session, nav: &mut Navigation, text: String) {
    nav.history.push(text.clone());
    if text.starts_with('/') {
        match Filter::parse(&text) {
            Ok(filter) => {
                nav.filter = filter;
                nav.filter_text = text;
                nav.selected = None;
                nav.status.clear();
            }
            Err(_) => nav.status = "Invalid or oversized filter; previous filter retained".into(),
        };
        return;
    }
    let mut parts = text.trim_start_matches(':').splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("");
    let value = parts.next().unwrap_or("").trim();
    if let Some(view) = View::parse(name) {
        open(session, nav, view);
        return;
    }
    match name {
        "namespace" | "ns" => {
            let mut config = session.config.clone();
            config.cluster = None;
            if value == "all" {
                config.namespace = None;
                config.all_namespaces = true;
            } else if Key::new(value, "scope").is_ok() {
                config.namespace = Some(value.into());
                config.all_namespaces = false;
            } else {
                nav.status = "Invalid namespace".into();
                return;
            }
            session.replace_scope(config);
            nav.selected = None;
            nav.auto_mode = true;
            nav.status = "Changing namespace; prior scope fenced".into();
        }
        "context" => {
            if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
                nav.status = "Invalid context".into();
                return;
            }
            let mut config = session.config.clone();
            config.context = Some(value.into());
            config.namespace = None;
            config.all_namespaces = false;
            config.cluster = None;
            config.operator_namespace = None;
            session.replace_scope(config);
            nav.selected = None;
            nav.auto_mode = true;
            nav.status = "Changing context; prior scope fenced".into();
        }
        "selector" => {
            let mut config = session.config.clone();
            config.selector = if value.is_empty() {
                None
            } else {
                Some(value.into())
            };
            session.replace_scope(config);
            nav.selected = None;
            nav.auto_mode = true;
        }
        "logscope" => {
            let scope = match value {
                "selected" => logs::timeline::Scope::Selected,
                "cluster" => logs::timeline::Scope::Cluster,
                "operator" => logs::timeline::Scope::Operator,
                "unmatched" => logs::timeline::Scope::Unmatched,
                v if v.starts_with("backup ") => {
                    let Some(cluster) = session.store.selected.as_ref() else {
                        nav.status = "Select a cluster first".into();
                        return;
                    };
                    let Ok(key) =
                        Key::new(&cluster.namespace, v.trim_start_matches("backup ").trim())
                    else {
                        nav.status = "Invalid backup name".into();
                        return;
                    };
                    if session.store.evidence_at(Kind::Backup, &key).is_none() {
                        nav.status = "Backup is not visible in the selected namespace".into();
                        return;
                    }
                    logs::timeline::Scope::Backup(key)
                }
                _ => {
                    nav.status =
                        "Use :logscope selected, cluster, backup NAME, operator, or unmatched"
                            .into();
                    return;
                }
            };
            open(session, nav, View::Logs);
            session.set_log_scope(scope);
            nav.log_window = logs::timeline::Window::Follow;
        }
        "logsource" => {
            let mut parts = value.split_whitespace();
            let name = parts.next().unwrap_or("");
            let enabled = match parts.next() {
                Some("on") => true,
                Some("off") => false,
                _ => {
                    nav.status =
                        "Use :logsource instances|poolers|backups|operator|events on|off".into();
                    return;
                }
            };
            let field = match name {
                "instances" => &mut session.log_sources.instances,
                "poolers" => &mut session.log_sources.poolers,
                "backups" => &mut session.log_sources.backups,
                "operator" => &mut session.log_sources.operator,
                "events" => &mut session.log_sources.events,
                _ => {
                    nav.status = "Unknown log source group".into();
                    return;
                }
            };
            *field = enabled;
            session.log_projection_revision += 1;
        }
        "since" => {
            nav.since = match value {
                "5m" => 300,
                "15m" => 900,
                "1h" => 3600,
                _ => {
                    nav.status = "Use :since 5m, 15m, or 1h".into();
                    return;
                }
            };
            session.close_logs();
        }
        "severity" => {
            nav.severity = match value {
                "all" => None,
                "warning" | "warn" => Some(logs::Level::Warning),
                "error" => Some(logs::Level::Error),
                "fatal" => Some(logs::Level::Fatal),
                _ => {
                    nav.status = "Use :severity all, warning, error, or fatal".into();
                    return;
                }
            };
        }
        "pane" => {
            nav.pane = match value {
                "poolers" => screens::Secondary::Poolers,
                "events" => screens::Secondary::Events,
                _ => {
                    nav.status = "Use :pane poolers or :pane events".into();
                    return;
                }
            };
        }
        "clear" => {
            nav.filter = Filter::All;
            nav.filter_text.clear();
        }
        "dump" => {
            nav.status = match diagnostics::write(session, &nav.last_screen) {
                Ok(_) => "Support dump saved in configured private dump directory".into(),
                Err(e) => format!("Dump failed: {e}; contents suppressed"),
            };
        }
        _ => nav.status = "Unknown read-only command; ? lists supported views".into(),
    }
}
fn inspect(session: &Session, nav: &mut Navigation, content: &Content) {
    let Some(row) = content
        .rows
        .iter()
        .find(|r| Some(&r.id) == nav.selected.as_ref())
    else {
        nav.status = "No selected row to inspect".into();
        return;
    };
    let mut text = content
        .headers
        .iter()
        .zip(&row.cells)
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    if session.view == View::Logs
        && let Some(sequence) = row
            .id
            .rsplit(':')
            .next()
            .and_then(|v| v.parse::<u64>().ok())
    {
        for (source, retained) in &session.logs.sources {
            if let Some(line) = retained.lines.iter().find(|l| l.sequence == sequence) {
                text = format!(
                    "source: {} / {}\ntimestamp: {:?}\nlevel: {:?}\n\n{}",
                    source.key, source.container, line.timestamp, line.level, line.text
                );
                break;
            }
        }
    }
    nav.detail = Some((
        format!("{} > {} detail", content.title, row.key),
        format!(
            "Snapshot at {} ms; scope {}. i refreshes this snapshot.\n\n{text}",
            session.clock.now().0,
            session.store.scope.0
        ),
    ));
    nav.detail_scroll = 0;
}
async fn key(session: &mut Session, nav: &mut Navigation, content: &Content, event: KeyEvent) {
    if let Some(input) = &mut nav.input {
        match event.code {
            KeyCode::Esc => nav.input = None,
            KeyCode::Enter => {
                let text = nav.input.take().unwrap();
                command(session, nav, text).await;
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Up => {
                if let Some(value) = nav.history.previous() {
                    *input = value.into();
                }
            }
            KeyCode::Down => {
                if let Some(value) = nav.history.next_entry() {
                    *input = value.into();
                }
            }
            KeyCode::Char(c) if !c.is_control() && input.len() + c.len_utf8() <= 512 => {
                input.push(c)
            }
            _ => {}
        }
        return;
    }
    if nav.detail.is_none() && matches!(session.view, View::Doctor | View::Profile) {
        match event.code {
            KeyCode::Down | KeyCode::Char('j') => {
                nav.detail_scroll = nav.detail_scroll.saturating_add(1);
                return;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                nav.detail_scroll = nav.detail_scroll.saturating_sub(1);
                return;
            }
            KeyCode::PageDown => {
                nav.detail_scroll = nav.detail_scroll.saturating_add(10);
                return;
            }
            KeyCode::PageUp => {
                nav.detail_scroll = nav.detail_scroll.saturating_sub(10);
                return;
            }
            _ => {}
        }
    }
    if nav.detail.is_some() {
        match event.code {
            KeyCode::Esc | KeyCode::Left => {
                nav.detail = None;
                return;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                nav.detail_scroll = nav.detail_scroll.saturating_add(1);
                return;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                nav.detail_scroll = nav.detail_scroll.saturating_sub(1);
                return;
            }
            KeyCode::PageDown => {
                nav.detail_scroll = nav.detail_scroll.saturating_add(10);
                return;
            }
            KeyCode::PageUp => {
                nav.detail_scroll = nav.detail_scroll.saturating_sub(10);
                return;
            }
            _ => nav.detail = None,
        }
    }
    match event.code {
        KeyCode::Char('i') => inspect(session, nav, content),
        KeyCode::Char(':') | KeyCode::Char('/') => {
            nav.input = Some(
                if event.code == KeyCode::Char(':') {
                    ":"
                } else {
                    "/"
                }
                .into(),
            )
        }
        KeyCode::Char('j') | KeyCode::Down | KeyCode::Char('k') | KeyCode::Up => {
            if !content.rows.is_empty() {
                let old = content
                    .rows
                    .iter()
                    .position(|r| Some(&r.id) == nav.selected.as_ref())
                    .unwrap_or(0);
                let next = if matches!(event.code, KeyCode::Char('j') | KeyCode::Down) {
                    (old + 1).min(content.rows.len() - 1)
                } else {
                    old.saturating_sub(1)
                };
                nav.selected = Some(content.rows[next].id.clone());
                if session.view == View::Logs && nav.log_window.following() {
                    nav.log_window = logs::timeline::Window::Paused(session.logs.last_sequence());
                }
            }
        }
        KeyCode::Enter => {
            if let Some(row) = content
                .rows
                .iter()
                .find(|r| Some(&r.id) == nav.selected.as_ref())
            {
                if session.view == View::Fleet {
                    session.select_cluster(Some(row.key.clone()));
                    open(session, nav, View::Instances);
                } else if session.view == View::Logs || row.target.is_none() {
                    inspect(session, nav, content);
                } else if let Some(view) = row.target {
                    let key = row.key.clone();
                    open(session, nav, view);
                    if view == View::Logs {
                        nav.selected = Some(format!("pod:{key}"));
                    }
                }
            }
        }
        KeyCode::Esc | KeyCode::Left => {
            if nav.help {
                nav.help = false;
                return;
            }
            if let Some(view) = nav.trail.back(session.view) {
                session.set_view(view);
            } else {
                session.set_view(View::Instances);
            }
            nav.selected = None;
        }
        KeyCode::Right => {
            if let Some(view) = nav.trail.forward(session.view) {
                session.set_view(view);
                nav.selected = None;
                nav.auto_mode = false;
            }
        }
        KeyCode::Char('?') => nav.help = !nav.help,
        KeyCode::Char('d') => open(session, nav, View::Doctor),
        KeyCode::Char('p') => open(session, nav, View::Profile),
        KeyCode::Char('l') => open(session, nav, View::Logs),
        KeyCode::Char('1' | '2' | '3' | '4') if session.view == View::Profile => {
            let p = match event.code {
                KeyCode::Char('1') => Profile::Minimal,
                KeyCode::Char('2') => Profile::Standard,
                KeyCode::Char('3') => Profile::Incident,
                _ => Profile::Secure,
            };
            session.set_profile(p).await;
            nav.selected = None;
        }
        KeyCode::Tab => {
            let keys: Vec<_> = session.store.clusters().map(|c| c.id.key.clone()).collect();
            if !keys.is_empty() {
                let index = keys
                    .iter()
                    .position(|k| Some(k) == session.store.selected.as_ref())
                    .unwrap_or(0);
                session.select_cluster(Some(keys[(index + 1) % keys.len()].clone()));
                nav.selected = None;
            }
        }
        KeyCode::Char('r') => session.refresh().await,
        KeyCode::Char(' ') if session.view == View::Logs => {
            nav.log_window = if nav.log_window.following() {
                logs::timeline::Window::Paused(session.logs.last_sequence())
            } else {
                logs::timeline::Window::Follow
            };
        }
        KeyCode::Char('s') if session.view == View::Logs => {
            nav.log_group = (nav.log_group + 1) % 4;
            session.set_log_scope(logs::timeline::Scope::Selected);
            nav.log_focus = None;
            nav.log_window = logs::timeline::Window::Follow;
            nav.selected = None;
        }
        KeyCode::Char('g') if session.view == View::Logs => {
            nav.log_window = logs::timeline::Window::Oldest(session.logs.last_sequence());
            nav.selected = content.rows.first().map(|r| r.id.clone());
        }
        KeyCode::Char('G') if session.view == View::Logs => {
            nav.log_window = logs::timeline::Window::Follow;
            nav.selected = content.rows.last().map(|r| r.id.clone());
        }
        KeyCode::F(12) => command(session, nav, ":dump".into()).await,
        _ => {}
    }
}
pub async fn watch(
    config: Config,
    terminal: &mut crate::terminal::TerminalGuard,
) -> anyhow::Result<()> {
    let mut session = Session::start(config)
        .await
        .map_err(|e| anyhow::anyhow!("Kubernetes startup: {e}; details suppressed"))?;
    let (tx, mut control) = queue::channel(128, 512 * 1024);
    let stop = session.root.child_token();
    let input_stop = stop.clone();
    session.tasks.spawn(async move {
        let mut events = EventStream::new();
        let mut editing = false;
        loop {
            let event = tokio::select! {_=input_stop.cancelled()=>break,event=events.next()=>event};
            match event {
                Some(Ok(Event::Key(k))) if k.kind == KeyEventKind::Press => {
                    if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL)
                        || !editing && k.code == KeyCode::Char('q')
                    {
                        input_stop.cancel();
                        break;
                    }
                    if !editing && matches!(k.code, KeyCode::Char(':') | KeyCode::Char('/')) {
                        editing = true;
                    } else if matches!(k.code, KeyCode::Enter | KeyCode::Esc) {
                        editing = false;
                    }
                    let _ = tx.try_send(k, 64);
                }
                Some(Ok(Event::Resize(..))) => {
                    let _ = tx.try_send(KeyEvent::new(KeyCode::Null, KeyModifiers::NONE), 64);
                }
                None | Some(Err(_)) => {
                    input_stop.cancel();
                    break;
                }
                _ => {}
            }
        }
    });
    let mut nav = Navigation::default();
    let mut content = Content::default();
    let mut content_stamp = None;
    let mut timer = tokio::time::interval(Duration::from_millis(session.budgets.render_ms));
    timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut dirty = true;
    let mut last_frame = None;
    let outcome: std::io::Result<()> = async {
        loop {
            let wake = session.wake.clone();
            tokio::select! {
                _ = stop.cancelled() => break,
                _ = tokio::signal::ctrl_c() => break,
                event = control.recv() => {
                    if let Some(event) = event {
                        key(&mut session, &mut nav, &content, event.value).await;
                        dirty = true;
                    }
                },
                _ = wake.notified() => {
                    session.tick();
                    if session.has_pending_work() { wake.notify_one(); }
                },
                _ = timer.tick() => {
                    session.tick();
                    if nav.scope!=session.store.scope {nav.scope=session.store.scope;nav.log_window=logs::timeline::Window::Follow;nav.detail=None;nav.detail_scroll=0;}
                    if nav.auto_mode {
                        let view = if session.config.cluster.is_some() || session.store.clusters().count() == 1 {
                            View::Instances
                        } else {
                            View::Fleet
                        };
                        session.set_view(view);
                    }
                    if session.view == View::Logs {
                        session.sync_log_events();
                        let (sources,omitted)=desired_logs(&session,&nav);session.log_omitted=omitted;
                        session.set_log_sources(sources, nav.since);
                    }
                    let stamp = (
                        session.store.revision, session.logs.revision, session.clock.now().0 / 1000,
                        session.view, nav.filter_text.clone(), nav.log_window, nav.severity,session.log_projection_revision,
                    );
                    if content_stamp.as_ref() != Some(&stamp) {
                        content = screens::content(&session, session.view, &nav.filter, nav.log_window, nav.severity);
                        content_stamp = Some(stamp);
                        dirty = true;
                    }
                    let following = session.view == View::Logs && nav.log_window.following();
                    crate::navigation::reconcile_rows(&mut nav.selected,content.rows.iter().map(|r|r.id.as_str()),following);
                    let size = terminal.screen().size()?;
                    let frame_stamp = (session.store.revision, session.logs.revision, session.clock.now().0 / 1000, session.view, size);
                    if dirty || last_frame != Some(frame_stamp) {
                        draw_frame(&session, &mut nav, &content, terminal)?;
                        last_frame = Some(frame_stamp);
                        dirty = false;
                    }
                },
            }
        }
        Ok(())
    }.await;
    let cleanup = session.shutdown().await;
    outcome?;
    anyhow::ensure!(cleanup == 0, "collector cleanup exceeded its grace period");
    Ok(())
}

fn draw_frame(
    session: &Session,
    nav: &mut Navigation,
    content: &Content,
    terminal: &mut crate::terminal::TerminalGuard,
) -> std::io::Result<()> {
    let config = &session.config;
    let caps = crate::capabilities::observed_for(&session.store, config, session.sql.clone());
    let reduced = caps
        .iter()
        .filter(|c| c.requested && !matches!(c.state, "available" | "empty" | "view_inactive"))
        .count();
    let header = format!(
        "cnpg-watch | ctx {} | ns {} | {:?} | reduced {}\n{} > {:?} | scope {} | SQL {:?} | operator {:?}",
        config.context.as_deref().unwrap_or("current"),
        if config.all_namespaces {
            "all"
        } else {
            &session.namespace
        },
        config.profile,
        reduced,
        session
            .store
            .selected
            .as_ref()
            .map_or_else(|| "fleet".into(), ToString::to_string),
        session.view,
        session.store.scope.0,
        session.sql,
        session.operator.as_ref().map(|o| o.state),
    );
    if session.view != View::Logs {
        nav.focus = selected_key(content, nav).cloned();
    }
    nav.last_screen = format!(
        "{}\n{}\n{}\n{}",
        header,
        content.title,
        content
            .rows
            .iter()
            .take(40)
            .map(|r| r.cells.join(" | "))
            .collect::<Vec<_>>()
            .join("\n"),
        content.extra
    );
    let status = nav.input.as_ref().cloned().unwrap_or_else(|| {
        format!(
            "{:?} | {} | q quit | Enter related | i detail | : command | / filter | ? help\n{}",
            session.view, nav.filter_text, nav.status
        )
    });
    terminal.screen().draw(|frame| {
        if let Some((title, text)) = &nav.detail {
            screens::render::detail(
                frame,
                &header,
                title,
                text,
                "Detail snapshot | j/k scroll | i refresh snapshot | Esc back | q quit",
                nav.detail_scroll,
            );
        } else if matches!(session.view, View::Doctor | View::Profile) && !nav.help {
            let mut details = crate::doctor::live_text(&session.store, config, session.sql.clone());
            if session.view == View::Profile {
                details = format!(
                    "Change profile: 1 minimal | 2 standard | 3 incident | 4 secure\n{details}"
                );
            }
            details.push_str(&format!(
                "\nqueues {} B | tasks {} | rejected {} | scope error {:?}\n{status}",
                session.queue_bytes(),
                session.task_count(),
                session.store.rejected,
                session.scope_error
            ));
            screens::render::detail(
                frame,
                &header,
                if session.view == View::Doctor {
                    "Doctor / accepted evidence"
                } else {
                    "Profile"
                },
                &details,
                "j/k scroll | Esc back | Right forward | q quit | : command",
                nav.detail_scroll,
            );
        } else {
            screens::render::draw(
                frame,
                content,
                &header,
                &status,
                nav.selected.as_deref(),
                nav.help,
                nav.pane,
            );
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod dispatcher_tests {
    use super::*;
    #[tokio::test]
    async fn injected_keys_share_production_navigation_without_terminal_or_network() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("watch.toml");
        std::fs::write(&file, "").unwrap();
        let config = crate::config::Config::resolve(
            crate::cli::Options {
                config: Some(file),
                namespace: Some("app-db".into()),
                profile: Some(Profile::Minimal),
                no_operator_watch: true,
                ..Default::default()
            },
            crate::config::Paths::discover().unwrap(),
        )
        .unwrap();
        let service = tower::service_fn(|_: http::Request<kube::client::Body>| async {
            Ok::<_, std::convert::Infallible>(
                http::Response::builder()
                    .status(403)
                    .body(kube::client::Body::from(
                        r#"{"apiVersion":"v1","kind":"Status","reason":"Forbidden","code":403}"#
                            .as_bytes()
                            .to_vec(),
                    ))
                    .unwrap(),
            )
        });
        let mut session = Session::with_client(
            config,
            kube::Client::new(service, "app-db"),
            "app-db".into(),
        )
        .await
        .unwrap();
        session.set_view(View::Instances);
        let mut nav = Navigation {
            selected: Some("row".into()),
            auto_mode: false,
            ..Default::default()
        };
        let content = Content {
            title: "Instances".into(),
            headers: vec!["instance", "sample"],
            rows: vec![screens::ScreenRow {
                id: "row".into(),
                key: Key::new("app-db", "payments-1").unwrap(),
                cells: vec!["payments-1".into(), "never sampled".into()],
                target: None,
                severity: None,
            }],
            ..Default::default()
        };
        let event = |code| KeyEvent::new(code, KeyModifiers::NONE);
        key(&mut session, &mut nav, &content, event(KeyCode::Char('i'))).await;
        assert!(nav.detail.is_some());
        key(&mut session, &mut nav, &content, event(KeyCode::PageDown)).await;
        assert_eq!(nav.detail_scroll, 10);
        key(&mut session, &mut nav, &content, event(KeyCode::Esc)).await;
        assert!(nav.detail.is_none());
        key(&mut session, &mut nav, &content, event(KeyCode::Char('d'))).await;
        assert_eq!(session.view, View::Doctor);
        assert_eq!(nav.detail_scroll, 0);
        key(&mut session, &mut nav, &content, event(KeyCode::PageDown)).await;
        assert_eq!(nav.detail_scroll, 10);
        key(&mut session, &mut nav, &content, event(KeyCode::Left)).await;
        assert_eq!(session.view, View::Instances);
        key(&mut session, &mut nav, &content, event(KeyCode::Right)).await;
        assert_eq!(session.view, View::Doctor);
        command(&mut session, &mut nav, ":pane poolers".into()).await;
        assert_eq!(nav.pane, screens::Secondary::Poolers);
        command(&mut session, &mut nav, "/payments".into()).await;
        let previous = nav.filter_text.clone();
        command(&mut session, &mut nav, "/[".into()).await;
        assert_eq!(nav.filter_text, previous);
        assert_eq!(session.shutdown().await, 0);
    }
}
