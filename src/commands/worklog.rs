//! `bureau worklog`.

use std::cmp::Reverse;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, bail};
use chrono::{Local, NaiveDate};
use dialoguer::FuzzySelect;

use crate::Result;
use crate::cli::WorklogArgs;
use crate::git;
use crate::template;
use crate::worklog;

/// The directories, inside the repository, that bureau reads and writes.
const DOSSIERS: &str = "dossiers";
const ENTRIES: &str = "entries";

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
    let dossiers = by_recency(read_dossiers(root)?);
    let dossier = select(&dossiers, args)?;
    let name = worklog::stem(&dossier);
    let chosen = relative(&dossier, root);

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

    let entry_path = root.join(ENTRIES).join(format!("{date}.md"));
    let entry = entry_contents(&entry_path, date)?;
    let entry_shown = relative(&entry_path, root);
    let filename = file_name(&dossier);
    let dossier_link = format!("../{DOSSIERS}/{filename}");
    let linked = worklog::add_link(&entry, &name, &dossier_link);

    println!("Logging to {name}");
    let message = prompt()?;

    let entry_link = format!("../{ENTRIES}/{date}.md");
    let updated = worklog::append_message(&contents, date, &message, &entry_link)
        .with_context(|| format!("'{}' has no '## Worklog' section", chosen.display()))?;

    // The entry is written first on purpose: if the dossier write then fails,
    // the link is already recorded, so re-running adds the bullet once rather
    // than twice.
    if let Some(updated) = &linked {
        let entries_dir = root.join(ENTRIES);
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
    if let Some(filter) = args.filter.as_deref() {
        return match worklog::matching(dossiers, filter).as_slice() {
            [] => bail!("no dossier matches '{filter}'"),
            [only] => Ok(only.clone()),
            several => pick(several),
        };
    }

    if dossiers.is_empty() {
        bail!("there are no dossiers to log against yet");
    }

    if args.menu {
        return pick(dossiers);
    }

    match newest(dossiers) {
        [only] => Ok(only.clone()),
        tied => pick(tied),
    }
}

/// Ask the user to choose one of `dossiers`.
fn pick(dossiers: &[PathBuf]) -> Result<PathBuf> {
    let names: Vec<String> = dossiers
        .iter()
        .map(|dossier| worklog::stem(dossier))
        .collect();

    let choice = FuzzySelect::new()
        .with_prompt("Select a dossier (Esc or q to cancel)")
        .items(&names)
        .interact_opt()
        .context("could not read your selection")?;

    let Some(index) = choice else {
        bail!("nothing was selected, so nothing was written");
    };

    let dossier = dossiers
        .get(index)
        .context("the picker returned a dossier that is not there")?;

    Ok(dossier.clone())
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

/// Every `.md` file in the repository's dossiers directory.
fn read_dossiers(root: &Path) -> Result<Vec<PathBuf>> {
    let directory = root.join(DOSSIERS);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        // No dossiers directory yet simply means no dossiers.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read '{}'", directory.display()));
        }
    };

    let mut dossiers = Vec::new();
    for entry in entries {
        let path = entry
            .with_context(|| format!("could not read '{}'", directory.display()))?
            .path();
        if path.extension().is_some_and(|extension| extension == "md") {
            dossiers.push(path);
        }
    }

    Ok(dossiers)
}

/// Dossiers sorted most recently modified first, by name within equal times.
fn by_recency(mut dossiers: Vec<PathBuf>) -> Vec<PathBuf> {
    dossiers.sort_by_cached_key(|dossier| (Reverse(modified(dossier.as_path())), dossier.clone()));

    dossiers
}

/// A file's modification time, when the filesystem will tell us one.
fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

/// The dossiers sharing the newest modification time.
///
/// A fresh `git clone` stamps every file with the same time, so this is often
/// more than one dossier, and the caller asks instead of guessing.
fn newest(dossiers: &[PathBuf]) -> &[PathBuf] {
    let Some(first) = dossiers.first() else {
        return &[];
    };

    let stamp = modified(first.as_path());
    let tied = dossiers
        .iter()
        .take_while(|dossier| modified(dossier.as_path()) == stamp)
        .count();

    dossiers.get(..tied).unwrap_or(dossiers)
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
            relative(dossier, root).display()
        );
    }
}

/// A path as the user sees it: relative to the repository root when it is
/// inside it.
fn relative<'a>(path: &'a Path, root: &Path) -> &'a Path {
    path.strip_prefix(root).unwrap_or(path)
}

/// A dossier's file name, extension included.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    /// The dossier every test logs against.
    const DOSSIER: &str = "dossiers/0007 - legacy.md";

    /// A scratch repository root that deletes itself afterwards.
    struct Scratch(PathBuf);

    impl Scratch {
        /// An empty root for one test.
        fn new(name: &str) -> io::Result<Self> {
            let pid = std::process::id();
            let path = std::env::temp_dir().join(format!("bureau-{pid}-{name}"));
            fs::remove_dir_all(&path).ok();
            fs::create_dir_all(&path)?;
            Ok(Self(path))
        }

        /// The root, for handing to `run_in`.
        fn path(&self) -> &Path {
            self.0.as_path()
        }

        /// Write `contents` to a path below the root.
        fn write(&self, relative: &str, contents: &str) -> io::Result<()> {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, contents)
        }

        /// Read a path below the root back.
        fn read(&self, relative: &str) -> io::Result<String> {
            fs::read_to_string(self.0.join(relative))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

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
