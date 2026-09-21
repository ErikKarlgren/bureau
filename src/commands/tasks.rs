//! `bureau tasks`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::NaiveDate;

use crate::cli::{Filter, TasksArgs};
use crate::commands::paths::{DOSSIERS_DIR, ENTRIES_DIR};
use crate::commands::selection::{self, Request};
use crate::commands::sources::{self, Source};
use crate::git;
use crate::tasks::{Section, Tree};

/// The date a daily entry is named after, as `YYYY-MM-DD`.
const DATE_FORMAT: &str = "%Y-%m-%d";

/// How a section announces itself.
const SECTION_RULE: &str = "===";

/// What a run prints when no section has anything in it.
const NOTHING_ACTIONABLE: &str = "No actionable tasks";

/// List the tasks in every open dossier and entry, as a tree.
///
/// # Errors
///
/// Fails when the repository root cannot be found, when a filter matches
/// nothing or is cancelled at the picker, or when a note cannot be read.
/// Nothing is written either way: this command only reports.
pub fn run(args: &TasksArgs) -> Result<()> {
    let root = git::toplevel()?;
    println!("{}", run_in(&root, args, selection::pick)?);
    Ok(())
}

/// The rest of `run`, with the repository root and the picker handed in, so a
/// test can drive it without a git checkout or a terminal.
///
/// The picker is only consulted for `--filter` and `--menu`, which are also
/// the only ways to leave the entries out of a run.
fn run_in(
    root: &Path,
    args: &TasksArgs,
    picker: impl Fn(&[PathBuf]) -> Result<PathBuf>,
) -> Result<String> {
    let request = Request {
        pattern: args.filter.as_ref().and_then(Filter::pattern),
        menu: args.menu,
    };

    let selected = if request.menu || args.filter.is_some() {
        let chosen = selection::select(
            &sources::open_paths(&sources::read_markdown(root, DOSSIERS_DIR)?),
            request,
            picker,
        )?;
        Some(
            chosen
                .into_iter()
                .next()
                .context("the selection returned no dossier")?,
        )
    } else {
        None
    };

    match selected {
        Some(dossier) => render_one(&dossier, args.all),
        None => render_all(
            &read_entries(root)?,
            &sources::read_markdown(root, DOSSIERS_DIR)?,
            args.all,
        ),
    }
}

/// Every open source, rendered into the sections it has something for.
fn render_all(entries: &[Entry], dossiers: &[Source], all: bool) -> Result<String> {
    let mut sections: Vec<(Section, Vec<String>)> = Vec::new();

    for &section in sections_of(all) {
        let mut lines = Vec::new();

        // Entries are listed while they still have something open, and never
        // in `FINISHED`: a task ticked off in a day's notes has served its
        // purpose, and the entry is its own record. `--all` reaches dossiers
        // only, which is why the two loops are not the same shape.
        if section != Section::Finished {
            for entry in entries {
                lines.extend(source_lines(&entry.path, &entry.heading, section)?);
            }
        }
        for dossier in dossiers.iter().filter(|dossier| dossier.is_open()) {
            let heading = crate::worklog::stem(dossier.path.as_path());
            lines.extend(source_lines(&dossier.path, &heading, section)?);
        }

        if !lines.is_empty() {
            sections.push((section, lines));
        }
    }

    Ok(assemble(&sections))
}

/// The one dossier a filtered run prints.
fn render_one(path: &Path, all: bool) -> Result<String> {
    let heading = crate::worklog::stem(path);
    let mut sections: Vec<(Section, Vec<String>)> = Vec::new();

    for &section in sections_of(all) {
        let lines = source_lines(path, &heading, section)?;
        if !lines.is_empty() {
            sections.push((section, lines));
        }
    }

    Ok(assemble(&sections))
}

/// The sections a run prints: `ACTIONABLE` and `BLOCKED`, plus `FINISHED` with
/// `--all`.
const fn sections_of(all: bool) -> &'static [Section] {
    if all {
        &[Section::Actionable, Section::Blocked, Section::Finished]
    } else {
        &[Section::Actionable, Section::Blocked]
    }
}

