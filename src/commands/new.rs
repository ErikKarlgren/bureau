//! `bureau new dossier` and `bureau new entry`.

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Component, Path};

use anyhow::{Context, bail};
use chrono::Local;

use crate::Result;
use crate::cli::DossierArgs;
use crate::commands::paths::{DOSSIERS_DIR, ENTRIES_DIR};
use crate::commands::sources;
use crate::dossier;
use crate::git;
use crate::style::{self, Palette};
use crate::template;

/// Create a dossier named after the arguments, then commit it.
///
/// # Errors
///
/// Fails when the current directory is not in a git repository, when a dossier
/// with the same name already exists, or when the dossier cannot be written.
pub fn dossier(args: &DossierArgs) -> Result<()> {
    run_in(
        &git::toplevel()?,
        args,
        style::for_stdout(),
        io::stdin().lock(),
    )
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

    if path.exists() {
        bail!("an entry already exists at path '{}'", path.display());
    }

    let base = std::env::current_dir().ok();
    println!("{}", created(&path, base.as_deref(), style::for_stdout()));
    write_and_commit(
        &path,
        &template::daily_entry(&date),
        "entry",
        &format!("New entry {date}"),
    )
}

/// The rest of [`dossier`], with the repository root, the colours and the
/// answers to its two prompts handed in, so a test can drive it without a git
/// checkout, a terminal or an environment.
fn run_in(root: &Path, args: &DossierArgs, palette: Palette, answers: impl BufRead) -> Result<()> {
    let name = args.name.join(" ");
    let path = root
        .join(DOSSIERS_DIR)
        .join(dossier::name_to_filename(&name));

    if path.exists() {
        bail!("a dossier already exists at path '{}'", path.display());
    }

    let creation_date = now();
    let (description, link) = read_answers(answers)?;

    let contents = template::render(
        template::DOSSIER,
        &[
            ("DOSSIER_NAME", name.as_str()),
            ("CREATION_DATE", creation_date.as_str()),
            ("DESCRIPTION", description.as_str()),
            ("DOSSIER_LINK", link.as_str()),
        ],
    );

    let base = std::env::current_dir().ok();
    println!("{}", created(&path, base.as_deref(), palette));
    write_and_commit(&path, &contents, "dossier", &format!("New dossier {name}"))
}

/// The current date and time, as `bureau new dossier` writes it into the
/// template and into the file's name.
fn now() -> String {
    let now = Local::now();

    format!("{} {}", now.format("%Y-%m-%d"), now.format("%H:%M:%S"))
}

/// Read the description and the optional link, in the order the prompts ask for
/// them.
fn read_answers(mut answers: impl BufRead) -> Result<(String, String)> {
    let description = read_description(&mut answers)?;
    let link = read_link(&mut answers)?;

    Ok((description, link))
}

/// Read the description, ending at a line containing only a dot.
fn read_description(answers: &mut impl BufRead) -> Result<String> {
    println!("Enter dossier's description. When done, write a line containing only '.'");

    let mut lines = Vec::new();
    for line in answers.lines() {
        let line = line?;
        if line == "." {
            break;
        }
        lines.push(line);
    }

    Ok(lines.join("\n"))
}

/// Read the optional link to the task this dossier is about.
fn read_link(answers: &mut impl BufRead) -> Result<String> {
    print!("Link to issue (e.g. Jira). If none, press Enter: ");
    io::stdout().flush()?;

    let mut link = String::new();
    if answers.read_line(&mut link)? == 0 {
        // The answers ended, which means no link.
        return Ok(String::new());
    }

    let link = link.trim_end_matches(['\r', '\n']);
    if link.is_empty() {
        return Ok(String::new());
    }

    Ok(format!("- [Link to task]({link})\n"))
}

/// Write a new file, creating its directory, then commit it.
///
/// A git failure is only a warning: the file on disk is useful either way.
/// Everything this prints is relative to the repository root rather than the
/// machine's idea of where the notes live.
fn write_and_commit(path: &Path, contents: &str, kind: &str, message: &str) -> Result<()> {
    let shown = sources::relative(path, &git::toplevel()?);

    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)
            .with_context(|| format!("could not create '{}'", directory.display()))?;
    }

    fs::write(path, contents).with_context(|| format!("could not write '{}'", shown.display()))?;

    if let Err(error) = git::commit(&[path], message) {
        eprintln!("warning: {error:?}");
        eprintln!(
            "warning: the {kind} was created at '{}' but is not committed",
            shown.display()
        );
    }

    Ok(())
}

