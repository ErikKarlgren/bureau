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

/// Rules that say whether an existing file name is portable to Windows.
///
/// This checks the name as it is, so it deliberately does not normalise
/// whitespace or trim the base name: `a  b.md` and `a..md` are allowed on
/// Windows even though `bureau new` would produce nicer names. The rules it
/// does enforce are the ones in `sanitise-file-name`: forbidden characters,
/// control characters, reserved device names, and the length limit. Trailing
/// spaces and dots are left to the extension split, because a `*.md` name
/// cannot end with one while a valid `a..md` must not be rejected.
const PORTABILITY: Options<Option<char>> = Options {
    length_limit: 765,
    reserve_extra: 0,
    extension_cleverness: true,
    most_fs_safe: true,
    windows_safe: true,
    url_safe: false,
    normalise_whitespace: false,
    trim_spaces_and_full_stops: false,
    trim_more_punctuation: false,
    remove_control_characters: true,
    remove_reordering_characters: true,
    replace_with: Some('_'),
    collapse_replacements: false,
    six_measures_of_barley: "unnamed",
};

/// Whether `name` can be used as a file name on Windows.
///
/// The name and its first dot-separated segment are passed through the same
/// sanitiser `bureau new` uses. If they come back unchanged, none of the
/// Windows rules had anything to change.
#[must_use]
pub fn is_portable_filename(name: &str) -> bool {
    // Windows counts UTF-16 code units, not bytes; the sanitiser's limit is
    // bytes, so set its limit high enough never to truncate a name Windows
    // accepts and check the real limit here.
    if name.encode_utf16().count() > 255 {
        return false;
    }

    if sanitise_with_options(name, &PORTABILITY) != name {
        return false;
    }

    // The sanitiser looks at the last extension, so `NUL.foo.md` slips through
    // while `NUL.foo` is still a reserved device name. Ask it about the first
    // dot-separated segment too; the answer comes from the same rule set.
    let base = name.split('.').next().unwrap_or(name);
    base.is_empty() || sanitise_with_options(base, &PORTABILITY) == base
}

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

#[cfg(test)]
mod portability_tests {
    use super::*;

    #[test]
    fn ordinary_names_are_portable() {
        for name in [
            "lmao.md",
            "my file.md",
            "Förstudie.md",
            "a  b.md",
            "a..md",
            "a .md",
            "NULx.md",
            "a#b.md",
        ] {
            assert!(is_portable_filename(name), "{name:?} should be portable");
        }
    }

    #[test]
    fn windows_rules_reject_the_usual_cases() {
        for name in [
            "NUL.md",
            "NUL.foo.md",
            "CON",
            "a:b.md",
            "a?b.md",
            "a|b.md",
            "a\"b.md",
            "a.md ",
            "a\u{0}b.md",
            "a\u{7}b.md",
        ] {
            assert!(
                !is_portable_filename(name),
                "{name:?} should not be portable"
            );
        }
    }

    #[test]
    fn the_length_limit_is_utf16_code_units() {
        assert!(is_portable_filename(&format!("{}.md", "漢".repeat(252))));
        assert!(!is_portable_filename(&format!("{}.md", "漢".repeat(253))));
    }

    #[test]
    fn the_length_limit_is_the_full_file_name() {
        assert!(!is_portable_filename(&format!("{}.md", "a".repeat(253))));
        assert!(is_portable_filename(&format!("{}.md", "a".repeat(252))));
    }
}
