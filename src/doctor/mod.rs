//! The formatting checks behind `bureau doctor`.
//!
//! The command does the IO; everything here is a pure function over the
//! contents of the notes, plus a small injected lookup for "does this path
//! exist, and is it a file". That keeps the checks testable without a
//! repository.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use self::document::Document;

mod checks;
mod document;
mod links;
mod worklog;

#[cfg(test)]
mod tests;

/// How a daily entry date is written.
const DATE_FORMAT: &str = "%Y-%m-%d";

/// The dossier section `bureau worklog` needs.
const WORKLOG_HEADING: &str = "## Worklog";

/// The entry section `bureau worklog` links dossiers from.
const LINKS_HEADING: &str = "## Worked on Dossiers";

/// The directory a dossier's day links point at.
const DOSSIERS_DIR: &str = "dossiers";

/// The directory an entry's dossier links point at.
const ENTRIES_DIR: &str = "entries";

/// How a finding is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// A command cannot use the note as intended, or content is hidden.
    Error,
    /// The note works, but its shape is likely a mistake.
    Warning,
}

impl Severity {
    /// The word printed before the message.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Error => "Error",
            Self::Warning => "Warning",
        }
    }
}

/// What a finding is about, used to keep findings on one line in a stable
/// order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Check {
    Read,
    Fence,
    Name,
    Encoding,
    Worklog,
    EntrySection,
    EmptyTask,
    TaskMarker,
    TaskBullet,
    Link,
    WorklogDay,
    CrossLink,
}

/// One thing worth changing in a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The note the finding belongs to.
    pub path: PathBuf,
    /// The 1-based line, or 1 for a whole-file finding.
    pub line: usize,
    /// How seriously to take it.
    pub severity: Severity,
    /// What is wrong.
    pub message: String,
    /// Which check produced it, for deterministic ordering.
    check: Check,
}

impl Finding {
    pub(super) fn new(
        path: &Path,
        line: usize,
        severity: Severity,
        check: Check,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: path.to_path_buf(),
            line,
            severity,
            message: message.into(),
            check,
        }
    }
}

/// A note that could not be read, as a finding.
#[must_use]
pub fn unreadable(path: &Path, message: String) -> Finding {
    Finding::new(path, 1, Severity::Error, Check::Read, message)
}

/// Put findings in the order the command prints them.
pub fn sort(findings: &mut [Finding]) {
    findings.sort_by(|left, right| {
        (&left.path, left.line, left.check).cmp(&(&right.path, right.line, right.check))
    });
}

/// Whether any finding is an error.
#[must_use]
pub fn has_errors(findings: &[Finding]) -> bool {
    findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
}

/// One dossier file.
#[derive(Debug)]
pub struct Note {
    /// Where the file is.
    pub path: PathBuf,
    /// What it contains.
    pub contents: String,
}

/// One daily entry file, already known to be date-named.
#[derive(Debug)]
pub struct EntryNote {
    /// Where the file is.
    pub path: PathBuf,
    /// The day the file name names.
    pub date: NaiveDate,
    /// What it contains.
    pub contents: String,
}

/// The notes a run checks.
#[derive(Debug, Default)]
pub struct Input {
    /// Every open dossier.
    pub dossiers: Vec<Note>,
    /// Every date-named entry.
    pub entries: Vec<EntryNote>,
}

/// Check every note in `input`.
///
/// `lookup` answers whether a path is a file, a directory, or missing. The
/// command passes the filesystem; a test passes a map.
#[must_use]
pub fn check(input: &Input, lookup: impl Fn(&Path) -> Option<bool>) -> Vec<Finding> {
    let dossiers: Vec<Parsed<'_>> = input
        .dossiers
        .iter()
        .map(|note| Parsed::new(&note.path, &note.contents))
        .collect();
    let entries: Vec<ParsedEntry<'_>> = input
        .entries
        .iter()
        .map(|entry| ParsedEntry {
            note: Parsed::new(&entry.path, &entry.contents),
            date: entry.date,
        })
        .collect();

    let mut findings = Vec::new();

    for note in &dossiers {
        checks::check_note(note, &mut findings);
    }
    for entry in &entries {
        checks::check_note(&entry.note, &mut findings);
        worklog::check_entry_section(entry, &mut findings);
    }

    checks::check_case_collisions(dossiers.iter().map(|note| note.path), &mut findings);
    checks::check_case_collisions(entries.iter().map(|entry| entry.note.path), &mut findings);

    let entries_by_date: BTreeMap<NaiveDate, &ParsedEntry<'_>> =
        entries.iter().map(|entry| (entry.date, entry)).collect();
    let dossiers_by_name: HashMap<&str, &Parsed<'_>> = dossiers
        .iter()
        .map(|dossier| (worklog::dossier_name(dossier.path), dossier))
        .collect();

    let worklogs = worklog::check_worklogs(&dossiers, &entries_by_date, &lookup, &mut findings);

    for dossier in &dossiers {
        let empty = HashSet::new();
        let skip = worklogs
            .get(dossier.path)
            .map_or(&empty, worklog::WorklogInfo::day_lines);
        links::check_links(dossier, skip, &lookup, &mut findings);
    }
    for entry in &entries {
        links::check_links(&entry.note, &HashSet::new(), &lookup, &mut findings);
    }

    worklog::check_entry_links(&entries, &dossiers_by_name, &worklogs, &mut findings);

    findings
}

/// One note with its line structure already read.
struct Parsed<'a> {
    pub(super) path: &'a Path,
    pub(super) contents: &'a str,
    pub(super) document: Document,
}

impl<'a> Parsed<'a> {
    pub(super) fn new(path: &'a Path, contents: &'a str) -> Self {
        Self {
            path,
            contents,
            document: Document::parse(contents),
        }
    }
}

/// One daily entry with its date.
struct ParsedEntry<'a> {
    pub(super) note: Parsed<'a>,
    pub(super) date: NaiveDate,
}
