//! `bureau worklog`.

use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use chrono::{Local, NaiveDate};
use dialoguer::Input;

use crate::Result;
use crate::cli::WorklogArgs;
use crate::commands::files::Files;
use crate::commands::paths::{DOSSIERS_DIR, ENTRIES_DIR};
use crate::commands::{selection, sources};
use crate::git;
use crate::prompt::{self, PrefixTheme};
use crate::style::{self, Palette};
use crate::template;
use crate::worklog;

/// How `worklog` writes: the colours for its own output, and for warnings.
#[derive(Clone, Copy)]
struct Colours {
    palette: Palette,
    warning_palette: Palette,
}

/// Log one line of work against a dossier, and link it from that day's entry.
///
/// # Errors
///
/// Fails when the repository root cannot be found, when `--date` cannot be
/// read as a date, when no dossier matches, when the picker is cancelled, when
/// the chosen dossier has no `## Worklog` section, or when the entry's own
/// commit cannot be made. A failure after the worklog is on disk is a warning
/// rather than an error, because the line the user typed is already saved.
pub fn run(args: &WorklogArgs) -> Result<()> {
    let root = git::toplevel()?;
    let colours = Colours {
        palette: style::for_stdout(),
        warning_palette: style::for_stderr(),
    };
    let mut out = io::stdout();
    let mut err = io::stderr();

    run_in(
        &root,
        args,
        &colours,
        prompt_for_message,
        &mut out,
        &mut err,
    )
}

/// The rest of `run`, with the root, the colours, the prompt and the two
/// streams handed in, so a test can drive it without a terminal or an
/// environment. The repository is still a real one.
fn run_in(
    root: &Path,
    args: &WorklogArgs,
    colours: &Colours,
    prompt: impl FnOnce() -> Result<String>,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    let date = target_date(args.date.as_deref())?;
    let mut files = Files::new();
    let dossiers = sources::read_dossiers(&mut files, root)?;
    let dossier = select(&dossiers, args)?;
    let name = worklog::stem(&dossier);
    let chosen_shown = sources::relative(&dossier, root).display().to_string();

    // Everything that can fail is done before the prompt, so a dossier that
    // cannot take a worklog never costs the user the line they typed.
    let contents = files.read(&dossier)?;
    if !worklog::has_worklog(contents) {
        bail!("'{chosen_shown}' has no '## Worklog' section, so there is nowhere to log this");
    }

    let entry_path = root.join(ENTRIES_DIR).join(format!("{date}.md"));
    let entry_exists = entry_path.exists();
    let entry = entry_contents(&entry_path, date)?;
    let entry_shown_text = sources::relative(&entry_path, root).display().to_string();

    // Preview what is already on disk before asking for the line. The new line
    // is not in this diff: it does not exist yet.
    preview_dossier(root, &dossier, &chosen_shown, colours, out, err)?;

    writeln!(
        out,
        "Logging to {} on {}{}",
        colours.palette.quoted(&name),
        colours.palette.quoted(&date.to_string()),
        if entry_exists { "" } else { " (new entry)" }
    )?;
    out.flush()?;

    let message = prompt()?;

    // The run's summary starts from the commit the entry commits are about to
    // build on, before any of this run's commits exist.
    let start = git::head(root)?;
    let mut committed = commit_entry(
        root,
        &entry_path,
        entry_exists,
        date,
        &entry_shown_text,
        colours,
        out,
    )?;

    let filename = sources::file_name(&dossier);
    let dossier_link = format!("../{DOSSIERS_DIR}/{filename}");
    let linked = worklog::add_link(&entry, &name, &dossier_link);
    let entry_link = format!("../{ENTRIES_DIR}/{date}.md");
    let updated = worklog::append_message(contents, date, &message, &entry_link)
        .with_context(|| format!("'{chosen_shown}' has no '## Worklog' section"))?;

    if let Some(linked) = &linked {
        fs::write(&entry_path, linked)
            .with_context(|| format!("could not write '{entry_shown_text}'"))?;
    }

    fs::write(&dossier, updated).with_context(|| format!("could not write '{chosen_shown}'"))?;

    committed |= commit_worklog(
        root,
        &dossier,
        &entry_path,
        linked.is_some(),
        colours,
        out,
        err,
    )?;

    if committed {
        print_summary(
            root,
            start.as_deref(),
            &[dossier.as_path(), entry_path.as_path()],
            colours,
            out,
            err,
        )?;
    }

    Ok(())
}

