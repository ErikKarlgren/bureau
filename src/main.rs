//! bureau: git-based work dossiers for legacy systems.

mod cli;
mod commands;
mod dossier;
mod git;
mod template;

use std::error::Error;

use clap::Parser;

use crate::cli::Cli;

/// What every fallible step returns: `main` prints the error and exits with 1.
pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let cli = Cli::parse();
    commands::run(cli.command)
}
