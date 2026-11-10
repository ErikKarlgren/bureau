//! `bureau init`.

use std::fmt::Write as _;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use chrono::Local;
use dialoguer::{Confirm, Input, Select};

use crate::Result;
use crate::commands::paths::DOSSIERS_DIR;
use crate::commands::{new as new_command, sources};
use crate::dossier;
use crate::git;
use crate::prompt::BureauTheme;
use crate::style::{self, Palette};
use crate::template;

/// The dossier name the tutorial is stored under.
const TUTORIAL_NAME: &str = "0000 - welcome to bureau";

/// The files `bureau init` recommends, with their template and explanation.
const RECOMMENDED_FILES: [(&str, &str, &str); 3] = [
    (
        ".gitattributes",
        template::GITATTRIBUTES,
        "Normalize text files to LF and use Git's Markdown diff driver; this also changes `bureau report` hunk headers.",
    ),
    (
        ".gitignore",
        template::GITIGNORE,
        "Keep editor swap files, OS metadata, and other local noise out of Git.",
    ),
    (
        ".editorconfig",
        template::EDITORCONFIG,
        "Use UTF-8, LF, and four-space indentation for notes and config files.",
    ),
];

/// What the tutorial and editor prompts say.
const TUTORIAL_HINT: &str = "(a dossier to teach you about bureau)";
const EDITOR_HINT: &str = "(optional; None is fine for VS Code, Obsidian, etc.)";
const RECOMMENDED_HINT: &str = "(Git attributes, ignores, EditorConfig and local Git settings)";
const IDENTITY_HINT: &str = "(used as the Git author)";
const APPLY_HINT: &str = "(you can edit every generated file afterwards)";

/// The editor configs `bureau init` can write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Editor {
    None,
    Neovim,
}

impl Editor {
    /// Every editor the picker offers, in the order it shows them.
    const ALL: [Self; 2] = [Self::None, Self::Neovim];

    /// The name the picker shows.
    const fn label(self) -> &'static str {
        match self {
            Self::None => "None (no editor-specific config)",
            Self::Neovim => "Neovim (.nvim.lua with automatic folds)",
        }
    }

    /// The index of the choice that is this editor.
    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|editor| *editor == self)
            .unwrap_or(0)
    }

    /// The editor a picker index names.
    fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::None)
    }

    /// The file this editor's project-local configuration lives in.
    const fn config_path(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Neovim => Some(".nvim.lua"),
        }
    }
}

/// The answers `bureau init` asks for, with the terminal left to the caller.
///
/// A test supplies these directly, so the flow is testable without a terminal
/// or a TTY, just like `bureau new`.
trait Answers {
    /// Whether to create a repository here when there is none.
    fn initialize(&mut self) -> Result<bool>;

    /// The Git author name and email, with the current values as defaults.
    fn identity(&mut self, name: Option<&str>, email: Option<&str>) -> Result<(String, String)>;

    /// Which editor configuration to write.
    fn editor(&mut self, default: Editor) -> Result<Editor>;

    /// Whether to create the welcome dossier.
    fn tutorial(&mut self, default: bool) -> Result<bool>;

    /// Whether to create the recommended files and apply the Git settings.
    fn recommended(&mut self) -> Result<bool>;

    /// Whether to apply the summary that was just printed.
    fn confirm(&mut self) -> Result<bool>;
}

/// Answers read from the terminal.
struct InteractiveAnswers {
    palette: Palette,
}

impl InteractiveAnswers {
    fn ask_confirm(&self, label: &str, hint: &str, default: bool) -> Result<bool> {
        let theme = BureauTheme::new(self.palette, label, hint);
        Confirm::with_theme(&theme)
            .with_prompt("")
            .default(default)
            .interact()
            .with_context(|| format!("could not read the {label} answer"))
    }

    fn ask_select(&self, label: &str, hint: &str, items: &[&str], default: usize) -> Result<usize> {
        let theme = BureauTheme::new(self.palette, label, hint);
        Select::with_theme(&theme)
            .with_prompt("")
            .items(items)
            .default(default)
            .interact()
            .with_context(|| format!("could not read the {label} choice"))
    }

    fn ask_input(&self, label: &str, hint: &str, default: Option<&str>) -> Result<String> {
        let theme = BureauTheme::new(self.palette, label, hint);
        let mut input = Input::<String>::with_theme(&theme)
            .with_prompt("")
            .allow_empty(false);

        if let Some(default) = default {
            input = input.default(default.to_owned());
        }

        input
            .interact_text()
            .with_context(|| format!("could not read the {label} answer"))
    }
}

impl Answers for InteractiveAnswers {
    fn initialize(&mut self) -> Result<bool> {
        self.ask_confirm(
            "Initialize repository",
            "(there is no Git repository here yet)",
            true,
        )
    }

    fn identity(&mut self, name: Option<&str>, email: Option<&str>) -> Result<(String, String)> {
        let name = self.ask_input("Your name", IDENTITY_HINT, name)?;
        let email = self.ask_input("Email", IDENTITY_HINT, email)?;

        Ok((name, email))
    }

    fn editor(&mut self, default: Editor) -> Result<Editor> {
        let items: Vec<&str> = Editor::ALL.iter().map(|editor| editor.label()).collect();
        let index = self.ask_select("Editor", EDITOR_HINT, &items, default.index())?;

        Ok(Editor::from_index(index))
    }