/// Show the dossier's uncommitted changes, or say that there are none.
fn preview_dossier(
    root: &Path,
    dossier: &Path,
    chosen_shown: &str,
    colours: &Colours,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    if git::has_changes(root, dossier)? {
        let diff = git::diff_worktree(root, dossier, colours.palette == Palette::ON)?;
        writeln!(out, "{}", colours.palette.scaffold().paint("{{{ git diff"))?;
        write!(out, "{diff}")?;
        if !diff.ends_with('\n') {
            writeln!(out)?;
        }
        writeln!(out, "{}", colours.palette.scaffold().paint("}}}"))?;
        writeln!(out)?;
    } else {
        writeln!(
            err,
            "{}",
            colours.warning_palette.warning_line(format_args!(
                "there are no uncommitted changes to '{chosen_shown}'"
            ))
        )?;
    }

    Ok(())
}

/// Commit the entry's pre-existing changes, or create and commit it when it is
/// missing, and report which of those happened.
///
/// A missing entry is committed as the template alone; the link to the dossier
/// is added afterwards, in the combined commit.
fn commit_entry(
    root: &Path,
    entry_path: &Path,
    entry_exists: bool,
    date: NaiveDate,
    entry_shown: &str,
    colours: &Colours,
    out: &mut impl Write,
) -> Result<bool> {
    if entry_exists && git::is_dirty(root, entry_path)? {
        git::commit(
            root,
            &[entry_path],
            &git::commit_title("update entry", entry_path),
        )
        .with_context(|| format!("could not commit the existing changes to '{entry_shown}'"))?;
        writeln!(
            out,
            "{} {}",
            colours.palette.heading().paint("Updated entry"),
            colours.palette.quoted(entry_shown)
        )?;

        return Ok(true);
    }

    if entry_exists {
        return Ok(false);
    }

    let entries_dir = root.join(ENTRIES_DIR);
    fs::create_dir_all(&entries_dir)
        .with_context(|| format!("could not create '{}'", entries_dir.display()))?;
    fs::write(entry_path, template::daily_entry(&date))
        .with_context(|| format!("could not write '{entry_shown}'"))?;
    git::commit(
        root,
        &[entry_path],
        &git::commit_title("new entry", entry_path),
    )
    .with_context(|| format!("could not commit the new entry '{entry_shown}'"))?;
    writeln!(
        out,
        "{} {}",
        colours.palette.heading().paint("Created entry"),
        colours.palette.quoted(entry_shown)
    )?;

    Ok(true)
}

