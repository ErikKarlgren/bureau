//! The little bit of `git` that bureau needs.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use anyhow::{Context, bail};
use chrono::NaiveDate;

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

    let path = String::from_utf8(output.stdout).context("git returned a path that is not UTF-8")?;
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

/// The settings pinned on every `git diff`, so the bytes a report prints do not
/// depend on a user's or a repository's configuration.
///
/// `-c` has to come before the subcommand, and each of these is a setting that
/// changes the output or runs somebody's tool if it is left alone.
const PINNED: [&str; 12] = [
    "diff.algorithm=histogram",
    "diff.renames=true",
    "diff.noprefix=false",
    "diff.mnemonicPrefix=false",
    "diff.wsErrorHighlight=none",
    "color.moved=false",
    "color.diff.meta=bold",
    "color.diff.frag=bold",
    "color.diff.old=red",
    "color.diff.new=green",
    "color.diff.func=",
    "core.quotepath=false",
];

/// The git version, as `git --version` prints it after its own name.
///
/// # Errors
///
/// Fails when `git` cannot be run, or says something that is not a version.
pub fn version() -> Result<String> {
    let output = Command::new("git")
        .arg("--version")
        .output()
        .context("could not run git")?;

    if !output.status.success() {
        bail!("git --version failed: {}", stderr(&output));
    }

    let text =
        String::from_utf8(output.stdout).context("git returned a version that is not UTF-8")?;
    let version = text
        .trim()
        .strip_prefix("git version ")
        .context("could not read git's version")?;

    Ok(version.to_owned())
}

/// Whether `version` is new enough for `git diff --follow`, which arrived in
/// git 2.47.
///
/// The version string is parsed and git's error text is not: git's messages are
/// localised, its version is not.
#[must_use]
pub fn follows_renames(version: &str) -> bool {
    let mut numbers = version
        .split('.')
        .filter_map(|part| part.parse::<u32>().ok());
    let major = numbers.next();
    let minor = numbers.next();

    matches!((major, minor), (Some(major), Some(minor)) if (major, minor) >= (2, 47))
}

/// Whether the repository has a commit to compare against.
///
/// An unborn `HEAD` is not an error: a fresh repository simply has no history,
/// and a report over it has no diffs.
///
/// # Errors
///
/// Fails when `git` cannot be run.
pub fn has_head(root: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .current_dir(root)
        .output()
        .context("could not run git")?;

    Ok(output.status.success())
}

/// The newest commit touching `path` strictly before `date`.
///
/// # Errors
///
/// Fails when `git` cannot be run or the history cannot be read.
pub fn last_commit(root: &Path, date: NaiveDate, path: &Path) -> Result<Option<String>> {
    let before = format!("--before={date}T00:00:00");
    let output = Command::new("git")
        .arg("--no-pager")
        .args(["rev-list", "-1", &before, "HEAD", "--"])
        .arg(path)
        .current_dir(root)
        .output()
        .context("could not run git")?;

    if !output.status.success() {
        bail!("git rev-list failed: {}", stderr(&output));
    }

    let hash = String::from_utf8_lossy(&output.stdout);
    let hash = hash.trim();

    Ok(if hash.is_empty() {
        None
    } else {
        Some(hash.to_owned())
    })
}

/// The hash of the empty tree, in whatever object format the repository uses.
///
/// A dossier created inside the period has no commit before `--from` to diff
/// against, so the empty tree is its base.
///
/// # Errors
///
/// Fails when `git` cannot be run.
pub fn empty_tree(root: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("mktree")
        .stdin(Stdio::null())
        .current_dir(root)
        .output()
        .context("could not run git")?;

    if !output.status.success() {
        bail!("git mktree failed: {}", stderr(&output));
    }

    let hash = String::from_utf8_lossy(&output.stdout);
    Ok(hash.trim().to_owned())
}

