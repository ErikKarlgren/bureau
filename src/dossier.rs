//! Turning a dossier name into a file name.

use sanitise_file_name::{Options, sanitise_with_options};
use std::fs;
use std::path::Path;

use chrono::{DateTime, Local, NaiveDate};

/// The extension every dossier file gets, appended after sanitisation.
const EXTENSION: &str = ".md";

/// Rules that make a name valid on both Windows and Linux.
///
/// The name is otherwise left alone, so `Förstudie` stays `Förstudie` rather
/// than losing its accents. `length_limit` is 255 minus the three bytes of
/// [`EXTENSION`], since the extension is appended after sanitisation.
const OPTIONS: Options<Option<char>> = Options {
    length_limit: 252,
    reserve_extra: 3,
    extension_cleverness: false,
    most_fs_safe: true,
    windows_safe: true,
    url_safe: false,
    normalise_whitespace: true,
    trim_spaces_and_full_stops: true,
    trim_more_punctuation: false,
    remove_control_characters: true,
    remove_reordering_characters: true,
    replace_with: Some('_'),
    collapse_replacements: true,
    six_measures_of_barley: "unnamed",
};

/// Rules that say whether an existing file name is portable to Windows.
///
/// This checks the name as it is, so it deliberately does not normalise
/// whitespace or trim the base name: `a  b.md` and `a..md` are allowed on
/// Windows even though `bureau new` would produce nicer names. The rules it
/// does enforce are the ones in `sanitise-file-name`: forbidden characters,
/// control characters, reserved device names, and the length limit. Trailing
/// spaces and dots are left to the extension split, because a `*.md` name
/// cannot end with one while a valid `a..md` must not be rejected.
const PORTABILITY: Options<Option<char>> = Options {
    length_limit: 765,
    reserve_extra: 0,
    extension_cleverness: true,
    most_fs_safe: true,
    windows_safe: true,
    url_safe: false,
    normalise_whitespace: false,
    trim_spaces_and_full_stops: false,
    trim_more_punctuation: false,
    remove_control_characters: true,
    remove_reordering_characters: true,
    replace_with: Some('_'),
    collapse_replacements: false,
    six_measures_of_barley: "unnamed",
};

/// Whether `name` can be used as a file name on Windows.
///
/// The name and its first dot-separated segment are passed through the same
/// sanitiser `bureau new` uses. If they come back unchanged, none of the
/// Windows rules had anything to change.
#[must_use]
pub fn is_portable_filename(name: &str) -> bool {
    // Windows counts UTF-16 code units, not bytes; the sanitiser's limit is
    // bytes, so set its limit high enough never to truncate a name Windows
    // accepts and check the real limit here.
    if name.encode_utf16().count() > 255 {
        return false;
    }

    if sanitise_with_options(name, &PORTABILITY) != name {
        return false;
    }

    // The sanitiser looks at the last extension, so `NUL.foo.md` slips through
    // while `NUL.foo` is still a reserved device name. Ask it about the first
    // dot-separated segment too; the answer comes from the same rule set.
    let base = name.split('.').next().unwrap_or(name);
    base.is_empty() || sanitise_with_options(base, &PORTABILITY) == base
}

/// The file name to store a dossier under, `unnamed.md` if nothing is left.
#[must_use]
pub fn name_to_filename(name: &str) -> String {
    let mut filename = sanitise_with_options(name, &OPTIONS);
    filename.push_str(EXTENSION);
    filename
}

/// Global lifecycle state of a dossier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DossierStatus {
    /// Dossier is actively worked on.
    Active,
    /// Dossier is on hold pending an external factor or blocker (requires a reason).
    OnHold { reason: String },
    /// Dossier's goals are fully met.
    Done,
    /// Dossier is archived/cancelled (requires a reason).
    Archived { reason: String },
}

impl DossierStatus {
    /// Whether the dossier is in active state.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Whether the dossier is on hold.
    #[must_use]
    pub const fn is_on_hold(&self) -> bool {
        matches!(self, Self::OnHold { .. })
    }