/// The section headings and their trees, with a blank line between sections.
fn assemble(sections: &[(Section, Vec<String>)]) -> String {
    if sections.is_empty() {
        return NOTHING_ACTIONABLE.to_owned();
    }

    let mut blocks: Vec<String> = Vec::new();
    for (section, lines) in sections {
        let mut block = format!("{SECTION_RULE} {} {SECTION_RULE}\n", section.heading());
        for line in lines {
            block.push_str(line);
            block.push('\n');
        }
        blocks.push(block);
    }

    blocks.join("\n")
}

/// One source's lines for one section, under its heading.
fn source_lines(path: &Path, heading: &str, section: Section) -> Result<Vec<String>> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("could not read '{}'", path.display()))?;

    let lines = Tree::parse(&contents).render(section);
    if lines.is_empty() {
        return Ok(lines);
    }

    let mut shown = vec![format!("# {heading}")];
    shown.extend(lines);
    Ok(shown)
}

/// A daily entry: its path, and how it is announced.
struct Entry {
    path: PathBuf,
    heading: String,
}

/// Every daily entry, newest date first.
///
/// The date comes from the file name rather than from the filesystem, because
/// a day is what the entry is about. A file in `entries/` that is not named
/// after a date is not an entry, and is skipped.
fn read_entries(root: &Path) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();

    for source in sources::read_markdown(root, ENTRIES_DIR)? {
        let Some(stem) = source
            .path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_owned)
        else {
            continue;
        };

        if NaiveDate::parse_from_str(&stem, DATE_FORMAT).is_err() {
            continue;
        }

        entries.push(Entry {
            path: source.path,
            heading: format!("{stem} (entry)"),
        });
    }

    // Newest first, which the same format sorts as text.
    entries.sort_by(|left, right| right.heading.cmp(&left.heading));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::commands::tests::Scratch;

    /// A dossier with unfinished, blocked and finished work in it.
    const DOSSIER: &str = "\
# 1234 - refactor auth
- [x] Check current auth docs
- [o] Split auth.rs
  - [x] Data structures
  - [?] API: waiting on John
- [x] Create tests
  - [o] Data structures
    - [ ] Evil path
