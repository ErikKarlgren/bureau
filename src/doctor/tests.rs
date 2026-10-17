use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use chrono::NaiveDate;

use super::*;

fn note(path: &str, contents: &str) -> Note {
    Note {
        path: PathBuf::from(path),
        contents: contents.to_owned(),
    }
}

fn entry(path: &str, date: NaiveDate, contents: &str) -> EntryNote {
    EntryNote {
        path: PathBuf::from(path),
        date,
        contents: contents.to_owned(),
    }
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn normalise(path: &Path) -> PathBuf {
    let mut normalised = PathBuf::new();

    for component in path.components() {
        match component {
            Component::ParentDir => {
                normalised.pop();
            }
            Component::CurDir => {}
            other => normalised.push(other.as_os_str()),
        }
    }

    normalised
}

fn lookup(files: &[(&str, bool)]) -> impl Fn(&Path) -> Option<bool> {
    let files: HashMap<PathBuf, bool> = files
        .iter()
        .map(|(path, is_file)| (normalise(Path::new(path)), *is_file))
        .collect();

    move |path| files.get(&normalise(path)).copied()
}

fn findings(input: &Input, files: &[(&str, bool)]) -> Vec<Finding> {
    check(input, lookup(files))
}

fn messages(findings: &[Finding]) -> Vec<String> {
    findings
        .iter()
        .map(|finding| finding.message.clone())
        .collect()
}

#[test]
fn empty_tasks_and_bad_markers_are_warnings() {
    let input = Input {
        dossiers: vec![note("dossiers/a.md", "## Worklog\n- [ ]\n- [X] maybe\n")],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found
            .iter()
            .all(|finding| finding.severity == Severity::Warning)
    );
    assert!(
        messages(&found)
            .iter()
            .any(|message| message.contains("empty task"))
    );
    assert!(
        messages(&found)
            .iter()
            .any(|message| message.contains("not a recognised"))
    );
}

#[test]
fn non_dash_tasks_are_warnings() {
    let input = Input {
        dossiers: vec![note(
            "dossiers/a.md",
            "## Worklog\n* [ ] star\n1. [x] ordered\n",
        )],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found
            .iter()
            .all(|finding| finding.message.contains("does not start with '-'"))
    );
}

#[test]
fn an_unclosed_fence_is_an_error() {
    let input = Input {
        dossiers: vec![note("dossiers/a.md", "## Worklog\n```\n")],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("unclosed")),
        "{found:?}"
    );
}

#[test]
fn a_missing_worklog_is_an_error() {
    let input = Input {
        dossiers: vec![note("dossiers/a.md", "# a\n")],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("missing '## Worklog'")),
        "{found:?}"
    );
}

#[test]
fn a_bare_worklog_day_is_an_error() {
    let input = Input {
        dossiers: vec![note(
            "dossiers/a.md",
            "## Worklog\n### 2026-09-20\n- did work\n",
        )],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("must link")),
        "{found:?}"
    );
}

#[test]
fn a_wrong_worklog_day_target_is_an_error() {
    let input = Input {
        dossiers: vec![note(
            "dossiers/a.md",
            "## Worklog\n### [2026-09-20](<../entries/2026-09-19.md>)\n",
        )],
        entries: Vec::new(),
    };

    let found = findings(&input, &[("entries/2026-09-19.md", true)]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("expected")),
        "{found:?}"
    );
}

#[test]
fn an_entry_without_the_links_section_is_a_warning() {
    let input = Input {
        dossiers: Vec::new(),
        entries: vec![entry(
            "entries/2026-09-20.md",
            date(2026, 9, 20),
            "# 2026-09-20\n",
        )],
    };

    let found = findings(&input, &[]);

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found
            .first()
            .is_some_and(|finding| finding.message.contains("bureau worklog"))
    );
}

#[test]
fn a_dossier_worklog_day_requires_the_entry_to_link_back() {
    let input = Input {
        dossiers: vec![note(
            "dossiers/1.md",
            "## Worklog\n### [2026-09-20](<../entries/2026-09-20.md>)\n- did work\n",
        )],
        entries: vec![entry(
            "entries/2026-09-20.md",
            date(2026, 9, 20),
            "# 2026-09-20\n## Worked on Dossiers\n- [2](<../dossiers/2.md>)\n",
        )],
    };

    let found = findings(
        &input,
        &[("dossiers/2.md", true), ("entries/2026-09-20.md", true)],
    );

    assert!(
        found.iter().any(|finding| {
            finding.message.contains("does not link it")
                && finding.path.as_path() == Path::new("entries/2026-09-20.md")
        }),
        "{found:?}"
    );
}

