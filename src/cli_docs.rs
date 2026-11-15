//! Offline CLI artifacts generated from the same clap command tree as argument parsing.
use crate::cli::Cli;
use clap::{Command, CommandFactory};
use std::io::{self, Write};

pub fn completion(shell: clap_complete::Shell) -> Vec<u8> {
    let mut output = Vec::new();
    clap_complete::generate(shell, &mut Cli::command(), "cnpg-watch", &mut output);
    output
}
pub fn reference() -> String {
    fn visit(mut command: Command, path: String, output: &mut String) {
        command.build();
        let env_args: Vec<_> = command
            .get_arguments()
            .filter(|a| a.get_env().is_some())
            .map(|a| a.get_id().clone())
            .collect();
        for id in env_args {
            command = command.mut_arg(id, |arg| arg.hide_env_values(true));
        }
        command = command
            .bin_name(&path)
            .term_width(100)
            .disable_colored_help(true);
        output.push_str(&format!(
            "\n## `{path}`\n\n```text\n{}\n```\n",
            command.render_long_help()
        ));
        for child in command.get_subcommands().filter(|c| c.get_name() != "help") {
            visit(
                child.clone(),
                format!("{path} {}", child.get_name()),
                output,
            );
        }
    }
    let mut output = format!(
        "# Command reference\n\nGenerated from cnpg-watch {}. Run `cnpg-watch command-reference` to regenerate.\n",
        env!("CARGO_PKG_VERSION")
    );
    visit(Cli::command(), "cnpg-watch".into(), &mut output);
    output
}
pub fn write(bytes: &[u8]) -> io::Result<()> {
    io::stdout().lock().write_all(bytes)
}
