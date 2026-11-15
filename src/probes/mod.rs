pub mod registry;
use crate::{budget::Budgets, model::*};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api, Client,
    api::{AttachParams, AttachedProcess},
};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::{net::IpAddr, path::Path, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    time::{Instant, timeout_at},
};
use tokio_util::sync::CancellationToken;

pub const FAST_SQL: &str = include_str!("fast.sql");
pub const FAST_BUNDLE_ID: &str = "postgres18.fast.v1";
#[derive(Debug, serde::Serialize)]
pub struct QuerySpec {
    pub id: &'static str,
    pub class: &'static str,
    pub min_server: u32,
    pub max_server_exclusive: u32,
    pub role: &'static str,
    pub schema_version: u32,
    pub required_functions: &'static [&'static str],
    pub required_views: &'static [&'static str],
}
pub const FAST_SPEC: QuerySpec = QuerySpec {
    id: FAST_BUNDLE_ID,
    class: "fast_sql",
    min_server: 180000,
    max_server_exclusive: 190000,
    role: "pg_monitor",
    schema_version: 1,
    required_functions: &[
        "pg_control_system",
        "pg_control_checkpoint",
        "pg_is_in_recovery",
        "pg_current_wal_lsn",
        "pg_last_wal_receive_lsn",
        "pg_last_wal_replay_lsn",
    ],
    required_views: &["pg_stat_replication", "pg_stat_archiver", "pg_roles"],
};
pub const MIN_SERVER: u32 = FAST_SPEC.min_server;
pub const MAX_SERVER: u32 = FAST_SPEC.max_server_exclusive;

pub async fn password_file(path: &Path) -> Result<SecretString, ErrorClass> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| ErrorClass::Authentication)?;
    let meta = file
        .metadata()
        .await
        .map_err(|_| ErrorClass::Authentication)?;
    if !meta.is_file() || meta.len() > 4096 {
        return Err(ErrorClass::Authentication);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(ErrorClass::Authentication);
        }
    }
    let bytes = read_capped(file, 4096)
        .await
        .map_err(|_| ErrorClass::Authentication)?;
    let mut value = String::from_utf8(bytes).map_err(|_| ErrorClass::Authentication)?;
    let length = value.trim_end_matches(['\r', '\n']).len();
    value.truncate(length);
    if value.is_empty() || value.contains(['\r', '\n', '\0']) {
        return Err(ErrorClass::Authentication);
    }
    Ok(SecretString::from(value))
}

pub fn api_error(error: &kube::Error) -> ErrorClass {
    match error {
        kube::Error::Api(response) if response.code == 403 => ErrorClass::Forbidden,
        kube::Error::Api(response) if response.code == 401 => ErrorClass::Authentication,
        kube::Error::Api(response) if response.code == 404 => ErrorClass::Unavailable,
        kube::Error::Api(response) if response.code == 429 => ErrorClass::Overloaded,
        _ => ErrorClass::Transport,
    }
}
fn stderr_class(bytes: &[u8]) -> ErrorClass {
    let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    if text.contains("57014") || text.contains("statement timeout") {
        ErrorClass::Timeout
    } else if text.contains("53300")
        || text.contains("too many clients")
        || text.contains("too many connections")
        || text.contains("remaining connection slots")
        || text.contains("reserved connections are in use")
    {
        ErrorClass::Overloaded
    } else if text.contains("28p01")
        || text.contains("password authentication")
        || text.contains("no password")
        || text.contains("28000")
    {
        ErrorClass::Authentication
    } else if text.contains("42501") || text.contains("permission denied") {
        ErrorClass::Forbidden
    } else {
        ErrorClass::Transport
    }
}

pub async fn read_capped(
    mut reader: impl AsyncRead + Unpin,
    limit: usize,
) -> Result<Vec<u8>, ErrorClass> {
    let mut result = Vec::with_capacity(limit.min(8192));
    let mut chunk = [0u8; 4096];
    loop {
        let read = reader
            .read(&mut chunk)
            .await
            .map_err(|_| ErrorClass::Transport)?;
        if read == 0 {
            return Ok(result);
        }
        if result.len() + read > limit {
            return Err(ErrorClass::OutputLimit);
        }
        result.extend_from_slice(&chunk[..read]);
    }
}

