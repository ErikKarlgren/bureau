//! `bureau new dossier` and `bureau new entry`.

use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Component, Path};

use anyhow::{Context, bail};
use chrono::Local;
use dialoguer::Input;

use crate::Result;
use crate::cli::DossierArgs;
use crate::commands::paths::{DOSSIERS_DIR, ENTRIES_DIR};
use crate::dossier;
use crate::git;
use crate::prompt::{self, PrefixTheme};
use crate::style::{self, Palette};
use crate::template;

/// What the description prompt asks the user to do.
const DESCRIPTION_HINT: &str = "(finish with a line containing only '.')";

/// What the optional link prompt asks the user to do.
const LINK_HINT: &str = "(optional, e.g. Jira; press Enter to skip)";

/// Create a dossier named after the arguments, then commit it.
///
/// # Errors
///
/// Fails when the current directory is not in a git repository, when a dossier
/// with the same name already exists, or when the dossier cannot be written.
pub fn dossier(args: &DossierArgs) -> Result<()> {
    let root = git::toplevel()?;
    let base = std::env::current_dir().ok();
    let palette = style::for_stdout();
    let prompt_palette = style::for_stderr();

    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        let mut answers = InteractiveAnswers;
        run_in(
            &root,
            base.as_deref(),
            args,
            palette,
            prompt_palette,
            &mut answers,
        )
    } else {
        let stdin = io::stdin();
        let mut answers = PipedAnswers {
            answers: stdin.lock(),
        };
        run_in(
            &root,
            base.as_deref(),
            args,
            palette,
            prompt_palette,
            &mut answers,
        )
    }
}

/// Create today's daily entry, then commit it.
///
/// # Errors
///
/// Fails when the current directory is not in a git repository, when today's
/// entry already exists, or when the entry cannot be written.
pub fn entry() -> Result<()> {
    let root = git::toplevel()?;
    let date = Local::now().format("%Y-%m-%d").to_string();
    let path = root.join(ENTRIES_DIR).join(format!("{date}.md"));

    let base = std::env::current_dir().ok();
    let shown = shown_from(&path, base.as_deref());

    if path.exists() {
        bail!("an entry already exists at '{shown}'");
    }

    println!("{}", created(&shown, style::for_stdout()));
    write_and_commit(
        &root,
        &path,
        &shown,
        &template::daily_entry(&date),
        "entry",
        "new entry",
    )
}

/// The answers to the description and link prompts.
///
/// The two implementations keep the same flow testable without a terminal
/// while letting a person edit each line at one.
trait Answers {
    fn read(&mut self, palette: Palette) -> Result<(String, String)>;
}

/// Answers read one line at a time from a pipe, a file or a test.
struct PipedAnswers<R> {
    answers: R,
}

impl<R: BufRead> Answers for PipedAnswers<R> {
    fn read(&mut self, palette: Palette) -> Result<(String, String)> {
        let description = read_description(&mut self.answers, palette)?;
        let link = read_link(&mut self.answers, palette)?;

        Ok((description, link))
    }
}

/// Answers read through a line editor, one line per prompt.
struct InteractiveAnswers;

impl Answers for InteractiveAnswers {
    fn read(&mut self, palette: Palette) -> Result<(String, String)> {
        let description = read_description_interactive(palette)?;
        let link = read_link_interactive(palette)?;

        Ok((description, link))
    }
}

/// The rest of [`dossier`], with the repository root, the directory the names
/// are read from, the colours and the answers handed in, so a test can drive it
/// without a git checkout, a terminal or an environment.
fn run_in(
    root: &Path,
    base: Option<&Path>,
    args: &DossierArgs,
    palette: Palette,
    prompt_palette: Palette,
    answers: &mut impl Answers,
) -> Result<()> {
    let name = args.name.join(" ");
    let path = root
        .join(DOSSIERS_DIR)
        .join(dossier::name_to_filename(&name));
    let shown = shown_from(&path, base);

    if path.exists() {
        bail!("a dossier already exists at '{shown}'");
    }

    println!("{}", creating_dossier(&name, palette));
    println!();

    let (description, link) = answers.read(prompt_palette)?;

    let creation_date = now();
    let contents = template::render(
        template::DOSSIER,
        &[
            ("DOSSIER_NAME", name.as_str()),
            ("CREATION_DATE", creation_date.as_str()),
            ("DESCRIPTION", description.as_str()),
            ("DOSSIER_LINK", link.as_str()),
        ],
    );

    println!("{}", created(&shown, palette));
    write_and_commit(root, &path, &shown, &contents, "dossier", "new dossier")
}

