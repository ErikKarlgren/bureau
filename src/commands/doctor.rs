//! `bureau doctor`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use chrono::NaiveDate;

use crate::commands::files::Files;
use crate::commands::paths::{DOSSIERS_DIR, ENTRIES_DIR};
use crate::commands::sources;
use crate::doctor::{self, EntryNote, Input, Note};
use crate::git;
use crate::style::{self, Palette};

/// How a daily entry is named.
const DATE_FORMAT: &str = "%Y-%m-%d";

/// Check the notes for formatting problems.
///
/// # Errors
///
/// Fails when the repository root cannot be found, or when a notes directory
/// itself cannot be read. A note that cannot be read is a finding, not a
/// failure of the whole run.
pub fn run() -> Result<ExitCode> {
    let root = git::toplevel()?;
    let palette = style::for_stdout();
    let (output, errors) = run_in(&root, palette)?;

    print!("{output}");
    Ok(if errors {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// The rest of `run`, with the root and the colours handed in, so a test can
/// drive it without a git checkout or a terminal.
fn run_in(root: &Path, palette: Palette) -> Result<(String, bool)> {
    let mut files = Files::new();
    let mut findings = Vec::new();
    let input = read_input(root, &mut files, &mut findings)?;

    findings.extend(doctor::check(&input, |path| {
        fs::metadata(path).ok().map(|metadata| metadata.is_file())
    }));
    doctor::sort(&mut findings);

    let output = render(&findings, root, palette);
    Ok((output, doctor::has_errors(&findings)))
}

/// Read every dossier and date-named entry into the checker's input.
fn read_input(
    root: &Path,
    files: &mut Files,
    findings: &mut Vec<doctor::Finding>,
) -> Result<Input> {
    let mut dossiers = Vec::new();
    for path in note_paths(root, DOSSIERS_DIR)? {
        match files.read(&path) {
            Ok(contents) => {
                // Sealing is not implemented yet; when it is, doctor should
                // probably still check sealed dossiers. For now the command
                // follows the action commands and leaves them out.
                if sources::is_sealed(contents) {
                    continue;
                }
                dossiers.push(Note {
                    path: path.clone(),
                    contents: contents.to_owned(),
                });
            }
            Err(error) => findings.push(doctor::unreadable(&path, format!("{error:#}"))),
        }
    }

    let mut entries = Vec::new();
    for path in note_paths(root, ENTRIES_DIR)? {
        let Some(date) = date_of(&path) else {
            continue;
        };

        match files.read(&path) {
            Ok(contents) => entries.push(EntryNote {
                path: path.clone(),
                date,
                contents: contents.to_owned(),
            }),
            Err(error) => findings.push(doctor::unreadable(&path, format!("{error:#}"))),
        }
    }

    Ok(Input { dossiers, entries })
}

/// The `.md` files in one notes directory, sorted by name.
fn note_paths(root: &Path, directory: &str) -> Result<Vec<PathBuf>> {
    let directory = root.join(directory);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read '{}'", directory.display()));
        }
    };

    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .with_context(|| format!("could not read '{}'", directory.display()))?
            .path();

        if path.extension().is_some_and(|extension| extension == "md") {
            paths.push(path);
        }
    }

    paths.sort();
    Ok(paths)
}

/// The date an entry file is named after.
fn date_of(path: &Path) -> Option<NaiveDate> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| NaiveDate::parse_from_str(stem, DATE_FORMAT).ok())
}

/// Render findings as `path:line: Severity: message`, then a summary.
fn render(findings: &[doctor::Finding], root: &Path, palette: Palette) -> String {
    let mut output = String::new();
    let mut errors = 0usize;
    let mut warnings = 0usize;

    for finding in findings {
        match finding.severity {
            doctor::Severity::Error => errors = errors.saturating_add(1),
            doctor::Severity::Warning => warnings = warnings.saturating_add(1),
        }

        let path = sources::relative(&finding.path, root);
        let path = path
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let location = format!("{path}:{}:", finding.line);
        let severity = match finding.severity {
            doctor::Severity::Error => palette.error(),
            doctor::Severity::Warning => palette.warning(),
        };
        let message = format!("{}: {}", finding.severity.label(), finding.message);

        output.push_str(&palette.path().paint(&location));
        output.push(' ');
        output.push_str(&severity.paint(&message));
        output.push('\n');
    }

    let summary = if errors == 0 && warnings == 0 {
        "No formatting issues found".to_owned()
    } else {
        summary(errors, warnings)
    };
    output.push_str(&summary);
    output.push('\n');

    output
}