/// What to print once `path` is on disk: the one line a person wants after
/// asking for a file, with the path painted.
///
/// The path is shown relative to `base`, the current working directory, so the
/// line names the file from where the user is standing rather than repeating
/// the machine's idea of where the notes live. When there is no working
/// directory to measure against, it falls back to the path itself, which is at
/// least true.
fn created(path: &Path, base: Option<&Path>, palette: Palette) -> String {
    let fallback = || path.to_string_lossy().into_owned();
    let shown = relative_to(path, base).unwrap_or_else(fallback);

    format!("Created '{}'", palette.path().paint(&shown))
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

    use super::*;
    use crate::commands::tests::Scratch;

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

        assert_eq!(
            created(&entry, Some(root), Palette::OFF),
            "Created 'entries/2026-09-20.md'"
        );

        // From below the file, the line is just its name.
        assert_eq!(
            created(&entry, Some(&root.join("entries")), Palette::OFF),
            "Created '2026-09-20.md'"
        );

        // From a sibling, which is where `bureau new` is run from more often
        // than not, the line says how far back up the file is.
        assert_eq!(
            created(&entry, Some(&root.join("dossiers")), Palette::OFF),
            "Created '../entries/2026-09-20.md'"
        );
        assert_eq!(
            created(&entry, Some(&root.join("dossiers/2026/09")), Palette::OFF),
            "Created '../../../entries/2026-09-20.md'"
        );

        // A working directory outside the file's own tree still names it the
        // long way round rather than guessing.
        assert_eq!(
            created(&entry, Some(Path::new("/home/user/other")), Palette::OFF),
            "Created '../notes/entries/2026-09-20.md'"
        );
    }

    #[test]
    fn only_the_path_of_a_created_file_takes_a_hue() {
        let root = Path::new("/home/user/notes");
        let font = root.join("dossiers/1234 - fewafw.md");

        // The sentence around it is not painted: only the path was asked for,
        // so a copy of the line pastes the path and not a screenful of escapes.
        assert_eq!(
            created(&font, Some(root), Palette::ON),
            "Created '\x1b[36mdossiers/1234 - fewafw.md\x1b[0m'"
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
            created(entry, None, Palette::OFF),
            "Created '/home/user/notes/entries/2026-09-20.md'"
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
        let answers = Cursor::new("A dossier about the font.\n.\nThe link.\n");

        run_in(
            scratch.path(),
            &args("1234 - fewafw"),
            Palette::OFF,
            answers,
        )
        .unwrap();

        let written = scratch.read("dossiers/1234 - fewafw.md").unwrap();
        assert!(written.contains("A dossier about the font."), "{written:?}");
        assert!(written.contains("[Link to task](The link.)"), "{written:?}");
    }

    #[test]
    fn a_dossier_takes_no_link_when_the_answers_end_after_the_description() {
        let scratch = Scratch::new("new-dossier-no-link").unwrap();
        let answers = Cursor::new("Nothing to link to.\n.\n");

        run_in(scratch.path(), &args("5678 - quiet"), Palette::OFF, answers).unwrap();

        let written = scratch.read("dossiers/5678 - quiet.md").unwrap();
        assert!(written.contains("Nothing to link to."), "{written:?}");
        assert!(!written.contains("[Link to task]"), "{written:?}");
    }

    #[test]
    fn a_dossier_that_already_exists_is_left_alone() {
        let scratch = Scratch::new("new-dossier-twice").unwrap();
        scratch.write("dossiers/1 - here.md", "# Mine\n").unwrap();

        let error = run_in(
            scratch.path(),
            &args("1 - here"),
            Palette::OFF,
            Cursor::new("...\n"),
        )
        .unwrap_err();

        assert!(error.to_string().contains("already exists"), "{error}");
        assert_eq!(scratch.read("dossiers/1 - here.md").unwrap(), "# Mine\n");
    }
}
