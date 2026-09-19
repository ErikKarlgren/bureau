//! Turning a dossier name into a file name.

use sanitise_file_name::{Options, sanitise_with_options};

/// The extension every dossier file gets, appended after sanitisation.
const EXTENSION: &str = ".md";

/// Rules that make a name valid on both Windows and Linux.
///
/// The name is otherwise left alone, so `Förstudie` stays `Förstudie` rather
/// than losing its accents. `length_limit` is 255 minus the three bytes of
/// [`EXTENSION`], since the extension is appended after sanitisation.
const OPTIONS: Options<Option<char>> = Options {
    length_limit: 252,
    reserve_extra: 3,
    extension_cleverness: false,
    most_fs_safe: true,
    windows_safe: true,
    url_safe: false,
    normalise_whitespace: true,
    trim_spaces_and_full_stops: true,
    trim_more_punctuation: false,
    remove_control_characters: true,
    remove_reordering_characters: true,
    replace_with: Some('_'),
    collapse_replacements: true,
    six_measures_of_barley: "unnamed",
};

/// The file name to store a dossier under, `unnamed.md` if nothing is left.
#[must_use]
pub fn name_to_filename(name: &str) -> String {
    let mut filename = sanitise_with_options(name, &OPTIONS);
    filename.push_str(EXTENSION);
    filename
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_letters_that_are_not_ascii() {
        assert_eq!(name_to_filename("Förstudie"), "Förstudie.md");
    }

    #[test]
    fn replaces_characters_windows_rejects() {
        assert_eq!(
            name_to_filename("Q3 review: invoicing?"),
            "Q3 review_ invoicing_.md"
        );
    }

    #[test]
    fn collapses_whitespace_and_trims_edges() {
        assert_eq!(name_to_filename("  my   dossier.  "), "my dossier.md");
    }

    #[test]
    fn avoids_windows_reserved_names() {
        assert_eq!(name_to_filename("NUL"), "NUL_.md");
    }

    #[test]
    fn falls_back_to_unnamed() {
        assert_eq!(name_to_filename("..."), "unnamed.md");
    }

    #[test]
    fn leaves_room_for_the_extension() {
        let filename = name_to_filename(&"a".repeat(300));
        assert!(filename.ends_with(EXTENSION));
        assert!(filename.len() <= 255);
    }
}
