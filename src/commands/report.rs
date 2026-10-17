//! `bureau report`.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDate};

use crate::cli::ReportArgs;
use crate::commands::paths::{DOSSIERS_DIR, ENTRIES_DIR};
use crate::commands::{selection, sources};
use crate::git;
use crate::report::{self, Changes, Dossier, Input};
use crate::style::{self, Palette};
use crate::worklog;

/// How a daily entry is named.
const DATE_FORMAT: &str = "%Y-%m-%d";

/// One dossier's diff, as the command asks for it.
///
/// The differ is handed one of these and returns the diff text, which is what
/// lets a test drive the whole report without a git repository.
#[derive(Debug)]
pub struct DiffRequest {
    /// The dossier file to diff.
    pub path: PathBuf,
    /// First day of the period, inclusive.
    pub from: NaiveDate,
    /// Last day of the period, inclusive.
    pub to: NaiveDate,
    /// The day the command is running.
    pub today: NaiveDate,
    /// Whether the report is being printed in colour.
    pub color: bool,
}

/// The diffs for a whole report's worth of dossiers, in the order asked.
///
/// The differ is handed every request at once rather than one at a time,
/// because the expensive part of a report -- walking the history to find each
/// dossier's base commit, and diffing it -- can be done for all of them in a
/// handful of git calls instead of several per dossier. A test hands in a
/// closure with this shape to drive the report without a git repository.
pub type Differ<'a> = dyn Fn(&[DiffRequest]) -> Result<Vec<DiffOutcome>> + 'a;

/// One dossier's diff, or the error that kept the report from reading it.
pub type DiffOutcome = Result<String>;

/// The report's diffs and the warnings they produced, one per dossier and in
/// the order the report prints them.
type DiffSection = (Vec<(String, Changes)>, Vec<String>);

/// Print a report of the work in a period.
///
/// # Errors
///
/// Fails when the repository root cannot be found, when a date cannot be read,
/// when the filter matches nothing, when the picker is cancelled, or when the
/// report would hold no work at all.
pub fn run(args: &ReportArgs) -> Result<()> {
    let root = git::toplevel()?;
    let today = Local::now().date_naive();
    let palette = style::for_stdout();

    // A report with no diff section touches no commit and no tree, so the
    // repository is not read at all: no HEAD check, no empty tree, no walk.
    let has_head = if args.no_diff {
        true
    } else {
        git::has_head(&root)?
    };

    let differ = |requests: &[DiffRequest]| batch_diffs(&root, requests, has_head);

    let (output, warnings) = run_in(&root, args, palette, today, &differ, selection::pick)?;

    let warning_style = style::for_stderr().warning();
    for warning in &warnings {
        eprintln!("{}", warning_style.paint(&format!("warning: {warning}")));
    }

    print!("{output}");
    Ok(())
}

