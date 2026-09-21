//! bureau: git-based work dossiers for legacy systems.

mod cli;
mod commands;
mod dossier;
mod git;
mod tasks;
mod template;
mod worklog;

use clap::Parser;

use crate::cli::Cli;

/// What every fallible step returns: `main` prints the report and exits with 1.
pub type Result<T> = anyhow::Result<T>;

fn main() -> Result<()> {
    let cli = Cli::parse();
    commands::run(cli.command)
}
