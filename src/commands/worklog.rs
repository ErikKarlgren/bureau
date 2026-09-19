//! `bureau worklog`.

use std::cmp::Reverse;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, bail};
use chrono::{Local, NaiveDate};
use dialoguer::FuzzySelect;

use crate::cli::WorklogArgs;
use crate::git;
use crate::template;
use crate::worklog;
use crate::Result;

/// The directories, inside the repository, that bureau reads and writes.
const DOSSIERS: &str = "dossiers";
const ENTRIES: &str = "entries";

/// Log one line of work against a dossier, and link it from that day's entry.
///
/// # Errors
///
/// Fails when the repository root cannot be found, when `--date` is not a
/// date, when no dossier matches, when the picker is cancelled, or when the
/// chosen dossier has no `## Worklog` section. Nothing is written unless every
/// step up to the write succeeds.
pub fn run(args: &WorklogArgs) -> Result<()> {
    let root = git::toplevel()?;
    let date = target_date(args.date.as_deref())?;
    let dossiers = by_recency(read_dossiers(&root)?);
    let dossier = select(&dossiers, args)?;
    let name = worklog::stem(&dossier);
    let chosen = relative(&dossier, &root);

    println!("Logging to {name}");
    let message = prompt_for_message()?;

    let contents = fs::read_to_string(&dossier)
        .with_context(|| format!("could not read '{}'", chosen.display()))?;

    let Some(updated) = worklog::append_message(&contents, date, &message) else {
        bail!(
            "'{}' has no '## Worklog' section, so there is nowhere to log this",
            chosen.display()
        );
    };
    fs::write(&dossier, updated)
        .with_context(|| format!("could not write '{}'", chosen.display()))?;

    let entry_path = root.join(ENTRIES).join(format!("{date}.md"));
    let entry = entry_contents(&entry_path, date)?;
    let filename = file_name(&dossier);
    let target = format!("../{DOSSIERS}/{filename}");
    let linked = worklog::add_link(&entry, &name, &target);
    let entry_shown = relative(&entry_path, &root);

    if let Some(updated) = &linked {
        let entries_dir = root.join(ENTRIES);
        fs::create_dir_all(&entries_dir)
            .with_context(|| format!("could not create '{}'", entries_dir.display()))?;
        fs::write(&entry_path, updated)
            .with_context(|| format!("could not write '{}'", entry_shown.display()))?;
    }

    commit(&root, &dossier, &entry_path, &name);

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
    dossiers
        .sort_by_cached_key(|dossier| (Reverse(modified(dossier.as_path())), dossier.clone()));

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