/// The rest of `run`, with the root, the colours, the differ and the picker
/// handed in, so a test can drive it without a git checkout or a terminal.
fn run_in(
    root: &Path,
    args: &ReportArgs,
    palette: Palette,
    today: NaiveDate,
    differ: &Differ<'_>,
    picker: impl Fn(&[PathBuf]) -> Result<PathBuf>,
) -> Result<(String, Vec<String>)> {
    let (from, to) = crate::date::range(
        args.date.as_deref(),
        args.from.as_deref(),
        args.to.as_deref(),
        today,
    )?;

    // The report looks at every dossier, sealed ones included: a seal hides a
    // dossier from action, not from history.
    let all = sources::read_markdown(root, DOSSIERS_DIR)?;
    let selected = if args.filter.is_some() || args.menu {
        let paths: Vec<PathBuf> = all.iter().map(|source| source.path.clone()).collect();
        let request = selection::Request {
            pattern: args.filter.as_deref(),
            menu: args.menu,
        };
        Some(selection::select(&paths, request, picker)?)
    } else {
        None
    };

    let chosen: Vec<PathBuf> = selected.as_ref().map_or_else(
        || all.iter().map(|source| source.path.clone()).collect(),
        |path| vec![path.clone()],
    );

    let mut dossiers = Vec::new();
    let mut scope = None;
    for path in &chosen {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("could not read '{}'", path.display()))?;
        let name = worklog::stem(path);

        // A filtered report says which dossier it is about, by the name the
        // dossier gives itself rather than the one its file had to take.
        if selected.as_ref() == Some(path) {
            scope = Some(report::dossier_title(&contents, &name));
        }

        dossiers.push(Dossier {
            name,
            days: worklog::worklog_days(&contents),
        });
    }

    // A filtered report is about one dossier, so the day's own notes are left
    // out: they belong to the whole day, not to that dossier.
    let mut entries = BTreeMap::new();
    if selected.is_none() {
        for source in sources::read_markdown(root, ENTRIES_DIR)? {
            let Some(date) = source
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| NaiveDate::parse_from_str(stem, DATE_FORMAT).ok())
            else {
                continue;
            };

            if date < from || date > to {
                continue;
            }

            let contents = fs::read_to_string(&source.path)
                .with_context(|| format!("could not read '{}'", source.path.display()))?;
            entries.insert(date, contents);
        }
    }

    let input = Input {
        from,
        to,
        dossiers,
        entries,
    };
    let days = report::days(&input);
    if !report::has_work(&days) {
        bail!(
            "{}",
            report::nothing_to_report(from, to, selected.is_some(), !input.entries.is_empty())
        );
    }

    let (diffs, warnings) = if args.no_diff {
        (Vec::new(), Vec::new())
    } else {
        collect_diffs(&days, &chosen, from, to, today, palette, differ)?
    };

    Ok((
        report::render(from, to, &days, &diffs, scope.as_deref(), palette),
        warnings,
    ))
}

/// Ask the differ about every dossier in the report, in the order they print.
///
/// Every dossier is asked about at once, so the history behind the report is
/// walked once for the period rather than once per dossier.
fn collect_diffs(
    days: &[report::Day],
    chosen: &[PathBuf],
    from: NaiveDate,
    to: NaiveDate,
    today: NaiveDate,
    palette: Palette,
    differ: &Differ<'_>,
) -> Result<DiffSection> {
    let requests: Vec<DiffRequest> = report::dossier_order(days)
        .into_iter()
        .filter_map(|name| {
            let path = chosen
                .iter()
                .find(|path| worklog::stem(path) == name)?
                .clone();

            Some(DiffRequest {
                path,
                from,
                to,
                today,
                color: palette == Palette::ON,
            })
        })
        .collect();

    let mut diffs = Vec::new();
    let mut warnings = Vec::new();

    for (request, result) in requests.iter().zip(differ(&requests)?) {
        let name = worklog::stem(&request.path);

        // A dossier the daily work mentions always gets a block: an empty diff
        // or a git that could not be read is said out loud in the report,
        // because a warning on stderr is lost the moment stdout is redirected
        // to a file.
        match result {
            Ok(diff) if !diff.trim().is_empty() => diffs.push((name, Changes::Diff(diff))),
            Ok(_) => diffs.push((name, Changes::None)),
            Err(error) => {
                warnings.push(format!("could not diff '{name}': {error:#}"));
                diffs.push((name, Changes::Unavailable));
            }
        }
    }

    Ok((diffs, warnings))
}