    fn tutorial(&mut self, default: bool) -> Result<bool> {
        self.ask_confirm("Tutorial dossier", TUTORIAL_HINT, default)
    }

    fn recommended(&mut self) -> Result<bool> {
        self.ask_confirm("Recommended setup", RECOMMENDED_HINT, true)
    }

    fn confirm(&mut self) -> Result<bool> {
        self.ask_confirm("Apply these changes", APPLY_HINT, true)
    }
}

/// One file init will write.
struct PlannedFile {
    path: PathBuf,
    contents: String,
    explanation: Option<String>,
}

/// One repository-local Git setting init will change.
struct ConfigChange {
    key: String,
    value: String,
}

/// The Git identity, and what was there before it.
struct Identity {
    name: String,
    email: String,
    current_name: Option<String>,
    current_email: Option<String>,
}

/// Everything init asked for, before anything is written.
struct Plan {
    root: PathBuf,
    needs_init: bool,
    files: Vec<PlannedFile>,
    skipped: Vec<PathBuf>,
    config: Vec<ConfigChange>,
    editor: Editor,
    tutorial: bool,
}

/// Set up a repository for bureau.
///
/// # Errors
///
/// Fails when the current directory cannot be read, when the command is not
/// run at a terminal, when a Git operation fails, or when a file cannot be
/// written.
pub fn run() -> Result<()> {
    let cwd = std::env::current_dir().context("could not read the current directory")?;

    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!("bureau init needs a terminal; run it interactively");
    }

    let mut answers = InteractiveAnswers {
        palette: style::for_stderr(),
    };

    run_in(&cwd, style::for_stdout(), style::for_stderr(), &mut answers)
}

/// The rest of [`run`], with the directory, colours and answers handed in.
fn run_in(
    cwd: &Path,
    palette: Palette,
    prompt_palette: Palette,
    answers: &mut impl Answers,
) -> Result<()> {
    let existing = git::discover(cwd)?;
    let needs_init = existing.is_none();
    let root = existing.unwrap_or_else(|| cwd.to_path_buf());
    let shown_root = shown_directory(&root, cwd);

    if needs_init {
        println!(
            "No Git repository found at {}.",
            palette.quoted(&shown_root)
        );

        if !answers.initialize()? {
            println!("Nothing was changed.");
            return Ok(());
        }
    } else {
        println!(
            "Using existing Git repository at {}.",
            palette.quoted(&shown_root)
        );
    }

    let (identity, identity_pending) = resolve_identity(&root, palette, answers)?;

    if identity.name.is_empty() || identity.email.is_empty() {
        bail!("a name and an email are required to make the first commit");
    }

    let mut pending = identity_pending;
    let (editor, editor_pending) = resolve_editor(&root, palette, answers)?;
    pending |= editor_pending;
    let (tutorial, tutorial_pending) = resolve_tutorial(&root, palette, answers)?;
    pending |= tutorial_pending;
    let (recommended, recommended_pending) = resolve_recommended(&root, palette, answers)?;
    pending |= recommended_pending;

    let plan = build_plan(&root, needs_init, &identity, editor, tutorial, recommended)?;

    if plan.files.is_empty() && plan.config.is_empty() && !plan.needs_init {
        if pending {
            println!("Nothing was changed.");
        } else {
            println!(
                "{}",
                palette
                    .heading()
                    .paint("Repository is already set up; nothing to do.")
            );
        }
        return Ok(());
    }

    eprint!("{}", summary(&plan, cwd, prompt_palette));

    if !answers.confirm()? {
        println!("Nothing was changed.");
        return Ok(());
    }

    apply(&plan, cwd, palette, prompt_palette)
}

/// Read the configured author, or ask for the missing parts.
fn resolve_identity(
    root: &Path,
    palette: Palette,
    answers: &mut impl Answers,
) -> Result<(Identity, bool)> {
    let current_name = git::config_get(root, "user.name")?.filter(|value| !value.trim().is_empty());
    let current_email =
        git::config_get(root, "user.email")?.filter(|value| !value.trim().is_empty());

    if let (Some(name), Some(email)) = (&current_name, &current_email) {
        println!(
            "Git author: {name} <{email}> {}",
            palette.context().paint("(already configured).")
        );

        Ok((
            Identity {
                name: name.clone(),
                email: email.clone(),
                current_name,
                current_email,
            },
            false,
        ))
    } else {
        println!("Git author: incomplete.");
        let (name, email) = answers.identity(current_name.as_deref(), current_email.as_deref())?;

        Ok((
            Identity {
                name: name.trim().to_owned(),
                email: email.trim().to_owned(),
                current_name,
                current_email,
            },
            true,
        ))
    }
}

/// Decide whether to write an editor config, asking only when none exists.
fn resolve_editor(
    root: &Path,
    palette: Palette,
    answers: &mut impl Answers,
) -> Result<(Editor, bool)> {
    let nvim_config = Editor::Neovim.config_path().unwrap_or(".nvim.lua");

    if root.join(nvim_config).exists() {
        println!(
            "{}",
            palette.context().paint(&format!(
                "Editor config: {nvim_config} already present; leaving it alone."
            ))
        );
        Ok((Editor::None, false))
    } else {
        Ok((answers.editor(default_editor())?, true))
    }
}

