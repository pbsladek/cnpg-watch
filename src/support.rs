use crate::{SCHEMA_VERSION, cli::Output, config::Config};
use anyhow::Result;
use serde_json::json;

pub fn info(config: &Config, output: Output) -> Result<()> {
    let data = json!({
        "schema_version": SCHEMA_VERSION,
        "binary_version": env!("CARGO_PKG_VERSION"),
        "platform": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
        "paths": if config.profile == crate::cli::Profile::Secure {serde_json::json!({"config":"<redacted>","cache":"<redacted>","logs":"<redacted>","dumps":"<redacted>"})} else {serde_json::to_value(&config.paths)?},
        "log_file": if config.profile == crate::cli::Profile::Secure {serde_json::Value::String("<redacted>".into())} else {serde_json::to_value(&config.log_file)?},
        "profile": config.profile,
        "refresh_ms": config.refresh.as_millis(),
        "scope": {"requested_context": config.context, "namespace": config.namespace,
                  "all_namespaces": config.all_namespaces, "cluster": config.cluster,
                  "operator_namespace": config.operator_namespace},
        "detected_context": null,
        "postgres_version": null,
        "cnpg_version": null,
        "redaction": if config.profile == crate::cli::Profile::Secure {"strict"} else {"standard"},
        "budgets": config.budgets,
        "fast_query": crate::probes::FAST_SPEC,
        "query_registry":crate::probes::registry::registry(),
        "operational":{"cluster_scoped_metadata":config.cluster_resources,"explicit_certificate_reads":config.certificate_secrets.len(),"operator_metrics_configured":config.operator_metrics_url.is_some(),"metrics_labels_retained":false,"secret_lists_or_watches":false},
        "pooler_stats": {"enabled":config.pooler_stats,"identity":"pgbouncer/local-peer","queries":["SHOW VERSION","SHOW POOLS","SHOW STATS"],"interval_ms":crate::pooler::INTERVAL_MS,"stale_ms":crate::pooler::STALE_MS,"max_rows_per_table":crate::pooler::MAX_ROWS,"timeout_ms":config.budgets.probe_timeout_ms},
        "show_query_text":config.show_query_text && config.profile==crate::cli::Profile::Incident,
        "sql_limits": {"statement_timeout_ms": config.budgets.statement_timeout_ms(), "lock_timeout_ms": config.budgets.lock_timeout_ms(), "connect_timeout_seconds": 2},
        "profile_envelope": crate::capabilities::envelope(config.profile),
        "capabilities": crate::capabilities::initial_for(config),
        "sql_credentials_configured": config.password_file.is_some(),
    });
    match output {
        Output::Json => println!("{}", serde_json::to_string_pretty(&data)?),
        Output::Text => println!(
            "cnpg-watch {}\n{}",
            env!("CARGO_PKG_VERSION"),
            serde_json::to_string_pretty(&data)?
        ),
    }
    Ok(())
}

/// External error text is deliberately excluded from operator-facing summaries.
/// Classifiers retain structured codes; arbitrary stderr/URLs can contain credentials.
pub fn safe_error(class: &str) -> String {
    format!("{class}; details suppressed to protect credentials")
}

/// A small bounded diagnostic file keeps terminal drawing free of stderr logs.
struct DiagnosticLog {
    file: std::fs::File,
    bytes: usize,
}
impl std::io::Write for DiagnosticLog {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        const LIMIT: usize = 1024 * 1024;
        if self.bytes.saturating_add(buffer.len()) > LIMIT {
            self.file.set_len(0)?;
            self.bytes = 0;
        }
        let written = std::io::Write::write(&mut self.file, &buffer[..buffer.len().min(LIMIT)])?;
        self.bytes += written;
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.file)
    }
}
pub fn init_watch_logging(config: &Config) -> Result<()> {
    use std::fs::OpenOptions;
    if let Some(parent) = config
        .log_file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&config.log_file)?;
    let bytes = file.metadata()?.len() as usize;
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(std::sync::Mutex::new(DiagnosticLog { file, bytes }))
        .with_env_filter(if config.verbose {
            "cnpg_watch=debug"
        } else {
            "cnpg_watch=warn"
        })
        .try_init()
        .map_err(|_| anyhow::anyhow!("cannot initialize diagnostic logging"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn diagnostic_retention_is_bounded_and_keeps_new_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("debug.log");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        let mut log = DiagnosticLog { file, bytes: 0 };
        for _ in 0..2048 {
            log.write_all(&[b'x'; 1024]).unwrap();
        }
        log.write_all(b"latest record\n").unwrap();
        log.flush().unwrap();
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.len() <= 1024 * 1024);
        assert!(bytes.ends_with(b"latest record\n"));
    }
}
