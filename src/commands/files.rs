//! A small cache of file contents, shared by the commands that read notes.
//!
//! Discovery reads a file to find out whether it is sealed, and the command
//! then needs the same contents to parse it. The cache is what keeps that from
//! becoming two reads of every note.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The contents of the files a run has read, by path.
#[derive(Debug, Default)]
pub struct Files {
    contents: HashMap<PathBuf, String>,
}

impl Files {
    /// An empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The contents of `path`, read from disk the first time it is asked for.
    ///
    /// The returned slice borrows the cache, so a caller that needs the
    /// contents after the next read should copy them first.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read as UTF-8 text.
    pub fn read(&mut self, path: &Path) -> Result<&str> {
        if !self.contents.contains_key(path) {
            let contents = fs::read_to_string(path)
                .with_context(|| format!("could not read '{}'", path.display()))?;
            self.contents.insert(path.to_path_buf(), contents);
        }

        self.contents
            .get(path)
            .map(String::as_str)
            .context("the cached file contents disappeared")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tests::Scratch;

    #[test]
    fn a_second_read_comes_from_memory() {
        let scratch = Scratch::new("files-cache").unwrap();
        let path = scratch.path().join("note.md");
        scratch.write("note.md", "first").unwrap();

        let mut files = Files::new();
        assert_eq!(files.read(&path).unwrap(), "first");

        // If the cache did not hold the contents, this read would fail.
        fs::remove_file(&path).unwrap();
        assert_eq!(files.read(&path).unwrap(), "first");
    }

    #[test]
    fn a_missing_file_is_an_error() {
        let scratch = Scratch::new("files-missing").unwrap();
        let mut files = Files::new();

        let error = files.read(&scratch.path().join("missing.md")).unwrap_err();

        assert!(error.to_string().contains("could not read"), "{error}");
    }
}