/// The last line of a run with findings.
fn summary(errors: usize, warnings: usize) -> String {
    let errors = count(errors, "error");
    let warnings = count(warnings, "warning");

    match (errors.as_str(), warnings.as_str()) {
        ("", "") => "No formatting issues found".to_owned(),
        (errors, "") => errors.to_owned(),
        ("", warnings) => warnings.to_owned(),
        (errors, warnings) => format!("{errors}, {warnings}"),
    }
}

/// A count with its noun, or nothing when the count is zero.
fn count(number: usize, noun: &str) -> String {
    match number {
        0 => String::new(),
        1 => format!("1 {noun}"),
        _ => format!("{number} {noun}s"),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::commands::tests::Scratch;

    /// Run the command in a scratch root with plain output.
    fn run_plain(scratch: &Scratch) -> (String, bool) {
        run_in(scratch.path(), Palette::OFF).unwrap()
    }

    #[test]
    fn no_notes_is_not_an_error() {
        let scratch = Scratch::new("doctor-empty").unwrap();

        let (output, errors) = run_plain(&scratch);

        assert_eq!(output, "No formatting issues found\n");
        assert!(!errors);
    }

    #[test]
    fn a_clean_repository_has_nothing_to_report() {
        let scratch = Scratch::new("doctor-clean").unwrap();
        scratch
            .write(
                "dossiers/1 - clean.md",
                "# 1 - clean\n- Creation date: 2026-09-20 10:00:00\n\n## Worklog\n\
                 ### [2026-09-20](<../entries/2026-09-20.md>)\n- did work\n",
            )
            .unwrap();
        scratch
            .write(
                "entries/2026-09-20.md",
                "# 2026-09-20\n\n## Notes\n- note\n\n## Worked on Dossiers\n\
                 - [1 - clean](<../dossiers/1 - clean.md>)\n",
            )
            .unwrap();

        let (output, errors) = run_plain(&scratch);

        assert_eq!(output, "No formatting issues found\n");
        assert!(!errors);
    }

    #[test]
    fn warnings_do_not_fail_the_command_but_errors_do() {
        let scratch = Scratch::new("doctor-exit").unwrap();
        scratch
            .write("dossiers/1 - warning.md", "# 1 - warning\n## Worklog\n")
            .unwrap();
        scratch
            .write(
                "dossiers/2 - error.md",
                "# 2 - error\n## Worklog\n### 2026-09-20\n- did work\n",
            )
            .unwrap();
        scratch
            .write("entries/2026-09-20.md", "# 2026-09-20\n")
            .unwrap();

        let (output, errors) = run_plain(&scratch);

        assert!(
            output.contains("Warning: missing '## Worked on Dossiers'"),
            "{output}"
        );
        assert!(
            output.contains("Error: worklog day heading must link"),
            "{output}"
        );
        assert!(errors);
    }

    #[test]
    fn a_missing_worklog_is_an_error() {
        let scratch = Scratch::new("doctor-worklog-missing").unwrap();
        scratch
            .write("dossiers/1 - no-worklog.md", "# 1\n")
            .unwrap();

        let (output, errors) = run_plain(&scratch);

        assert!(
            output.contains("Error: missing '## Worklog' section"),
            "{output}"
        );
        assert!(errors);
    }

    #[test]
    fn a_bare_worklog_day_is_an_error() {
        let scratch = Scratch::new("doctor-worklog-bare").unwrap();
        scratch
            .write(
                "dossiers/1 - bare.md",
                "# 1\n## Worklog\n### 2026-09-20\n- did work\n",
            )
            .unwrap();

        let (output, errors) = run_plain(&scratch);

        assert!(
            output.contains("Error: worklog day heading must link to its daily entry"),
            "{output}"
        );
        assert!(errors);
    }

    #[test]
    fn a_missing_entry_section_is_a_warning() {
        let scratch = Scratch::new("doctor-entry-section").unwrap();
        scratch
            .write(
                "entries/2026-09-20.md",
                "# 2026-09-20\n\n## Notes\n- note\n",
            )
            .unwrap();

        let (output, errors) = run_plain(&scratch);

        assert!(
            output.contains("Warning: missing '## Worked on Dossiers' section"),
            "{output}"
        );
        assert!(output.contains("bureau worklog"), "{output}");
        assert!(!errors);
    }

    #[test]
    fn empty_tasks_are_warnings() {
        let scratch = Scratch::new("doctor-empty-task").unwrap();
        scratch
            .write("dossiers/1 - empty.md", "## Worklog\n- [ ] \n")
            .unwrap();

        let (output, errors) = run_plain(&scratch);

        assert!(output.contains("Warning: empty task"), "{output}");
        assert!(!errors);
    }

    #[test]
    fn unknown_and_non_dash_markers_are_warnings() {
        let scratch = Scratch::new("doctor-markers").unwrap();
        scratch
            .write(
                "dossiers/1 - markers.md",
                "## Worklog\n- [X] maybe\n* [ ] star\n1. [x] ordered\n",
            )
            .unwrap();

        let (output, errors) = run_plain(&scratch);

        assert!(
            output.contains("Warning: not a recognised task marker"),
            "{output}"
        );
        assert!(
            output.contains("Warning: task marker on a bullet"),
            "{output}"
        );
        assert!(!errors);
    }

    #[test]
    fn broken_relative_links_are_warnings() {
        let scratch = Scratch::new("doctor-links").unwrap();
        scratch
            .write(
                "dossiers/1 - links.md",
                "# 1\n## Worklog\n\n## Notes\n- [missing](<../missing.md>)\n",
            )
            .unwrap();

        let (output, errors) = run_plain(&scratch);

        assert!(
            output.contains("Warning: link target '../missing.md' does not exist"),
            "{output}"
        );
        assert!(!errors);
    }

    #[test]
    fn cross_links_are_checked_in_both_directions() {
        let scratch = Scratch::new("doctor-cross").unwrap();
        scratch
            .write(
                "dossiers/1 - one.md",
                "# 1\n## Worklog\n### [2026-09-20](<../entries/2026-09-20.md>)\n- did work\n",
            )
            .unwrap();
        scratch
            .write(
                "entries/2026-09-20.md",
                "# 2026-09-20\n\n## Notes\n- note\n\n## Worked on Dossiers\n\
                 - [2 - two](<../dossiers/2 - two.md>)\n",
            )
            .unwrap();
        scratch
            .write("dossiers/2 - two.md", "# 2\n## Worklog\n")
            .unwrap();

        let (output, errors) = run_plain(&scratch);

        assert!(
            output.contains(
                "dossier '1 - one' logs work on 2026-09-20, but this entry does not link it"
            ),
            "{output}"
        );
        assert!(
            output
                .contains("entry links dossier '2 - two', but that dossier has no worklog heading"),
            "{output}"
        );
        assert!(!errors);
    }

    #[test]
    fn a_non_portable_name_is_a_warning() {
        let scratch = Scratch::new("doctor-name").unwrap();
        scratch
            .write("dossiers/NUL.md", "# NUL\n## Worklog\n")
            .unwrap();

        let (output, errors) = run_plain(&scratch);

        assert!(
            output.contains("Warning: file name is not portable to Windows"),
            "{output}"
        );
        assert!(!errors);
    }

    #[test]
    fn sealed_dossiers_are_skipped() {
        let scratch = Scratch::new("doctor-sealed").unwrap();
        scratch
            .write(
                "dossiers/1 - sealed.md",
                "---\nsealed: 2026-09-20\n---\n# 1\n",
            )
            .unwrap();

        let (output, errors) = run_plain(&scratch);

        assert_eq!(output, "No formatting issues found\n");
        assert!(!errors);
    }

    #[test]
    fn an_unreadable_note_is_an_error_finding() {
        let scratch = Scratch::new("doctor-unreadable").unwrap();
        let path = scratch.path().join("dossiers/1 - broken.md");
        fs::create_dir_all(scratch.path().join("dossiers")).unwrap();
        fs::write(&path, b"\xff").unwrap();

        let (output, errors) = run_plain(&scratch);

        assert!(output.contains("Error: could not read"), "{output}");
        assert!(errors);
    }

    #[test]
    fn colours_paint_the_location_and_the_severity() {
        let scratch = Scratch::new("doctor-colour").unwrap();
        scratch
            .write("entries/2026-09-20.md", "# 2026-09-20\n")
            .unwrap();

        let (output, _) = run_in(scratch.path(), Palette::ON).unwrap();

        assert!(
            output.contains("\x1b[36mentries/2026-09-20.md:1:\x1b[0m"),
            "{output:?}"
        );
        assert!(
            output.contains("\x1b[1;33mWarning: missing '## Worked on Dossiers'"),
            "{output:?}"
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn the_summary_pluralizes_and_omits_zeroes() {
        assert_eq!(summary(0, 0), "No formatting issues found");
        assert_eq!(summary(0, 1), "1 warning");
        assert_eq!(summary(0, 2), "2 warnings");
        assert_eq!(summary(1, 0), "1 error");
        assert_eq!(summary(1, 1), "1 error, 1 warning");
        assert_eq!(summary(2, 1), "2 errors, 1 warning");
    }
}
