//! `bureau worklog`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use chrono::{Local, NaiveDate};

use crate::Result;
use crate::cli::WorklogArgs;
use crate::commands::paths::{DOSSIERS_DIR, ENTRIES_DIR};
use crate::commands::selection;
use crate::commands::sources;
use crate::git;
use crate::template;
use crate::worklog;

/// Log one line of work against a dossier, and link it from that day's entry.
///
/// # Errors
///
/// Fails when the repository root cannot be found, when `--date` is not a
/// date, when no dossier matches, when the picker is cancelled, or when the
/// chosen dossier has no `## Worklog` section. Nothing is written until the
/// message is in hand, and a failure between the two writes leaves the entry
/// already linked, so re-running logs the line exactly once.
pub fn run(args: &WorklogArgs) -> Result<()> {
    let root = git::toplevel()?;
    run_in(&root, args, prompt_for_message)
}

/// The rest of `run`, with the repository root and the prompt handed in, so a
/// test can drive it without a git checkout or a terminal.
fn run_in(root: &Path, args: &WorklogArgs, prompt: impl FnOnce() -> Result<String>) -> Result<()> {
    let date = target_date(args.date.as_deref())?;
    let dossiers = sources::read_dossiers(root)?;
    let dossier = select(&dossiers, args)?;
    let name = worklog::stem(&dossier);
    let chosen = sources::relative(&dossier, root);

    // Everything that can fail is done before the prompt, so a dossier that
    // cannot take a worklog never costs the user the line they typed.
    let contents = fs::read_to_string(&dossier)
        .with_context(|| format!("could not read '{}'", chosen.display()))?;
    if !worklog::has_worklog(&contents) {
        bail!(
            "'{}' has no '## Worklog' section, so there is nowhere to log this",
            chosen.display()
        );
    }

    let entry_path = root.join(ENTRIES_DIR).join(format!("{date}.md"));
    let entry = entry_contents(&entry_path, date)?;
    let entry_shown = sources::relative(&entry_path, root);
    let filename = sources::file_name(&dossier);
    let dossier_link = format!("../{DOSSIERS_DIR}/{filename}");
    let linked = worklog::add_link(&entry, &name, &dossier_link);

    println!("Logging to {name}");
    let message = prompt()?;

    let entry_link = format!("../{ENTRIES_DIR}/{date}.md");
    let updated = worklog::append_message(&contents, date, &message, &entry_link)
        .with_context(|| format!("'{}' has no '## Worklog' section", chosen.display()))?;

    // The entry is written first on purpose: if the dossier write then fails,
    // the link is already recorded, so re-running adds the bullet once rather
    // than twice.
    if let Some(updated) = &linked {
        let entries_dir = root.join(ENTRIES_DIR);
        fs::create_dir_all(&entries_dir)
            .with_context(|| format!("could not create '{}'", entries_dir.display()))?;
        fs::write(&entry_path, updated)
            .with_context(|| format!("could not write '{}'", entry_shown.display()))?;
    }

    fs::write(&dossier, updated)
        .with_context(|| format!("could not write '{}'", chosen.display()))?;

    commit(root, &dossier, &entry_path, &name);

    println!("Logged to '{}'", chosen.display());
    if linked.is_some() {
        println!("Linked from '{}'", entry_shown.display());
    } else {
        println!("'{}' already linked to it", entry_shown.display());
    }

    Ok(())
}

/// The date to log against: `--date` when given, today otherwise.
fn target_date(date: Option<&str>) -> Result<NaiveDate> {
    let Some(text) = date else {
        return Ok(Local::now().date_naive());
    };

    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .with_context(|| format!("'{text}' is not a date; expected YYYY-MM-DD"))
}

/// Decide which dossier to log against.
///
/// A filter that matches several dossiers asks which one; a filter that
/// matches one is used as it is. Without a filter, `--menu` offers every
/// dossier, and otherwise the most recently modified one wins -- unless
/// several share that timestamp, in which case they are offered as a choice.
fn select(dossiers: &[PathBuf], args: &WorklogArgs) -> Result<PathBuf> {
    let request = selection::Request {
        pattern: args.filter.as_deref(),
        menu: args.menu,
    };
    let mut chosen = selection::select(dossiers, request, selection::pick)?;

    chosen
        .pop()
        .context("the selection returned no dossier to log against")
}

/// Read the single line to log, or nothing when there is none to log.
fn prompt_for_message() -> Result<String> {
    print!("Message (Ctrl-C to abort): ");
    io::stdout().flush()?;

    let mut message = String::new();
    if io::stdin().read_line(&mut message)? == 0 {
        bail!("no message given, so nothing was logged");
    }

    let message = message.trim_end_matches(['\r', '\n']);
    if message.trim().is_empty() {
        bail!("no message given, so nothing was logged");
    }

    Ok(message.to_owned())
}

/// The daily entry for `date`: read from disk, or fresh from the template.
fn entry_contents(path: &Path, date: NaiveDate) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(template::daily_entry(&date)),
        Err(error) => Err(error).with_context(|| format!("could not read '{}'", path.display())),
    }
}

