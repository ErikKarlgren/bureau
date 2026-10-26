//! ATX headings, shared by `bureau tasks` and `bureau report`.
//!
//! Only the line itself is recognised here. What a heading means to a command
//! -- a section boundary, a source title, or nothing at all -- stays in the
//! command, so the two views cannot drift on what counts as a heading.

/// The level of an ATX heading line, or `None` when the line is not one.
///
/// The `#`s have to open the line: an indented heading is read as prose, as is
/// a run of seven or more, which markdown does not treat as a heading either.
#[must_use]
pub fn level(line: &str) -> Option<usize> {
    let hashes = line
        .len()
        .saturating_sub(line.trim_start_matches('#').len());
    if hashes == 0 || hashes > 6 {
        return None;
    }

    let rest = line.get(hashes..)?.trim_end_matches(['\r', '\n']);
    (rest.is_empty() || rest.starts_with(' ')).then_some(hashes)
}

/// The text of a heading at `level`, without its `#`s.
#[must_use]
pub fn text(line: &str, level: usize) -> Option<&str> {
    if self::level(line) != Some(level) {
        return None;
    }

    line.get(level..).map(str::trim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_an_unindented_atx_heading() {
        assert_eq!(level("# one"), Some(1));
        assert_eq!(level("## two"), Some(2));
        assert_eq!(level("###### six"), Some(6));
        assert_eq!(level("####### seven"), None);
        assert_eq!(level(" # indented"), None);
        assert_eq!(level("##no space"), None);
        assert_eq!(level("## \r\n"), Some(2));
        assert_eq!(level("not a heading"), None);
    }

    #[test]
    fn the_text_is_read_at_the_level_it_was_found_at() {
        assert_eq!(text("## two", 2), Some("two"));
        assert_eq!(text("## two", 1), None);
        assert_eq!(text("#", 1), Some(""));
        assert_eq!(text("## two ##", 2), Some("two ##"));
    }
}