/// The current date and time, as `bureau new dossier` writes it into the
/// template and into the file's name.
fn now() -> String {
    let now = Local::now();

    format!("{} {}", now.format("%Y-%m-%d"), now.format("%H:%M:%S"))
}

/// Read the description, ending at a line containing only a dot.
fn read_description(answers: &mut impl BufRead, palette: Palette) -> Result<String> {
    eprintln!(
        "{}",
        prompt::label(palette, "Description", DESCRIPTION_HINT)
    );

    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        if answers.read_line(&mut line)? == 0 {
            break;
        }

        let line = line.trim_end_matches(['\r', '\n']);
        if line == "." {
            break;
        }
        lines.push(line.to_owned());
    }
    eprintln!();

    Ok(lines.join("\n"))
}

/// Read the optional link to the task this dossier is about.
fn read_link(answers: &mut impl BufRead, palette: Palette) -> Result<String> {
    eprint!("{}", prompt::prompt(palette, "Issue link", LINK_HINT));
    io::stderr().flush()?;

    let mut link = String::new();
    if answers.read_line(&mut link)? == 0 {
        // The answers ended, which means no link.
        eprintln!();
        return Ok(String::new());
    }

    let link = link.trim();
    eprintln!();
    if link.is_empty() {
        return Ok(String::new());
    }

    Ok(format!("- [Link to task]({link})\n"))
}

/// Read the description with a line editor, one `> ` prompt per line.
fn read_description_interactive(palette: Palette) -> Result<String> {
    eprintln!(
        "{}",
        prompt::label(palette, "Description", DESCRIPTION_HINT)
    );

    let theme = PrefixTheme::new(prompt::gutter(palette));
    let mut lines = Vec::new();
    loop {
        let line = Input::<String>::with_theme(&theme)
            .allow_empty(true)
            .with_prompt("")
            .interact_text()
            .context("could not read the description")?;

        if line == "." {
            break;
        }
        lines.push(line);
    }
    eprintln!();

    Ok(lines.join("\n"))
}

/// Read the optional link with a line editor.
fn read_link_interactive(palette: Palette) -> Result<String> {
    let theme = PrefixTheme::new(prompt::prompt(palette, "Issue link", LINK_HINT));
    let link = Input::<String>::with_theme(&theme)
        .allow_empty(true)
        .with_prompt("")
        .interact_text()
        .context("could not read the issue link")?;

    let link = link.trim();
    if link.is_empty() {
        return Ok(String::new());
    }

    Ok(format!("- [Link to task]({link})\n"))
}

/// Write a new file, creating its directory, then commit it.
///
/// A git failure is only a warning: the file on disk is useful either way.
/// `root` is the repository root the path belongs to. `shown` is the name of
/// the file relative to the working directory, which is what every message says
/// it by. `action` is the first half of the commit title, so the file's own
/// name is taken from `path`.
fn write_and_commit(
    root: &Path,
    path: &Path,
    shown: &str,
    contents: &str,
    kind: &str,
    action: &str,
) -> Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)
            .with_context(|| format!("could not create '{}'", directory.display()))?;
    }

    fs::write(path, contents).with_context(|| format!("could not write '{shown}'"))?;

    if let Err(error) = git::commit(root, &[path], &git::commit_title(action, path)) {
        eprintln!("warning: {error:?}");
        eprintln!("warning: the {kind} was created at '{shown}' but is not committed");
    }

    Ok(())
}

/// What to print once the file is on disk: the bold verb and the painted name.
fn created(shown: &str, palette: Palette) -> String {
    format!(
        "{} {}",
        palette.heading().paint("Created"),
        palette.quoted(shown)
    )
}

/// What to print before the prompts: the name the user gave, painted.
fn creating_dossier(name: &str, palette: Palette) -> String {
    format!("Creating dossier {}", palette.quoted(name))
}