#[test]
fn an_entry_link_requires_a_worklog_day() {
    let input = Input {
        dossiers: vec![note("dossiers/1.md", "## Worklog\n")],
        entries: vec![entry(
            "entries/2026-09-20.md",
            date(2026, 9, 20),
            "# 2026-09-20\n## Worked on Dossiers\n- [1](<../dossiers/1.md>)\n",
        )],
    };

    let found = findings(&input, &[("dossiers/1.md", true)]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("no worklog heading")),
        "{found:?}"
    );
}

#[test]
fn a_broken_relative_link_is_a_warning() {
    let input = Input {
        dossiers: vec![note(
            "dossiers/a.md",
            "## Worklog\n- [missing](<../missing.md>)\n",
        )],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("does not exist")),
        "{found:?}"
    );
}

#[test]
fn a_link_to_a_directory_is_a_warning() {
    let input = Input {
        dossiers: vec![note(
            "dossiers/a.md",
            "## Worklog\n- [directory](<../dossiers>)\n",
        )],
        entries: Vec::new(),
    };

    let found = findings(&input, &[("dossiers", false)]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("is not a file")),
        "{found:?}"
    );
}

#[test]
fn non_portable_names_are_warnings() {
    let input = Input {
        dossiers: vec![note("dossiers/NUL.md", "## Worklog\n")],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("not portable")),
        "{found:?}"
    );
}

#[test]
fn case_collisions_are_warnings() {
    let input = Input {
        dossiers: vec![
            note("dossiers/A.md", "## Worklog\n"),
            note("dossiers/a.md", "## Worklog\n"),
        ],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert_eq!(
        found
            .iter()
            .filter(|finding| finding.message.contains("differs only by case"))
            .count(),
        2,
        "{found:?}"
    );
}

#[test]
fn line_endings_and_bom_are_warnings() {
    let input = Input {
        dossiers: vec![note("dossiers/a.md", "\u{feff}# a\r\n## Worklog\r\n")],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found.iter().any(|finding| finding.message.contains("BOM")),
        "{found:?}"
    );
    assert!(
        found.iter().any(|finding| finding.message.contains("CRLF")),
        "{found:?}"
    );
}

#[test]
fn a_task_marker_without_a_space_after_the_dash_is_still_seen() {
    let input = Input {
        dossiers: vec![note("dossiers/a.md", "## Worklog\n-[ ]\n")],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("empty task")),
        "{found:?}"
    );
}

#[test]
fn a_link_with_spaces_needs_angle_brackets() {
    let input = Input {
        dossiers: vec![note(
            "dossiers/a.md",
            "## Worklog\n- [file](<../my file.md>)\n- [file](<../my file.md>)\n",
        )],
        entries: Vec::new(),
    };

    let found = findings(&input, &[("my file.md", true)]);

    assert!(
        found
            .iter()
            .all(|finding| !finding.message.contains("contains spaces")),
        "{found:?}"
    );
}

#[test]
fn a_space_in_an_unwrapped_target_is_reported() {
    let input = Input {
        dossiers: vec![note("dossiers/a.md", "## Worklog\n- [file](my file.md)\n")],
        entries: Vec::new(),
    };

    let found = findings(&input, &[("my file.md", true)]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("contains spaces")),
        "{found:?}"
    );
}

#[test]
fn a_malformed_link_is_reported() {
    let input = Input {
        dossiers: vec![note("dossiers/a.md", "## Worklog\n- [file](<../file.md>\n")],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("malformed link")),
        "{found:?}"
    );
}

#[test]
fn a_duplicate_worklog_section_is_an_error() {
    let input = Input {
        dossiers: vec![note("dossiers/a.md", "## Worklog\n## Worklog\n- [ ]\n")],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("duplicate '## Worklog'")),
        "{found:?}"
    );
}

#[test]
fn a_closed_fence_hides_a_task() {
    let input = Input {
        dossiers: vec![note(
            "dossiers/a.md",
            "## Worklog\n```\n- [ ] not a task\n```\n",
        )],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found
            .iter()
            .all(|finding| !finding.message.contains("empty task")),
        "{found:?}"
    );
}

#[test]
fn a_forbidden_character_is_not_portable() {
    let input = Input {
        dossiers: vec![note("dossiers/a:b.md", "## Worklog\n")],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("not portable")),
        "{found:?}"
    );
}

#[test]
fn a_missing_final_newline_is_a_warning() {
    let input = Input {
        dossiers: vec![note("dossiers/a.md", "## Worklog")],
        entries: Vec::new(),
    };

    let found = findings(&input, &[]);

    assert!(
        found
            .iter()
            .any(|finding| finding.message.contains("does not end with a newline")),
        "{found:?}"
    );
}
