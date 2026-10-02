//! `bureau report`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDate};

use crate::cli::ReportArgs;
use crate::commands::paths::{DOSSIERS_DIR, ENTRIES_DIR};
use crate::commands::{selection, sources};
use crate::git;
use crate::report::{self, Dossier, Input};
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

    // A report with no diff section needs neither the version nor a HEAD, so
    // neither is asked for.
    let (follow, follow_warning, has_head) = if args.no_diff {
        (true, None, true)
    } else {
        let version = git::version()?;
        let supported = git::follows_renames(&version);
        let warning = (!supported).then(|| {
            format!(
                "git {version} does not support 'git diff --follow' (needs 2.47), \
                 so renames are not followed"
            )
        });
        (supported, warning, git::has_head(&root)?)
    };

    let differ = |request: &DiffRequest| {
        if has_head {
            real_diff(&root, request, follow)
        } else {
            Ok(String::new())
        }
    };

    let (output, mut warnings) = run_in(&root, args, palette, today, differ, selection::pick)?;
    if let Some(warning) = follow_warning {
        warnings.insert(0, warning);
    }

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
    differ: impl Fn(&DiffRequest) -> Result<String>,
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
    for path in &chosen {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("could not read '{}'", path.display()))?;
        dossiers.push(Dossier {
            name: worklog::stem(path),
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
    let days = report::days(&input, palette);
    if !report::has_work(&days) {
        bail!(
            "{}",
            report::nothing_to_report(from, to, selected.is_some(), !input.entries.is_empty())
        );
    }

    let mut diffs = Vec::new();
    let mut warnings = Vec::new();
    if !args.no_diff {
        for name in report::dossier_order(&days) {
            let Some(path) = chosen.iter().find(|path| worklog::stem(path) == name) else {
                continue;
            };

            let request = DiffRequest {
                path: path.clone(),
                from,
                to,
                today,
                color: palette == Palette::ON,
            };

            match differ(&request) {
                Ok(diff) if !diff.trim().is_empty() => diffs.push((name, diff)),
                Ok(_) => {}
                Err(error) => warnings.push(format!("could not diff '{name}': {error:#}")),
            }
        }
    }

    Ok((report::render(from, to, &days, &diffs), warnings))
}

/// The diff of one dossier over the period, as git sees it.
///
/// The base is the newest commit touching the dossier strictly before `from`,
/// or the empty tree when the dossier did not exist yet; the head is `HEAD`, or
/// the newest commit on or before `to` when the period ends in the past.
fn real_diff(root: &Path, request: &DiffRequest, follow: bool) -> Result<String> {
    let empty = git::empty_tree(root)?;
    let base =
        git::last_commit(root, request.from, &request.path)?.unwrap_or_else(|| empty.clone());

    let head = if request.to < request.today {
        let before = request
            .to
            .succ_opt()
            .context("the period ends at the last date this tool can represent")?;
        match git::last_commit(root, before, &request.path)? {
            Some(hash) => hash,
            None => return Ok(String::new()),
        }
    } else {
        String::from("HEAD")
    };

    git::diff(
        root,
        &base,
        &head,
        &request.path,
        follow && base != empty,
        request.color,
    )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::commands::tests::Scratch;

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

    /// A differ that names the dossier it was asked about.
    fn named() -> impl Fn(&DiffRequest) -> Result<String> {
        |request| Ok(format!("diff of {}\n", request.path.display()))
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
            named(),
            no_picker,
        )
        .unwrap();

        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(
            output.contains("Report from 2026-09-01 to 2026-09-30"),
            "{output}"
        );
        assert!(output.contains("## 2026-09-20"), "{output}");
        assert!(output.contains("- [ ] buy shampoo"), "{output}");
        assert!(output.contains("### A\n- did A"), "{output}");
        assert!(output.contains("{{{ A\ndiff of "), "{output}");
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
            named(),
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
        let differ = |_: &DiffRequest| {
            called.set(true);
            Ok(String::new())
        };
        let mut args = args();
        args.no_diff = true;

        let (output, warnings) = run_in(
            scratch.path(),
            &args,
            Palette::OFF,
            today(),
            differ,
            no_picker,
        )
        .unwrap();

        assert!(!called.get(), "the differ was asked for a diff");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(output.contains("### A"), "{output}");
        assert!(!output.contains("# Git diff"), "{output}");
    }

    #[test]
    fn a_failed_diff_is_a_warning_not_a_lost_report() {
        let scratch = with_dossier("report-diff-fails");
        let differ = |_: &DiffRequest| bail!("git exploded");

        let (output, warnings) = run_in(
            scratch.path(),
            &args(),
            Palette::OFF,
            today(),
            differ,
            no_picker,
        )
        .unwrap();

        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings.first().unwrap().contains("git exploded"),
            "{warnings:?}"
        );
        assert!(output.contains("### A"), "{output}");
        assert!(!output.contains("# Git diff"), "{output}");
    }

    /// `run_in` with the test's differ and picker, so a test only spells out
    /// what makes it different.
    fn run_report(scratch: &Scratch, args: &ReportArgs) -> Result<(String, Vec<String>)> {
        run_in(
            scratch.path(),
            args,
            Palette::OFF,
            today(),
            named(),
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
            output.contains("## 2026-09-21\n\n(Entry exists but no work was found)"),
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
        assert!(!output.contains("Entry exists"), "{output}");
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
            named(),
            no_picker,
        )
        .unwrap();

        assert!(output.contains("### B"), "{output}");
        assert!(!output.contains("### A"), "{output}");
        assert!(!output.contains("entry note"), "{output}");
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

        let (output, _) = run_in(
            scratch.path(),
            &args,
            Palette::OFF,
            today(),
            named(),
            picker,
        )
        .unwrap();

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
            named(),
            no_picker,
        )
        .unwrap();

        assert!(
            output.contains("Report from 2026-09-20 to 2026-09-20"),
            "{output}"
        );
    }
}