/// The diff of one path between two trees, with every setting that could move
/// the bytes pinned.
///
/// `follow` is only passed when the caller knows git supports it and `base` is
/// a real commit; `--follow` needs exactly one path, which this call already
/// is. `color` is the report's own decision, so a pipe gets no escapes.
///
/// # Errors
///
/// Fails when `git` cannot be run or the diff cannot be produced.
pub fn diff(
    root: &Path,
    base: &str,
    head: &str,
    path: &Path,
    follow: bool,
    color: bool,
) -> Result<String> {
    let mut command = Command::new("git");
    command.arg("--no-pager").current_dir(root);

    for setting in PINNED {
        command.args(["-c", setting]);
    }

    command
        .arg("diff")
        .args(["--no-ext-diff", "--no-textconv", "--no-color-moved"])
        .arg(if color {
            "--color=always"
        } else {
            "--no-color"
        });

    if follow {
        command.arg("--follow");
    }

    clear_injected_config(&mut command);

    let output = command
        .arg(base)
        .arg(head)
        .arg("--")
        .arg(path)
        .env_remove("GIT_EXTERNAL_DIFF")
        .env_remove("GIT_DIFF_OPTS")
        .output()
        .context("could not run git")?;

    if !output.status.success() {
        bail!("git diff failed: {}", stderr(&output));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Drop the environment's own `-c` injections, so the pinned settings are the
/// only ones in play.
fn clear_injected_config(command: &mut Command) {
    for (key, _) in std::env::vars_os() {
        let Some(key) = key.to_str() else {
            continue;
        };

        if key == "GIT_CONFIG_COUNT"
            || key.starts_with("GIT_CONFIG_KEY_")
            || key.starts_with("GIT_CONFIG_VALUE_")
        {
            command.env_remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::commands::tests::Scratch;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    /// Run git in `root`, failing the test when it does.
    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
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

    /// A repository with one committed file and a second commit changing it.
    fn repository(name: &str) -> Scratch {
        let scratch = Scratch::new(name).unwrap();
        let root = scratch.path();
        git(root, &["init", "-q"]);
        scratch.write("f.md", "one\ntwo\n").unwrap();
        git(root, &["add", "f.md"]);
        git(root, &["commit", "-qm", "one"]);
        scratch.write("f.md", "one\nTWO\n").unwrap();
        git(root, &["commit", "-qam", "two"]);
        scratch
    }

    #[test]
    fn reads_the_git_version() {
        assert!(version().unwrap().starts_with(char::is_numeric));
    }

    #[test]
    fn follow_needs_git_2_47() {
        for (version, supported) in [
            ("2.46.1", false),
            ("2.47.0", true),
            ("2.47.0.windows.1", true),
            ("2.55.0", true),
            ("3.0.0", true),
            ("nonsense", false),
        ] {
            assert_eq!(follows_renames(version), supported, "{version}");
        }
    }

    #[test]
    fn history_helpers_read_the_repository() {
        let scratch = repository("git-history");
        let root = scratch.path();

        assert!(has_head(root).unwrap());
        assert!(
            last_commit(root, date(2030, 1, 1), Path::new("f.md"))
                .unwrap()
                .is_some()
        );
        assert!(
            last_commit(root, date(2000, 1, 1), Path::new("f.md"))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            empty_tree(root).unwrap(),
            "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
        );

        let unborn = Scratch::new("git-unborn").unwrap();
        git(unborn.path(), &["init", "-q"]);
        assert!(!has_head(unborn.path()).unwrap());
    }

    #[test]
    fn a_hostile_configuration_cannot_change_the_diff() {
        let scratch = repository("git-pinned");
        let root = scratch.path();
        git(root, &["config", "diff.algorithm", "myers"]);
        git(root, &["config", "diff.noprefix", "true"]);
        git(root, &["config", "color.diff.meta", "yellow"]);
        git(root, &["config", "color.diff.frag", "magenta"]);
        git(
            root,
            &["config", "diff.external", "sh -c 'echo EXTERNAL_RAN'"],
        );
        git(
            root,
            &["config", "diff.bin.textconv", "sh -c 'echo TEXTCONV_RAN'"],
        );
        scratch.write(".gitattributes", "*.md diff=bin\n").unwrap();

        // Control: unpinned, git really does run the configured tool.
        let raw = Command::new("git")
            .args(["diff", "HEAD~1", "HEAD", "--", "f.md"])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&raw.stdout).contains("EXTERNAL_RAN"),
            "the control did not run the external diff"
        );

        let diff = diff(root, "HEAD~1", "HEAD", Path::new("f.md"), false, false).unwrap();

        assert!(diff.contains("-two"), "{diff}");
        assert!(diff.contains("+TWO"), "{diff}");
        assert!(!diff.contains('\x1b'), "{diff:?}");
        assert!(!diff.contains("EXTERNAL_RAN"), "{diff}");
        assert!(!diff.contains("TEXTCONV_RAN"), "{diff}");
    }

    #[test]
    fn a_coloured_diff_uses_only_the_pinned_slots() {
        let scratch = repository("git-colour");
        let root = scratch.path();
        git(root, &["config", "color.diff.meta", "yellow"]);
        git(root, &["config", "color.diff.frag", "magenta"]);
        git(root, &["config", "color.moved", "true"]);

        // Control: unpinned, the hostile slots leak into the output.
        let raw = Command::new("git")
            .args([
                "-c",
                "color.ui=always",
                "diff",
                "HEAD~1",
                "HEAD",
                "--",
                "f.md",
            ])
            .current_dir(root)
            .output()
            .unwrap();
        let raw = String::from_utf8_lossy(&raw.stdout).into_owned();
        assert!(raw.contains("33m"), "the control lost its yellow: {raw:?}");

        let diff = diff(root, "HEAD~1", "HEAD", Path::new("f.md"), false, true).unwrap();

        assert!(diff.contains("31m"), "no red: {diff:?}");
        assert!(diff.contains("32m"), "no green: {diff:?}");
        for leaked in ["33m", "34m", "35m", "36m"] {
            assert!(!diff.contains(leaked), "{leaked} leaked: {diff:?}");
        }
    }

    #[test]
    fn a_missing_path_is_not_a_source_of_bytes() {
        let scratch = repository("git-missing");
        let root = scratch.path();
        fs::write(root.join("other.md"), "x\n").unwrap();

        let diff = diff(root, "HEAD~1", "HEAD", Path::new("other.md"), false, false).unwrap();

        assert!(diff.is_empty(), "{diff}");
    }
}