/// Commit the two files this command touched.
///
/// Never fatal: the work is on disk, and the message the user typed must not
/// be lost because git would not commit it.
fn commit(root: &Path, dossier: &Path, entry: &Path, name: &str) {
    if let Err(error) = git::commit(&[dossier, entry], &format!("Worklog {name}")) {
        eprintln!("warning: {error:?}");
        eprintln!(
            "warning: the worklog was written to '{}' but is not committed",
            sources::relative(dossier, root).display()
        );
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::commands::tests::Scratch;

    /// The dossier every test logs against.
    const DOSSIER: &str = "dossiers/0007 - legacy.md";

    /// Arguments that select the one scratch dossier, so no picker is needed.
    fn args(date: &str) -> WorklogArgs {
        WorklogArgs {
            filter: Some("legacy".to_owned()),
            date: Some(date.to_owned()),
            menu: false,
        }
    }

    #[test]
    fn refuses_a_dossier_without_a_worklog_before_prompting() {
        let scratch = Scratch::new("no-worklog").unwrap();
        scratch
            .write(DOSSIER, "# Legacy\n## Notes\n- nothing\n")
            .unwrap();

        let prompted = Cell::new(false);
        let prompt = || {
            prompted.set(true);
            Ok("Did some work".to_owned())
        };

        let error = run_in(scratch.path(), &args("2026-03-23"), prompt).unwrap_err();

        assert!(
            !prompted.get(),
            "the message was prompted for before the dossier was checked"
        );
        assert!(error.to_string().contains("no '## Worklog' section"));
        assert_eq!(
            scratch.read(DOSSIER).unwrap(),
            "# Legacy\n## Notes\n- nothing\n"
        );
    }

    #[test]
    fn leaves_the_dossier_alone_when_no_entry_can_be_used() {
        let scratch = Scratch::new("entry-in-the-way").unwrap();
        scratch.write(DOSSIER, "# Legacy\n## Worklog\n").unwrap();
        // A file where the entries directory belongs, so no entry can be read
        // or written.
        scratch.write("entries", "in the way\n").unwrap();

        let result = run_in(scratch.path(), &args("2026-03-23"), || {
            Ok("Did some work".to_owned())
        });

        assert!(result.is_err());
        assert_eq!(
            scratch.read(DOSSIER).unwrap(),
            "# Legacy\n## Worklog\n",
            "the dossier was written before the entry was dealt with"
        );
    }

    #[test]
    fn logs_a_line_and_links_the_entry() {
        let scratch = Scratch::new("happy-path").unwrap();
        scratch.write(DOSSIER, "# Legacy\n## Worklog\n").unwrap();

        run_in(scratch.path(), &args("2026-03-23"), || {
            Ok("Did some work".to_owned())
        })
        .unwrap();

        assert_eq!(
            scratch.read(DOSSIER).unwrap(),
            "# Legacy\n## Worklog\n\n### [2026-03-23](<../entries/2026-03-23.md>)\n- Did some work\n"
        );
        assert_eq!(
            scratch.read("entries/2026-03-23.md").unwrap(),
            "# 2026-03-23\n\n## Notes\n- \n\n## Worked on Dossiers\n\
- [0007 - legacy](<../dossiers/0007 - legacy.md>)\n"
        );
    }

    #[test]
    fn running_twice_logs_the_line_twice_but_links_the_entry_once() {
        let scratch = Scratch::new("twice").unwrap();
        scratch.write(DOSSIER, "# Legacy\n## Worklog\n").unwrap();

        run_in(scratch.path(), &args("2026-03-23"), || {
            Ok("First".to_owned())
        })
        .unwrap();
        run_in(scratch.path(), &args("2026-03-23"), || {
            Ok("Second".to_owned())
        })
        .unwrap();

        assert_eq!(
            scratch.read(DOSSIER).unwrap(),
            "# Legacy\n## Worklog\n\n### [2026-03-23](<../entries/2026-03-23.md>)\n- First\n- Second\n"
        );
        assert_eq!(
            scratch
                .read("entries/2026-03-23.md")
                .unwrap()
                .matches("- [0007 - legacy]")
                .count(),
            1
        );
    }

    #[test]
    fn a_failed_dossier_write_still_leaves_the_entry_linked() {
        let scratch = Scratch::new("dossier-write-fails").unwrap();
        scratch.write(DOSSIER, "# Legacy\n## Worklog\n").unwrap();

        // The prompt runs after both files have been read and before either is
        // written, so this is the one moment a test can break the dossier write
        // without depending on file permissions.
        let dossier_path = scratch.path().join(DOSSIER);
        let in_the_way = dossier_path.clone();
        let prompt = move || {
            fs::remove_file(&in_the_way)?;
            fs::create_dir(&in_the_way)?;
            Ok("Did some work".to_owned())
        };

        assert!(run_in(scratch.path(), &args("2026-03-23"), prompt).is_err());
        assert!(
            scratch
                .read("entries/2026-03-23.md")
                .unwrap()
                .contains("- [0007 - legacy]"),
            "the entry should have been written before the dossier"
        );

        // Put the dossier back and log the same line again: the entry is
        // already linked, so the bullet is added exactly once.
        fs::remove_dir(&dossier_path).unwrap();
        fs::write(&dossier_path, "# Legacy\n## Worklog\n").unwrap();
        run_in(scratch.path(), &args("2026-03-23"), || {
            Ok("Did some work".to_owned())
        })
        .unwrap();

        assert_eq!(
            scratch.read(DOSSIER).unwrap(),
            "# Legacy\n## Worklog\n\n### [2026-03-23](<../entries/2026-03-23.md>)\n- Did some work\n"
        );
        assert_eq!(
            scratch
                .read("entries/2026-03-23.md")
                .unwrap()
                .matches("- [0007 - legacy]")
                .count(),
            1
        );
    }
}