/// The diffs of a whole report, with as few git calls as the shape allows.
///
/// Two history walks answer every dossier at once: one for the base commit each
/// dossier's diff starts from, and -- only when the period ends in the past,
/// where the head is not simply `HEAD` -- one for the head each one ends at.
/// The diffs themselves are then one `git diff` per distinct (base, head) pair,
/// which in the usual case is a single pair and so a single call.
///
/// A pair whose diff fails marks only its own dossiers unreadable; the rest of
/// the report is still produced.
fn batch_diffs(
    root: &Path,
    requests: &[DiffRequest],
    has_head: bool,
) -> Result<Vec<Result<String>>> {
    if !has_head {
        return Ok(requests.iter().map(|_| Ok(String::new())).collect());
    }

    let Some(first) = requests.first() else {
        return Ok(Vec::new());
    };

    // The empty tree is what a dossier created inside the period is diffed
    // against, and it is computed once for the whole batch rather than once per
    // dossier, which is where a `git mktree` per dossier used to come from.
    let empty = git::empty_tree(root)?;

    // Git names paths relative to the root, whatever the pathspec looked like,
    // so every lookup has to use that form.
    let paths: Vec<&Path> = requests
        .iter()
        .map(|request| git::relative(root, &request.path))
        .collect();
    let bases = git::last_commits(root, first.from, paths.iter().copied())?;

    // A period that ends in the past stops at the newest commit on or before
    // its last day; a period that runs up to today stops at `HEAD`.
    let heads = if first.to < first.today {
        let after = first
            .to
            .succ_opt()
            .context("the period ends at the last date this tool can represent")?;
        Some(git::last_commits(root, after, paths.iter().copied())?)
    } else {
        None
    };

    // The pair each path is compared under, and the paths that share one, so
    // that a single diff call answers every dossier with the same pair. An
    // empty head means the period ended before the dossier existed.
    let mut pairs: HashMap<&Path, (String, String)> = HashMap::new();
    let mut groups: HashMap<(String, String), Vec<&Path>> = HashMap::new();

    for path in &paths {
        let key = path
            .to_str()
            .with_context(|| format!("'{}' is not a UTF-8 path", path.display()))?;

        let base = bases.get(key).cloned().unwrap_or_else(|| empty.clone());

        let head = if let Some(heads) = &heads {
            let Some(head) = heads.get(key) else {
                // The period ended before this dossier existed at all.
                pairs.insert(path, (base, String::new()));
                continue;
            };
            head.clone()
        } else {
            String::from("HEAD")
        };

        groups
            .entry((base.clone(), head.clone()))
            .or_default()
            .push(path);
        pairs.insert(path, (base, head));
    }

    let mut diffs: HashMap<&Path, Result<String>> = HashMap::new();
    for ((base, head), paths) in &groups {
        // A failed call is one answer for every path that shared its pair.
        match git::diffs(root, Some(base), head, paths.iter().copied(), first.color) {
            Ok(all) => {
                for path in paths {
                    // `all` is keyed the way git named the path, which is the
                    // relative path this report collected, not the absolute
                    // one the request carries.
                    let key = path
                        .to_str()
                        .with_context(|| format!("'{}' is not a UTF-8 path", path.display()))?;
                    diffs.insert(path, Ok(all.get(key).cloned().unwrap_or_default()));
                }
            }
            Err(error) => {
                let message = format!("{error:#}");
                for path in paths {
                    diffs.insert(path, Err(anyhow::Error::msg(message.clone())));
                }
            }
        }
    }

    Ok(requests
        .iter()
        .zip(&paths)
        .map(|(_, path)| {
            let path = *path;
            let Some((_, head)) = pairs.get(path) else {
                return Ok(String::new());
            };

            // A period that ended before the dossier existed has no diff.
            if head.is_empty() {
                return Ok(String::new());
            }

            diffs.remove(path).unwrap_or_else(|| Ok(String::new()))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::process::Command;

    use super::*;
    use crate::commands::tests::Scratch;

    /// A date, spelled out so a test reads as the day it means.
    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    /// Run git in `root`, failing the test when it does.
    fn git(root: &Path, args: &[&str]) {
        git_at(root, args, "2026-09-15T12:00:00");
    }

    /// Run git with the committer and author dates pinned, so a test that asks
    /// the history for "before this day" does not depend on the day it runs.
    fn git_at(root: &Path, args: &[&str], when: &str) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "bureau")
            .env("GIT_AUTHOR_EMAIL", "bureau@example.invalid")
            .env("GIT_COMMITTER_NAME", "bureau")
            .env("GIT_COMMITTER_EMAIL", "bureau@example.invalid")
            .env("GIT_AUTHOR_DATE", when)
            .env("GIT_COMMITTER_DATE", when)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// A committed dossier, changed and committed again, plus one that is
    /// touched a third time, so a batch has two commits to group by.
    fn committed(name: &str) -> Scratch {
        let scratch = Scratch::new(name).unwrap();
        let root = scratch.path();
        git(root, &["init", "-q"]);
        scratch
            .write("dossiers/A.md", "## Worklog\n### 2026-09-20\n- did A\n")
            .unwrap();
        scratch
            .write("dossiers/B.md", "## Worklog\n### 2026-09-20\n- did B\n")
            .unwrap();
        git(root, &["add", "-A"]);
        git_at(root, &["commit", "-qm", "base"], "2026-09-10T12:00:00");
        scratch
            .write(
                "dossiers/A.md",
                "## Worklog\n### 2026-09-20\n- did A\n- and again\n",
            )
            .unwrap();
        git_at(
            root,
            &["commit", "-qam", "A moves on"],
            "2026-09-20T12:00:00",
        );
        scratch
            .write(
                "dossiers/B.md",
                "## Worklog\n### 2026-09-20\n- did B\n- and again\n",
            )
            .unwrap();
        git_at(
            root,
            &["commit", "-qam", "B moves on"],
            "2026-09-21T12:00:00",
        );
        scratch
    }

    /// One dossier's diff, as a single-path `git diff` with the report's own
    /// pinned settings produces it.
    fn one_path(root: &Path, base: &str, head: &str, relative: &str) -> String {
        let output = Command::new("git")
            .args(["-c", "diff.algorithm=histogram"])
            .args(["diff", "--no-color", base, head, "--", relative])
            .current_dir(root)
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    #[test]
    fn a_batch_answers_each_dossier_what_git_would_alone() {
        let scratch = committed("report-batch");
        let root = scratch.path();

        // The report's period ends in the past, so the head is a commit rather
        // than `HEAD` and both walks are exercised.
        // Absolute paths, because that is what `sources::read_markdown` hands
        // the report: git answers with paths relative to the root either way,
        // which is the mismatch this test exists to catch.
        let names = ["dossiers/A.md", "dossiers/B.md"];
        let requests: Vec<DiffRequest> = names
            .iter()
            .map(|path| DiffRequest {
                path: root.join(path),
                from: date(2026, 9, 15),
                to: date(2026, 9, 25),
                today: date(2026, 12, 31),
                color: false,
            })
            .collect();

        let diffs = batch_diffs(root, &requests, true).unwrap();
        assert_eq!(diffs.len(), 2);

        for ((name, request), diff) in names.iter().zip(&requests).zip(diffs) {
            let diff = diff.unwrap();

            // The same base and head the batch should have chosen, asked for
            // one dossier at a time.
            let path = git::relative(root, &request.path);
            let base = git::last_commits(root, request.from, [path])
                .unwrap()
                .get(*name)
                .cloned()
                .unwrap();
            let head = git::last_commits(root, date(2026, 9, 26), [path])
                .unwrap()
                .get(*name)
                .cloned()
                .unwrap();

            assert_eq!(diff, one_path(root, &base, &head, name));
            assert!(!diff.is_empty(), "{name} has no diff");
        }
    }

    /// Arguments for one fixed period, so no test depends on today.
    fn args() -> ReportArgs {
        ReportArgs {
            date: None,
            from: Some("2026-09-01".to_owned()),
            to: Some("2026-09-30".to_owned()),
            no_diff: false,
            filter: None,
            menu: false,
        }
    }

    /// The day every test runs on.
    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 30).unwrap()
    }

    /// A picker that fails the test when it is opened.
    fn no_picker(_: &[PathBuf]) -> Result<PathBuf> {
        bail!("the picker should not have been opened")
    }

    /// A differ that names each dossier it was asked about.
    // The differ's shape has a `Result` for the whole batch because the git
    // one can fail; this stand-in cannot, and the signature is the point.
    #[allow(clippy::unnecessary_wraps)]
    fn named(requests: &[DiffRequest]) -> Result<Vec<DiffOutcome>> {
        Ok(requests
            .iter()
            .map(|request| Ok(format!("diff of {}\n", request.path.display())))
            .collect())
    }

    /// A differ that answers every dossier the same way.
    fn always(diff: &'static str) -> impl Fn(&[DiffRequest]) -> Result<Vec<DiffOutcome>> {
        move |requests: &[DiffRequest]| Ok(requests.iter().map(|_| Ok(diff.to_owned())).collect())
    }

    /// A differ whose every dossier fails, with git's own words in the error.
    fn exploding() -> impl Fn(&[DiffRequest]) -> Result<Vec<DiffOutcome>> {
        |requests: &[DiffRequest]| {
            Ok(requests
                .iter()
                .map(|_| Err(anyhow::Error::msg("git exploded")))
                .collect())
        }
    }

    /// A scratch root with one dossier that logged work on the 20th.
    fn with_dossier(name: &str) -> Scratch {
        let scratch = Scratch::new(name).unwrap();
        scratch
            .write("dossiers/A.md", "## Worklog\n### 2026-09-20\n- did A\n")
            .unwrap();
        scratch
    }

    #[test]
    fn reports_the_day_and_the_diff() {
        let scratch = with_dossier("report-happy");
        scratch
            .write(
                "entries/2026-09-20.md",
                "# 2026-09-20\n\n## Notes\n- [ ] buy shampoo\n\n## Worked on Dossiers\n\
                 - [A](<../dossiers/A.md>)\n",
            )
            .unwrap();

        let (output, warnings) = run_in(
            scratch.path(),
            &args(),
            Palette::OFF,
            today(),
            &named,
            no_picker,
        )
        .unwrap();

        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(
            output.contains("# Report from 2026-09-01 to 2026-09-30"),
            "{output}"
        );
        assert!(output.contains("## 2026-09-20 (Sunday)"), "{output}");
        assert!(!output.contains("Dossier:"), "{output}");
        assert!(output.contains("### Notes\n- [ ] buy shampoo"), "{output}");
        assert!(output.contains("### A\n- did A"), "{output}");
        assert!(
            output.contains("{{{ git diff\n```diff\ndiff of "),
            "{output}"
        );
    }

    #[test]
    fn a_sealed_dossier_is_still_reported() {
        let scratch = Scratch::new("report-sealed").unwrap();
        scratch
            .write(
                "dossiers/9 - sealed.md",
                "---\nsealed: 2026-09-21\n---\n## Worklog\n### 2026-09-20\n- sealed work\n",
            )
            .unwrap();

        let (output, _) = run_in(
            scratch.path(),
            &args(),
            Palette::OFF,
            today(),
            &named,
            no_picker,
        )
        .unwrap();

        assert!(output.contains("### 9 - sealed"), "{output}");
        assert!(output.contains("- sealed work"), "{output}");
    }

    #[test]
    fn no_diff_never_asks_for_a_diff() {
        let scratch = with_dossier("report-no-diff");
        let called = Cell::new(false);
        let differ = |requests: &[DiffRequest]| {
            called.set(true);
            Ok(requests.iter().map(|_| Ok(String::new())).collect())
        };
        let mut args = args();
        args.no_diff = true;

        let (output, warnings) = run_in(
            scratch.path(),
            &args,
            Palette::OFF,
            today(),
            &differ,
            no_picker,
        )
        .unwrap();

        assert!(!called.get(), "the differ was asked for a diff");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(output.contains("### A"), "{output}");
        assert!(!output.contains("## Git diff"), "{output}");
    }

    #[test]
    fn a_failed_diff_is_a_warning_and_a_note_in_the_report() {
        let scratch = with_dossier("report-diff-fails");
        let differ = exploding();

        let (output, warnings) = run_in(
            scratch.path(),
            &args(),
            Palette::OFF,
            today(),
            &differ,
            no_picker,
        )
        .unwrap();

        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings.first().unwrap().contains("git exploded"),
            "{warnings:?}"
        );
        assert!(output.contains("### A"), "{output}");
        assert!(
            output.contains("## Git diff\n\n### A\n(Git changes could not be read)\n"),
            "{output}"
        );
        assert!(!output.contains("No git changes were found"), "{output}");
    }

    #[test]
    fn a_dossier_with_no_commits_says_so_and_warns_about_nothing() {
        let scratch = with_dossier("report-no-commits");
        let differ = always("");

        let (output, warnings) = run_in(
            scratch.path(),
            &args(),
            Palette::OFF,
            today(),
            &differ,
            no_picker,
        )
        .unwrap();

        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(
            output.contains("## Git diff\n\n### A\n(No git changes were found)\n"),
            "{output}"
        );
        assert!(!output.contains("{{{ git diff"), "{output}");
    }

    /// `run_in` with the test's differ and picker, so a test only spells out
    /// what makes it different.
    fn run_report(scratch: &Scratch, args: &ReportArgs) -> Result<(String, Vec<String>)> {
        run_in(
            scratch.path(),
            args,
            Palette::OFF,
            today(),
            &named,
            no_picker,
        )
    }

    #[test]
    fn no_entries_in_the_period_is_an_error() {
        let scratch = Scratch::new("report-empty").unwrap();

        let error = run_report(&scratch, &args()).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("No entries found from 2026-09-01 to 2026-09-30"),
            "{error}"
        );
    }

    #[test]
    fn a_single_day_with_no_entry_names_the_day() {
        let scratch = Scratch::new("report-empty-day").unwrap();
        let mut args = args();
        args.from = None;
        args.to = None;
        args.date = Some("2026-09-20".to_owned());

        let error = run_report(&scratch, &args).unwrap_err();

        assert!(
            error.to_string().contains("No entry found for 2026-09-20"),
            "{error}"
        );
        assert!(!error.to_string().contains("period"), "{error}");
    }

    #[test]
    fn a_period_whose_entries_are_all_workless_is_an_error() {
        let scratch = Scratch::new("report-entries-empty").unwrap();
        scratch
            .write("entries/2026-09-20.md", "# 2026-09-20\n\n## Notes\n- \n")
            .unwrap();

        let error = run_report(&scratch, &args()).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("All entries from 2026-09-01 to 2026-09-30 contain no work"),
            "{error}"
        );
    }

    #[test]
    fn a_single_workless_entry_names_the_entry() {
        let scratch = Scratch::new("report-entry-empty").unwrap();
        scratch
            .write("entries/2026-09-20.md", "# 2026-09-20\n\n## Notes\n- \n")
            .unwrap();
        let mut args = args();
        args.from = None;
        args.to = None;
        args.date = Some("2026-09-20".to_owned());

        let error = run_report(&scratch, &args).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Entry found for 2026-09-20 but no work found"),
            "{error}"
        );
    }

    #[test]
    fn a_one_day_range_uses_the_single_day_wording() {
        let scratch = Scratch::new("report-one-day-empty").unwrap();
        let mut args = args();
        args.from = Some("2026-09-20".to_owned());
        args.to = Some("2026-09-20".to_owned());

        let error = run_report(&scratch, &args).unwrap_err();

        assert!(
            error.to_string().contains("No entry found for 2026-09-20"),
            "{error}"
        );
        assert!(!error.to_string().contains("period"), "{error}");
    }

    #[test]
    fn a_workless_entry_is_printed_beside_a_day_that_has_work() {
        let scratch = with_dossier("report-mixed");
        scratch
            .write("entries/2026-09-21.md", "# 2026-09-21\n\n## Notes\n- \n")
            .unwrap();

        let (output, warnings) = run_report(&scratch, &args()).unwrap();

        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(output.contains("### A\n- did A"), "{output}");
        assert!(
            output.contains("## 2026-09-21 (Monday)\n(No work found)"),
            "{output}"
        );
    }

    #[test]
    fn a_worklog_day_without_an_entry_is_a_report() {
        let scratch = with_dossier("report-no-entry");
        let mut args = args();
        args.from = Some("2026-09-20".to_owned());
        args.to = Some("2026-09-20".to_owned());

        let (output, _) = run_report(&scratch, &args).unwrap();

        assert!(output.contains("### A"), "{output}");
        assert!(!output.contains("No work found"), "{output}");
    }

    #[test]
    fn a_filtered_report_keeps_the_no_work_wording() {
        let scratch = Scratch::new("report-filter-empty").unwrap();
        scratch
            .write("dossiers/A.md", "# A\n\n## Worklog\n")
            .unwrap();
        let mut args = args();
        args.filter = Some("A".to_owned());

        let error = run_report(&scratch, &args).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("no work in the period 2026-09-01 to 2026-09-30"),
            "{error}"
        );
    }

    #[test]
    fn a_filter_reports_one_dossier_without_the_entry() {
        let scratch = with_dossier("report-filter");
        scratch
            .write("dossiers/B.md", "## Worklog\n### 2026-09-20\n- did B\n")
            .unwrap();
        scratch
            .write(
                "entries/2026-09-20.md",
                "# 2026-09-20\n\n## Notes\n- [ ] entry note\n\n## Worked on Dossiers\n\
                 - [A](<../dossiers/A.md>)\n",
            )
            .unwrap();

        let mut args = args();
        args.filter = Some("B".to_owned());

        let (output, _) = run_in(
            scratch.path(),
            &args,
            Palette::OFF,
            today(),
            &named,
            no_picker,
        )
        .unwrap();

        assert!(output.contains("### B"), "{output}");
        assert!(!output.contains("### A"), "{output}");
        assert!(!output.contains("entry note"), "{output}");
        assert!(output.contains("\nDossier: B\n"), "{output}");
    }

    #[test]
    fn a_filtered_report_names_the_dossier_as_it_names_itself() {
        let scratch = Scratch::new("report-scope").unwrap();
        scratch
            .write(
                "dossiers/a_b.md",
                "# a::b\n\n## Worklog\n### 2026-09-20\n- did the thing\n",
            )
            .unwrap();

        let mut args = args();
        args.filter = Some("a_b".to_owned());

        let (output, _) = run_report(&scratch, &args).unwrap();

        assert!(
            output.starts_with("# Report from 2026-09-01 to 2026-09-30\n\nDossier: a::b\n"),
            "{output:?}"
        );
        assert!(output.contains("### a_b"), "{output}");
    }

    #[test]
    fn a_menu_opens_the_picker_over_every_dossier() {
        let scratch = with_dossier("report-menu");
        scratch
            .write("dossiers/B.md", "## Worklog\n### 2026-09-20\n- did B\n")
            .unwrap();
        let chosen_path = scratch.path().join("dossiers/B.md");
        let picker = move |_: &[PathBuf]| Ok(chosen_path.clone());
        let mut args = args();
        args.menu = true;

        let (output, _) =
            run_in(scratch.path(), &args, Palette::OFF, today(), &named, picker).unwrap();

        assert!(output.contains("### B"), "{output}");
        assert!(!output.contains("### A"), "{output}");
    }

    #[test]
    fn a_single_date_argument_is_a_one_day_period() {
        let scratch = with_dossier("report-one-day");
        let mut args = args();
        args.from = None;
        args.to = None;
        args.date = Some("2026-09-20".to_owned());

        let (output, _) = run_in(
            scratch.path(),
            &args,
            Palette::OFF,
            today(),
            &named,
            no_picker,
        )
        .unwrap();

        assert!(output.contains("# Report for 2026-09-20"), "{output}");
        assert!(!output.contains(" to "), "{output}");
    }
}
