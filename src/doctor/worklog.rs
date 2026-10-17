use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use super::document::{Heading, whole_link};
use super::links::{dossier_link_name, entry_links};
use super::{
    Check, DATE_FORMAT, ENTRIES_DIR, Finding, LINKS_HEADING, Parsed, ParsedEntry, Severity,
    WORKLOG_HEADING,
};

pub(super) struct WorklogInfo {
    has_worklog: bool,
    day_lines: HashSet<usize>,
    dates: BTreeSet<NaiveDate>,
    valid_days: BTreeMap<NaiveDate, usize>,
}

impl WorklogInfo {
    pub(super) const fn day_lines(&self) -> &HashSet<usize> {
        &self.day_lines
    }
}

pub(super) fn check_worklogs<'a>(
    dossiers: &[Parsed<'a>],
    entries_by_date: &BTreeMap<NaiveDate, &ParsedEntry<'a>>,
    lookup: &impl Fn(&Path) -> Option<bool>,
    findings: &mut Vec<Finding>,
) -> HashMap<PathBuf, WorklogInfo> {
    let mut worklogs = HashMap::new();

    for dossier in dossiers {
        let info = check_one_worklog(dossier, lookup, findings);
        check_worklog_backlinks(dossier, &info, entries_by_date, findings);
        worklogs.insert(dossier.path.to_path_buf(), info);
    }

    worklogs
}

fn check_one_worklog(
    dossier: &Parsed<'_>,
    lookup: &impl Fn(&Path) -> Option<bool>,
    findings: &mut Vec<Finding>,
) -> WorklogInfo {
    let sections = dossier.document.exact_sections(WORKLOG_HEADING);
    let Some(first) = sections.first() else {
        findings.push(Finding::new(
            dossier.path,
            1,
            Severity::Error,
            Check::Worklog,
            "missing '## Worklog' section",
        ));
        return WorklogInfo {
            has_worklog: false,
            day_lines: HashSet::new(),
            dates: BTreeSet::new(),
            valid_days: BTreeMap::new(),
        };
    };

    for duplicate in sections.iter().skip(1) {
        findings.push(Finding::new(
            dossier.path,
            duplicate.line,
            Severity::Error,
            Check::Worklog,
            "duplicate '## Worklog' section; only the first one is used",
        ));
    }

    let end = dossier.document.section_end(first);
    let mut info = WorklogInfo {
        has_worklog: true,
        day_lines: HashSet::new(),
        dates: BTreeSet::new(),
        valid_days: BTreeMap::new(),
    };

    for heading in dossier.document.headings_between(first.line, end, 3) {
        info.day_lines.insert(heading.line);

        if let Some(date) = day_heading_date(heading) {
            info.dates.insert(date);
        }

        check_day_heading(dossier, heading, lookup, &mut info, findings);
    }

    info
}

fn check_day_heading(
    dossier: &Parsed<'_>,
    heading: &Heading,
    lookup: &impl Fn(&Path) -> Option<bool>,
    info: &mut WorklogInfo,
    findings: &mut Vec<Finding>,
) {
    let (date, target) = match validate_day_heading(heading) {
        Ok(day) => day,
        Err(message) => {
            findings.push(Finding::new(
                dossier.path,
                heading.line,
                Severity::Error,
                Check::WorklogDay,
                message,
            ));
            return;
        }
    };

    let expected = format!("../{ENTRIES_DIR}/{date}.md");
    if target != expected {
        findings.push(Finding::new(
            dossier.path,
            heading.line,
            Severity::Error,
            Check::WorklogDay,
            format!("worklog day link points at '{target}', expected '{expected}'"),
        ));
        return;
    }

    let resolved = dossier.path.parent().map(|parent| parent.join(&target));
    match resolved.as_deref().and_then(lookup) {
        Some(true) => {
            info.valid_days.insert(date, heading.line);
        }
        Some(false) => findings.push(Finding::new(
            dossier.path,
            heading.line,
            Severity::Error,
            Check::WorklogDay,
            format!("worklog day link target '{target}' is not a file"),
        )),
        None => findings.push(Finding::new(
            dossier.path,
            heading.line,
            Severity::Error,
            Check::WorklogDay,
            format!("worklog day link target '{target}' does not exist"),
        )),
    }
}

