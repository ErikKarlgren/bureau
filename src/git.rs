//! The little bit of `git` that bureau needs.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, bail};

use crate::Result;

/// The root of the repository bureau was run from.
///
/// # Errors
///
/// Fails when the current directory is not inside a git repository, or when
/// `git` itself cannot be run.
pub fn toplevel() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("could not run git")?;

    if !output.status.success() {
        bail!("not inside a git repository: {}", stderr(&output));
    }

    let path =
        String::from_utf8(output.stdout).context("git returned a path that is not UTF-8")?;
    Ok(PathBuf::from(path.trim_end()))
}

/// Stage and commit the given files, leaving any other staged changes alone.
///
/// # Errors
///
/// Fails when `git add` or `git commit` exits with an error. The message
/// carries git's own explanation.
pub fn commit(paths: &[&Path], message: &str) -> Result<()> {
    let added = Command::new("git")
        .arg("add")
        .args(paths)
        .output()
        .context("could not run git")?;
    if !added.status.success() {
        bail!("git add failed: {}", stderr(&added));
    }

    let committed = Command::new("git")
        .args(["commit", "--only", "-m", message])
        .args(paths)
        .output()
        .context("could not run git")?;
    if !committed.status.success() {
        bail!("git commit failed: {}", stderr(&committed));
    }

    Ok(())
}

/// Whatever git had to say for itself, for use in an error message.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_owned()
}
