//! The command line interface.

use std::convert::Infallible;
use std::str::FromStr;

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
    /// List the tasks in your dossiers and entries.
    Tasks(TasksArgs),
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

/// Arguments for `bureau tasks`.
#[derive(Debug, Args)]
pub struct TasksArgs {
    /// List one dossier: the newest on its own, or the one matching PATTERN
    #[arg(
        long,
        value_name = "PATTERN",
        num_args = 0..=1,
        default_missing_value = ""
    )]
    pub filter: Option<Filter>,

    /// Choose a dossier from all of them
    #[arg(long)]
    pub menu: bool,

    /// Also list finished and cancelled dossier tasks
    #[arg(long)]
    pub all: bool,
}

/// Which dossier `bureau tasks` is about, when `--filter` was given.
///
/// The flag has two meanings and the field is an `Option` because of it: no
/// `--filter` at all is `None`, `--filter` with nothing after it is
/// [`Filter::Newest`], and `--filter PATTERN` is [`Filter::Matching`]. An
/// `Option<Option<_>>` would say the same thing less clearly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    /// `--filter` on its own: the most recently modified dossier.
    Newest,
    /// `--filter PATTERN`: the dossier whose name matches it.
    Matching(String),
}

impl Filter {
    /// The pattern to match dossier names against, when there is one.
    #[must_use]
    pub const fn pattern(&self) -> Option<&str> {
        match self {
            Self::Matching(pattern) => Some(pattern.as_str()),
            Self::Newest => None,
        }
    }
}

impl FromStr for Filter {
    type Err = Infallible;

    /// The empty string is clap's "the flag was given with no value", which is
    /// what `default_missing_value` hands over; anything else is the pattern.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(if value.is_empty() {
            Self::Newest
        } else {
            Self::Matching(value.to_owned())
        })
    }
}
