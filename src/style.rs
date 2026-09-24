//! Colour for `bureau tasks`.
//!
//! The rules are in `docs/subcommands/tasks.md`; the whole of the
//! implementation is the table in [`Palette::ON`]. Colour is a redundant
//! encoding on purpose: every hue repeats something the line already says, so a
//! run with colour off loses nothing but the aid to scanning.

use std::env;
use std::io::IsTerminal;

use crate::tasks::{Section, State};

/// How a line is drawn: the sequence that opens it, empty when nothing is
/// drawn. Every style is closed by the same reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    open: &'static str,
}

impl Style {
    /// A style that draws nothing, so the text comes out as it was.
    pub const NONE: Self = Self { open: "" };

    /// `text` in this style.
    #[must_use]
    pub fn paint(self, text: &str) -> String {
        if self.open.is_empty() {
            return text.to_owned();
        }

        format!("{}{text}{RESET}", self.open)
    }
}

/// What closes every sequence.
///
/// One reset is enough because nothing here nests: a line is either a coloured
/// marker with plain text after it, or dim, or bold.
const RESET: &str = "\x1b[0m";

/// The basic `ANSI` slots, and the two attributes.
///
/// No bright slot and no truecolor: the terminal's theme owns the shades, and
/// these names are what it is guaranteed to have an opinion about.
const BLUE: Style = Style { open: "\x1b[34m" };
const CYAN: Style = Style { open: "\x1b[36m" };
const YELLOW: Style = Style { open: "\x1b[33m" };
const GREEN: Style = Style { open: "\x1b[32m" };
const DIM: Style = Style { open: "\x1b[2m" };
const BOLD: Style = Style { open: "\x1b[1m" };
const BOLD_BLUE: Style = Style { open: "\x1b[1;34m" };
const BOLD_YELLOW: Style = Style { open: "\x1b[1;33m" };
const BOLD_GREEN: Style = Style { open: "\x1b[1;32m" };

/// The colours a run draws with.
///
/// [`Self::OFF`] is every style empty, which is what makes a run with colour
/// turned off print exactly what it printed before colour existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// The rule that opens each section.
    actionable: Style,
    blocked: Style,
    finished: Style,
    /// The markers of the work each section is about.
    todo: Style,
    started: Style,
    waiting: Style,
    done: Style,
    /// A source heading, entry or dossier.
    heading: Style,
    /// Everything else printed: the lines that only lead to a task.
    context: Style,
}

impl Palette {
    /// No colour at all.
    pub const OFF: Self = Self {
        actionable: Style::NONE,
        blocked: Style::NONE,
        finished: Style::NONE,
        todo: Style::NONE,
        started: Style::NONE,
        waiting: Style::NONE,
        done: Style::NONE,
        heading: Style::NONE,
        context: Style::NONE,
    };

    /// Blue for what can be picked up, cyan for what has been started, yellow
    /// for what is waiting on somebody else, green for what is over.
    pub const ON: Self = Self {
        actionable: BOLD_BLUE,
        blocked: BOLD_YELLOW,
        finished: BOLD_GREEN,
        todo: BLUE,
        started: CYAN,
        waiting: YELLOW,
        done: GREEN,
        heading: BOLD,
        context: DIM,
    };

    /// The style a section's rule is drawn in.
    #[must_use]
    pub const fn section(self, section: Section) -> Style {
        match section {
            Section::Actionable => self.actionable,
            Section::Blocked => self.blocked,
            Section::Finished => self.finished,
        }
    }

    /// The style a task's marker is drawn in, or `None` when the marker is not
    /// one of the section's own states.
    ///
    /// `None` means the line is drawn as context instead: the whole line
    /// recedes rather than its marker taking a hue. This is a question about
    /// the marker and not about what the section matches, which is why the
    /// branch `--all` adds to `BLOCKED` is context even though the section
    /// counts every task in it.
    #[must_use]
    pub const fn marker(self, section: Section, state: State) -> Option<Style> {
        match (section, state) {
            (Section::Actionable, State::Todo) => Some(self.todo),
            (Section::Actionable, State::Doing | State::Almost) => Some(self.started),
            (Section::Blocked, State::Waiting) => Some(self.waiting),
            (Section::Finished, State::Done | State::Cancelled) => Some(self.done),
            _ => None,
        }
    }

    /// The style a source heading is drawn in.
    #[must_use]
    pub const fn heading(self) -> Style {
        self.heading
    }

    /// The style a line that only leads to a task is drawn in.
    #[must_use]
    pub const fn context(self) -> Style {
        self.context
    }
}

/// The palette for a run writing to standard output.
///
/// Colour is for a person reading a terminal, so a pipe or a redirect gets
/// plain text, and so do the two ways a user asks for it: `NO_COLOR` set to
/// anything non-empty, and `TERM=dumb`.
#[must_use]
pub fn for_stdout() -> Palette {
    let no_color = env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
    let dumb_terminal = env::var_os("TERM").is_some_and(|term| term == "dumb");

    if wanted(std::io::stdout().is_terminal(), no_color, dumb_terminal) {
        Palette::ON
    } else {
        Palette::OFF
    }
}

