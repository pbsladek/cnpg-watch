use anyhow::Result;
use clap::Parser;
use cnpg_watch::{
    cli::{Cli, Command},
    config::{Config, Paths},
    support,
    terminal::TerminalGuard,
};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("cnpg-watch: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Some(Command::Completions { shell }) => {
            return Ok(cnpg_watch::cli_docs::write(
                &cnpg_watch::cli_docs::completion(*shell),
            )?);
        }
        Some(Command::CommandReference) => {
            return Ok(cnpg_watch::cli_docs::write(
                cnpg_watch::cli_docs::reference().as_bytes(),
            )?);
        }
        _ => {}
    }
    let config = Config::resolve(cli.options, Paths::discover()?)?;
    match cli.command.unwrap_or(Command::Watch) {
        Command::Info { output } => support::info(&config, output),
        Command::Doctor {
            output,
            include_cluster_resources,
        } => cnpg_watch::doctor::run(&config, output, include_cluster_resources).await,
        Command::Setup { command } => cnpg_watch::setup::run(&config, command).await,
        Command::Completions { .. } | Command::CommandReference => {
            unreachable!("offline command handled above")
        }
        Command::Watch => {
            let mut terminal = TerminalGuard::enter()?;
            support::init_watch_logging(&config)?;
            cnpg_watch::app::watch(config, &mut terminal).await?;
            Ok(())
        }
    }
}
