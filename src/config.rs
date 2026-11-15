use crate::cli::{Options, Profile};
use anyhow::{Context, Result, ensure};
use directories::ProjectDirs;
use serde::Serialize;
use std::{io::Read, path::PathBuf, time::Duration};

#[derive(Debug, Clone, Serialize)]
pub struct Paths {
    pub config: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub dumps: PathBuf,
}

impl Paths {
    pub fn discover() -> Result<Self> {
        let dirs = ProjectDirs::from("io", "cnpg-watch", "cnpg-watch")
            .context("cannot determine user configuration directories")?;
        Ok(Self {
            config: dirs.config_dir().join("config.toml"),
            cache: dirs.cache_dir().into(),
            logs: dirs.data_local_dir().join("logs"),
            dumps: dirs.data_local_dir().join("dumps"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub context: Option<String>,
    pub namespace: Option<String>,
    pub all_namespaces: bool,
    pub cluster: Option<String>,
    pub selector: Option<String>,
    pub show_query_text: bool,
    pub pooler_stats: bool,
    pub pooler_socket: String,
    pub cluster_resources: bool,
    pub certificate_secrets: Vec<crate::model::Key>,
    pub operator_metrics_url: Option<String>,
    pub operator_namespace: Option<String>,
    pub operator_selector: Option<String>,
    pub operator_watch: bool,
    pub profile: Profile,
    pub refresh: Duration,
    pub refresh_explicit: bool,
    pub password_file: Option<PathBuf>,
    pub log_file: PathBuf,
    pub db_role: String,
    pub verbose: bool,
    pub paths: Paths,
    pub budgets: crate::budget::Budgets,
}

impl Config {
    /// clap has already applied CLI > environment precedence; this applies file > defaults.
    pub fn resolve(cli: Options, mut paths: Paths) -> Result<Self> {
        let explicit = cli.config.is_some();
        if let Some(path) = &cli.config {
            paths.config = path.clone();
        }
        let file: Options = match std::fs::File::open(&paths.config) {
            Ok(file) => {
                let mut text = String::new();
                file.take(65537)
                    .read_to_string(&mut text)
                    .context("cannot read UTF-8 configuration")?;
                ensure!(text.len() <= 65536, "config file exceeds 64 KiB limit");
                toml::from_str(&text).map_err(|_| {
                    anyhow::anyhow!(
                        "invalid config file (expected known TOML keys; contents suppressed)"
                    )
                })?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !explicit => Options::default(),
            Err(e) => return Err(e).context("cannot read config file"),
        };
        let context = cli.kube_context.or(file.kube_context);
        let cluster = cli.cluster.or(file.cluster);
        let selector = cli.selector.or(file.selector);
        let operator_selector = cli.operator_selector.or(file.operator_selector);
        for value in [selector.as_deref(), operator_selector.as_deref()]
            .into_iter()
            .flatten()
        {
            ensure!(
                value.len() <= 1024 && !value.chars().any(char::is_control),
                "selectors must be bounded and contain no control characters"
            );
        }
        let operator_namespace = cli.operator_namespace.or(file.operator_namespace);
        let all_namespaces = cli.all_namespaces.unwrap_or_else(|| {
            if cli.namespace.is_some() {
                false
            } else {
                file.all_namespaces.unwrap_or(false)
            }
        });
        let namespace = if cli.all_namespaces == Some(true) {
            cli.namespace
        } else {
            cli.namespace.or(file.namespace)
        };
        ensure!(
            !all_namespaces || namespace.is_none(),
            "namespace conflicts with all-namespaces"
        );
        let profile = cli.profile.or(file.profile).unwrap_or_default();
        let raw_refresh = cli.refresh.or(file.refresh);
        let refresh_explicit = raw_refresh.is_some();
        let refresh = match raw_refresh {
            Some(value) => humantime::parse_duration(&value)
                .context("refresh must be a duration such as 2s")?,
            None => {
                Duration::from_millis(crate::capabilities::envelope(profile).default_refresh_ms)
            }
        };
        ensure!(
            refresh >= Duration::from_millis(100),
            "refresh must be at least 100ms"
        );
        let db_role = cli
            .db_role
            .or(file.db_role)
            .unwrap_or_else(|| "watch_monitor".into());
        ensure!(
            !db_role.is_empty()
                && db_role != "postgres"
                && db_role.len() <= 63
                && db_role
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "db-role must be a dedicated non-postgres role (letters, digits, underscore; <=63 bytes)"
        );
        for value in [
            namespace.as_deref(),
            cluster.as_deref(),
            context.as_deref(),
            operator_namespace.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            ensure!(
                !value.trim().is_empty()
                    && value.len() <= 1024
                    && !value.chars().any(char::is_control),
                "scope values must be nonempty, bounded, and contain no control characters"
            );
        }
        if let Some(ns) = &namespace {
            crate::model::Key::new(ns.clone(), "scope")
                .map_err(|_| anyhow::anyhow!("invalid namespace"))?;
        }
        if let Some(ns) = &operator_namespace {
            crate::model::Key::new(ns.clone(), "scope")
                .map_err(|_| anyhow::anyhow!("invalid operator namespace"))?;
        }
        if let Some(name) = &cluster {
            crate::model::Key::new("scope", name.clone())
                .map_err(|_| anyhow::anyhow!("invalid cluster name"))?;
        }
        let raw_secrets = if cli.certificate_secret.is_empty() {
            file.certificate_secret
        } else {
            cli.certificate_secret
        };
        ensure!(
            raw_secrets.len() <= 8,
            "at most eight explicit certificate Secrets may be configured"
        );
        let mut certificate_secrets = raw_secrets
            .into_iter()
            .map(|s| {
                let (ns, name) = s
                    .split_once('/')
                    .context("certificate-secret must be namespace/name")?;
                crate::model::Key::new(ns, name)
                    .map_err(|_| anyhow::anyhow!("invalid certificate-secret reference"))
            })
            .collect::<Result<Vec<_>>>()?;
        certificate_secrets.sort();
        certificate_secrets.dedup();
        let operator_metrics_url = cli.operator_metrics_url.or(file.operator_metrics_url);
        if let Some(value) = &operator_metrics_url {
            ensure!(value.len() <= 2048, "metrics URL is too long");
            let url = reqwest::Url::parse(value).context("invalid operator metrics URL")?;
            ensure!(
                ["http", "https"].contains(&url.scheme())
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none(),
                "metrics URL must use HTTP(S), without embedded credentials, query, or fragment"
            );
        }
        let pooler_socket = cli
            .pooler_socket
            .or(file.pooler_socket)
            .unwrap_or_else(|| "/controller/run".into());
        ensure!(
            pooler_socket.starts_with('/')
                && pooler_socket.len() <= 100
                && !pooler_socket.chars().any(char::is_control),
            "pooler-socket must be a bounded absolute Unix socket directory"
        );
        let budgets = cli.budgets.or(file.budgets).unwrap_or_default();
        budgets.validate()?;
        if let Some(dir) = cli.dump_dir.or(file.dump_dir) {
            paths.dumps = dir;
        }
        let log_file = cli
            .log_file
            .or(file.log_file)
            .unwrap_or_else(|| paths.logs.join("debug.log"));
        Ok(Self {
            budgets,
            log_file,
            context,
            namespace,
            all_namespaces,
            cluster,
            selector,
            show_query_text: cli.show_query_text || file.show_query_text,
            pooler_stats: cli.pooler_stats || file.pooler_stats,
            pooler_socket,
            cluster_resources: cli.cluster_resources || file.cluster_resources,
            certificate_secrets,
            operator_metrics_url,
            operator_namespace,
            operator_selector,
            operator_watch: !(cli.no_operator_watch || file.no_operator_watch),
            profile,
            refresh_explicit,
            refresh,
            password_file: cli.password_file.or(file.password_file),
            db_role,
            verbose: cli.verbose || file.verbose,
            paths,
        })
    }
}