#[derive(Clone)]
pub struct PodExec {
    client: Client,
    password: std::sync::Arc<SecretString>,
    role: String,
    budgets: Budgets,
}
impl PodExec {
    pub fn new(client: Client, password: SecretString, role: String, budgets: Budgets) -> Self {
        Self {
            client,
            password: std::sync::Arc::new(password),
            role,
            budgets,
        }
    }
    pub async fn execute(
        &self,
        ticket: &ProbeTicket,
        cancel: &CancellationToken,
    ) -> Result<FastSample, ErrorClass> {
        let bytes = self.execute_sql(&ticket.pod.key, FAST_SQL, cancel).await?;
        parse(&bytes, &self.role)
    }
    /// Static SQL is also used by isolated failure-injection contracts; no CLI accepts SQL.
    pub async fn execute_sql(
        &self,
        pod: &Key,
        sql: &'static str,
        cancel: &CancellationToken,
    ) -> Result<Vec<u8>, ErrorClass> {
        self.execute_sql_with_limits(pod, sql, &self.budgets, cancel)
            .await
    }
    pub async fn execute_bundle(
        &self,
        pod: &Key,
        bundle: registry::Bundle,
        cancel: &CancellationToken,
    ) -> Result<Vec<u8>, ErrorClass> {
        let mut budgets = self.budgets.clone();
        if matches!(
            bundle.spec().class,
            registry::Class::Medium | registry::Class::Deep
        ) {
            budgets.probe_timeout_ms = budgets.diagnostic_timeout_ms;
        }
        self.execute_sql_with_limits(pod, bundle.sql(), &budgets, cancel)
            .await
    }
    async fn execute_sql_with_limits(
        &self,
        pod: &Key,
        sql: &'static str,
        budgets: &Budgets,
        cancel: &CancellationToken,
    ) -> Result<Vec<u8>, ErrorClass> {
        let script = format!(
            "IFS= read -r PGPASSWORD || exit 1; export PGPASSWORD; export LC_ALL=C PGAPPNAME=cnpg-watch PGCONNECT_TIMEOUT=2 PGSSLMODE=require PGOPTIONS='-c search_path=pg_catalog -c statement_timeout={statement} -c lock_timeout={lock} -c idle_in_transaction_session_timeout=1000 -c default_transaction_read_only=on'; exec psql -X -A -t -w -v ON_ERROR_STOP=1 -v VERBOSITY=sqlstate -h 127.0.0.1 -d postgres -U \"$1\" -c \"$2\"",
            statement = if budgets.probe_timeout_ms == self.budgets.probe_timeout_ms {
                budgets.statement_timeout_ms()
            } else {
                (budgets.probe_timeout_ms / 2).min(1000)
            },
            lock = budgets.lock_timeout_ms()
        );
        let command = vec![
            "sh".into(),
            "-c".into(),
            script,
            "cnpg-watch".into(),
            self.role.clone(),
            sql.into(),
        ];
        let input = SecretString::from(format!("{}\n", self.password.expose_secret()));
        exec_input(
            self.client.clone(),
            pod,
            command,
            input.expose_secret().as_bytes(),
            budgets,
            cancel,
        )
        .await
    }
}

/// Bounded exec attachment shared by the monitor and explicit bootstrap command.
/// Input is never copied into Kubernetes command arguments or returned errors.
pub async fn exec_input(
    client: Client,
    pod: &Key,
    command: Vec<String>,
    input: &[u8],
    budgets: &Budgets,
    cancel: &CancellationToken,
) -> Result<Vec<u8>, ErrorClass> {
    exec_input_in(client, pod, "postgres", command, input, budgets, cancel).await
}

pub async fn exec_input_in(
    client: Client,
    pod: &Key,
    container: &str,
    command: Vec<String>,
    input: &[u8],
    budgets: &Budgets,
    cancel: &CancellationToken,
) -> Result<Vec<u8>, ErrorClass> {
    let api: Api<Pod> = Api::namespaced(client, &pod.namespace);
    let mut process: Option<AttachedProcess> = None;
    let deadline = Instant::now() + budgets.timeout();
    let cleanup = Duration::from_millis(100).min(budgets.timeout() / 4);
    let operation = async {
        process = Some(
            api.exec(
                &pod.name,
                command,
                &AttachParams::default()
                    .container(container)
                    .stdin(true)
                    .stdout(true)
                    .stderr(true)
                    .tty(false),
            )
            .await
            .map_err(|e| api_error(&e))?,
        );
        let p = process.as_mut().unwrap();
        let mut stdin = p.stdin().ok_or(ErrorClass::Transport)?;
        let stdout = p.stdout().ok_or(ErrorClass::Transport)?;
        let stderr = p.stderr().ok_or(ErrorClass::Transport)?;
        let status = p.take_status().ok_or(ErrorClass::Transport)?;
        let write = async {
            stdin
                .write_all(input)
                .await
                .map_err(|_| ErrorClass::Transport)?;
            stdin.shutdown().await.map_err(|_| ErrorClass::Transport)
        };
        let (out, err, status, ()) = tokio::try_join!(
            read_capped(stdout, budgets.stdout_bytes),
            read_capped(stderr, budgets.stderr_bytes),
            async { status.await.ok_or(ErrorClass::Transport) },
            write
        )?;
        if status.status.as_deref() != Some("Success") {
            return Err(stderr_class(&err));
        }
        Ok(out)
    };
    let result = tokio::select! {
        _=cancel.cancelled()=>Err(ErrorClass::Cancelled),
        result=timeout_at(deadline-cleanup,operation)=>result.unwrap_or(Err(ErrorClass::Timeout)),
    };
    if let Some(p) = process {
        if result.is_err() {
            p.abort();
        }
        let joined = timeout_at(deadline, p.join()).await;
        if result.is_ok() && !matches!(joined, Ok(Ok(()))) {
            return Err(ErrorClass::Transport);
        }
    }
    result
}

