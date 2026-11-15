//! Generate reviewable CLI artifacts without configuration or network access.
use clap_complete::Shell;
fn main() -> anyhow::Result<()> {
    let out = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("usage: generate_cli_artifacts OUTPUT_DIRECTORY"))?,
    );
    std::fs::create_dir_all(out.join("completions"))?;
    for (name, shell) in [
        ("cnpg-watch.bash", Shell::Bash),
        ("_cnpg-watch", Shell::Zsh),
        ("cnpg-watch.fish", Shell::Fish),
        ("cnpg-watch.ps1", Shell::PowerShell),
        ("cnpg-watch.elv", Shell::Elvish),
    ] {
        std::fs::write(
            out.join("completions").join(name),
            cnpg_watch::cli_docs::completion(shell),
        )?;
    }
    std::fs::write(
        out.join("command-reference.md"),
        cnpg_watch::cli_docs::reference(),
    )?;
    Ok(())
}
