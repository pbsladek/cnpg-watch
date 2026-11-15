//! Review-only guidance: CNPG's built-in peer console cannot become a stats-only login by PostgreSQL DDL.
use crate::{
    cli::{Output, PoolerRoleArgs},
    config::Config,
    model::Key,
};
use kube::{
    Api, ResourceExt,
    api::DynamicObject,
    core::{ApiResource, GroupVersionKind},
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Plan {
    pub schema_version: u32,
    pub namespace: Option<String>,
    pub pooler: String,
    pub transport: &'static str,
    pub identity: &'static str,
    pub mutation_supported: bool,
    pub live_layout: &'static str,
    pub guidance: Vec<&'static str>,
}
pub async fn plan(config: &Config, args: &PoolerRoleArgs) -> anyhow::Result<Plan> {
    Key::new(config.namespace.as_deref().unwrap_or("scope"), &args.pooler)
        .map_err(|_| anyhow::anyhow!("invalid pooler name"))?;
    let mut plan = Plan {
        schema_version: 1,
        namespace: config.namespace.clone(),
        pooler: args.pooler.clone(),
        transport: "pod exec; local Unix peer console",
        identity: "pgbouncer (built-in console identity; not a stats-only PostgreSQL role)",
        mutation_supported: false,
        live_layout: "not_checked",
        guidance: vec![
            "Prefer a dedicated stats_users identity where the installation supports password/certificate console authentication.",
            "Stock CNPG uses local peer authentication for its administration database. Creating a PostgreSQL role alone does not grant stats access.",
            "Review and configure custom console authentication through the owning Pooler/operator workflow. This command does not patch generated configuration, create Secrets, or change admin_users.",
            "For the pinned stock CNPG image, explicitly enable --pooler-stats to use the built-in local peer identity. Runtime sends only SHOW VERSION, SHOW POOLS, and SHOW STATS.",
            "Unsupported sockets, images, versions, or denied exec degrade statistics only. Kubernetes readiness and direct PostgreSQL monitoring continue.",
        ],
    };
    if args.diff {
        anyhow::ensure!(
            !config.all_namespaces,
            "pooler-role --diff requires one namespace"
        );
        let (client, namespace) = crate::kubernetes::client(config, &config.budgets)
            .await
            .map_err(|e| anyhow::anyhow!("Pooler review unavailable: {e}; contents suppressed"))?;
        let resource =
            ApiResource::from_gvk(&GroupVersionKind::gvk("postgresql.cnpg.io", "v1", "Pooler"));
        let api: Api<DynamicObject> = Api::namespaced_with(client, &namespace, &resource);
        let object = tokio::time::timeout(std::time::Duration::from_secs(5), api.get(&args.pooler))
            .await
            .map_err(|_| anyhow::anyhow!("Pooler review timed out"))?
            .map_err(|e| {
                anyhow::anyhow!(
                    "Pooler review failed: {}; contents suppressed",
                    crate::probes::api_error(&e)
                )
            })?;
        plan.namespace = object.namespace();
        plan.live_layout = if object
            .data
            .pointer("/spec/pgbouncer/authQuerySecret")
            .is_some()
            || object.data.pointer("/spec/pgbouncer/authType").is_some()
        {
            "custom_auth_requires_operator_review"
        } else {
            "stock_candidate_requires_actual_peer_probe"
        };
    }
    Ok(plan)
}
pub async fn run(config: &Config, args: &PoolerRoleArgs) -> anyhow::Result<()> {
    let plan = plan(config, args).await?;
    match args.output {
        Output::Json => println!("{}", serde_json::to_string_pretty(&plan)?),
        Output::Text => {
            println!(
                "Pooler {} — {}\nIdentity: {}\nLayout: {}; automatic mutation unsupported",
                plan.pooler, plan.transport, plan.identity, plan.live_layout
            );
            for item in plan.guidance {
                println!("- {item}");
            }
        }
    }
    Ok(())
}
