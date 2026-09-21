//! The shared test scaffolding for the command modules.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// A scratch repository root that deletes itself afterwards.
pub struct Scratch(PathBuf);

impl Scratch {
    /// An empty root for one test.
    ///
    /// # Errors
    ///
    /// Fails when the temporary directory cannot be created.
    pub fn new(name: &str) -> io::Result<Self> {
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("bureau-{pid}-{name}"));
        fs::remove_dir_all(&path).ok();
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    /// The root, for handing to a `run_in`.
    pub fn path(&self) -> &Path {
        self.0.as_path()
    }

    /// Write `contents` to a path below the root.
    ///
    /// # Errors
    ///
    /// Fails when the directories or the file cannot be written.
    pub fn write(&self, relative: &str, contents: &str) -> io::Result<()> {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, contents)
    }

    /// Read a path below the root back.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read.
    pub fn read(&self, relative: &str) -> io::Result<String> {
        fs::read_to_string(self.0.join(relative))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}