/// Whether colour is wanted: a terminal that did not ask for plain text.
///
/// Kept apart from reading the environment and asking `stdout`, so the rule
/// itself is testable without either.
const fn wanted(terminal: bool, no_color: bool, dumb_terminal: bool) -> bool {
    terminal && !no_color && !dumb_terminal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_style_that_draws_nothing_changes_nothing() {
        assert_eq!(Style::NONE.paint("- [x] a task"), "- [x] a task");
    }

    #[test]
    fn each_section_hues_its_own_markers() {
        let palette = Palette::ON;

        assert_eq!(
            palette
                .marker(Section::Actionable, State::Todo)
                .map(|s| s.paint("[ ]")),
            Some("\x1b[34m[ ]\x1b[0m".to_owned())
        );
        assert_eq!(
            palette
                .marker(Section::Actionable, State::Doing)
                .map(|s| s.paint("[.]")),
            Some("\x1b[36m[.]\x1b[0m".to_owned())
        );
        assert_eq!(
            palette
                .marker(Section::Actionable, State::Almost)
                .map(|s| s.paint("[o]")),
            Some("\x1b[36m[o]\x1b[0m".to_owned())
        );
        assert_eq!(
            palette
                .marker(Section::Blocked, State::Waiting)
                .map(|s| s.paint("[?]")),
            Some("\x1b[33m[?]\x1b[0m".to_owned())
        );
        assert_eq!(
            palette
                .marker(Section::Finished, State::Done)
                .map(|s| s.paint("[x]")),
            Some("\x1b[32m[x]\x1b[0m".to_owned())
        );
        assert_eq!(
            palette
                .marker(Section::Finished, State::Cancelled)
                .map(|s| s.paint("[-]")),
            Some("\x1b[32m[-]\x1b[0m".to_owned())
        );
    }

    #[test]
    fn a_marker_of_another_section_takes_no_hue() {
        let palette = Palette::ON;

        // Nothing but `[?]` is hued in `BLOCKED`, including the branch `--all`
        // adds; `[ ]` is not usable work there, so it is not work's colour.
        assert_eq!(palette.marker(Section::Blocked, State::Todo), None);
        assert_eq!(palette.marker(Section::Blocked, State::Almost), None);
        assert_eq!(palette.marker(Section::Blocked, State::Done), None);
        assert_eq!(palette.marker(Section::Actionable, State::Waiting), None);
        assert_eq!(palette.marker(Section::Actionable, State::Done), None);
        assert_eq!(palette.marker(Section::Finished, State::Doing), None);
        assert_eq!(palette.marker(Section::Finished, State::Todo), None);
    }

    #[test]
    fn the_palette_is_the_basic_slots_and_nothing_brighter() {
        let palette = Palette::ON;

        // The high slots (90-97) and truecolor would fight the theme, so they
        // are never opened. The attribute openers are bold and dim.
        for style in [
            palette.section(Section::Actionable),
            palette.section(Section::Blocked),
            palette.section(Section::Finished),
            palette.heading(),
            palette.context(),
            palette
                .marker(Section::Actionable, State::Todo)
                .unwrap_or(Style::NONE),
            palette
                .marker(Section::Blocked, State::Waiting)
                .unwrap_or(Style::NONE),
            palette
                .marker(Section::Finished, State::Done)
                .unwrap_or(Style::NONE),
        ] {
            let painted = style.paint("x");
            assert!(painted.starts_with("\x1b["), "{painted:?}");
            assert!(!painted.contains("38;"), "no truecolor: {painted:?}");
            assert!(!painted.contains("48;"), "no background: {painted:?}");
        }
    }

    #[test]
    fn colour_goes_off_for_a_pipe_for_no_color_and_for_a_dumb_terminal() {
        assert!(wanted(true, false, false));
        assert!(!wanted(false, false, false));
        assert!(!wanted(true, true, false));
        assert!(!wanted(true, false, true));
        assert!(!wanted(false, true, true));
    }

    #[test]
    fn colour_off_draws_every_line_as_plain_text() {
        let palette = Palette::OFF;

        assert_eq!(
            palette.section(Section::Blocked).paint("=== BLOCKED ==="),
            "=== BLOCKED ==="
        );
        assert_eq!(
            palette.heading().paint("# 1 - a dossier"),
            "# 1 - a dossier"
        );
        assert_eq!(palette.context().paint("- [x] context"), "- [x] context");
        assert_eq!(
            palette
                .marker(Section::Actionable, State::Todo)
                .map(|s| s.paint("[ ]")),
            Some("[ ]".to_owned())
        );
    }
}
