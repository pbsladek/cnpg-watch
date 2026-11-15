//! Bootstrap I/O is reachable only through explicit setup command dispatch.
pub mod pooler;
pub mod rbac;
pub mod roles;
use crate::{
    cli::{RbacOutput, RoleOutput, Setup},
    config::Config,
};
use anyhow::Result;
pub async fn run(config: &Config, command: Setup) -> Result<()> {
    match command {
        Setup::KubernetesRbac(args) => {
            let desired = rbac::generate(config, &args)?;
            if !args.apply && !args.diff {
                match args.output {
                    RbacOutput::Yaml => print!("{}", rbac::yaml(&desired)?),
                    _ => println!("{}", serde_json::to_string_pretty(&desired)?),
                }
            } else {
                let (client, _) = crate::kubernetes::client(config, &config.budgets)
                    .await
                    .map_err(|e| {
                        anyhow::anyhow!("Kubernetes setup failed: {e}; details suppressed")
                    })?;
                let report = rbac::reconcile(client, desired, args.apply, args.prune).await?;
                match args.output {
                    RbacOutput::Yaml => print!("{}", serde_yaml::to_string(&report)?),
                    _ => println!("{}", serde_json::to_string_pretty(&report)?),
                }
            }
        }
        Setup::PoolerRole(args) => pooler::run(config, &args).await?,
        Setup::PostgresRole(args) => {
            let plan = roles::RolePlan::new(config, &args)?;
            if !args.apply && !args.diff {
                match args.output {
                    RoleOutput::Json => println!("{}", serde_json::to_string_pretty(&plan)?),
                    _ => print!("{}", plan.sql()),
                }
            } else {
                let report = roles::reconcile(config, &args, &plan).await?;
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
    }
    Ok(())
}
