//! Choosing one dossier out of several, for the commands that need one.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use dialoguer::FuzzySelect;

use crate::commands::sources::{by_recency, newest};
use crate::worklog;

/// What the picker's prompt says about leaving without choosing.
pub const CANCEL_HINT: &str = "Esc or q to cancel";

/// What the user asked for, in terms the selection can act on.
///
/// There is no "was a filter given" field, because a request only reaches
/// [`select`] when one was: callers check that themselves before calling.
#[derive(Debug, Clone, Copy, Default)]
pub struct Request<'a> {
    /// The pattern dossier names must contain, when one was given.
    pub pattern: Option<&'a str>,
    /// Whether `--menu` was given.
    pub menu: bool,
}

/// The `--filter` and `--menu` selection, for the commands that take both.
///
/// A filter that matches several candidates asks which one, a filter that
/// matches one is used as it is, and `--menu` asks over everything. Callers
/// only reach this when one of those two was given: listing everything, or
/// taking the newest, is their own business.
///
/// The picker is handed in so a test can decide without a terminal; callers
/// pass [`pick`].
///
/// # Errors
///
/// Fails when a filter matches nothing, when there is nothing to choose from,
/// or when the picker cannot be read or is cancelled.
pub fn select(
    paths: &[PathBuf],
    request: Request<'_>,
    picker: impl Fn(&[PathBuf]) -> Result<PathBuf>,
) -> Result<Vec<PathBuf>> {
    if let Some(pattern) = request.pattern {
        return match worklog::matching(paths, pattern).as_slice() {
            [] => bail!("no dossier matches '{pattern}'"),
            [only] => Ok(vec![only.clone()]),
            several => Ok(vec![picker(several)?]),
        };
    }

    if paths.is_empty() {
        bail!("there are no dossiers to choose from yet");
    }

    if request.menu {
        return Ok(vec![picker(paths)?]);
    }

    // A filter with no pattern asks for the newest dossier. The picker opens
    // only when several share that timestamp, which is the normal state of a
    // fresh clone.
    match newest(&by_recency(paths.to_vec())) {
        [only] => Ok(vec![only.clone()]),
        tied => Ok(vec![picker(tied)?]),
    }
}

/// Ask the user to choose one of `dossiers` at the terminal.
///
/// # Errors
///
/// Fails when the terminal cannot be read, or when the user leaves without
/// choosing.
pub fn pick(dossiers: &[PathBuf]) -> Result<PathBuf> {
    let names: Vec<String> = dossiers
        .iter()
        .map(|dossier| worklog::stem(dossier.as_path()))
        .collect();

    let choice = FuzzySelect::new()
        .with_prompt(format!("Select a dossier ({CANCEL_HINT})"))
        .items(&names)
        .interact_opt()
        .context("could not read your selection")?;

    let Some(index) = choice else {
        bail!("nothing was selected");
    };

    let dossier = dossiers
        .get(index)
        .context("the picker returned a dossier that is not there")?;

    Ok(dossier.clone())
}