    /// Whether the dossier is marked done.
    #[must_use]
    pub const fn is_done(&self) -> bool {
        matches!(self, Self::Done)
    }

    /// Whether the dossier is archived.
    #[must_use]
    pub const fn is_archived(&self) -> bool {
        matches!(self, Self::Archived { .. })
    }

    /// Short label for the status.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::OnHold { .. } => "on hold",
            Self::Done => "done",
            Self::Archived { .. } => "archived",
        }
    }

    /// Human-readable representation including any reason.
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Active => "active".to_owned(),
            Self::OnHold { reason } => {
                if reason.is_empty() {
                    "on hold".to_owned()
                } else {
                    format!("on hold: {reason}")
                }
            }
            Self::Done => "done".to_owned(),
            Self::Archived { reason } => {
                if reason.is_empty() {
                    "archived".to_owned()
                } else {
                    format!("archived: {reason}")
                }
            }
        }
    }

    /// Parse dossier status from markdown contents.
    #[must_use]
    pub fn from_contents(contents: &str) -> Self {
        Self::from_contents_with_explicit(contents).0
    }

    /// Parse dossier status and whether it was explicitly specified in frontmatter.
    #[must_use]
    pub fn from_contents_with_explicit(contents: &str) -> (Self, bool) {
        let mut lines = contents.lines();
        if lines.next().map(str::trim_end) != Some("---") {
            return (Self::Active, false);
        }

        let mut status_raw: Option<String> = None;
        let mut reason_raw: Option<String> = None;
        let mut sealed_date: Option<String> = None;

        for line in lines.take_while(|line| line.trim_end() != "---") {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("status:") {
                status_raw = Some(rest.trim().to_owned());
            } else if let Some(rest) = trimmed.strip_prefix("reason:") {
                reason_raw = Some(rest.trim().to_owned());
            } else if let Some(rest) = trimmed.strip_prefix("sealed:") {
                sealed_date = Some(rest.trim().to_owned());
            }
        }

        if let Some(raw) = status_raw {
            let lower = raw.to_ascii_lowercase();
            if lower == "active" {
                return (Self::Active, true);
            }
            if lower == "done" {
                return (Self::Done, true);
            }
            if lower.starts_with("on hold")
                || lower.starts_with("on_hold")
                || lower.starts_with("on-hold")
            {
                let reason = if let Some((_, r)) = raw.split_once(':') {
                    r.trim().to_owned()
                } else {
                    reason_raw.unwrap_or_default()
                };
                return (Self::OnHold { reason }, true);
            }
            if lower.starts_with("archived") || lower.starts_with("archive") {
                let reason = if let Some((_, r)) = raw.split_once(':') {
                    r.trim().to_owned()
                } else {
                    reason_raw.unwrap_or_default()
                };
                return (Self::Archived { reason }, true);
            }
        }

        if let Some(date) = sealed_date {
            return (
                Self::Archived {
                    reason: format!("sealed: {date}"),
                },
                true,
            );
        }

        (Self::Active, false)
    }
}

/// Staleness information about a dossier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StalenessWarning {
    pub dossier_name: String,
    pub status: DossierStatus,
    pub days_inactive: i64,
}

impl StalenessWarning {
    #[must_use]
    pub fn message(&self) -> String {
        match &self.status {
            DossierStatus::Active => {
                format!(
                    "dossier '{}' is active but has not been updated in {} days",
                    self.dossier_name, self.days_inactive
                )
            }
            DossierStatus::OnHold { reason } => {
                if reason.is_empty() {
                    format!(
                        "dossier '{}' is on hold but has not been updated in {} days",
                        self.dossier_name, self.days_inactive
                    )
                } else {
                    format!(
                        "dossier '{}' is on hold ({reason}) and has not been updated in {} days",
                        self.dossier_name, self.days_inactive
                    )
                }
            }
            _ => format!(
                "dossier '{}' has not been updated in {} days",
                self.dossier_name, self.days_inactive
            ),
        }
    }
}