#[derive(Deserialize)]
struct RawPeer {
    application: String,
    address: Option<IpAddr>,
    replay: Option<String>,
}
#[derive(Deserialize)]
struct Raw {
    server_version: u32,
    current_user: String,
    session_user: String,
    superuser: bool,
    monitor: bool,
    server_time: String,
    in_recovery: bool,
    system_id: String,
    timeline: u32,
    current: Option<String>,
    received: Option<String>,
    replay: Option<String>,
    last_replayed_transaction_age_ms: Option<f64>,
    peers: Vec<RawPeer>,
    #[serde(default)]
    connections: Option<registry::Connections>,
    archive: Archive,
}
pub fn parse(bytes: &[u8], expected_role: &str) -> Result<FastSample, ErrorClass> {
    let raw: Raw = serde_json::from_slice(bytes).map_err(|_| ErrorClass::InvalidData)?;
    if !(MIN_SERVER..MAX_SERVER).contains(&raw.server_version) {
        return Err(ErrorClass::Unsupported);
    }
    if raw.superuser
        || raw.current_user != expected_role
        || raw.session_user != expected_role
        || !raw.monitor
    {
        return Err(ErrorClass::Forbidden);
    }
    if raw.peers.len() > 128
        || raw.peers.iter().any(|p| p.application.len() > 253)
        || raw.server_time.len() > 64
    {
        return Err(ErrorClass::InvalidData);
    }
    if chrono::DateTime::parse_from_rfc3339(&raw.server_time).is_err() {
        return Err(ErrorClass::InvalidData);
    }
    for timestamp in [
        &raw.archive.stats_reset,
        &raw.archive.last_archived_time,
        &raw.archive.last_failed_time,
    ]
    .into_iter()
    .flatten()
    {
        if timestamp.len() > 64 || chrono::DateTime::parse_from_rfc3339(timestamp).is_err() {
            return Err(ErrorClass::InvalidData);
        }
    }
    let lsn = |value: Option<String>| {
        value
            .map(|v| v.parse().map_err(|_| ErrorClass::InvalidData))
            .transpose()
    };
    let current = lsn(raw.current)?;
    let received = lsn(raw.received)?;
    let replay = lsn(raw.replay)?;
    if !raw.in_recovery && current.is_none() {
        return Err(ErrorClass::InvalidData);
    }
    let peers = raw
        .peers
        .into_iter()
        .map(|p| {
            Ok(Peer {
                application: p.application,
                address: p.address,
                replay: lsn(p.replay)?,
            })
        })
        .collect::<Result<_, ErrorClass>>()?;
    Ok(FastSample {
        server_version: raw.server_version,
        role: if raw.in_recovery {
            Role::Replica
        } else {
            Role::Primary
        },
        system_id: raw.system_id.parse().map_err(|_| ErrorClass::InvalidData)?,
        timeline: raw.timeline,
        current,
        received,
        replay,
        server_time: raw.server_time,
        last_replayed_transaction_age_ms: raw
            .last_replayed_transaction_age_ms
            .filter(|v| v.is_finite() && *v >= 0. && *v <= u64::MAX as f64)
            .map(|v| v as u64),
        peers,
        connections: raw.connections,
        archive: raw.archive,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_reserved_slot_errors_are_overload_without_sqlstate() {
        assert_eq!(stderr_class(b"psql: FATAL: remaining connection slots are reserved for roles with privileges of the pg_use_reserved_connections role"),ErrorClass::Overloaded);
    }
    #[tokio::test]
    async fn output_is_bounded_before_parsing() {
        assert_eq!(
            read_capped(&b"0123456789"[..], 4).await,
            Err(ErrorClass::OutputLimit)
        );
        assert_eq!(read_capped(&b"1234"[..], 4).await.unwrap(), b"1234");
    }
}