/// The name to print for `path`: how to walk to it from `base`, the current
/// working directory, so the line reads from where the user is standing rather
/// than repeating the machine's idea of where the notes live.
///
/// When there is no working directory to measure against, or one that is not
/// absolute, it falls back to the path itself, which is at least true. A name
/// is never invented: `../notes/a.md` for a file that is not there would be
/// worse than a long one that is right.
fn shown_from(path: &Path, base: Option<&Path>) -> String {
    relative_to(path, base).unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// A path as it reads from `base`, with `/` for every separator so the same
/// line comes out on either platform. `../entries/2026-09-20.md` is what it
/// means: the way to walk to `path` from `base`.
///
/// `None` when either path is relative, because then there is no common ground
/// to walk from, and the caller's cue to name the file some other way. A path
/// made up without one would name a file that is not there.
fn relative_to(path: &Path, base: Option<&Path>) -> Option<String> {
    let path = names(path)?;
    let base = names(base?)?;

    let common = path
        .iter()
        .zip(base.iter())
        .take_while(|(here, there)| here == there)
        .count();

    let mut steps: Vec<String> = vec!["..".to_owned(); base.len().saturating_sub(common)];
    steps.extend(path.iter().skip(common).cloned());

    Some(steps.join("/"))
}

/// The names in an absolute path, root first, or `None` when it is relative.
///
/// `..` in a path is out of place in what bureau prints, so it stays as written
/// rather than being resolved: a working directory that git cannot name
/// absolutely is not one this command can describe.
fn names(path: &Path) -> Option<Vec<String>> {
    if !path.is_absolute() {
        return None;
    }

    Some(
        path.components()
            .filter_map(|component| match component {
                Component::Normal(text) => Some(text.to_string_lossy().into_owned()),
                Component::ParentDir => Some("..".to_owned()),
                Component::CurDir | Component::RootDir | Component::Prefix(_) => None,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::PathBuf;

    use super::*;
    use crate::commands::tests::Scratch;

    /// The directory the names in a test are read from: the scratch root, so a
    /// test sees the same `dossiers/x.md` a user at the root would.
    fn base(scratch: &Scratch) -> PathBuf {
        scratch.path().to_path_buf()
    }

    /// Arguments for a dossier named `name`.
    fn args(name: &str) -> DossierArgs {
        DossierArgs {
            name: vec![name.to_owned()],
        }
    }

    #[test]
    fn a_created_path_reads_from_where_the_user_stands() {
        let root = Path::new("/home/user/notes");
        let entry = root.join("entries/2026-09-20.md");

        assert_eq!(shown_from(&entry, Some(root)), "entries/2026-09-20.md");

        // From below the file, the line is just its name.
        assert_eq!(
            shown_from(&entry, Some(&root.join("entries"))),
            "2026-09-20.md"
        );

        // From a sibling, which is where `bureau new` is run from more often
        // than not, the line says how far back up the file is.
        assert_eq!(
            shown_from(&entry, Some(&root.join("dossiers"))),
            "../entries/2026-09-20.md"
        );
        assert_eq!(
            shown_from(&entry, Some(&root.join("dossiers/2026/09"))),
            "../../../entries/2026-09-20.md"
        );

        // A working directory outside the file's own tree still names it the
        // long way round rather than guessing.
        assert_eq!(
            shown_from(&entry, Some(Path::new("/home/user/other"))),
            "../notes/entries/2026-09-20.md"
        );
    }

    /// Run the dossier flow with answers from a string.
    fn run_answers(
        scratch: &Scratch,
        base: &Path,
        args: &DossierArgs,
        answers: &str,
    ) -> Result<()> {
        let mut answers = PipedAnswers {
            answers: Cursor::new(answers),
        };

        run_in(
            scratch.path(),
            Some(base),
            args,
            Palette::OFF,
            Palette::OFF,
            &mut answers,
        )
    }

    #[test]
    fn a_created_file_bolds_only_the_verb_and_paints_only_the_path() {
        // The verb is bold, the path is cyan, and the quotes stay plain: a copy
        // of the line pastes the path and not a screenful of escapes.
        assert_eq!(
            created("dossiers/1234 - fewafw.md", Palette::ON),
            "\x1b[1mCreated\x1b[0m '\x1b[36mdossiers/1234 - fewafw.md\x1b[0m'"
        );
        assert_eq!(
            created("dossiers/1234 - fewafw.md", Palette::OFF),
            "Created 'dossiers/1234 - fewafw.md'"
        );
    }

    #[test]
    fn a_creating_line_paints_only_the_name() {
        assert_eq!(
            creating_dossier("1234 - fewafw", Palette::ON),
            "Creating dossier '\x1b[36m1234 - fewafw\x1b[0m'"
        );
        assert_eq!(
            creating_dossier("1234 - fewafw", Palette::OFF),
            "Creating dossier '1234 - fewafw'"
        );
    }

    #[test]
    fn a_path_the_working_directory_cannot_be_measured_against_keeps_its_own() {
        // No working directory at all, or one that is not absolute: there is no
        // common ground to walk from, so the line falls back to what it has.
        let entry = Path::new("/home/user/notes/entries/2026-09-20.md");

        assert_eq!(relative_to(entry, None), None);
        assert_eq!(relative_to(entry, Some(Path::new("notes"))), None);
        assert_eq!(
            shown_from(entry, None),
            "/home/user/notes/entries/2026-09-20.md"
        );
    }

    #[test]
    fn a_path_uses_forward_slashes_whatever_the_platform_says() {
        // `bureau new` names a file the same way on Windows, so the line a
        // person copies out of it means the same thing in git and in a link.
        // Walking up is a `..` step with the same separator as any other.
        let root = Path::new("/home/user/notes");

        assert_eq!(
            relative_to(&root.join("entries/2026-09-20.md"), Some(root)),
            Some("entries/2026-09-20.md".to_owned())
        );
        assert_eq!(
            relative_to(
                &root.join("entries/2026-09-20.md"),
                Some(&root.join("dossiers"))
            ),
            Some("../entries/2026-09-20.md".to_owned())
        );
    }

    #[test]
    fn a_new_dossier_is_named_after_its_arguments() {
        let scratch = Scratch::new("new-dossier").unwrap();

        let base = base(&scratch);
        run_answers(
            &scratch,
            &base,
            &args("1234 - fewafw"),
            "A dossier about the font.\n.\nThe link.\n",
        )
        .unwrap();

        let written = scratch.read("dossiers/1234 - fewafw.md").unwrap();
        assert!(written.contains("A dossier about the font."), "{written:?}");
        assert!(written.contains("[Link to task](The link.)"), "{written:?}");
    }

    #[test]
    fn a_dossier_takes_no_link_when_the_answers_end_after_the_description() {
        let scratch = Scratch::new("new-dossier-no-link").unwrap();

        let base = base(&scratch);
        run_answers(
            &scratch,
            &base,
            &args("5678 - quiet"),
            "Nothing to link to.\n.\n",
        )
        .unwrap();

        let written = scratch.read("dossiers/5678 - quiet.md").unwrap();
        assert!(written.contains("Nothing to link to."), "{written:?}");
        assert!(!written.contains("[Link to task]"), "{written:?}");
    }

    #[test]
    fn a_new_dossier_commit_is_titled_after_its_file() {
        let scratch = Scratch::new("new-dossier-commit").unwrap();
        scratch.init_git();

        let base = base(&scratch);
        run_answers(
            &scratch,
            &base,
            &args("1234 - fewafw"),
            "A dossier about the font.\n.\n\n",
        )
        .unwrap();

        assert_eq!(
            scratch.git_output(&["log", "--format=%s"]).trim(),
            "new dossier: 1234 - fewafw"
        );
    }

    #[test]
    fn a_dossier_that_already_exists_is_left_alone() {
        let scratch = Scratch::new("new-dossier-twice").unwrap();
        scratch.write("dossiers/1 - here.md", "# Mine\n").unwrap();

        let base = base(&scratch);
        let error = run_answers(&scratch, &base, &args("1 - here"), "").unwrap_err();

        // The refusal names the file the way the message would have: the same
        // name relative to where the user is standing.
        assert!(
            error
                .to_string()
                .contains("already exists at 'dossiers/1 - here.md'"),
            "{error}"
        );
        assert_eq!(scratch.read("dossiers/1 - here.md").unwrap(), "# Mine\n");
    }
}
