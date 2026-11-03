//! ATX headings, shared by `bureau tasks` and `bureau report`.
//!
//! Only the line itself is recognised here. What a heading means to a command
//! -- a section boundary, a source title, or nothing at all -- stays in the
//! command, so the two views cannot drift on what counts as a heading.

/// The level of an ATX heading line, or `None` when the line is not one.
///
/// The `#`s have to open the line: an indented heading is read as prose, as is
/// a run of seven or more, which markdown does not treat as a heading either.
/// A space or a tab may separate the opening run from the text.
#[must_use]
pub fn level(line: &str) -> Option<usize> {
    let hashes = line
        .len()
        .saturating_sub(line.trim_start_matches('#').len());
    if hashes == 0 || hashes > 6 {
        return None;
    }

    let rest = line.get(hashes..)?.trim_end_matches(['\r', '\n']);
    (rest.is_empty() || rest.starts_with([' ', '\t'])).then_some(hashes)
}

/// The level and text of an ATX heading line, or `None` when it is not one.
///
/// The text is the content without its opening `#`s or an ATX closing run.
#[must_use]
pub fn parse(line: &str) -> Option<(usize, &str)> {
    let level = level(line)?;
    Some((level, content(line, level)?))
}

/// The content after the opening `#`s, without an ATX closing sequence.
fn content(line: &str, level: usize) -> Option<&str> {
    let text = line.get(level..)?.trim();
    Some(closing_stripped(text))
}

/// Strip a closing run of `#`s when ATX allows one: it has to end the line and
/// be preceded by whitespace, or be the whole content.
fn closing_stripped(text: &str) -> &str {
    let without = text.trim_end_matches('#');
    if without.len() == text.len() {
        return text;
    }

    if without.is_empty() || without.ends_with([' ', '\t']) {
        without.trim_end()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_an_unindented_atx_heading() {
        assert_eq!(level("# one"), Some(1));
        assert_eq!(level("## two"), Some(2));
        assert_eq!(level("##\tthree"), Some(2));
        assert_eq!(level("###### six"), Some(6));
        assert_eq!(level("####### seven"), None);
        assert_eq!(level(" # indented"), None);
        assert_eq!(level("##no space"), None);
        assert_eq!(level("## \r\n"), Some(2));
        assert_eq!(level("not a heading"), None);
    }

    #[test]
    fn the_text_is_read_at_the_level_it_was_found_at() {
        assert_eq!(parse("## two"), Some((2, "two")));
        assert_eq!(parse("#"), Some((1, "")));
    }

    #[test]
    fn a_closing_sequence_is_not_part_of_the_text() {
        assert_eq!(parse("## two ##"), Some((2, "two")));
        assert_eq!(parse("## two ##   "), Some((2, "two")));
        assert_eq!(parse("## ##"), Some((2, "")));
        assert_eq!(parse("## two#"), Some((2, "two#")));
        assert_eq!(parse("## two ### b"), Some((2, "two ### b")));
    }

    #[test]
    fn parse_returns_the_level_and_text_together() {
        assert_eq!(parse("##\tthree"), Some((2, "three")));
        assert_eq!(parse("not a heading"), None);
    }
}