/// Commit the dossier's new line and the entry's link together.
///
/// Returns whether the combined commit was made. A failed commit is a warning,
/// not an error: both files are already on disk, and the run's earlier commits
/// still stand.
fn commit_worklog(
    root: &Path,
    dossier: &Path,
    entry_path: &Path,
    linked: bool,
    colours: &Colours,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<bool> {
    let chosen_shown = sources::relative(dossier, root).display().to_string();
    let entry_shown = sources::relative(entry_path, root).display().to_string();

    match git::commit(
        root,
        &[dossier, entry_path],
        &git::commit_title("worklog", dossier),
    ) {
        Ok(()) => {
            writeln!(
                out,
                "{} {}",
                colours.palette.heading().paint("Logged to"),
                colours.palette.quoted(&chosen_shown)
            )?;
            if linked {
                writeln!(
                    out,
                    "{} {}",
                    colours.palette.heading().paint("Linked entry"),
                    colours.palette.quoted(&entry_shown)
                )?;
            } else {
                writeln!(
                    out,
                    "{} {} already links to it",
                    colours.palette.heading().paint("Entry"),
                    colours.palette.quoted(&entry_shown)
                )?;
            }

            Ok(true)
        }
        Err(error) => {
            out.flush()?;
            writeln!(
                err,
                "{}",
                colours
                    .warning_palette
                    .warning_line(format_args!("{error:#}"))
            )?;
            writeln!(
                err,
                "{}",
                colours.warning_palette.warning_line(format_args!(
                    "the worklog was written to '{chosen_shown}' but is not committed"
                ))
            )?;

            Ok(false)
        }
    }
}

/// Print the compact summary of every commit this run made, as the last thing
/// the command writes.
///
/// The base is the commit the run started from, or the empty tree when the run
/// made the repository's first commit. A failure to summarize is a warning:
/// the commits themselves already succeeded.
fn print_summary(
    root: &Path,
    start: Option<&str>,
    paths: &[&Path],
    colours: &Colours,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    let base = match start {
        Some(head) => head.to_owned(),
        None => match git::empty_tree(root) {
            Ok(tree) => tree,
            Err(error) => {
                out.flush()?;
                writeln!(
                    err,
                    "{}",
                    colours
                        .warning_palette
                        .warning_line(format_args!("could not summarize the commits: {error:#}"))
                )?;
                return Ok(());
            }
        },
    };

    match git::summary(
        root,
        &base,
        paths.iter().copied(),
        colours.palette == Palette::ON,
    ) {
        Ok(summary) if !summary.trim().is_empty() => {
            writeln!(out)?;
            writeln!(out, "{}", colours.palette.heading().paint("Summary"))?;
            write!(out, "{summary}")?;
            if !summary.ends_with('\n') {
                writeln!(out)?;
            }
            out.flush()?;
        }
        Ok(_) => {}
        Err(error) => {
            out.flush()?;
            writeln!(
                err,
                "{}",
                colours
                    .warning_palette
                    .warning_line(format_args!("could not summarize the commits: {error:#}"))
            )?;
        }
    }

    Ok(())
}

/// The date to log against: `--date` when given, today otherwise.
///
/// The parsing is [`crate::date::parse`], so `--date` accepts the same plain
/// English as `bureau report`, e.g. `--date "a week ago"`.
fn target_date(date: Option<&str>) -> Result<NaiveDate> {
    let today = Local::now().date_naive();
    date.map_or(Ok(today), |text| crate::date::parse(text, today))
}

/// Decide which dossier to log against.
///
/// A filter that matches several dossiers asks which one; a filter that
/// matches one is used as it is. Without a filter, `--menu` offers every
/// dossier, and otherwise the most recently modified one wins -- unless
/// several share that timestamp, in which case they are offered as a choice.
fn select(dossiers: &[PathBuf], args: &WorklogArgs) -> Result<PathBuf> {
    let request = selection::Request {
        pattern: args.filter.as_deref(),
        menu: args.menu,
    };

    selection::select(dossiers, request, selection::pick)
}

/// Read the single line to log, or nothing when there is none.
///
/// On a terminal this is a line editor, so the arrow keys move the cursor
/// instead of being typed as escape sequences. When standard input or standard
/// error is not a terminal, there is no editor to draw, so the plain line read
/// is used and a piped message still works.
fn prompt_for_message() -> Result<String> {
    let palette = style::for_stderr();

    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        let theme = PrefixTheme::new(prompt::prompt(palette, "Message", "(Ctrl-C to cancel)"));
        let message = Input::<String>::with_theme(&theme)
            .with_prompt("")
            .interact_text()
            .context("could not read the message")?;

        return checked(message);
    }

    eprint!(
        "{}",
        prompt::prompt(palette, "Message", "(Ctrl-C to cancel)")
    );
    io::stderr().flush()?;

    let mut message = String::new();
    if io::stdin().read_line(&mut message)? == 0 {
        bail!("no message given, so nothing was logged");
    }

    checked(message.trim_end_matches(['\r', '\n']).to_owned())
}

/// Refuse an empty message, which is the one thing there is no worklog for.
fn checked(message: String) -> Result<String> {
    if message.trim().is_empty() {
        bail!("no message given, so nothing was logged");
    }

    Ok(message)
}

