use crate::{
    cli::{RoleArgs, RoleProfile},
    config::Config,
    model::Key,
    probes,
};
use anyhow::{Result, ensure};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Serialize)]
pub struct RolePlan {
    schema_version: u32,
    postgres_major: u32,
    role: String,
    role_profile: RoleProfile,
    password_policy: &'static str,
    prune: bool,
    grants: Vec<&'static str>,
    settings: Vec<String>,
}
pub fn validate_role(role: &str) -> Result<()> {
    ensure!(
        !role.is_empty()
            && role.len() <= 63
            && role.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            && !role.starts_with("pg_")
            && !role.starts_with("cnpg_")
            && role != "postgres",
        "role must be a dedicated name (letters/digits/underscore; <=63 bytes; no pg_/cnpg_ prefix)"
    );
    Ok(())
}
impl RolePlan {
    pub fn new(config: &Config, args: &RoleArgs) -> Result<Self> {
        let role = args.role_name.clone().unwrap_or_else(|| {
            if args.role_profile == RoleProfile::Actions {
                "watch_actions".into()
            } else {
                config.db_role.clone()
            }
        });
        validate_role(&role)?;
        if args.role_profile == RoleProfile::Actions {
            ensure!(
                role != config.db_role,
                "action role must be separate from the monitoring role"
            );
            ensure!(
                !args.rotate_password,
                "action role is NOLOGIN and cannot rotate a login password"
            );
        }
        if args.rotate_password && args.apply {
            ensure!(
                config.password_file.is_some(),
                "--rotate-password --apply requires a protected --password-file"
            );
        }
        let grants = match args.role_profile {
            RoleProfile::Monitor => vec!["pg_monitor"],
            RoleProfile::MonitorReserved => vec!["pg_monitor", "pg_use_reserved_connections"],
            RoleProfile::Actions => vec!["pg_signal_backend"],
        };
        Ok(Self {
            schema_version: 1,
            postgres_major: 18,
            role,
            role_profile: args.role_profile,
            password_policy: if args.rotate_password {
                "rotate_from_environment"
            } else {
                "preserve"
            },
            prune: args.prune,
            grants,
            settings: vec![
                format!(
                    "statement_timeout={}ms",
                    config.budgets.statement_timeout_ms()
                ),
                format!("lock_timeout={}ms", config.budgets.lock_timeout_ms()),
                "idle_in_transaction_session_timeout=1000ms".into(),
            ],
        })
    }
    pub fn sql(&self) -> String {
        let role = &self.role;
        let login = if self.role_profile == RoleProfile::Actions {
            "NOLOGIN"
        } else {
            "LOGIN"
        };
        let mut sql = format!(
            r##"-- cnpg-watch setup schema 1; PostgreSQL 18; profile {:?}
-- Password policy: {}. No credentials are printed.
\set ON_ERROR_STOP on
BEGIN;
SET LOCAL search_path = pg_catalog;
SET LOCAL statement_timeout = '3s';
SET LOCAL lock_timeout = '500ms';
SELECT pg_advisory_xact_lock(178219, 2);
DO $setup$
BEGIN
  IF current_setting('server_version_num')::int NOT BETWEEN 180000 AND 189999 THEN
    RAISE EXCEPTION 'setup requires PostgreSQL 18';
  END IF;
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '{role}') THEN
    CREATE ROLE "{role}" {login} NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS INHERIT;
  END IF;
"##,
            self.role_profile, self.password_policy
        );
        if self.prune {
            sql.push_str(&format!("  ALTER ROLE \"{role}\" NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;\n"));
        } else {
            sql.push_str(&format!(r##"  IF EXISTS (SELECT FROM pg_roles WHERE rolname='{role}' AND (rolsuper OR rolcreatedb OR rolcreaterole OR rolreplication OR rolbypassrls)) THEN
    RAISE EXCEPTION 'existing role has elevated attributes; review before --prune';
  END IF;
"##));
        }
        sql.push_str(&format!(
            "END\n$setup$;\nALTER ROLE \"{role}\" {login} INHERIT;\n"
        ));
        if self.prune {
            let allowed = self
                .grants
                .iter()
                .map(|g| format!("'{g}'"))
                .collect::<Vec<_>>()
                .join(",");
            sql.push_str(&format!(r##"-- Explicit prune: remove extra direct role memberships only.
SELECT format('REVOKE %I FROM %I', parent.rolname, '{role}') FROM pg_auth_members m JOIN pg_roles parent ON parent.oid=m.roleid JOIN pg_roles child ON child.oid=m.member WHERE child.rolname='{role}' AND parent.rolname NOT IN ({allowed}) ORDER BY parent.rolname
\gexec
"##));
        }
        for grant in &self.grants {
            sql.push_str(&format!("GRANT {grant} TO \"{role}\" WITH INHERIT TRUE;\n"));
        }
        for setting in &self.settings {
            let (key, value) = setting.split_once('=').unwrap();
            sql.push_str(&format!("ALTER ROLE \"{role}\" SET {key} = '{value}';\n"));
        }
        sql.push_str(&format!(
            "COMMENT ON ROLE \"{role}\" IS 'Managed by cnpg-watch; {:?}; bootstrap only';\n",
            self.role_profile
        ));
        if self.password_policy != "preserve" {
            sql.push_str(
                r##"-- The caller supplies this environment value; never pass it in argv.
\getenv cnpg_watch_new_password CNPG_WATCH_NEW_PASSWORD
\if :{?cnpg_watch_new_password}
"##,
            );
            sql.push_str(&format!("SELECT format('ALTER ROLE %I PASSWORD %L', '{role}', :'cnpg_watch_new_password')\n\\gexec\n"));
            sql.push_str(
                "\\else\n\\warn 'rotation requires CNPG_WATCH_NEW_PASSWORD'\n\\quit 3\n\\endif\n",
            );
        }
        sql.push_str("COMMIT;\n");
        sql
    }
    fn inspect_sql(&self) -> String {
        let grants = self
            .grants
            .iter()
            .map(|g| format!("'{g}'"))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"BEGIN READ ONLY;
SELECT json_build_object(
  'server_version', current_setting('server_version_num')::int,
  'role', (SELECT json_build_object(
    'login', rolcanlogin, 'inherit', rolinherit,
    'comment', shobj_description(r.oid, 'pg_authid'),
    'unsafe', rolsuper OR rolcreatedb OR rolcreaterole OR rolreplication OR rolbypassrls,
    'memberships', COALESCE((SELECT json_agg(parent.rolname ORDER BY parent.rolname)
      FROM pg_auth_members m JOIN pg_roles parent ON parent.oid=m.roleid
      WHERE m.member=r.oid), '[]'::json),
    'usable_grants', COALESCE((SELECT json_agg(parent.rolname ORDER BY parent.rolname)
      FROM pg_roles parent WHERE parent.rolname IN ({grants})
      AND pg_has_role(r.oid, parent.oid, 'USAGE')), '[]'::json),
    'settings', COALESCE(to_json(rolconfig), '[]'::json))
    FROM pg_roles r WHERE rolname='{role}'));
COMMIT;
"#,
            role = self.role
        )
    }
}
#[derive(Deserialize)]
struct Inventory {
    server_version: u32,
    role: Option<RawRole>,
}
#[derive(Deserialize)]
struct RawRole {
    login: bool,
    inherit: bool,
    comment: Option<String>,
    #[serde(rename = "unsafe")]
    unsafe_: bool,
    memberships: Vec<String>,
    usable_grants: Vec<String>,
    settings: Vec<String>,
}
#[derive(Serialize)]
pub struct RoleReport {
    pub schema_version: u32,
    pub role: String,
    pub role_profile: RoleProfile,
    pub applied: bool,
    pub would_change: bool,
    pub missing: Vec<String>,
    pub extra: Vec<String>,
    pub password_policy: &'static str,
    pub validated: bool,
    pub role_authenticated: bool,
}
fn diff(plan: &RolePlan, inventory: Inventory) -> Result<RoleReport> {
    ensure!(
        (180000..190000).contains(&inventory.server_version),
        "setup requires PostgreSQL 18"
    );
    let mut missing = Vec::new();
    let mut extra = Vec::new();
    if let Some(role) = inventory.role {
        if !role.inherit {
            missing.push("inherit attribute".into());
        }
        let expected_comment = format!(
            "Managed by cnpg-watch; {:?}; bootstrap only",
            plan.role_profile
        );
        if role.comment.as_deref() != Some(&expected_comment) {
            missing.push("managed role comment".into());
        }
        if role.login != (plan.role_profile != RoleProfile::Actions) {
            missing.push("login attribute".into());
        }
        if role.unsafe_ {
            missing.push("unsafe existing role attributes (requires --prune)".into());
        }
        for g in &plan.grants {
            if !role.usable_grants.iter().any(|m| m == g) {
                missing.push(format!("grant {g}"));
            }
        }
        for g in role.memberships {
            if !plan.grants.contains(&g.as_str()) {
                extra.push(format!("membership {g}"));
            }
        }
        for setting in &plan.settings {
            if !role.settings.contains(setting) {
                missing.push(format!("setting {setting}"));
            }
        }
    } else {
        missing.push("role".into());
    }
    let would_change = !missing.is_empty()
        || (plan.prune && !extra.is_empty())
        || plan.password_policy != "preserve";
    Ok(RoleReport {
        schema_version: 1,
        role: plan.role.clone(),
        role_profile: plan.role_profile,
        applied: false,
        would_change,
        missing,
        extra,
        password_policy: plan.password_policy,
        validated: false,
        role_authenticated: false,
    })
}
async fn admin_sql(
    client: kube::Client,
    pod: &Key,
    role: &str,
    password: &SecretString,
    new_password: Option<&SecretString>,
    sql: &str,
    config: &Config,
) -> Result<Vec<u8>> {
    ensure!(
        !role.is_empty()
            && role.len() <= 63
            && role.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'),
        "invalid administrative login"
    );
    // Credentials and SQL travel only over stdin. Never echo SQL or remote stderr.
    let script = "IFS= read -r PGPASSWORD || exit 1; IFS= read -r CNPG_WATCH_NEW_PASSWORD || exit 1; export PGPASSWORD CNPG_WATCH_NEW_PASSWORD; export PGSSLMODE=require PGCONNECT_TIMEOUT=2 PGAPPNAME=cnpg-watch-setup PGOPTIONS='-c search_path=pg_catalog -c statement_timeout=3000 -c lock_timeout=500'; exec psql -XAtqw -v ON_ERROR_STOP=1 -v VERBOSITY=sqlstate -h 127.0.0.1 -d postgres -U \"$1\" -f -";
    let mut input = String::new();
    input.push_str(password.expose_secret());
    input.push('\n');
    if let Some(p) = new_password {
        input.push_str(p.expose_secret());
    }
    input.push('\n');
    input.push_str(sql);
    let input = SecretString::from(input);
    let mut budgets = config.budgets.clone();
    budgets.probe_timeout_ms = 5000;
    probes::exec_input(
        client,
        pod,
        vec![
            "sh".into(),
            "-c".into(),
            script.into(),
            "cnpg-watch-setup".into(),
            role.into(),
        ],
        input.expose_secret().as_bytes(),
        &budgets,
        &CancellationToken::new(),
    )
    .await
    .map_err(|e| anyhow::anyhow!("PostgreSQL setup: {e}; details suppressed"))
}
pub async fn reconcile(config: &Config, args: &RoleArgs, plan: &RolePlan) -> Result<RoleReport> {
    ensure!(
        !config.all_namespaces && config.namespace.is_some() && config.cluster.is_some(),
        "role setup requires explicit --namespace and --cluster"
    );
    if args.validate {
        ensure!(
            plan.role_profile != RoleProfile::Actions && config.password_file.is_some(),
            "--validate requires a monitor profile and protected --password-file"
        );
    }
    let admin_role=args.admin_role.as_deref().ok_or_else(||anyhow::anyhow!("--admin-role and --admin-password-file are required for diff/apply; no implicit admin fallback"))?;
    let password = probes::password_file(
        args.admin_password_file
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("--admin-password-file is required"))?,
    )
    .await
    .map_err(|e| anyhow::anyhow!("admin credential file: {e}; details suppressed"))?;
    let (client, namespace) = crate::kubernetes::client(config, &config.budgets)
        .await
        .map_err(|e| anyhow::anyhow!("setup client: {e}; details suppressed"))?;
    let pod = crate::kubernetes::primary(
        client.clone(),
        &namespace,
        config.cluster.as_deref().unwrap(),
    )
    .await
    .map_err(|e| anyhow::anyhow!("primary lookup: {e}; details suppressed"))?;
    let raw = admin_sql(
        client.clone(),
        &pod,
        admin_role,
        &password,
        None,
        &plan.inspect_sql(),
        config,
    )
    .await?;
    let inventory = serde_json::from_slice(&raw)
        .map_err(|_| anyhow::anyhow!("invalid setup inventory; contents suppressed"))?;
    let mut report = diff(plan, inventory)?;
    if args.apply {
        let new_password = if args.rotate_password {
            Some(
                probes::password_file(config.password_file.as_ref().unwrap())
                    .await
                    .map_err(|e| anyhow::anyhow!("new credential file: {e}; details suppressed"))?,
            )
        } else {
            None
        };
        if report.would_change {
            admin_sql(
                client.clone(),
                &pod,
                admin_role,
                &password,
                new_password.as_ref(),
                &plan.sql(),
                config,
            )
            .await?;
        }
        let raw = admin_sql(
            client.clone(),
            &pod,
            admin_role,
            &password,
            None,
            &plan.inspect_sql(),
            config,
        )
        .await?;
        let after = diff(
            plan,
            serde_json::from_slice(&raw)
                .map_err(|_| anyhow::anyhow!("invalid role validation; contents suppressed"))?,
        )?;
        ensure!(
            after.missing.is_empty() && (!plan.prune || after.extra.is_empty()),
            "resulting role failed grant/attribute validation"
        );
        report.applied = true;
        report.validated = true;
        if args.validate {
            let monitor_password = probes::password_file(config.password_file.as_ref().unwrap())
                .await
                .map_err(|e| anyhow::anyhow!("monitor credential: {e}; details suppressed"))?;
            let monitor = probes::PodExec::new(
                client,
                monitor_password,
                plan.role.clone(),
                config.budgets.clone(),
            );
            let bytes=monitor.execute_sql(&pod,probes::FAST_SQL,&CancellationToken::new()).await.map_err(|e|anyhow::anyhow!("role applied but monitor authentication/bundle failed: {e}; details suppressed"))?;
            probes::parse(&bytes, &plan.role).map_err(|e| {
                anyhow::anyhow!(
                    "role applied but monitor validation failed: {e}; details suppressed"
                )
            })?;
            report.role_authenticated = true;
        }
    }
    Ok(report)
}
