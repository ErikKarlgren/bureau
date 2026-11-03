//! `bureau commit` and `bureau sync`.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

use crate::cli::CommitArgs;
use crate::commands::paths::{DOSSIERS_DIR, ENTRIES_DIR};
use crate::commands::sources;
use crate::git;
use crate::style::{self, Palette};

/// Commit uncommitted entries and dossiers.
///
/// # Errors
///
/// Fails when the git toplevel cannot be determined, or when committing a file fails.
pub fn run(args: &CommitArgs) -> Result<()> {
    let root = git::toplevel()?;
    let palette = style::for_stdout();
    let mut out = std::io::stdout();
    run_in(&root, args, palette, &mut out)?;
    Ok(())
}

/// Run commit in the given repository root.
pub fn run_in(
    root: &Path,
    args: &CommitArgs,
    palette: Palette,
    out: &mut impl Write,
) -> Result<Vec<String>> {
    let check_entries = !args.dossiers;
    let check_dossiers = !args.entries;

    let mut committed = Vec::new();

    if check_entries {
        let entry_paths = sources::markdown_paths(root, ENTRIES_DIR)?;
        for path in entry_paths {
            if git::is_dirty(root, &path)? {
                let is_tracked = git::is_tracked(root, &path)?;
                let action = if is_tracked {
                    "update entry"
                } else {
                    "new entry"
                };
                let title = git::commit_title(action, &path);
                let shown = sources::relative(&path, root).display().to_string();

                git::commit(root, &[&path], &title)
                    .with_context(|| format!("could not commit '{shown}'"))?;

                let verb = if is_tracked {
                    "Updated entry"
                } else {
                    "Created entry"
                };
                writeln!(
                    out,
                    "{} {}",
                    palette.heading().paint(verb),
                    palette.quoted(&shown)
                )?;
                committed.push(shown);
            }
        }
    }

    if check_dossiers {
        let dossier_paths = sources::markdown_paths(root, DOSSIERS_DIR)?;
        for path in dossier_paths {
            if git::is_dirty(root, &path)? {
                let is_tracked = git::is_tracked(root, &path)?;
                let action = if is_tracked {
                    "update dossier"
                } else {
                    "new dossier"
                };
                let title = git::commit_title(action, &path);
                let shown = sources::relative(&path, root).display().to_string();

                git::commit(root, &[&path], &title)
                    .with_context(|| format!("could not commit '{shown}'"))?;

                let verb = if is_tracked {
                    "Updated dossier"
                } else {
                    "Created dossier"
                };
                writeln!(
                    out,
                    "{} {}",
                    palette.heading().paint(verb),
                    palette.quoted(&shown)
                )?;
                committed.push(shown);
            }
        }
    }

    if committed.is_empty() {
        writeln!(out, "Nothing to commit (working tree clean)")?;
    }

    Ok(committed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tests::Scratch;

    #[test]
    fn clean_repo_reports_nothing_to_commit() {
        let scratch = Scratch::new("commit-clean").unwrap();
        scratch.init_git();
        let mut out = Vec::new();
        let committed = run_in(
            scratch.path(),
            &CommitArgs::default(),
            Palette::OFF,
            &mut out,
        )
        .unwrap();

        assert!(committed.is_empty());
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("Nothing to commit"));
    }

    #[test]
    fn commits_modified_tracked_entry() {
        let scratch = Scratch::new("commit-tracked-entry").unwrap();
        scratch.init_git();
        scratch
            .write("entries/2026-10-15.md", "# 2026-10-15\n- original\n")
            .unwrap();
        scratch.git(&["add", "entries/2026-10-15.md"]);
        scratch.git(&["commit", "-m", "initial entry"]);

        // Modify the entry
        scratch
            .write(
                "entries/2026-10-15.md",
                "# 2026-10-15\n- original\n- [x] completed task\n",
            )
            .unwrap();

        let mut out = Vec::new();
        let committed = run_in(
            scratch.path(),
            &CommitArgs::default(),
            Palette::OFF,
            &mut out,
        )
        .unwrap();

        assert_eq!(committed.len(), 1);
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("Updated entry"));
        assert!(output.contains("entries/2026-10-15.md"));

        // Verify git is clean now
        assert!(
            !git::is_dirty(
                scratch.path(),
                &scratch.path().join("entries/2026-10-15.md")
            )
            .unwrap()
        );
    }

    #[test]
    fn commits_untracked_new_entry() {
        let scratch = Scratch::new("commit-untracked-entry").unwrap();
        scratch.init_git();
        scratch
            .write("entries/2026-10-16.md", "# 2026-10-16\n- [ ] new task\n")
            .unwrap();

        let mut out = Vec::new();
        let committed = run_in(
            scratch.path(),
            &CommitArgs::default(),
            Palette::OFF,
            &mut out,
        )
        .unwrap();

        assert_eq!(committed.len(), 1);
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("Created entry"));
        assert!(output.contains("entries/2026-10-16.md"));

        assert!(
            !git::is_dirty(
                scratch.path(),
                &scratch.path().join("entries/2026-10-16.md")
            )
            .unwrap()
        );
    }

    #[test]
    fn commits_only_entries_when_flag_given() {
        let scratch = Scratch::new("commit-only-entries").unwrap();
        scratch.init_git();
        scratch
            .write("entries/2026-10-17.md", "# 2026-10-17\n")
            .unwrap();
        scratch.write("dossiers/task.md", "# task\n").unwrap();

        let args = CommitArgs {
            entries: true,
            dossiers: false,
        };

        let mut out = Vec::new();
        let committed = run_in(scratch.path(), &args, Palette::OFF, &mut out).unwrap();

        assert_eq!(committed.len(), 1);
        assert!(committed[0].contains("entries/2026-10-17.md"));

        // Dossier remains uncommitted
        assert!(git::is_dirty(scratch.path(), &scratch.path().join("dossiers/task.md")).unwrap());
    }

    #[test]
    fn unrelated_files_are_not_committed() {
        let scratch = Scratch::new("commit-unrelated").unwrap();
        scratch.init_git();
        scratch.write("README.md", "# My Repo\n").unwrap();
        scratch.write("secret.txt", "password\n").unwrap();

        let mut out = Vec::new();
        let committed = run_in(
            scratch.path(),
            &CommitArgs::default(),
            Palette::OFF,
            &mut out,
        )
        .unwrap();

        assert!(committed.is_empty());
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("Nothing to commit"));

        // Secret and readme are still uncommitted
        assert!(git::is_dirty(scratch.path(), &scratch.path().join("secret.txt")).unwrap());
        assert!(git::is_dirty(scratch.path(), &scratch.path().join("README.md")).unwrap());
    }
}
