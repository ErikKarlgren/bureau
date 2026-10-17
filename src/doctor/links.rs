use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Component, Path};

use super::document::{Document, Heading};
use super::{Check, DOSSIERS_DIR, Finding, Parsed, Severity};

/// Check every local link in one note.
pub(super) fn check_links(
    note: &Parsed<'_>,
    skip_lines: &HashSet<usize>,
    lookup: &impl Fn(&Path) -> Option<bool>,
    findings: &mut Vec<Finding>,
) {
    for link in note.document.links() {
        if skip_lines.contains(&link.line) {
            continue;
        }

        if link.malformed {
            findings.push(Finding::new(
                note.path,
                link.line,
                Severity::Warning,
                Check::Link,
                "malformed link: the target or its closing parenthesis is missing",
            ));
            continue;
        }

        if link.target.trim().is_empty() {
            findings.push(Finding::new(
                note.path,
                link.line,
                Severity::Warning,
                Check::Link,
                "empty link target",
            ));
            continue;
        }

        if !link.angle && link.target.chars().any(char::is_whitespace) {
            findings.push(Finding::new(
                note.path,
                link.line,
                Severity::Warning,
                Check::Link,
                "link target contains spaces; wrap it in <...>",
            ));
        }

        let Some(target) = local_target(&link.target) else {
            continue;
        };
        let Some(parent) = note.path.parent() else {
            continue;
        };
        let resolved = parent.join(target);

        match lookup(&resolved) {
            None => findings.push(Finding::new(
                note.path,
                link.line,
                Severity::Warning,
                Check::Link,
                format!("link target '{}' does not exist", link.target),
            )),
            Some(false) => findings.push(Finding::new(
                note.path,
                link.line,
                Severity::Warning,
                Check::Link,
                format!("link target '{}' is not a file", link.target),
            )),
            Some(true) => {}
        }
    }
}

pub(super) fn entry_links(document: &Document, section: &Heading) -> Vec<String> {
    let end = document.section_end(section);
    document
        .links_between(section.line, end)
        .into_iter()
        .filter_map(|link| dossier_link_name(&link.target))
        .collect()
}

/// The dossier name a link in `## Worked on Dossiers` points at.
pub(super) fn dossier_link_name(target: &str) -> Option<String> {
    let target = local_target(target)?;
    let mut components = Path::new(target).components();

    if components.next() != Some(Component::ParentDir) {
        return None;
    }
    let Component::Normal(directory) = components.next()? else {
        return None;
    };
    if directory != OsStr::new(DOSSIERS_DIR) {
        return None;
    }
    let Component::Normal(file) = components.next()? else {
        return None;
    };
    if components.next().is_some() {
        return None;
    }

    Path::new(file)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
}

/// The relative part of a target, when the target is local.
fn local_target(target: &str) -> Option<&str> {
    let target = target.trim();

    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with("//")
        || target.starts_with('/')
        || has_scheme(target)
    {
        return None;
    }

    let end = target.find(['#', '?']).unwrap_or(target.len());
    let target = target.get(..end)?;

    (!target.is_empty()).then_some(target)
}

/// Whether a link target starts with a URL scheme.
fn has_scheme(target: &str) -> bool {
    let mut characters = target.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    if !first.is_ascii_alphabetic() {
        return false;
    }

    for character in characters {
        if character == ':' {
            return true;
        }
        if !(character.is_ascii_alphanumeric() || "+-.".contains(character)) {
            return false;
        }
    }

    false
}
