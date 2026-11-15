//! Fixed layout breakpoints and borrowed table rows; rendering never performs I/O.
use super::{Content, ScreenRow};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Cell, Paragraph, Row, Table, Wrap},
};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Secondary {
    Poolers,
    #[default]
    Events,
}
#[derive(Debug, Clone, Default)]
pub struct Dashboard {
    pub health: String,
    pub poolers: String,
    pub events: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breakpoint {
    Full,
    Compressed,
    Narrow,
}
impl Breakpoint {
    pub fn at(area: Rect) -> Self {
        if area.width >= 120 && area.height >= 40 {
            Self::Full
        } else if area.width >= 100 && area.height >= 30 {
            Self::Compressed
        } else {
            Self::Narrow
        }
    }
}
const HELP: &str = "Observe-only: no actions or setup from the TUI\nq / Ctrl-C quit | Esc / Left back | Right forward | i row detail\nEnter related view | j/k rows | Tab cluster | r refresh | ? help\n: command | / regex | /! inverse | /-l key=value | /-f fuzzy\n:fleet :instances :health :hints :events :services :storage :poolers\n:operator :logs :connections :waits :replication :doctor :profile\n:backups :databases :roles :subscriptions :replicas :plugins :certs\n:namespace NAME|all :context NAME :selector LABEL_SELECTOR\n:pane poolers|events selects the compressed dashboard pane\nl logs | d doctor | p profile | F12 / :dump support\nLogs: Enter detail; space pause; g/G oldest/newest; s source group\n:logscope selected|cluster|backup NAME|operator|unmatched\n:logsource instances|poolers|backups|operator|events on|off\n:since 5m|15m|1h :severity warning|error|all\nProfile: 1 minimal, 2 standard, 3 incident, 4 secure\nUp/Down recalls bounded command/filter history";
pub fn draw(
    frame: &mut Frame<'_>,
    content: &Content,
    header: &str,
    status: &str,
    selected: Option<&str>,
    help: bool,
    secondary: Secondary,
) {
    let area = frame.area();
    let mode = Breakpoint::at(area);
    let dashboard = content
        .dashboard
        .as_ref()
        .filter(|_| !help && mode != Breakpoint::Narrow);
    let sections = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(if dashboard.is_some() { 3 } else { 0 }),
        Constraint::Min(1),
        Constraint::Length(if dashboard.is_some() {
            if mode == Breakpoint::Full { 8 } else { 5 }
        } else {
            0
        }),
        Constraint::Length(if mode == Breakpoint::Narrow { 1 } else { 2 }),
        Constraint::Length(2),
    ])
    .split(area);
    frame.render_widget(Paragraph::new(header), sections[0]);
    if help {
        frame.render_widget(
            Paragraph::new(HELP)
                .wrap(Wrap { trim: false })
                .block(Block::bordered().title("Help")),
            sections[2],
        );
    } else {
        table(frame, content, sections[2], selected, mode);
    }
    if let Some(dashboard) = dashboard {
        frame.render_widget(
            Paragraph::new(dashboard.health.as_str())
                .wrap(Wrap { trim: false })
                .block(Block::bordered().title("Health / evidence")),
            sections[1],
        );
        if mode == Breakpoint::Full {
            let halves =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(sections[3]);
            pane(frame, halves[0], "Poolers", &dashboard.poolers);
            pane(frame, halves[1], "Recent events", &dashboard.events);
        } else {
            let (title, text) = match secondary {
                Secondary::Poolers => ("Poolers", &dashboard.poolers),
                Secondary::Events => ("Recent events", &dashboard.events),
            };
            pane(frame, sections[3], title, text);
        }
    }
    let extra = if mode == Breakpoint::Narrow {
        format!("Compact; i detail | {}", content.extra)
    } else {
        content.extra.clone()
    };
    frame.render_widget(Paragraph::new(extra), sections[4]);
    frame.render_widget(Paragraph::new(status), sections[5]);
}
fn pane(frame: &mut Frame<'_>, area: Rect, title: &str, text: &str) {
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(title)),
        area,
    );
}
fn widths(content: &Content, narrow: bool) -> Vec<Constraint> {
    if narrow {
        return vec![Constraint::Percentage(40), Constraint::Min(1)];
    }
    content
        .headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            if i == 0 {
                Constraint::Percentage(24)
            } else if i + 1 == content.headers.len() {
                Constraint::Min(12)
            } else {
                Constraint::Length(match *h {
                    "role" => 10,
                    "ready" => 7,
                    "LSN" => 14,
                    "lag" => 14,
                    _ => {
                        if content.headers.len() > 4 {
                            12
                        } else {
                            18
                        }
                    }
                })
            }
        })
        .collect()
}
fn cells(row: &ScreenRow, narrow: bool, sample: Option<usize>) -> Vec<Cell<'_>> {
    if narrow {
        vec![
            Cell::from(row.cells.first().map_or("", String::as_str)),
            Cell::from(
                sample
                    .and_then(|i| row.cells.get(i))
                    .into_iter()
                    .chain(
                        row.cells
                            .iter()
                            .enumerate()
                            .skip(1)
                            .filter(|(i, _)| Some(*i) != sample)
                            .map(|(_, cell)| cell),
                    )
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" | "),
            ),
        ]
    } else {
        row.cells
            .iter()
            .map(|cell| Cell::from(cell.as_str()))
            .collect()
    }
}
fn table(
    frame: &mut Frame<'_>,
    content: &Content,
    area: Rect,
    selected: Option<&str>,
    mode: Breakpoint,
) {
    if content.rows.is_empty() {
        frame.render_widget(
            Paragraph::new(content.empty.as_str())
                .wrap(Wrap { trim: false })
                .block(Block::bordered().title(content.title.as_str())),
            area,
        );
        return;
    }
    let visible = usize::from(area.height.saturating_sub(3)).max(1);
    let position = content
        .rows
        .iter()
        .position(|r| Some(r.id.as_str()) == selected)
        .unwrap_or(0);
    let start = position.saturating_sub(visible.saturating_sub(1));
    let narrow = mode == Breakpoint::Narrow;
    let rows = content.rows.iter().skip(start).take(visible).map(|r| {
        let style = match r.severity {
            Some(crate::health::Severity::Critical) => Style::default().fg(Color::Red),
            Some(crate::health::Severity::Warning) => Style::default().fg(Color::Yellow),
            Some(crate::health::Severity::Info) => Style::default().fg(Color::Cyan),
            None => Style::default(),
        };
        Row::new(cells(
            r,
            narrow,
            content.headers.iter().position(|h| *h == "sample"),
        ))
        .style(if Some(r.id.as_str()) == selected {
            style.bg(Color::DarkGray)
        } else {
            style
        })
    });
    let headers = if narrow {
        vec![
            content.headers.first().copied().unwrap_or("resource"),
            "state / details",
        ]
    } else {
        content.headers.clone()
    };
    frame.render_widget(
        Table::new(rows, widths(content, narrow))
            .header(Row::new(headers).style(Style::default().fg(Color::Cyan)))
            .block(Block::bordered().title(content.title.as_str())),
        area,
    );
}
pub fn detail(
    frame: &mut Frame<'_>,
    header: &str,
    title: &str,
    text: &str,
    status: &str,
    scroll: u16,
) {
    let areas = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(frame.area());
    frame.render_widget(Paragraph::new(header), areas[0]);
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0))
            .block(Block::bordered().title(title)),
        areas[1],
    );
    frame.render_widget(Paragraph::new(status), areas[2]);
}