/// Decide whether to create the welcome dossier, asking only when it is useful.
fn resolve_tutorial(
    root: &Path,
    palette: Palette,
    answers: &mut impl Answers,
) -> Result<(bool, bool)> {
    if tutorial_path(root).exists() {
        println!(
            "{}",
            palette
                .context()
                .paint("Welcome dossier: already present; leaving it alone.")
        );
        Ok((false, false))
    } else if !sources::markdown_paths(root, DOSSIERS_DIR)?.is_empty() {
        println!(
            "{}",
            palette
                .context()
                .paint("Welcome dossier: skipped; this repository already has dossiers.")
        );
        Ok((false, false))
    } else {
        Ok((answers.tutorial(true)?, true))
    }
}

/// Decide whether to apply the recommended setup, asking only when it is incomplete.
fn resolve_recommended(
    root: &Path,
    palette: Palette,
    answers: &mut impl Answers,
) -> Result<(bool, bool)> {
    if recommended_setup_needed(root)? {
        Ok((answers.recommended()?, true))
    } else {
        println!(
            "{}",
            palette
                .context()
                .paint("Recommended setup: already in place.")
        );
        Ok((false, false))
    }
}

/// What the plan should contain, given the answers.
fn build_plan(
    root: &Path,
    needs_init: bool,
    identity: &Identity,
    editor: Editor,
    tutorial: bool,
    recommended: bool,
) -> Result<Plan> {
    let mut files = Vec::new();
    let mut skipped = Vec::new();

    if recommended {
        for (relative, contents, explanation) in RECOMMENDED_FILES {
            add_file(
                &mut files,
                &mut skipped,
                root,
                relative,
                contents,
                Some(explanation),
            );
        }
    }

    if let Some(path) = editor.config_path() {
        add_file(
            &mut files,
            &mut skipped,
            root,
            path,
            template::NVIM_CONFIG,
            Some("Add automatic Markdown folds to Neovim for this repository only."),
        );
    }

    if tutorial {
        let path = tutorial_path(root);
        let creation_date = now();
        let contents = template::render(
            template::TUTORIAL,
            &[("CREATION_DATE", creation_date.as_str())],
        );
        add_existing_or_new(
            &mut files,
            &mut skipped,
            path,
            contents,
            Some("Create a tutorial dossier with an open checklist.".to_owned()),
        );
    }

    let mut config = Vec::new();

    if identity.current_name.as_deref() != Some(identity.name.as_str()) {
        config.push(ConfigChange {
            key: "user.name".to_owned(),
            value: identity.name.clone(),
        });
    }
    if identity.current_email.as_deref() != Some(identity.email.as_str()) {
        config.push(ConfigChange {
            key: "user.email".to_owned(),
            value: identity.email.clone(),
        });
    }

    if recommended {
        config.extend(recommended_config(root)?);
    }

    Ok(Plan {
        root: root.to_path_buf(),
        needs_init,
        files,
        skipped,
        config,
        editor,
        tutorial,
    })
}

/// The path of the welcome dossier.
fn tutorial_path(root: &Path) -> PathBuf {
    root.join(DOSSIERS_DIR)
        .join(dossier::name_to_filename(TUTORIAL_NAME))
}

/// The recommended files that are not in the repository yet.
fn missing_recommended_files(root: &Path) -> Vec<&'static str> {
    RECOMMENDED_FILES
        .iter()
        .map(|(relative, _, _)| *relative)
        .filter(|relative| !root.join(relative).exists())
        .collect()
}

/// The recommended local Git settings that are not in place yet.
fn recommended_config(root: &Path) -> Result<Vec<ConfigChange>> {
    let mut config = Vec::new();

    add_config(&mut config, root, "core.quotepath", "false")?;
    add_config(&mut config, root, "core.autocrlf", "false")?;
    add_config(&mut config, root, "diff.algorithm", "histogram")?;

    let version = git::version(root)?;
    let conflict_style = if version >= (2, 35) {
        "zdiff3"
    } else {
        "diff3"
    };
    add_config(&mut config, root, "merge.conflictStyle", conflict_style)?;
    add_config(&mut config, root, "fetch.writeCommitGraph", "true")?;

    Ok(config)
}

/// Whether any recommended file or setting is missing.
fn recommended_setup_needed(root: &Path) -> Result<bool> {
    Ok(!missing_recommended_files(root).is_empty() || !recommended_config(root)?.is_empty())
}

/// Add a file with a known contents, unless it is already there.
fn add_file(
    files: &mut Vec<PlannedFile>,
    skipped: &mut Vec<PathBuf>,
    root: &Path,
    relative: &str,
    contents: &str,
    explanation: Option<&str>,
) {
    add_existing_or_new(
        files,
        skipped,
        root.join(relative),
        contents.to_owned(),
        explanation.map(String::from),
    );
}

/// Add a file, unless it is already there.
fn add_existing_or_new(
    files: &mut Vec<PlannedFile>,
    skipped: &mut Vec<PathBuf>,
    path: PathBuf,
    contents: String,
    explanation: Option<String>,
) {
    if path.exists() {
        skipped.push(path);
    } else {
        files.push(PlannedFile {
            path,
            contents,
            explanation,
        });
    }
}

