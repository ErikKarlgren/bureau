//! Subcommand implementations.

mod doctor;
mod files;
mod new;
mod paths;
mod report;
mod selection;
mod sources;
mod tasks;
mod worklog;

#[cfg(test)]
pub mod tests;

use std::process::ExitCode;

use crate::Result;
use crate::cli::{Command, NewCommand};

/// Run whichever subcommand the user asked for.
///
/// # Errors
///
/// Whatever the chosen subcommand fails with, ready for `main` to print.
///
/// A successful run returns the `ExitCode` the command decided on: `SUCCESS`
/// for the action commands, and whatever `doctor` chooses (`FAILURE` when it
/// reports formatting errors, even though the run itself succeeded).
pub fn run(command: Command) -> Result<ExitCode> {
    match command {
        Command::New {
            command: subcommand,
        } => {
            match subcommand {
                NewCommand::Dossier(args) => new::dossier(&args)?,
                NewCommand::Entry => new::entry()?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Worklog(args) => {
            worklog::run(&args)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Tasks(args) => {
            tasks::run(&args)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Report(args) => {
            report::run(&args)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Doctor => doctor::run(),
    }
}
