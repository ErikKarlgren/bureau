//! The little bit of `git` that bureau needs.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
        .output()?;

    if !output.status.success() {
        return Err(format!("not inside a git repository: {}", stderr(&output)).into());
    }

    let path = String::from_utf8(output.stdout)?;
    Ok(PathBuf::from(path.trim_end()))
}

/// Stage and commit one file, leaving any other staged changes alone.
///
/// # Errors
///
/// Fails when `git add` or `git commit` exits with an error. The message
/// carries git's own explanation.
pub fn commit(path: &Path, message: &str) -> Result<()> {
    let added = Command::new("git").arg("add").arg(path).output()?;
    if !added.status.success() {
        return Err(format!("git add failed: {}", stderr(&added)).into());
    }

    let committed = Command::new("git")
        .args(["commit", "--only"])
        .arg(path)
        .args(["-m", message])
        .output()?;
    if !committed.status.success() {
        return Err(format!("git commit failed: {}", stderr(&committed)).into());
    }

    Ok(())
}

/// Whatever git had to say for itself, for use in an error message.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_owned()
}
