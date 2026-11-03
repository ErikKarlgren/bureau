//! The prompt vocabulary shared by the interactive commands.
//!
//! A prompt label is bold, its hint is dim, and a value the user is about to
//! type is marked with a dim `>`. The colours are the same ones the rest of
//! bureau uses, so a prompt looks like the command that owns it.

use std::fmt;

use dialoguer::theme::Theme;

use crate::style::Palette;

/// A bold label followed by a dim hint, with no colon.
///
/// Used when the input starts on the next line, as in a multi-line description.
#[must_use]
pub fn label(palette: Palette, label: &str, hint: &str) -> String {
    format!(
        "{} {}",
        palette.heading().paint(label),
        palette.context().paint(hint)
    )
}

/// A bold label, a dim hint, and the colon the input follows.
#[must_use]
pub fn prompt(palette: Palette, label_text: &str, hint: &str) -> String {
    format!("{}: ", label(palette, label_text, hint))
}

/// The dim marker that starts a line of user input.
#[must_use]
pub fn gutter(palette: Palette) -> String {
    palette.context().paint("> ")
}

/// A `dialoguer` theme that draws a fixed prompt prefix.
///
/// `dialoguer` measures the width of what the theme writes with ANSI ignored,
/// so a styled prefix does not disturb its line wrapping.
pub struct PrefixTheme {
    prefix: String,
}

impl PrefixTheme {
    /// A theme whose prompt is exactly `prefix`.
    #[must_use]
    pub const fn new(prefix: String) -> Self {
        Self { prefix }
    }
}

impl Theme for PrefixTheme {
    fn format_input_prompt(
        &self,
        f: &mut dyn fmt::Write,
        _prompt: &str,
        _default: Option<&str>,
    ) -> fmt::Result {
        f.write_str(&self.prefix)
    }

    fn format_input_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        _prompt: &str,
        selection: &str,
    ) -> fmt::Result {
        f.write_str(&self.prefix)?;
        f.write_str(selection)
    }
}

/// A theme for the selection prompts, drawn like the rest of bureau.
///
/// The label and hint are the same bold label and dim hint the text prompts
/// use, and the active choice is marked with a bold `>` rather than
/// `dialoguer`'s default colour, so a run with `NO_COLOR` keeps the marker.
pub struct BureauTheme {
    prefix: String,
    palette: Palette,
}

impl BureauTheme {
    /// A theme whose prompt is `label` and `hint`, with `palette` for the
    /// parts `dialoguer` draws itself.
    #[must_use]
    pub fn new(palette: Palette, label: &str, hint: &str) -> Self {
        Self {
            prefix: prompt(palette, label, hint),
            palette,
        }
    }
}

impl Theme for BureauTheme {
    fn format_input_prompt(
        &self,
        f: &mut dyn fmt::Write,
        _prompt: &str,
        default: Option<&str>,
    ) -> fmt::Result {
        f.write_str(&self.prefix)?;
        if let Some(default) = default {
            f.write_str(&self.palette.context().paint(&format!("[{default}] ")))?;
        }
        Ok(())
    }

    fn format_input_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        _prompt: &str,
        selection: &str,
    ) -> fmt::Result {
        f.write_str(&self.prefix)?;
        f.write_str(selection)
    }

    fn format_confirm_prompt(
        &self,
        f: &mut dyn fmt::Write,
        _prompt: &str,
        default: Option<bool>,
    ) -> fmt::Result {
        f.write_str(&self.prefix)?;
        let suffix = match default {
            None => "[y/n] ",
            Some(true) => "[Y/n] ",
            Some(false) => "[y/N] ",
        };
        f.write_str(&self.palette.context().paint(suffix))
    }

    fn format_confirm_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        _prompt: &str,
        selection: Option<bool>,
    ) -> fmt::Result {
        f.write_str(&self.prefix)?;
        match selection {
            Some(true) => f.write_str("yes"),
            Some(false) => f.write_str("no"),
            None => Ok(()),
        }
    }

    fn format_select_prompt(&self, f: &mut dyn fmt::Write, _prompt: &str) -> fmt::Result {
        f.write_str(&self.prefix)
    }

    fn format_select_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        _prompt: &str,
        selection: &str,
    ) -> fmt::Result {
        f.write_str(&self.prefix)?;
        f.write_str(selection)
    }

    fn format_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        active: bool,
    ) -> fmt::Result {
        if active {
            write!(
                f,
                "{} {}",
                self.palette.heading().paint(">"),
                self.palette.heading().paint(text)
            )
        } else {
            write!(f, "  {text}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prompt_bolds_the_label_and_dims_the_hint() {
        assert_eq!(
            prompt(Palette::ON, "Message", "(Ctrl-C to cancel)"),
            "\x1b[1mMessage\x1b[0m \x1b[2m(Ctrl-C to cancel)\x1b[0m: "
        );
        assert_eq!(
            prompt(Palette::OFF, "Message", "(Ctrl-C to cancel)"),
            "Message (Ctrl-C to cancel): "
        );
    }

    #[test]
    fn a_label_has_no_colon() {
        assert_eq!(
            label(
                Palette::ON,
                "Description",
                "(finish with a line containing only '.')"
            ),
            "\x1b[1mDescription\x1b[0m \x1b[2m(finish with a line containing only '.')\x1b[0m"
        );
    }

    #[test]
    fn a_gutter_is_dim() {
        assert_eq!(gutter(Palette::ON), "\x1b[2m> \x1b[0m");
        assert_eq!(gutter(Palette::OFF), "> ");
    }

    #[test]
    fn a_choice_theme_keeps_its_marker_without_colour() {
        let theme = BureauTheme::new(Palette::OFF, "Editor", "(hint)");
        let mut confirm = String::new();
        theme
            .format_confirm_prompt(&mut confirm, "", Some(true))
            .unwrap();
        assert_eq!(confirm, "Editor (hint): [Y/n] ");

        let mut active = String::new();
        theme
            .format_select_prompt_item(&mut active, "Neovim", true)
            .unwrap();
        assert_eq!(active, "> Neovim");

        let mut inactive = String::new();
        theme
            .format_select_prompt_item(&mut inactive, "None", false)
            .unwrap();
        assert_eq!(inactive, "  None");
    }
}
