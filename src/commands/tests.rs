//! The shared test scaffolding for the command modules.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

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

    /// Run git in this root, failing the test when it does.
    pub fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .env("GIT_AUTHOR_NAME", "bureau")
            .env("GIT_AUTHOR_EMAIL", "bureau@example.invalid")
            .env("GIT_COMMITTER_NAME", "bureau")
            .env("GIT_COMMITTER_EMAIL", "bureau@example.invalid")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Run git in this root and return what it wrote to standard output.
    #[must_use]
    pub fn git_output(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .env("GIT_AUTHOR_NAME", "bureau")
            .env("GIT_AUTHOR_EMAIL", "bureau@example.invalid")
            .env("GIT_COMMITTER_NAME", "bureau")
            .env("GIT_COMMITTER_EMAIL", "bureau@example.invalid")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Make this root a git repository with a local identity, so a command
    /// that commits needs neither a global configuration nor an environment.
    pub fn init_git(&self) {
        self.git(&["init", "-q"]);
        self.git(&["config", "user.name", "bureau"]);
        self.git(&["config", "user.email", "bureau@example.invalid"]);
        self.git(&["config", "commit.gpgsign", "false"]);
        self.git(&["config", "core.hooksPath", ".git/hooks"]);
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}
