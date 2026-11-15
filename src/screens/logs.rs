//! Pure projection of accepted evidence into rows.
use super::*;
pub(super) fn content(
    session: &Session,
    view: View,
    filter: &Filter,
    log_window: crate::logs::timeline::Window,
    severity: Option<crate::logs::Level>,
) -> Content {
    let store = &session.store;
    let mut out = Content {
        title: format!("{view:?}"),
        ..Default::default()
    };
    let mut push = |row: ScreenRow, labels: &Labels| {
        if filter.matches(&row.cells.join(" "), labels) {
            out.rows.push(row);
        }
    };

    out.headers = vec![
        "source namespace/pod",
        "timestamp / level",
        "correlation / message",
    ];
    let projection = crate::logs::timeline::project(
        store,
        &session.logs,
        &session.log_scope,
        session.log_sources,
        log_window,
    );
    for entry in projection.entries {
        let source = entry.source;
        let line = entry.line;
        if severity.is_some_and(|level| line.level < level) {
            continue;
        }
        let confidence = entry
            .correlation
            .map_or("unmatched; excluded from cluster scope", |c| c.confidence);
        push(
            row(
                format!("{:?}:{}:{}", source.uid, source.key, line.sequence),
                &source.key,
                vec![
                    source.key.to_string(),
                    format!(
                        "{} {:?}",
                        line.timestamp.as_deref().unwrap_or("unknown timestamp"),
                        line.level
                    ),
                    format!(
                        "{confidence}: {}",
                        line.text.chars().take(512).collect::<String>()
                    ),
                ],
                None,
            ),
            &Labels::new(),
        );
    }
    out.empty = if session.config().profile != crate::cli::Profile::Incident {
        "Logs disabled by profile; select incident explicitly".into()
    } else {
        format!(
            "No matching canonical entries; source states {:?}; unknown operator context stays unmatched",
            session
                .logs
                .sources
                .iter()
                .map(|(s, r)| (&s.key, r.state))
                .collect::<Vec<_>>()
        )
    };
    out.extra = format!(
        "Scope {:?}; {:?}; sources {:?}; queue drops {} truncation {} retained {} B; index {} B; projection/index omissions {}; stream candidates omitted {}",
        session.log_scope,
        log_window,
        session.log_sources,
        session.log_drop_count(),
        session.logs.truncated,
        session.logs.bytes,
        projection.index_bytes,
        projection.omitted + session.event_logs.omitted,
        session.log_omitted
    );

    out
}
