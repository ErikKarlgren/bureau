//! bureau: git-based work dossiers for legacy systems.

mod cli;
mod commands;
mod date;
mod dossier;
mod git;
mod prompt;
mod report;
mod style;
mod tasks;
mod template;
mod worklog;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;

/// What every fallible step returns: `main` prints the error and exits with 1.
pub type Result<T> = anyhow::Result<T>;

fn main() -> ExitCode {
    let cli = Cli::parse();

    match commands::run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let error_palette = style::for_stderr();
            eprintln!("{}", error_palette.error_line(format_args!("{error:?}")));
            ExitCode::FAILURE
        }
    }
}
