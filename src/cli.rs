use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    version,
    about,
    long_about = "Observe CloudNativePG topology and PostgreSQL runtime state.\nWatch mode is read-only. Bootstrap operations belong to explicit setup commands."
)]
pub struct Cli {
    #[command(flatten)]
    pub options: Options,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Default, Args, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    #[arg(long, global = true, env = "CNPG_WATCH_CONFIG")]
    #[serde(skip)]
    pub config: Option<PathBuf>,
    #[arg(long, global = true, env = "CNPG_WATCH_CONTEXT")]
    pub kube_context: Option<String>,
    #[arg(
        short,
        long,
        global = true,
        env = "CNPG_WATCH_NAMESPACE",
        conflicts_with = "all_namespaces"
    )]
    pub namespace: Option<String>,
    #[arg(long, global = true, env = "CNPG_WATCH_ALL_NAMESPACES", num_args = 0..=1, require_equals = true, default_missing_value = "true")]
    pub all_namespaces: Option<bool>,
    #[arg(long, global = true)]
    pub cluster: Option<String>,
    /// Kubernetes label selector for Cluster resources.
    #[arg(long, global = true)]
    pub selector: Option<String>,
    /// Display bounded query text in incident wait diagnostics; dumps always redact it.
    #[arg(long, global = true)]
    pub show_query_text: bool,
    /// Read fixed SHOW commands over CNPG's built-in local-peer PgBouncer console.
    #[arg(long, global = true)]
    pub pooler_stats: bool,
    #[arg(long, global = true)]
    pub pooler_socket: Option<String>,
    /// Observe optional cluster-scoped webhook and image catalog metadata.
    #[arg(long, global = true)]
    pub cluster_resources: bool,
    /// Explicit named certificate Secret reads (namespace/name); no Secret lists/watches.
    #[arg(long, global = true)]
    pub certificate_secret: Vec<String>,
    /// Optional bounded metrics scrape; disabled unless a URL is supplied.
    #[arg(long, global = true)]
    pub operator_metrics_url: Option<String>,
    #[arg(long, global = true)]
    pub operator_namespace: Option<String>,
    #[arg(long, global = true)]
    pub operator_selector: Option<String>,
    #[arg(long, global = true)]
    pub no_operator_watch: bool,
    #[arg(long, global = true, env = "CNPG_WATCH_PROFILE")]
    pub profile: Option<Profile>,
    #[arg(long, global = true, env = "CNPG_WATCH_REFRESH")]
    pub refresh: Option<String>,
    #[arg(long, global = true, env = "CNPG_WATCH_PASSWORD_FILE")]
    pub password_file: Option<PathBuf>,
    #[arg(long, global = true, env = "CNPG_WATCH_LOG_FILE")]
    pub log_file: Option<PathBuf>,
    #[arg(long, global = true, env = "CNPG_WATCH_DUMP_DIR")]
    pub dump_dir: Option<PathBuf>,
    #[arg(long, global = true, env = "CNPG_WATCH_DB_ROLE")]
    pub db_role: Option<String>,
    #[arg(long, global = true)]
    pub verbose: bool,
    #[arg(skip)]
    pub budgets: Option<crate::budget::Budgets>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    Minimal,
    #[default]
    Standard,
    Incident,
    Secure,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate shell completions without reading config or contacting Kubernetes.
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Print the Markdown command reference without reading config or contacting Kubernetes.
    CommandReference,
    /// Open the observe-only dashboard (also the default command).
    Watch,
    /// Print paths and local configuration without contacting Kubernetes.
    Info {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
    /// Check scoped access and the actual monitoring role without bootstrap changes.
    Doctor {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
        /// Also check optional catalog/webhook object access.
        #[arg(long)]
        include_cluster_resources: bool,
    },
    /// Review or explicitly apply runtime RBAC and PostgreSQL roles.
    Setup {
        #[command(subcommand)]
        command: Setup,
    },
}

#[derive(Debug, Subcommand)]
pub enum Setup {
    KubernetesRbac(RbacArgs),
    PostgresRole(RoleArgs),
    PoolerRole(PoolerRoleArgs),
}

#[derive(Debug, Args)]
#[group(skip)]
#[command(group(clap::ArgGroup::new("subject").required(true).multiple(false).args(["service_account", "user", "group"])))]
pub struct RbacArgs {
    #[arg(long)]
    pub service_account: Option<String>,
    #[arg(long, requires = "service_account")]
    pub service_account_namespace: Option<String>,
    #[arg(long)]
    pub user: Option<String>,
    #[arg(long)]
    pub group: Option<String>,
    #[arg(long, default_value = "cnpg-watch")]
    pub name: String,
    #[arg(long)]
    pub include_cluster_resources: bool,
    #[arg(long, conflicts_with_all = ["apply", "diff"])]
    pub dry_run: bool,
    #[arg(long, conflicts_with = "apply")]
    pub diff: bool,
    #[arg(long)]
    pub apply: bool,
    #[arg(long)]
    pub prune: bool,
    #[arg(long, value_enum, default_value = "yaml")]
    pub output: RbacOutput,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RbacOutput {
    Yaml,
    Json,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleProfile {
    Monitor,
    MonitorReserved,
    Actions,
}

#[derive(Debug, Args)]
pub struct RoleArgs {
    #[arg(long, value_enum, default_value = "monitor")]
    pub role_profile: RoleProfile,
    #[arg(long)]
    pub role_name: Option<String>,
    #[arg(long, conflicts_with_all = ["apply", "diff"])]
    pub print_sql: bool,
    #[arg(long, conflicts_with_all = ["apply", "diff"])]
    pub dry_run: bool,
    #[arg(long, conflicts_with = "apply")]
    pub diff: bool,
    #[arg(long)]
    pub apply: bool,
    #[arg(long)]
    pub prune: bool,
    /// Preserve existing password (the default); new roles have no password.
    #[arg(long, conflicts_with = "rotate_password")]
    pub preserve_password: bool,
    /// Set the password from the protected global --password-file; never print it.
    #[arg(long)]
    pub rotate_password: bool,
    /// Explicit administrative login for setup only; never used by watch.
    #[arg(long, requires = "admin_password_file")]
    pub admin_role: Option<String>,
    #[arg(long, requires = "admin_role")]
    pub admin_password_file: Option<PathBuf>,
    /// After apply, authenticate as the monitor and run the shipped fast bundle.
    #[arg(long, requires = "apply")]
    pub validate: bool,
    #[arg(long, value_enum, default_value = "sql")]
    pub output: RoleOutput,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RoleOutput {
    Sql,
    Json,
    Text,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Output {
    Text,
    Json,
}

#[derive(Debug, Args)]
pub struct PoolerRoleArgs {
    #[arg(long)]
    pub pooler: String,
    #[arg(long, conflicts_with = "diff")]
    pub dry_run: bool,
    #[arg(long)]
    pub diff: bool,
    #[arg(long, value_enum, default_value = "text")]
    pub output: Output,
}
