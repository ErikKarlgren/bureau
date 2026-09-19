//! Subcommand implementations.

mod new;
mod worklog;

use crate::cli::{Command, NewCommand};
use crate::Result;

/// Run whichever subcommand the user asked for.
///
/// # Errors
///
/// Whatever the chosen subcommand fails with, ready for `main` to print.
pub fn run(command: Command) -> Result<()> {
    match command {
        Command::New { command: subcommand } => match subcommand {
            NewCommand::Dossier(args) => new::dossier(&args),
            NewCommand::Entry => new::entry(),
        },
        Command::Worklog(args) => worklog::run(&args),
    }
}
