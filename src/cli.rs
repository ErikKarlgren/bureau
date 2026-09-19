//! The command line interface.

use clap::{Args, Parser, Subcommand};

/// Git-based work dossiers for legacy systems.
#[derive(Debug, Parser)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// What bureau can do.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new dossier or entry.
    New {
        #[command(subcommand)]
        command: NewCommand,
    },
    /// Log a line of work against a dossier.
    Worklog(WorklogArgs),
}

/// What `bureau new` can create.
#[derive(Debug, Subcommand)]
pub enum NewCommand {
    /// Create a new dossier and commit it.
    Dossier(DossierArgs),
    /// Create today's daily entry and commit it.
    Entry,
}

/// Arguments for `bureau new dossier`.
#[derive(Debug, Args)]
pub struct DossierArgs {
    /// Dossier name (no need to quote if it has spaces)
    #[arg(required = true, num_args = 1..)]
    pub name: Vec<String>,
}

/// Arguments for `bureau worklog`.
#[derive(Debug, Args)]
pub struct WorklogArgs {
    /// Part of a dossier's name to log against
    pub filter: Option<String>,

    /// Date to log against, as YYYY-MM-DD (defaults to today)
    #[arg(long, value_name = "YYYY-MM-DD")]
    pub date: Option<String>,

    /// Choose a dossier from all of them
    #[arg(long)]
    pub menu: bool,
}