/// Add a local setting, unless it already has the value we want.
fn add_config(config: &mut Vec<ConfigChange>, root: &Path, key: &str, value: &str) -> Result<()> {
    if git::config_get(root, key)?.as_deref() != Some(value) {
        config.push(ConfigChange {
            key: key.to_owned(),
            value: value.to_owned(),
        });
    }

    Ok(())
}

/// The editor to preselect: Neovim only when `$EDITOR` names it.
fn default_editor() -> Editor {
    editor_for(std::env::var("EDITOR").ok().as_deref())
}

/// The first token of `editor` when it starts with `quote`, without the quotes.
fn quoted_program(editor: &str, quote: char) -> Option<&str> {
    let rest = editor.strip_prefix(quote)?;
    Some(rest.split(quote).next().unwrap_or(rest))
}

/// The editor an `$EDITOR` value names, if it is one bureau supports.
fn editor_for(editor: Option<&str>) -> Editor {
    let Some(editor) = editor else {
        return Editor::None;
    };

    let editor = editor.trim();
    let program = quoted_program(editor, '"')
        .or_else(|| quoted_program(editor, '\''))
        .unwrap_or_else(|| editor.split_whitespace().next().unwrap_or_default());

    let name = Path::new(program)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    if name.eq_ignore_ascii_case("nvim") || name.eq_ignore_ascii_case("neovim") {
        Editor::Neovim
    } else {
        Editor::None
    }
}

/// A directory as it reads from `cwd`; a root with no relative path is `.`.
fn shown_directory(path: &Path, cwd: &Path) -> String {
    let shown = new_command::shown_from(path, Some(cwd));
    if shown.is_empty() {
        ".".to_owned()
    } else {
        shown
    }
}

/// The date written into the tutorial's `Creation date` line.
fn now() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Show what the plan is about to do, before asking whether to do it.
fn summary(plan: &Plan, cwd: &Path, palette: Palette) -> String {
    let mut output = String::new();

    if plan.needs_init {
        let _ = writeln!(
            output,
            "{} {}",
            palette.heading().paint("Repository:"),
            palette.quoted(&shown_directory(&plan.root, cwd))
        );
    }

    let _ = writeln!(output, "{}", palette.heading().paint("Files to create:"));
    if plan.files.is_empty() {
        let _ = writeln!(output, "  {}", palette.context().paint("(none)"));
    }
    for file in &plan.files {
        let _ = writeln!(
            output,
            "  {}",
            palette.quoted(&new_command::shown_from(&file.path, Some(cwd)))
        );
        if let Some(explanation) = &file.explanation {
            let _ = writeln!(output, "    {}", palette.context().paint(explanation));
        }
    }

    if !plan.skipped.is_empty() {
        let _ = writeln!(
            output,
            "{}",
            palette.heading().paint("Files left alone (already exist):")
        );
        for path in &plan.skipped {
            let _ = writeln!(
                output,
                "  {}",
                palette.quoted(&new_command::shown_from(path, Some(cwd)))
            );
        }
    }

    let _ = writeln!(
        output,
        "{}",
        palette.heading().paint("Git configuration to change:")
    );
    if plan.config.is_empty() {
        let _ = writeln!(output, "  {}", palette.context().paint("(none)"));
    }
    for change in &plan.config {
        let _ = writeln!(
            output,
            "  {} = {}",
            palette.path().paint(&change.key),
            change.value
        );
    }

    if !plan.files.is_empty() {
        let _ = writeln!(
            output,
            "{} {} {}",
            palette.heading().paint("Commit to create:"),
            palette.quoted("init: set up bureau"),
            palette
                .context()
                .paint(&format!("({})", count(plan.files.len(), "file")))
        );
    }

    output
}

/// Do what the summary said.
fn apply(plan: &Plan, cwd: &Path, palette: Palette, warning_palette: Palette) -> Result<()> {
    if plan.needs_init {
        git::init(&plan.root)?;
        println!(
            "{} {}",
            palette.heading().paint("Initialized repository at"),
            palette.quoted(&shown_directory(&plan.root, cwd))
        );
    }

    let mut created: Vec<PathBuf> = Vec::new();
    for file in &plan.files {
        if let Some(directory) = file.path.parent() {
            fs::create_dir_all(directory)
                .with_context(|| format!("could not create '{}'", directory.display()))?;
        }

        let shown = new_command::shown_from(&file.path, Some(cwd));
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file.path)
            .and_then(|mut handle| handle.write_all(file.contents.as_bytes()))
            .with_context(|| format!("could not write '{shown}'"))?;
        created.push(file.path.clone());
        println!(
            "{} {}",
            palette.heading().paint("Created"),
            palette.quoted(&shown)
        );
    }

    for change in &plan.config {
        git::config_set(&plan.root, &change.key, &change.value)?;
        println!(
            "{} {} = {}",
            palette.heading().paint("Configured"),
            change.key,
            change.value
        );
    }

    if !created.is_empty() {
        let paths: Vec<&Path> = created.iter().map(PathBuf::as_path).collect();

        match git::commit_force(&plan.root, &paths, "init: set up bureau") {
            Ok(()) => println!(
                "{} {} as {}",
                palette.heading().paint("Committed"),
                count(created.len(), "file"),
                palette.quoted("init: set up bureau")
            ),
            Err(error) => {
                eprintln!(
                    "{}",
                    warning_palette.warning_line(format_args!("{error:?}"))
                );
                eprintln!(
                    "{}",
                    warning_palette
                        .warning_line(format_args!("the files were created but are not committed"))
                );
            }
        }
    }

    if plan.tutorial {
        println!(
            "Read {} to get started.",
            palette.quoted(&new_command::shown_from(
                &tutorial_path(&plan.root),
                Some(cwd),
            ))
        );
    }

    if plan.editor == Editor::Neovim {
        println!(
            "{} To use '.nvim.lua', add `vim.opt.exrc = true` to your Neovim config, then open Neovim here and trust the file.",
            palette.heading().paint("Neovim setup:")
        );
    }

    Ok(())
}

