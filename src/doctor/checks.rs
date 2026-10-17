use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

use super::document::{Bullet, BulletMarker};
use crate::tasks::State;

use super::{Check, Finding, Parsed, Severity};

/// Windows device names that cannot be used as a file stem.
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Check the things that apply to every note.
pub(super) fn check_note(note: &Parsed<'_>, findings: &mut Vec<Finding>) {
    check_name(note, findings);
    check_encoding(note, findings);
    check_fence(note, findings);
    check_tasks(note, findings);
}

fn check_name(note: &Parsed<'_>, findings: &mut Vec<Finding>) {
    let Some(name) = note.path.file_name().and_then(OsStr::to_str) else {
        findings.push(Finding::new(
            note.path,
            1,
            Severity::Warning,
            Check::Name,
            "file name is not valid UTF-8",
        ));
        return;
    };

    if let Some(reason) = name_issue(name) {
        findings.push(Finding::new(
            note.path,
            1,
            Severity::Warning,
            Check::Name,
            format!("file name is not portable to Windows: {reason}"),
        ));
    }
}

fn check_encoding(note: &Parsed<'_>, findings: &mut Vec<Finding>) {
    let contents = note.contents;

    if contents.starts_with('\u{feff}') {
        findings.push(Finding::new(
            note.path,
            1,
            Severity::Warning,
            Check::Encoding,
            "file starts with a UTF-8 BOM",
        ));
    }

    let crlf = contents.matches("\r\n").count();
    let newlines = contents.matches('\n').count();
    let lone_lf = newlines.saturating_sub(crlf);

    if crlf > 0 && lone_lf > 0 {
        findings.push(Finding::new(
            note.path,
            1,
            Severity::Warning,
            Check::Encoding,
            "file mixes CRLF and LF line endings",
        ));
    } else if crlf > 0 {
        findings.push(Finding::new(
            note.path,
            1,
            Severity::Warning,
            Check::Encoding,
            "file uses CRLF line endings",
        ));
    }

    if !contents.is_empty() && !contents.ends_with('\n') {
        findings.push(Finding::new(
            note.path,
            1,
            Severity::Warning,
            Check::Encoding,
            "file does not end with a newline",
        ));
    }
}

fn check_fence(note: &Parsed<'_>, findings: &mut Vec<Finding>) {
    if let Some(line) = note.document.unclosed_fence() {
        findings.push(Finding::new(
            note.path,
            line,
            Severity::Error,
            Check::Fence,
            "unclosed fenced code block",
        ));
    }
}

fn check_tasks(note: &Parsed<'_>, findings: &mut Vec<Finding>) {
    for bullet in note.document.bullets() {
        match bullet.marker {
            BulletMarker::Dash => check_dash_task(note, bullet, findings),
            BulletMarker::Star | BulletMarker::Plus | BulletMarker::Ordered => {
                if short_marker(&bullet.text).is_some() {
                    findings.push(Finding::new(
                        note.path,
                        bullet.line,
                        Severity::Warning,
                        Check::TaskBullet,
                        "task marker on a bullet that does not start with '-'; bureau tasks only reads '-' bullets",
                    ));
                }
            }
        }
    }
}

fn check_dash_task(note: &Parsed<'_>, bullet: &Bullet, findings: &mut Vec<Finding>) {
    if let Some((state, text)) = task_marker(&bullet.text) {
        if text.trim().is_empty() {
            findings.push(Finding::new(
                note.path,
                bullet.line,
                Severity::Warning,
                Check::EmptyTask,
                format!(
                    "empty task; bureau tasks lists it as '- [{}]'",
                    state.symbol()
                ),
            ));
        }
        return;
    }

    if looks_like_invalid_marker(&bullet.text) {
        findings.push(Finding::new(
            note.path,
            bullet.line,
            Severity::Warning,
            Check::TaskMarker,
            "not a recognised task marker; bureau tasks treats it as prose",
        ));
    }
}

/// Check names that differ only by case in one directory.
pub(super) fn check_case_collisions<'a>(
    paths: impl Iterator<Item = &'a Path>,
    findings: &mut Vec<Finding>,
) {
    let mut groups: HashMap<String, Vec<&Path>> = HashMap::new();

    for path in paths {
        if let Some(name) = path.file_name().and_then(OsStr::to_str) {
            groups.entry(name.to_lowercase()).or_default().push(path);
        }
    }

    for paths in groups.values().filter(|paths| paths.len() > 1) {
        for (index, path) in paths.iter().enumerate() {
            let other = paths
                .iter()
                .enumerate()
                .find(|(other_index, _)| *other_index != index)
                .and_then(|(_, other)| other.file_name())
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());

            findings.push(Finding::new(
                path,
                1,
                Severity::Warning,
                Check::Name,
                format!("file name differs only by case from '{other}'"),
            ));
        }
    }
}

/// A reason a file name is not portable to Windows.
fn name_issue(name: &str) -> Option<&'static str> {
    if name
        .chars()
        .any(|character| "<>:\"/\\|?*".contains(character))
    {
        return Some("it contains a character Windows forbids");
    }
    if name.chars().any(char::is_control) {
        return Some("it contains a control character");
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Some("it ends with a dot or a space");
    }

    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    if RESERVED_NAMES.contains(&stem.as_str()) {
        return Some("it is a reserved Windows device name");
    }
    if name.len() > 255 {
        return Some("it is longer than 255 bytes");
    }

    None
}

fn task_marker(text: &str) -> Option<(State, &str)> {
    let text = text.trim_start_matches(|character: char| character.is_ascii_whitespace());
    let mut characters = text.chars();

    if characters.next()? != '[' {
        return None;
    }
    let state = State::from_symbol(characters.next()?)?;
    if characters.next()? != ']' {
        return None;
    }

    let after = text
        .char_indices()
        .nth(3)
        .map_or("", |(offset, _)| text.get(offset..).unwrap_or(""));

    Some((state, after))
}

/// Whether a bullet starts with a bracket shape a task marker could use.
fn short_marker(text: &str) -> Option<&str> {
    let text = text.trim_start_matches(|character: char| character.is_ascii_whitespace());
    let rest = text.strip_prefix('[')?;
    let close = rest.find(']')?;
    let content = rest.get(..close)?;

    (content.chars().count() <= 2).then_some(content)
}

/// Whether a bullet starts with something that looks like a mistyped task
/// marker.
fn looks_like_invalid_marker(text: &str) -> bool {
    let Some(content) = short_marker(text) else {
        return false;
    };

    let valid = content.chars().count() == 1
        && content
            .chars()
            .next()
            .and_then(State::from_symbol)
            .is_some();

    if valid {
        return false;
    }

    content.chars().count() <= 1
        || content
            .chars()
            .any(|character| character.is_whitespace() || "xXoO.?-!".contains(character))
}