";

    /// Arguments that list everything.
    fn args(all: bool) -> TasksArgs {
        TasksArgs {
            filter: None,
            menu: false,
            all,
        }
    }

    /// A picker that fails the test if it is ever consulted.
    fn no_picker(_: &[PathBuf]) -> Result<PathBuf> {
        anyhow::bail!("the picker should not have been opened")
    }

    /// One section of a run's output, headed and all.
    fn section(output: &str, heading: &str) -> String {
        let opened = format!("{SECTION_RULE} {heading} {SECTION_RULE}");
        let start = output
            .find(&opened)
            .unwrap_or_else(|| panic!("no {heading} section in:\n{output}"));
        let header_end = start.saturating_add(opened.len());

        // The next rule after this heading's own line ends the section.
        let end = output
            .get(header_end..)
            .and_then(|rest| rest.find(SECTION_RULE))
            .map_or(output.len(), |offset| header_end.saturating_add(offset));

        output
            .get(start..end)
            .unwrap_or_default()
            .trim_end()
            .to_owned()
    }

    /// Give a file a modification time, so recency is not a race.
    fn set_modified(path: &Path, when: SystemTime) {
        let file = fs::File::options()
            .write(true)
            .open(path)
            .unwrap_or_else(|error| panic!("could not open '{}': {error}", path.display()));
        file.set_modified(when).unwrap();
    }

    /// A scratch root holding [`DOSSIER`].
    fn with_dossier(name: &str) -> Scratch {
        let scratch = Scratch::new(name).unwrap();
        scratch
            .write("dossiers/1234 - refactor auth.md", DOSSIER)
            .unwrap();
        scratch
    }

    #[test]
    fn lists_pending_and_blocked_without_all() {
        let scratch = with_dossier("tasks-pending");
        let out = run_in(scratch.path(), &args(false), no_picker).unwrap();

        assert!(out.contains("=== ACTIONABLE ==="), "{out}");
        assert!(out.contains("=== BLOCKED ==="), "{out}");
        assert!(!out.contains("=== FINISHED ==="), "{out}");
        assert!(out.contains("# 1234 - refactor auth\n"), "{out}");
        assert!(out.contains("- [o] Split auth.rs\n"), "{out}");
        assert!(out.contains("  - [?] API: waiting on John\n"), "{out}");
        assert!(
            out.contains("- [x] Create tests\n  - [o] Data structures\n"),
            "{out}"
        );
    }

    #[test]
    fn adds_the_finished_section_with_all() {
        let scratch = with_dossier("tasks-all");
        let out = run_in(scratch.path(), &args(true), no_picker).unwrap();
        let finished = section(&out, "FINISHED");

        assert_eq!(
            finished,
            "=== FINISHED ===\n# 1234 - refactor auth\n- [x] Check current auth docs\n- [o] Split auth.rs\n  - [x] Data structures",
            "{out}"
        );
    }

    #[test]
    fn prints_nothing_pending_when_there_is_nothing_to_show() {
        let scratch = Scratch::new("tasks-empty").unwrap();
        let out = run_in(scratch.path(), &args(false), no_picker).unwrap();

        assert_eq!(out, NOTHING_ACTIONABLE);
    }

    #[test]
    fn a_missing_directory_is_not_an_error() {
        let scratch = Scratch::new("tasks-no-dirs").unwrap();
        assert!(run_in(scratch.path(), &args(true), no_picker).is_ok());
    }

    #[test]
    fn lists_a_sealed_dossier_nowhere() {
        let scratch = Scratch::new("tasks-sealed").unwrap();
        scratch
            .write(
                "dossiers/9999 - sealed.md",
                "---\nsealed: 2026-09-21\n---\n- [ ] Never to be seen\n",
            )
            .unwrap();
        scratch
            .write("dossiers/1234 - open.md", "- [ ] Visible\n")
            .unwrap();

        let out = run_in(scratch.path(), &args(true), no_picker).unwrap();

        assert!(out.contains("- [ ] Visible"), "{out}");
        assert!(!out.contains("Never to be seen"), "{out}");
    }

    #[test]
    fn prints_entries_before_dossiers_with_their_date() {
        let scratch = with_dossier("tasks-entries");
        scratch
            .write("entries/2026-09-19.md", "## Notes\n- [ ] From the entry\n")
            .unwrap();
        scratch
            .write("entries/2026-09-21.md", "## Notes\n- [ ] Later\n")
            .unwrap();
        scratch
            .write("entries/not-a-date.md", "## Notes\n- [ ] Skipped\n")
            .unwrap();

        let out = run_in(scratch.path(), &args(false), no_picker).unwrap();
        let later = out.find("# 2026-09-21 (entry)").unwrap();
        let earlier = out.find("# 2026-09-19 (entry)").unwrap();
        let dossier = out.find("# 1234 - refactor auth").unwrap();

        assert!(later < earlier, "{out}");
        assert!(earlier < dossier, "{out}");
        assert!(!out.contains("Skipped"), "{out}");
        assert!(!out.contains("## Notes"), "{out}");
    }

    #[test]
    fn a_finished_entry_task_is_printed_nowhere() {
        let scratch = with_dossier("tasks-entry-finished");
        scratch
            .write(
                "entries/2026-09-19.md",
                "## Notes\n- [x] Done that day\n- [ ] Still open\n",
            )
            .unwrap();

        let out = run_in(scratch.path(), &args(true), no_picker).unwrap();

        assert!(out.contains("- [ ] Still open"), "{out}");
        assert!(
            !out.contains("Done that day"),
            "a finished entry task is not listed, even with --all:\n{out}"
        );
        // The finished section is the dossier's, and the dossier here has
        // nothing closed in it.
        let finished = section(&out, "FINISHED");
        assert!(!finished.contains("2026-09-19"), "{out}");
    }

    #[test]
    fn a_dossier_with_only_finished_tasks_shows_up_with_all() {
        let scratch = Scratch::new("tasks-closed-only").unwrap();
        scratch
            .write("dossiers/1 - done.md", "- [x] All over\n")
            .unwrap();

        assert_eq!(
            run_in(scratch.path(), &args(false), no_picker).unwrap(),
            NOTHING_ACTIONABLE
        );

        let out = run_in(scratch.path(), &args(true), no_picker).unwrap();
        assert!(out.contains("# 1 - done\n- [x] All over\n"), "{out}");
    }

    #[test]
    fn a_filter_narrows_the_run_to_one_dossier() {
        let scratch = with_dossier("tasks-filter");
        scratch
            .write("dossiers/2 - other.md", "- [ ] Elsewhere\n")
            .unwrap();

        let filtered = TasksArgs {
            filter: Some(Filter::Matching("other".to_owned())),
            menu: false,
            all: false,
        };
        let out = run_in(scratch.path(), &filtered, no_picker).unwrap();

        assert!(out.contains("# 2 - other\n- [ ] Elsewhere\n"), "{out}");
        assert!(!out.contains("refactor auth"), "{out}");
    }

    #[test]
    fn a_filter_that_matches_nothing_is_an_error() {
        let scratch = with_dossier("tasks-no-match");
        let filtered = TasksArgs {
            filter: Some(Filter::Matching("nonsense".to_owned())),
            menu: false,
            all: false,
        };

        let error = run_in(scratch.path(), &filtered, no_picker).unwrap_err();
        assert!(error.to_string().contains("no dossier matches"), "{error}");
    }

    #[test]
    fn a_bare_filter_asks_when_dossiers_share_the_newest_time() {
        let scratch = with_dossier("tasks-tied");
        scratch
            .write("dossiers/5 - other.md", "- [ ] Elsewhere\n")
            .unwrap();
        let when = SystemTime::now() - Duration::from_secs(60);
        set_modified(
            &scratch.path().join("dossiers/1234 - refactor auth.md"),
            when,
        );
        set_modified(&scratch.path().join("dossiers/5 - other.md"), when);

        // A fresh checkout stamps every file with one time, so the tie is the
        // normal case, and the picker is what tells them apart.
        let picked = |candidates: &[PathBuf]| {
            assert_eq!(candidates.len(), 2, "the picker should offer both");
            // Name order breaks the tie, so the lowest name is first.
            candidates
                .last()
                .cloned()
                .context("there was nothing to pick")
        };
        let filtered = TasksArgs {
            filter: Some(Filter::Newest),
            menu: false,
            all: false,
        };
        let out = run_in(scratch.path(), &filtered, picked).unwrap();

        assert!(out.contains("Elsewhere"), "{out}");
        assert!(!out.contains("refactor auth"), "{out}");
    }

    #[test]
    fn a_bare_filter_takes_the_one_newest_dossier() {
        let scratch = with_dossier("tasks-one-newest");
        scratch
            .write("dossiers/5 - other.md", "- [ ] Elsewhere\n")
            .unwrap();
        let now = SystemTime::now();
        set_modified(
            &scratch.path().join("dossiers/1234 - refactor auth.md"),
            now - Duration::from_secs(600),
        );
        set_modified(&scratch.path().join("dossiers/5 - other.md"), now);

        let filtered = TasksArgs {
            filter: Some(Filter::Newest),
            menu: false,
            all: false,
        };
        let out = run_in(scratch.path(), &filtered, no_picker).unwrap();

        assert!(
            out.contains("Elsewhere"),
            "expected the newest dossier:\n{out}"
        );
        assert!(!out.contains("refactor auth"), "{out}");
    }
}