/// The daily entry for `date`: read from disk, or fresh from the template.
fn entry_contents(path: &Path, date: NaiveDate) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(template::daily_entry(&date)),
        Err(error) => Err(error).with_context(|| format!("could not read '{}'", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;

    use super::*;
    use crate::commands::tests::Scratch;

    /// The dossier every test logs against.
    const DOSSIER: &str = "dossiers/0007 - legacy.md";

    /// The entry every test logs to.
    const ENTRY: &str = "entries/2026-03-23.md";

    /// Arguments that select the one scratch dossier, so no picker is needed.
    fn args(date: &str) -> WorklogArgs {
        WorklogArgs {
            filter: Some("legacy".to_owned()),
            date: Some(date.to_owned()),
            menu: false,
        }
    }

    /// A repository with the dossier committed and nothing else.
    fn repository(name: &str, dossier: &str) -> Scratch {
        let scratch = Scratch::new(name).unwrap();
        scratch.init_git();
        scratch.write(DOSSIER, dossier).unwrap();
        scratch.git(&["add", "-A"]);
        scratch.git(&["commit", "-qm", "base"]);
        scratch
    }

    /// No colour, for a captured stream in a test.
    fn plain() -> Colours {
        Colours {
            palette: Palette::OFF,
            warning_palette: Palette::OFF,
        }
    }

    /// Run the command with the plain palette and capture both streams.
    fn run_in_scratch(
        scratch: &Scratch,
        args: &WorklogArgs,
        prompt: impl FnOnce() -> Result<String>,
    ) -> Result<(String, String)> {
        let mut out = Vec::new();
        let mut err = Vec::new();

        run_in(scratch.path(), args, &plain(), prompt, &mut out, &mut err)?;

        Ok((
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        ))
    }

    /// The commit titles, newest first.
    fn titles(scratch: &Scratch) -> Vec<String> {
        scratch
            .git_output(&["log", "--format=%s"])
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// The contents of `path` at `revision`.
    fn file_at(scratch: &Scratch, revision: &str, path: &str) -> String {
        let spec = format!("{revision}:{path}");
        scratch.git_output(&["show", &spec])
    }

    #[test]
    fn refuses_a_dossier_without_a_worklog_before_prompting() {
        let scratch = Scratch::new("no-worklog").unwrap();
        scratch
            .write(DOSSIER, "# Legacy\n## Notes\n- nothing\n")
            .unwrap();

        let prompted = Cell::new(false);
        let prompt = || {
            prompted.set(true);
            Ok("Did some work".to_owned())
        };
        let mut out = Vec::new();
        let mut err = Vec::new();

        let error = run_in(
            scratch.path(),
            &args("2026-03-23"),
            &plain(),
            prompt,
            &mut out,
            &mut err,
        )
        .unwrap_err();

        assert!(
            !prompted.get(),
            "the message was prompted for before the dossier was checked"
        );
        assert!(error.to_string().contains("no '## Worklog' section"));
        assert_eq!(
            scratch.read(DOSSIER).unwrap(),
            "# Legacy\n## Notes\n- nothing\n"
        );
    }

    #[test]
    fn leaves_the_dossier_alone_when_no_entry_can_be_used() {
        let scratch = Scratch::new("entry-in-the-way").unwrap();
        scratch.write(DOSSIER, "# Legacy\n## Worklog\n").unwrap();
        // A file where the entries directory belongs, so no entry can be read
        // or written.
        scratch.write("entries", "in the way\n").unwrap();

        let mut out = Vec::new();
        let mut err = Vec::new();
        let result = run_in(
            scratch.path(),
            &args("2026-03-23"),
            &plain(),
            || Ok("Did some work".to_owned()),
            &mut out,
            &mut err,
        );

        assert!(result.is_err());
        assert_eq!(
            scratch.read(DOSSIER).unwrap(),
            "# Legacy\n## Worklog\n",
            "the dossier was written before the entry was dealt with"
        );
    }

    #[test]
    fn logs_a_line_and_links_the_entry() {
        let scratch = repository("happy-path", "# Legacy\n## Worklog\n");

        let (out, err) = run_in_scratch(&scratch, &args("2026-03-23"), || {
            Ok("Did some work".to_owned())
        })
        .unwrap();

        assert_eq!(
            scratch.read(DOSSIER).unwrap(),
            "# Legacy\n## Worklog\n\n### [2026-03-23](<../entries/2026-03-23.md>)\n- Did some work\n"
        );
        assert_eq!(
            scratch.read(ENTRY).unwrap(),
            "# 2026-03-23\n\n## Notes\n- \n\n## Worked on Dossiers\n\
             - [0007 - legacy](<../dossiers/0007 - legacy.md>)\n"
        );

        assert!(
            out.contains("Logging to '0007 - legacy' on '2026-03-23' (new entry)"),
            "{out}"
        );
        assert!(
            out.contains("Created entry 'entries/2026-03-23.md'"),
            "{out}"
        );
        assert!(
            out.contains("Logged to 'dossiers/0007 - legacy.md'"),
            "{out}"
        );
        assert!(
            out.contains("Linked entry 'entries/2026-03-23.md'"),
            "{out}"
        );
        assert!(
            err.contains(
                "Warning: there are no uncommitted changes to 'dossiers/0007 - legacy.md'"
            ),
            "{err}"
        );
        assert!(
            out.contains("entries/2026-03-23.md (new)"),
            "the new entry is not in the summary: {out}"
        );
        assert!(
            out.contains("\n\nSummary\n"),
            "the Summary label is missing: {out}"
        );
        assert!(
            out.find("Summary") < out.find("changed,"),
            "the Summary label is after the stats: {out}"
        );
        assert!(
            out.trim_end()
                .lines()
                .last()
                .is_some_and(|line| line.contains("changed,")),
            "the summary is not last: {out}"
        );

        assert_eq!(
            titles(&scratch),
            vec![
                "worklog: 0007 - legacy".to_owned(),
                "new entry: 2026-03-23".to_owned(),
                "base".to_owned(),
            ]
        );

        let new_entry = file_at(&scratch, "HEAD~1", ENTRY);
        assert!(
            !new_entry.contains("[0007 - legacy]"),
            "the new entry commit already carried the link: {new_entry}"
        );
    }

    #[test]
    fn running_twice_logs_the_line_twice_but_links_the_entry_once() {
        let scratch = repository("twice", "# Legacy\n## Worklog\n");

        run_in_scratch(&scratch, &args("2026-03-23"), || Ok("First".to_owned())).unwrap();
        let (out, err) =
            run_in_scratch(&scratch, &args("2026-03-23"), || Ok("Second".to_owned())).unwrap();

        assert_eq!(
            scratch.read(DOSSIER).unwrap(),
            "# Legacy\n## Worklog\n\n### [2026-03-23](<../entries/2026-03-23.md>)\n- First\n- Second\n"
        );
        assert_eq!(
            scratch
                .read(ENTRY)
                .unwrap()
                .matches("- [0007 - legacy]")
                .count(),
            1
        );
        assert!(
            out.contains("Logged to 'dossiers/0007 - legacy.md'"),
            "{out}"
        );
        assert!(
            out.contains("Entry 'entries/2026-03-23.md' already links to it"),
            "{out}"
        );
        assert!(!out.contains("(new entry)"), "{out}");
        assert!(
            err.contains(
                "Warning: there are no uncommitted changes to 'dossiers/0007 - legacy.md'"
            ),
            "{err}"
        );
        assert_eq!(
            titles(&scratch)
                .iter()
                .filter(|title| title.starts_with("worklog: "))
                .count(),
            2
        );
    }

    #[test]
    fn a_failed_dossier_write_still_leaves_the_entry_linked() {
        let scratch = repository("dossier-write-fails", "# Legacy\n## Worklog\n");

        // The prompt runs after both files have been read and before either is
        // written, so this is the one moment a test can break the dossier write
        // without depending on file permissions.
        let dossier_path = scratch.path().join(DOSSIER);
        let in_the_way = dossier_path.clone();
        let prompt = move || {
            fs::remove_file(&in_the_way)?;
            fs::create_dir(&in_the_way)?;
            Ok("Did some work".to_owned())
        };

        let mut out = Vec::new();
        let mut err = Vec::new();
        assert!(
            run_in(
                scratch.path(),
                &args("2026-03-23"),
                &plain(),
                prompt,
                &mut out,
                &mut err,
            )
            .is_err()
        );
        assert!(
            scratch.read(ENTRY).unwrap().contains("- [0007 - legacy]"),
            "the entry should have been written before the dossier"
        );
        assert!(
            titles(&scratch)
                .iter()
                .any(|title| title == "new entry: 2026-03-23"),
            "the new entry should have been committed"
        );

        // Put the dossier back and log the same line again: the entry is
        // already linked, so the bullet is added exactly once.
        fs::remove_dir(&dossier_path).unwrap();
        fs::write(&dossier_path, "# Legacy\n## Worklog\n").unwrap();
        let (out, _err) = run_in_scratch(&scratch, &args("2026-03-23"), || {
            Ok("Did some work".to_owned())
        })
        .unwrap();

        assert_eq!(
            scratch.read(DOSSIER).unwrap(),
            "# Legacy\n## Worklog\n\n### [2026-03-23](<../entries/2026-03-23.md>)\n- Did some work\n"
        );
        assert_eq!(
            scratch
                .read(ENTRY)
                .unwrap()
                .matches("- [0007 - legacy]")
                .count(),
            1
        );
        assert!(
            out.contains("Updated entry 'entries/2026-03-23.md'"),
            "{out}"
        );
        assert!(
            out.contains("Entry 'entries/2026-03-23.md' already links to it"),
            "{out}"
        );
    }

    #[test]
    fn commits_pre_existing_entry_changes_before_the_worklog() {
        let scratch = Scratch::new("entry-changes").unwrap();
        scratch.init_git();
        scratch.write(DOSSIER, "# Legacy\n## Worklog\n").unwrap();
        scratch
            .write(
                ENTRY,
                "# 2026-03-23\n\n## Notes\n- \n\n## Worked on Dossiers\n",
            )
            .unwrap();
        scratch.git(&["add", "-A"]);
        scratch.git(&["commit", "-qm", "base"]);
        scratch
            .write(
                ENTRY,
                "# 2026-03-23\n\n## Notes\n- manual\n\n## Worked on Dossiers\n",
            )
            .unwrap();

        let (out, err) = run_in_scratch(&scratch, &args("2026-03-23"), || {
            Ok("Did some work".to_owned())
        })
        .unwrap();

        assert!(
            out.contains("Updated entry 'entries/2026-03-23.md'"),
            "{out}"
        );
        assert!(
            out.contains("Logged to 'dossiers/0007 - legacy.md'"),
            "{out}"
        );
        assert!(
            out.contains("Linked entry 'entries/2026-03-23.md'"),
            "{out}"
        );
        assert!(
            err.contains(
                "Warning: there are no uncommitted changes to 'dossiers/0007 - legacy.md'"
            ),
            "{err}"
        );
        assert_eq!(
            titles(&scratch),
            vec![
                "worklog: 0007 - legacy".to_owned(),
                "update entry: 2026-03-23".to_owned(),
                "base".to_owned(),
            ]
        );

        let updated = file_at(&scratch, "HEAD~1", ENTRY);
        assert!(updated.contains("- manual"), "{updated}");
        assert!(
            !updated.contains("[0007 - legacy]"),
            "the update entry commit already carried the link: {updated}"
        );
        assert_eq!(
            scratch.read(ENTRY).unwrap(),
            "# 2026-03-23\n\n## Notes\n- manual\n\n## Worked on Dossiers\n\
             - [0007 - legacy](<../dossiers/0007 - legacy.md>)\n"
        );
        assert!(
            out.trim_end()
                .lines()
                .last()
                .is_some_and(|line| line.contains("changed,")),
            "the summary is not last: {out}"
        );
    }

    #[test]
    fn previews_the_dossiers_uncommitted_changes_before_the_prompt() {
        let scratch = repository("preview", "# Legacy\n## Worklog\n");
        scratch
            .write(DOSSIER, "# Legacy\n## Worklog\n- manual\n")
            .unwrap();

        let (out, err) = run_in_scratch(&scratch, &args("2026-03-23"), || {
            Ok("Did some work".to_owned())
        })
        .unwrap();

        let open = out.find("{{{ git diff").unwrap();
        let diff = out.find("diff --git a/dossiers/0007 - legacy.md").unwrap();
        let close = out.find("}}}").unwrap();
        let logging = out.find("Logging to").unwrap();
        assert!(open < diff, "the preview is not inside its frame: {out}");
        assert!(diff < close, "the preview is not inside its frame: {out}");
        assert!(close < logging, "the frame does not end the preview: {out}");
        assert!(out.contains("+- manual"), "{out}");
        assert!(err.is_empty(), "{err}");
        assert_eq!(
            scratch.read(DOSSIER).unwrap(),
            "# Legacy\n## Worklog\n- manual\n\n### [2026-03-23](<../entries/2026-03-23.md>)\n- Did some work\n"
        );
    }

    #[test]
    fn warns_when_the_dossier_has_no_uncommitted_changes() {
        let scratch = repository("warning", "# Legacy\n## Worklog\n");

        let (out, err) = run_in_scratch(&scratch, &args("2026-03-23"), || {
            Ok("Did some work".to_owned())
        })
        .unwrap();

        assert!(
            err.contains(
                "Warning: there are no uncommitted changes to 'dossiers/0007 - legacy.md'"
            ),
            "{err}"
        );
        assert!(!out.contains("diff --git"), "{out}");
        assert!(!out.contains("{{{ git diff"), "{out}");
        assert!(out.contains("Logging to"), "{out}");
    }

    #[test]
    fn summarizes_the_first_commits_in_a_fresh_repository() {
        let scratch = Scratch::new("fresh-repo").unwrap();
        scratch.init_git();
        scratch.write(DOSSIER, "# Legacy\n## Worklog\n").unwrap();

        let (out, err) = run_in_scratch(&scratch, &args("2026-03-23"), || {
            Ok("Did some work".to_owned())
        })
        .unwrap();

        assert!(
            err.contains(
                "Warning: there are no uncommitted changes to 'dossiers/0007 - legacy.md'"
            ),
            "{err}"
        );
        assert!(out.contains("dossiers/0007 - legacy.md (new)"), "{out}");
        assert!(out.contains("entries/2026-03-23.md (new)"), "{out}");
        assert!(
            out.trim_end()
                .lines()
                .last()
                .is_some_and(|line| line.contains("changed,")),
            "the summary is not last: {out}"
        );
        assert_eq!(
            titles(&scratch),
            vec![
                "worklog: 0007 - legacy".to_owned(),
                "new entry: 2026-03-23".to_owned(),
            ]
        );
    }

    /// Make the combined `worklog` commit fail while the entry commit still
    /// succeeds, so the partial-summary path can be exercised.
    #[cfg(unix)]
    fn fail_dossier_commits(scratch: &Scratch) {
        use std::os::unix::fs::PermissionsExt;

        let hook = "#!/bin/sh\n\
                    if git diff --cached --name-only | grep -q '^dossiers/'; then\n\
                    \texit 1\n\
                    fi\n";
        scratch.write(".git/hooks/pre-commit", hook).unwrap();
        let hook_path = scratch.path().join(".git/hooks/pre-commit");
        let mut permissions = fs::metadata(&hook_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&hook_path, permissions).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_successful_entry_commit_is_summarized_when_the_worklog_commit_fails() {
        let scratch = repository("partial-commit", "# Legacy\n## Worklog\n");
        fail_dossier_commits(&scratch);

        let (out, err) = run_in_scratch(&scratch, &args("2026-03-23"), || {
            Ok("Did some work".to_owned())
        })
        .unwrap();

        assert!(
            out.contains("Created entry 'entries/2026-03-23.md'"),
            "{out}"
        );
        assert!(!out.contains("Logged to"), "{out}");
        assert!(out.contains("entries/2026-03-23.md (new)"), "{out}");
        assert!(
            out.trim_end()
                .lines()
                .last()
                .is_some_and(|line| line.contains("changed,")),
            "the summary is not last: {out}"
        );
        assert!(err.contains("not committed"), "{err}");
        assert_eq!(
            titles(&scratch),
            vec!["new entry: 2026-03-23".to_owned(), "base".to_owned(),]
        );
    }

    #[cfg(unix)]
    #[test]
    fn no_summary_is_printed_when_no_commit_succeeded() {
        let scratch = Scratch::new("no-commit").unwrap();
        scratch.init_git();
        scratch.write(DOSSIER, "# Legacy\n## Worklog\n").unwrap();
        scratch
            .write(
                ENTRY,
                "# 2026-03-23\n\n## Notes\n- \n\n## Worked on Dossiers\n\
                 - [0007 - legacy](<../dossiers/0007 - legacy.md>)\n",
            )
            .unwrap();
        scratch.git(&["add", "-A"]);
        scratch.git(&["commit", "-qm", "base"]);
        scratch
            .write(DOSSIER, "# Legacy\n## Worklog\n- manual\n")
            .unwrap();
        fail_dossier_commits(&scratch);

        let (out, err) = run_in_scratch(&scratch, &args("2026-03-23"), || {
            Ok("Did some work".to_owned())
        })
        .unwrap();

        assert!(!out.contains("Logged to"), "{out}");
        assert!(!out.contains("changed,"), "{out}");
        assert!(err.contains("not committed"), "{err}");
        assert_eq!(titles(&scratch), vec!["base".to_owned()]);
    }
}