fn check_worklog_backlinks(
    dossier: &Parsed<'_>,
    info: &WorklogInfo,
    entries_by_date: &BTreeMap<NaiveDate, &ParsedEntry<'_>>,
    findings: &mut Vec<Finding>,
) {
    for date in info.valid_days.keys() {
        let Some(entry) = entries_by_date.get(date) else {
            continue;
        };
        let sections = entry.note.document.exact_sections(LINKS_HEADING);
        let Some(section) = sections.first() else {
            continue;
        };

        let linked = entry_links(&entry.note.document, section);
        let name = dossier_name(dossier.path);
        if !linked.iter().any(|linked| linked.as_str() == name) {
            findings.push(Finding::new(
                entry.note.path,
                section.line,
                Severity::Warning,
                Check::CrossLink,
                format!("dossier '{name}' logs work on {date}, but this entry does not link it"),
            ));
        }
    }
}

pub(super) fn check_entry_section(entry: &ParsedEntry<'_>, findings: &mut Vec<Finding>) {
    if entry.note.document.exact_sections(LINKS_HEADING).is_empty() {
        findings.push(Finding::new(
            entry.note.path,
            1,
            Severity::Warning,
            Check::EntrySection,
            "missing '## Worked on Dossiers' section; 'bureau worklog' will create it the next time it links a dossier, or you can add it yourself",
        ));
    }
}

pub(super) fn check_entry_links(
    entries: &[ParsedEntry<'_>],
    dossiers_by_name: &HashMap<&str, &Parsed<'_>>,
    worklogs: &HashMap<PathBuf, WorklogInfo>,
    findings: &mut Vec<Finding>,
) {
    for entry in entries {
        let sections = entry.note.document.exact_sections(LINKS_HEADING);
        let Some(section) = sections.first() else {
            continue;
        };
        let end = entry.note.document.section_end(section);

        for link in entry.note.document.links_between(section.line, end) {
            let Some(name) = dossier_link_name(&link.target) else {
                continue;
            };
            let Some(dossier) = dossiers_by_name.get(name.as_str()) else {
                continue;
            };
            let Some(info) = worklogs.get(dossier.path) else {
                continue;
            };
            if !info.has_worklog {
                continue;
            }

            if !info.dates.contains(&entry.date) {
                findings.push(Finding::new(
                    entry.note.path,
                    link.line,
                    Severity::Warning,
                    Check::CrossLink,
                    format!(
                        "entry links dossier '{name}', but that dossier has no worklog heading for {}",
                        entry.date
                    ),
                ));
            }
        }
    }
}

fn day_heading_date(heading: &Heading) -> Option<NaiveDate> {
    if let Some((label, _)) = whole_link(&heading.text) {
        return NaiveDate::parse_from_str(&label, DATE_FORMAT).ok();
    }

    NaiveDate::parse_from_str(heading.text.trim(), DATE_FORMAT).ok()
}

/// Validate one `###` heading inside `## Worklog`.
fn validate_day_heading(heading: &Heading) -> Result<(NaiveDate, String), String> {
    if heading.indent != 0 {
        return Err("worklog day heading must start at the beginning of the line".to_owned());
    }

    let Some((label, target)) = whole_link(&heading.text) else {
        return Err("worklog day heading must link to its daily entry".to_owned());
    };

    let Some(date) = NaiveDate::parse_from_str(&label, DATE_FORMAT).ok() else {
        return Err("worklog day link label is not a YYYY-MM-DD date".to_owned());
    };

    Ok((date, target))
}

/// The dossier's name: its file stem.
pub(super) fn dossier_name(path: &Path) -> &str {
    path.file_stem().and_then(OsStr::to_str).unwrap_or("")
}