/// A count with its noun.
fn count(number: usize, noun: &str) -> String {
    if number == 1 {
        format!("1 {noun}")
    } else {
        format!("{number} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::commands::doctor;
    use crate::commands::tests::Scratch;
    use crate::style::Palette;

    /// A yes/no answer a test decides in advance.
    #[derive(Clone, Copy)]
    enum Answer {
        Yes,
        No,
    }

    impl Answer {
        const fn is_yes(self) -> bool {
            matches!(self, Self::Yes)
        }
    }

    /// A prompt a test can check was asked.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Prompt {
        Identity,
        Editor,
        Tutorial,
        Recommended,
        Confirm,
    }

    /// Answers that a test decides in advance.
    struct TestAnswers {
        initialize: Answer,
        name: String,
        email: String,
        editor: Editor,
        tutorial: Answer,
        tutorial_default: Option<bool>,
        recommended: Answer,
        confirm: Answer,
        asked: Vec<Prompt>,
    }

    impl TestAnswers {
        fn new() -> Self {
            Self {
                initialize: Answer::Yes,
                name: "Bureau".to_owned(),
                email: "bureau@example.invalid".to_owned(),
                editor: Editor::Neovim,
                tutorial: Answer::Yes,
                tutorial_default: None,
                recommended: Answer::Yes,
                confirm: Answer::Yes,
                asked: Vec::new(),
            }
        }

        /// Whether `prompt` was asked.
        fn asked(&self, prompt: Prompt) -> bool {
            self.asked.contains(&prompt)
        }
    }

    impl Answers for TestAnswers {
        fn initialize(&mut self) -> Result<bool> {
            Ok(self.initialize.is_yes())
        }

        fn identity(
            &mut self,
            _name: Option<&str>,
            _email: Option<&str>,
        ) -> Result<(String, String)> {
            self.asked.push(Prompt::Identity);
            Ok((self.name.clone(), self.email.clone()))
        }

        fn editor(&mut self, _default: Editor) -> Result<Editor> {
            self.asked.push(Prompt::Editor);
            Ok(self.editor)
        }

        fn tutorial(&mut self, default: bool) -> Result<bool> {
            self.asked.push(Prompt::Tutorial);
            self.tutorial_default = Some(default);
            Ok(self.tutorial.is_yes())
        }

        fn recommended(&mut self) -> Result<bool> {
            self.asked.push(Prompt::Recommended);
            Ok(self.recommended.is_yes())
        }

        fn confirm(&mut self) -> Result<bool> {
            self.asked.push(Prompt::Confirm);
            Ok(self.confirm.is_yes())
        }
    }

    fn run(scratch: &Scratch, answers: &mut TestAnswers) -> Result<()> {
        run_in(scratch.path(), Palette::OFF, Palette::OFF, answers)
    }

    /// Put the recommended local Git settings in `scratch`.
    fn configure_recommended(scratch: &Scratch) {
        scratch.git(&["config", "core.quotepath", "false"]);
        scratch.git(&["config", "core.autocrlf", "false"]);
        scratch.git(&["config", "diff.algorithm", "histogram"]);

        let conflict = if git::version(scratch.path()).unwrap() >= (2, 35) {
            "zdiff3"
        } else {
            "diff3"
        };
        scratch.git(&["config", "merge.conflictStyle", conflict]);
        scratch.git(&["config", "fetch.writeCommitGraph", "true"]);
    }

    /// Write the files an initialized repository already has.
    fn write_existing_notes(scratch: &Scratch) {
        scratch.write(".gitattributes", "").unwrap();
        scratch.write(".gitignore", "").unwrap();
        scratch.write(".editorconfig", "").unwrap();
        scratch
            .write(
                "dossiers/0000 - welcome to bureau.md",
                "# 0000 - welcome to bureau
## Worklog
",
            )
            .unwrap();
    }

    #[test]
    fn the_summary_names_the_files_and_config_changes() {
        let plan = Plan {
            root: PathBuf::from("/notes"),
            needs_init: true,
            files: vec![PlannedFile {
                path: PathBuf::from("/notes/.gitattributes"),
                contents: String::new(),
                explanation: Some("Because it matters.".to_owned()),
            }],
            skipped: vec![PathBuf::from("/notes/.gitignore")],
            config: vec![ConfigChange {
                key: "core.quotepath".to_owned(),
                value: "false".to_owned(),
            }],
            editor: Editor::None,
            tutorial: false,
        };

        assert_eq!(
            summary(&plan, Path::new("/notes"), Palette::OFF),
            "Repository: '.'\nFiles to create:\n  '.gitattributes'\n    Because it matters.\nFiles left alone (already exist):\n  '.gitignore'\nGit configuration to change:\n  core.quotepath = false\nCommit to create: 'init: set up bureau' (1 file)\n"
        );

        let coloured = summary(&plan, Path::new("/notes"), Palette::ON);
        assert!(
            coloured.contains("\x1b[2mBecause it matters.\x1b[0m"),
            "the explanation should be dim"
        );
        assert!(
            coloured.contains("\x1b[36mcore.quotepath\x1b[0m = false"),
            "the config key should be cyan: {coloured:?}"
        );
        assert!(
            !coloured.contains("\x1b[1mcore.quotepath"),
            "the config key should not be bold: {coloured:?}"
        );
    }

    #[test]
    fn the_editor_default_follows_editor_only_for_supported_editors() {
        assert_eq!(editor_for(None), Editor::None);
        assert_eq!(editor_for(Some("code")), Editor::None);
        assert_eq!(editor_for(Some("vim")), Editor::None);
        assert_eq!(editor_for(Some("nvim")), Editor::Neovim);
        assert_eq!(editor_for(Some("neovim -u NONE")), Editor::Neovim);
        assert_eq!(editor_for(Some("/usr/bin/nvim")), Editor::Neovim);
        assert_eq!(
            editor_for(Some("\"C:/Program Files/Neovim/bin/nvim.exe\"")),
            Editor::Neovim
        );
    }

    #[test]
    fn the_editor_choices_say_what_they_write() {
        assert!(Editor::None.label().contains("no editor-specific config"));
        assert!(Editor::Neovim.label().contains(".nvim.lua"));
        assert!(EDITOR_HINT.contains("None"));
    }

    #[test]
    fn creates_a_repository_with_the_tutorial_and_configuration() {
        let scratch = Scratch::new("init-fresh").unwrap();
        let mut answers = TestAnswers::new();

        run(&scratch, &mut answers).unwrap();

        assert_eq!(answers.tutorial_default, Some(true));

        for path in [
            ".gitattributes",
            ".gitignore",
            ".editorconfig",
            ".nvim.lua",
            "dossiers/0000 - welcome to bureau.md",
        ] {
            assert!(scratch.path().join(path).exists(), "{path} is missing");
        }

        assert_eq!(
            scratch.git_output(&["log", "--format=%s"]).trim(),
            "init: set up bureau"
        );
        assert_eq!(
            scratch
                .git_output(&["config", "--get", "core.quotepath"])
                .trim(),
            "false"
        );
        assert!(
            !scratch
                .git_output(&["config", "--get", "user.name"])
                .trim()
                .is_empty()
        );
        assert!(
            !scratch
                .git_output(&["config", "--get", "user.email"])
                .trim()
                .is_empty()
        );
        assert_eq!(
            scratch
                .git_output(&["config", "--get", "fetch.writeCommitGraph"])
                .trim(),
            "true"
        );
        assert!(answers.asked(Prompt::Editor));
        assert!(answers.asked(Prompt::Tutorial));
        assert!(answers.asked(Prompt::Recommended));
        assert!(answers.asked(Prompt::Confirm));

        let attributes = scratch.read(".gitattributes").unwrap();
        assert!(attributes.contains("* text=auto eol=lf"), "{attributes}");
        assert!(
            attributes.contains("*.md text diff=markdown"),
            "{attributes}"
        );
        assert!(attributes.contains("bureau report"), "{attributes}");

        let (output, errors) = doctor::run_in(scratch.path(), Palette::OFF).unwrap();
        assert_eq!(output, "No formatting issues found\n");
        assert!(!errors);
    }

    #[test]
    fn a_declined_tutorial_is_not_created() {
        let scratch = Scratch::new("init-no-tutorial").unwrap();
        let mut answers = TestAnswers::new();
        answers.tutorial = Answer::No;

        run(&scratch, &mut answers).unwrap();

        assert!(!scratch.path().join("dossiers").exists());
    }

    #[test]
    fn a_declined_editor_config_is_not_created() {
        let scratch = Scratch::new("init-no-editor").unwrap();
        let mut answers = TestAnswers::new();
        answers.editor = Editor::None;

        run(&scratch, &mut answers).unwrap();

        assert!(!scratch.path().join(".nvim.lua").exists());
    }

    #[test]
    fn existing_files_are_left_alone() {
        let scratch = Scratch::new("init-existing").unwrap();
        scratch.write(".gitignore", "mine\n").unwrap();
        let mut answers = TestAnswers::new();

        run(&scratch, &mut answers).unwrap();

        assert_eq!(scratch.read(".gitignore").unwrap(), "mine\n");
        assert!(scratch.path().join(".gitattributes").exists());
    }

    #[test]
    fn a_file_created_after_planning_is_not_overwritten() {
        let scratch = Scratch::new("init-race").unwrap();
        scratch.write("late.md", "mine\n").unwrap();

        let plan = Plan {
            root: scratch.path().to_path_buf(),
            needs_init: false,
            files: vec![PlannedFile {
                path: scratch.path().join("late.md"),
                contents: "template\n".to_owned(),
                explanation: None,
            }],
            skipped: Vec::new(),
            config: Vec::new(),
            editor: Editor::None,
            tutorial: false,
        };

        assert!(apply(&plan, scratch.path(), Palette::OFF, Palette::OFF).is_err());
        assert_eq!(scratch.read("late.md").unwrap(), "mine\n");
    }

    #[test]
    fn a_declined_apply_changes_nothing() {
        let scratch = Scratch::new("init-cancelled").unwrap();
        let mut answers = TestAnswers::new();
        answers.confirm = Answer::No;

        run(&scratch, &mut answers).unwrap();

        assert!(!scratch.path().join(".git").exists());
        assert!(!scratch.path().join(".gitattributes").exists());
    }

    #[test]
    fn a_globally_ignored_editor_config_is_force_added() {
        let scratch = Scratch::new("init-ignored-nvim").unwrap();
        scratch.write(".gitignore", ".nvim.lua\n").unwrap();
        let mut answers = TestAnswers::new();

        run(&scratch, &mut answers).unwrap();

        let tracked = scratch.git_output(&["ls-files"]);
        assert!(tracked.contains(".nvim.lua"), "{tracked}");
    }

    #[test]
    fn an_existing_repository_is_used_from_a_subdirectory() {
        let scratch = Scratch::new("init-subdir").unwrap();
        scratch.init_git();
        scratch.write("sub/.keep", "").unwrap();
        let subdirectory = scratch.path().join("sub");
        let mut answers = TestAnswers::new();

        run_in(&subdirectory, Palette::OFF, Palette::OFF, &mut answers).unwrap();

        assert!(scratch.path().join(".gitattributes").exists());
        assert!(!subdirectory.join(".git").exists());
    }

    #[test]
    fn a_declined_initialization_leaves_the_directory_alone() {
        let scratch = Scratch::new("init-declined").unwrap();
        let mut answers = TestAnswers::new();
        answers.initialize = Answer::No;

        run(&scratch, &mut answers).unwrap();

        assert!(!scratch.path().join(".git").exists());
        assert!(!scratch.path().join(".gitattributes").exists());
    }

    #[test]
    fn a_repository_with_notes_does_not_ask_for_the_tutorial() {
        let scratch = Scratch::new("init-existing-notes").unwrap();
        scratch
            .write("dossiers/1 - here.md", "# 1\n## Worklog\n")
            .unwrap();
        let mut answers = TestAnswers::new();
        answers.tutorial = Answer::No;

        run(&scratch, &mut answers).unwrap();

        assert!(!answers.asked(Prompt::Tutorial));
        assert_eq!(answers.tutorial_default, None);
        assert!(
            !scratch
                .path()
                .join("dossiers/0000 - welcome to bureau.md")
                .exists()
        );
    }

    #[test]
    fn a_fully_configured_repository_is_a_no_op() {
        let scratch = Scratch::new("init-complete").unwrap();
        scratch.init_git();
        configure_recommended(&scratch);
        write_existing_notes(&scratch);
        scratch.write(".nvim.lua", "").unwrap();

        let mut answers = TestAnswers::new();
        run(&scratch, &mut answers).unwrap();

        assert!(!answers.asked(Prompt::Identity));
        assert!(!answers.asked(Prompt::Editor));
        assert!(!answers.asked(Prompt::Tutorial));
        assert!(!answers.asked(Prompt::Recommended));
        assert!(!answers.asked(Prompt::Confirm));
    }

    #[test]
    fn an_empty_configured_identity_is_asked_for() {
        let scratch = Scratch::new("init-empty-identity").unwrap();
        scratch.init_git();
        scratch.git(&["config", "user.name", ""]);
        scratch.git(&["config", "user.email", ""]);
        configure_recommended(&scratch);
        write_existing_notes(&scratch);
        scratch.write(".nvim.lua", "").unwrap();

        let mut answers = TestAnswers::new();
        run(&scratch, &mut answers).unwrap();

        assert!(answers.asked(Prompt::Identity));
        assert_eq!(
            scratch.git_output(&["config", "--get", "user.name"]).trim(),
            "Bureau"
        );
        assert_eq!(
            scratch
                .git_output(&["config", "--get", "user.email"])
                .trim(),
            "bureau@example.invalid"
        );
    }

    #[test]
    fn only_the_missing_editor_config_is_asked_for() {
        let scratch = Scratch::new("init-only-editor").unwrap();
        scratch.init_git();
        configure_recommended(&scratch);
        write_existing_notes(&scratch);

        let mut answers = TestAnswers::new();
        run(&scratch, &mut answers).unwrap();

        assert!(!answers.asked(Prompt::Identity));
        assert!(answers.asked(Prompt::Editor));
        assert!(!answers.asked(Prompt::Tutorial));
        assert!(!answers.asked(Prompt::Recommended));
        assert!(answers.asked(Prompt::Confirm));
        assert!(scratch.path().join(".nvim.lua").exists());
    }

    #[test]
    fn only_the_missing_recommended_file_is_asked_for() {
        let scratch = Scratch::new("init-only-recommended").unwrap();
        scratch.init_git();
        configure_recommended(&scratch);
        scratch.write(".gitattributes", "").unwrap();
        scratch.write(".gitignore", "").unwrap();
        scratch.write(".nvim.lua", "").unwrap();
        scratch
            .write(
                "dossiers/0000 - welcome to bureau.md",
                "# 0000 - welcome to bureau\n## Worklog\n",
            )
            .unwrap();

        let mut answers = TestAnswers::new();
        run(&scratch, &mut answers).unwrap();

        assert!(!answers.asked(Prompt::Identity));
        assert!(!answers.asked(Prompt::Editor));
        assert!(!answers.asked(Prompt::Tutorial));
        assert!(answers.asked(Prompt::Recommended));
        assert!(answers.asked(Prompt::Confirm));
        assert!(scratch.path().join(".editorconfig").exists());
    }

    #[test]
    fn the_nvim_config_is_valid_lua_for_a_markdown_buffer() {
        // The fold expression itself is covered by the `fence`/`tasks` tests;
        // this catches a syntax error in the generated Lua before a user does.
        let scratch = Scratch::new("init-nvim-syntax").unwrap();
        let mut answers = TestAnswers::new();
        run(&scratch, &mut answers).unwrap();

        let output = std::process::Command::new("nvim")
            .current_dir(scratch.path())
            .args(["--headless", "-u", "NONE", "-n", "-i", "NONE"])
            .arg("-c")
            .arg("luafile .nvim.lua")
            .arg("-c")
            .arg("qa!")
            .output();

        let Ok(output) = output else {
            return;
        };
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "the generated .nvim.lua did not load: {stderr}"
        );
        assert!(
            !stderr.contains("E5112"),
            "the generated .nvim.lua has a syntax error: {stderr}"
        );
    }

    #[test]
    fn the_nvim_config_finds_its_root_and_matches_the_rust_fence_rules() {
        let scratch = Scratch::new("init-nvim-folds").unwrap();
        let mut answers = TestAnswers::new();
        run(&scratch, &mut answers).unwrap();

        scratch
            .write(
                "note.md",
                "# Title\n## Before\n~~~text\n```\n# not a heading\n~~~\n## After\nText.\n    ```\n# indented heading\nText.\n## Nesting\n- parent\n  - child\n    - grandchild\n- sibling\n####### not a heading\n#\nText.\n",
            )
            .unwrap();
        scratch
            .write(
                "check.lua",
                r#"assert(vim.wo.foldmethod == "expr", "foldmethod=" .. vim.wo.foldmethod)
assert(vim.wo.foldlevel == 99, "foldlevel=" .. vim.wo.foldlevel)
local root = vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":p:h")
local hash = vim.fn.sha256(vim.fs.normalize(root)):sub(1, 16)
local fold = _G["bureau_notes_fold_" .. hash]
assert(type(fold) == "function", "per-repository fold function is missing")
assert(vim.wo.foldexpr:match("bureau_notes_fold_") ~= nil, "foldexpr is not per-repository")
local expected = { ">1", ">2", "2", "2", "2", "2", ">2", "2", "3", ">1", "1", ">2", "2", "3", "4", "2", "2", ">1", "1" }
for index, value in ipairs(expected) do
  local got = fold(index)
  assert(got == value, "line " .. index .. " expected " .. value .. " got " .. tostring(got))
end
vim.cmd("edit .nvim.lua")
vim.bo.filetype = "lua"
vim.cmd("doautocmd BufEnter")
assert(vim.wo.foldmethod ~= "expr", "folds leaked into a non-Markdown buffer")
vim.api.nvim_out_write("ok\n")
"#,
            )
            .unwrap();

        let output = std::process::Command::new("nvim")
            .current_dir(scratch.path())
            .args(["--headless", "-u", "NONE", "-n", "-i", "NONE", "note.md"])
            .args([
                "-c",
                "luafile .nvim.lua",
                "-c",
                "setfiletype markdown",
                "-c",
                "doautocmd FileType markdown",
                "-c",
                "luafile check.lua",
                "-c",
                "qa!",
            ])
            .output();

        let Ok(output) = output else {
            return;
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            stdout.contains("ok") || stderr.contains("ok"),
            "folds check failed: stdout={stdout:?} stderr={stderr:?}"
        );
        assert!(
            !stderr.contains("E5112"),
            "the generated .nvim.lua failed: {stderr}"
        );
    }

    #[test]
    fn a_tutorial_has_no_doctor_findings() {
        let scratch = Scratch::new("init-tutorial-doctor").unwrap();
        let mut answers = TestAnswers::new();
        run(&scratch, &mut answers).unwrap();

        let path = scratch.path().join("dossiers/0000 - welcome to bureau.md");
        let contents = fs::read_to_string(&path).unwrap();
        let findings = crate::doctor::check(
            &crate::doctor::Input {
                dossiers: vec![crate::doctor::Note {
                    path: Path::new("dossiers/0000 - welcome to bureau.md").to_path_buf(),
                    contents,
                }],
                entries: Vec::new(),
            },
            |_| Some(true),
        );

        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn the_foldlevel_is_99() {
        let scratch = Scratch::new("init-foldlevel").unwrap();
        let mut answers = TestAnswers::new();
        run(&scratch, &mut answers).unwrap();

        let config = scratch.read(".nvim.lua").unwrap();
        assert!(config.contains("vim.wo.foldlevel = 99"), "{config}");
    }
}