/// Determine if a dossier is stale based on its last update date and its state.
///
/// An active dossier is stale if not updated in 14 days (or custom threshold).
/// An on-hold dossier is stale if not updated in 30 days (or custom threshold).
/// Done and Archived dossiers are not considered stale.
#[must_use]
pub fn check_staleness(
    path: &Path,
    contents: &str,
    today: NaiveDate,
    status: &DossierStatus,
    active_threshold: Option<i64>,
    on_hold_threshold: Option<i64>,
) -> Option<StalenessWarning> {
    if status.is_done() || status.is_archived() {
        return None;
    }

    let active_limit = active_threshold.unwrap_or(14);
    let on_hold_limit = on_hold_threshold.unwrap_or(30);

    // Latest date in worklog:
    let worklog_days = crate::worklog::worklog_days(contents);
    let last_worklog_date = worklog_days.iter().map(|(date, _)| *date).max();

    // Modification date from file metadata if available
    let mtime_date = fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(|mtime| {
            let dt: DateTime<Local> = mtime.into();
            dt.date_naive()
        });

    let latest_date = last_worklog_date.or(mtime_date);

    let latest = latest_date?;

    let diff = today.signed_duration_since(latest);
    let days = diff.num_days();

    let is_stale = if status.is_active() {
        days >= active_limit
    } else if status.is_on_hold() {
        days >= on_hold_limit
    } else {
        false
    };

    if is_stale {
        let dossier_name = path.file_stem().map_or_else(
            || "unnamed".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        );
        Some(StalenessWarning {
            dossier_name,
            status: status.clone(),
            days_inactive: days,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_letters_that_are_not_ascii() {
        assert_eq!(name_to_filename("Förstudie"), "Förstudie.md");
    }

    #[test]
    fn replaces_characters_windows_rejects() {
        assert_eq!(
            name_to_filename("Q3 review: invoicing?"),
            "Q3 review_ invoicing_.md"
        );
    }

    #[test]
    fn collapses_whitespace_and_trims_edges() {
        assert_eq!(name_to_filename("  my   dossier.  "), "my dossier.md");
    }

    #[test]
    fn avoids_windows_reserved_names() {
        assert_eq!(name_to_filename("NUL"), "NUL_.md");
    }

    #[test]
    fn falls_back_to_unnamed() {
        assert_eq!(name_to_filename("..."), "unnamed.md");
    }

    #[test]
    fn leaves_room_for_the_extension() {
        let filename = name_to_filename(&"a".repeat(300));
        assert!(filename.ends_with(EXTENSION));
        assert!(filename.len() <= 255);
    }
}

#[cfg(test)]
mod portability_tests {
    use super::*;

    #[test]
    fn ordinary_names_are_portable() {
        for name in [
            "lmao.md",
            "my file.md",
            "Förstudie.md",
            "a  b.md",
            "a..md",
            "a .md",
            "NULx.md",
            "a#b.md",
        ] {
            assert!(is_portable_filename(name), "{name:?} should be portable");
        }
    }

    #[test]
    fn windows_rules_reject_the_usual_cases() {
        for name in [
            "NUL.md",
            "NUL.foo.md",
            "CON",
            "a:b.md",
            "a?b.md",
            "a|b.md",
            "a\"b.md",
            "a.md ",
            "a\u{0}b.md",
            "a\u{7}b.md",
        ] {
            assert!(
                !is_portable_filename(name),
                "{name:?} should not be portable"
            );
        }
    }

    #[test]
    fn the_length_limit_is_utf16_code_units() {
        assert!(is_portable_filename(&format!("{}.md", "漢".repeat(252))));
        assert!(!is_portable_filename(&format!("{}.md", "漢".repeat(253))));
    }

    #[test]
    fn the_length_limit_is_the_full_file_name() {
        assert!(!is_portable_filename(&format!("{}.md", "a".repeat(253))));
        assert!(is_portable_filename(&format!("{}.md", "a".repeat(252))));
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    #[test]
    fn default_status_is_active() {
        assert_eq!(DossierStatus::from_contents(""), DossierStatus::Active);
        assert_eq!(
            DossierStatus::from_contents("# Title\n- [ ] task\n"),
            DossierStatus::Active
        );
        assert_eq!(
            DossierStatus::from_contents("---\ncreated: 2026-09-01\n---\n# Title"),
            DossierStatus::Active
        );
    }

    #[test]
    fn parses_active_status() {
        let content = "---\nstatus: active\n---\n# Title";
        assert_eq!(DossierStatus::from_contents(content), DossierStatus::Active);
        assert!(DossierStatus::from_contents(content).is_active());
        assert_eq!(DossierStatus::from_contents(content).label(), "active");
        assert_eq!(DossierStatus::from_contents(content).display(), "active");
    }

    #[test]
    fn parses_done_status() {
        let content = "---\nstatus: done\n---\n# Title";
        assert_eq!(DossierStatus::from_contents(content), DossierStatus::Done);
        assert!(DossierStatus::from_contents(content).is_done());
        assert_eq!(DossierStatus::from_contents(content).label(), "done");
        assert_eq!(DossierStatus::from_contents(content).display(), "done");
    }

    #[test]
    fn parses_on_hold_status_with_inline_reason() {
        let content = "---\nstatus: on hold: waiting for backend\n---\n# Title";
        let status = DossierStatus::from_contents(content);
        assert_eq!(
            status,
            DossierStatus::OnHold {
                reason: "waiting for backend".to_owned()
            }
        );
        assert!(status.is_on_hold());
        assert_eq!(status.label(), "on hold");
        assert_eq!(status.display(), "on hold: waiting for backend");
    }

    #[test]
    fn parses_on_hold_status_with_separate_reason_field() {
        let content = "---\nstatus: on hold\nreason: blocked by security audit\n---\n# Title";
        let status = DossierStatus::from_contents(content);
        assert_eq!(
            status,
            DossierStatus::OnHold {
                reason: "blocked by security audit".to_owned()
            }
        );
        assert_eq!(status.display(), "on hold: blocked by security audit");
    }

    #[test]
    fn parses_archived_status_with_reason() {
        let content = "---\nstatus: archived: superseded by v2\n---\n# Title";
        let status = DossierStatus::from_contents(content);
        assert_eq!(
            status,
            DossierStatus::Archived {
                reason: "superseded by v2".to_owned()
            }
        );
        assert!(status.is_archived());
        assert_eq!(status.label(), "archived");
        assert_eq!(status.display(), "archived: superseded by v2");
    }

    #[test]
    fn parses_sealed_as_archived() {
        let content = "---\nsealed: 2026-09-20\n---\n# Title";
        let status = DossierStatus::from_contents(content);
        assert_eq!(
            status,
            DossierStatus::Archived {
                reason: "sealed: 2026-09-20".to_owned()
            }
        );
    }

    #[test]
    fn staleness_detects_inactive_active_dossier() {
        let scratch = crate::commands::tests::Scratch::new("staleness-test").unwrap();
        let path = scratch.path().join("dossier.md");
        let content = "## Worklog\n### [2026-09-01](<../entries/2026-09-01.md>)\n- worked\n";
        fs::write(&path, content).unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let warning = check_staleness(
            &path,
            content,
            today,
            &DossierStatus::Active,
            Some(14),
            Some(30),
        );
        assert!(warning.is_some());
        let warning = warning.unwrap();
        assert_eq!(warning.days_inactive, 30);
        assert!(warning.message().contains("30 days"));
    }

    #[test]
    fn staleness_ignores_done_and_archived_dossiers() {
        let path = Path::new("test.md");
        let content = "## Worklog\n### [2026-01-01](<../entries/2026-01-01.md>)\n";
        let today = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();

        assert!(check_staleness(path, content, today, &DossierStatus::Done, None, None).is_none());
        assert!(
            check_staleness(
                path,
                content,
                today,
                &DossierStatus::Archived {
                    reason: "old".to_owned()
                },
                None,
                None
            )
            .is_none()
        );
    }
}
