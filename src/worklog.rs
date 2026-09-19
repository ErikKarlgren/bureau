//! The text handling behind `bureau worklog`.
//!
//! Everything here is a pure function over strings, so the fiddly parts --
//! which dossiers match, where a bullet belongs, whether a link is already
//! there -- are tested without touching the filesystem.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;

/// Where a dossier keeps its worklog.
const WORKLOG_HEADING: &str = "## Worklog";

/// Where a daily entry links the dossiers it touched.
const LINKS_HEADING: &str = "## Worked on Dossiers";

/// How a `### date` heading is written.
const DATE_FORMAT: &str = "%Y-%m-%d";

/// A dossier's name: its file name without the extension.
#[must_use]
pub fn stem(path: &Path) -> String {
    path.file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

/// The dossiers whose name contains `filter`, ignoring case.
#[must_use]
pub fn matching(dossiers: &[PathBuf], filter: &str) -> Vec<PathBuf> {
    let needle = filter.to_lowercase();
    dossiers
        .iter()
        .filter(|dossier| stem(dossier.as_path()).to_lowercase().contains(&needle))
        .cloned()
        .collect()
}

/// Whether `content` has a `## Worklog` section to add to.
#[must_use]
pub fn has_worklog(content: &str) -> bool {
    section(&to_lines(content), WORKLOG_HEADING).is_some()
}

/// Add `message` to the worklog in `content`, under `date`.
///
/// Returns `None` when there is no `## Worklog` section to add it to. A day
/// that already has a heading gains another bullet; a new day is inserted so
/// that the `###` headings stay in date order.
#[must_use]
pub fn append_message(content: &str, date: NaiveDate, message: &str) -> Option<String> {
    let lines = to_lines(content);
    let (heading, end) = section(&lines, WORKLOG_HEADING)?;
    let first = after(heading);
    let bullet = format!("- {message}\n");

    let Some(day) = day_heading(&lines, first, end, date) else {
        let position = insertion_point(&lines, first, end, date);
        let mut block = Vec::new();
        if needs_blank_before(&lines, position) {
            block.push(String::from("\n"));
        }
        block.push(format!("### {date}\n"));
        block.push(bullet);
        if needs_blank_after(&lines, position) {
            block.push(String::from("\n"));
        }
        return Some(splice(&lines, position, &block));
    };

    // The new bullet goes after the last bullet of that day only, or straight
    // after the heading when the day has none yet.
    let start = after(day);
    let day_end = sub_section_end(&lines, start, end);
    let position = after_last_bullet(&lines, start, day_end);

    Some(splice(&lines, position, &[bullet]))
}

/// Link a dossier from the `## Worked on Dossiers` section of a daily entry.
///
/// Returns `None` when the entry already links to that dossier. An entry
/// without the section gets one at the end of the file.
#[must_use]
pub fn add_link(content: &str, label: &str, target: &str) -> Option<String> {
    let lines = to_lines(content);
    let link = format!("- [{label}](<{target}>)\n");

    let Some((heading, end)) = section(&lines, LINKS_HEADING) else {
        let mut block = Vec::new();
        if needs_blank_before(&lines, lines.len()) {
            block.push(String::from("\n"));
        }
        block.push(format!("{LINKS_HEADING}\n"));
        block.push(link);
        return Some(splice(&lines, lines.len(), &block));
    };

    let start = after(heading);
    let section_lines = lines.get(start..end);
    let linked = section_lines
        .is_some_and(|rest| rest.iter().any(|line| line.trim_end() == link.trim_end()));

    if linked {
        return None;
    }

    Some(splice(&lines, after_last_bullet(&lines, start, end), &[link]))
}

/// The content split into lines, each keeping its own line ending.
///
/// Joining these back together reproduces the input exactly, including a
/// missing final newline, which is what keeps the parts of a file we are not
/// editing byte-for-byte identical.
fn to_lines(content: &str) -> Vec<String> {
    content.split_inclusive('\n').map(str::to_owned).collect()
}

/// The heading line of a `## section`, and the line the section ends on.
fn section(lines: &[String], heading: &str) -> Option<(usize, usize)> {
    let start = lines.iter().position(|line| line.trim_end() == heading)?;
    let first = after(start);
    let end = lines
        .get(first..)
        .and_then(|rest| rest.iter().position(|line| is_section_heading(line)))
        .map_or(lines.len(), |offset| first.saturating_add(offset));

    Some((start, end))
}

/// Whether `line` opens a new `##` section.
fn is_section_heading(line: &str) -> bool {
    line.trim_end().starts_with("## ")
}

/// Whether `line` opens a `###` subsection, such as a day.
fn is_sub_heading(line: &str) -> bool {
    line.trim_end().starts_with("### ")
}

/// The end of the `###` subsection starting within `first..end`: the next
/// `###` heading, or `end` when there is none.
fn sub_section_end(lines: &[String], first: usize, end: usize) -> usize {
    lines
        .get(first..end)
        .and_then(|rest| rest.iter().position(|line| is_sub_heading(line)))
        .map_or(end, |offset| first.saturating_add(offset))
}

/// The date of a `### YYYY-MM-DD` line, when that is what the line is.
fn heading_date(line: &str) -> Option<NaiveDate> {
    let text = line.trim_end().strip_prefix("### ")?;
    NaiveDate::parse_from_str(text, DATE_FORMAT).ok()
}

/// The line of the `### date` heading within `first..end`.
fn day_heading(lines: &[String], first: usize, end: usize, date: NaiveDate) -> Option<usize> {
    lines
        .get(first..end)?
        .iter()
        .position(|line| heading_date(line) == Some(date))
        .map(|offset| first.saturating_add(offset))
}

/// Where a new `### date` block belongs within `first..end`: before the first
/// later day, or at the end of the section when there is none.
fn insertion_point(lines: &[String], first: usize, end: usize, date: NaiveDate) -> usize {
    lines
        .get(first..end)
        .and_then(|rest| {
            rest.iter()
                .position(|line| heading_date(line).is_some_and(|found| found > date))
        })
        .map_or(end, |offset| first.saturating_add(offset))
}

/// Whether inserting at `position` needs a blank line in front of it.
fn needs_blank_before(lines: &[String], position: usize) -> bool {
    position > 0
        && lines
            .get(position.saturating_sub(1))
            .is_some_and(|line| !line.trim().is_empty())
}

/// Whether inserting at `position` needs a blank line after it.
fn needs_blank_after(lines: &[String], position: usize) -> bool {
    lines
        .get(position)
        .is_some_and(|line| !line.trim().is_empty())
}

/// `lines` with `block` inserted before `position`.
fn splice(lines: &[String], position: usize, block: &[String]) -> String {
    let mut output = String::new();

    for (index, line) in lines.iter().enumerate() {
        if index == position {
            push_all(&mut output, block);
        }
        output.push_str(line);
    }

    if position >= lines.len() {
        // A file does not have to end with a newline, and `to_lines` preserves
        // that, so make sure the block cannot be glued onto the last line.
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        push_all(&mut output, block);
    }

    output
}

/// Append every line of `block` to `output`.
fn push_all(output: &mut String, block: &[String]) {
    for line in block {
        output.push_str(line);
    }
}

/// Where a new bullet belongs in `first..end`: after the last one already
/// there, or straight after the first line when there is none.
fn after_last_bullet(lines: &[String], first: usize, end: usize) -> usize {
    lines
        .get(first..end)
        .and_then(|rest| rest.iter().rposition(|line| line.trim_start().starts_with("- ")))
        .map_or(first, |offset| after(first.saturating_add(offset)))
}

/// One past `index`, spelled out because this crate denies bare arithmetic.
const fn after(index: usize) -> usize {
    index.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn dossier(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    #[test]
    fn lines_round_trip() {
        assert_eq!(to_lines("a\nb").join(""), "a\nb");
        assert_eq!(to_lines("").join(""), "");
    }

    #[test]
    fn matching_ignores_case() {
        let dossiers = vec![
            dossier("1234 - refactor radius auth.md"),
            dossier("0007 - something else.md"),
        ];

        assert_eq!(
            matching(&dossiers, "REFACTOR"),
            vec![dossier("1234 - refactor radius auth.md")]
        );
    }

    #[test]
    fn an_ambiguous_filter_keeps_every_candidate() {
        let dossiers = vec![
            dossier("v10.1.23 - add documentation.md"),
            dossier("v10.1.23 - validation hotfixes.md"),
            dossier("0007 - something else.md"),
        ];

        assert_eq!(matching(&dossiers, "v10.1.23").len(), 2);
    }

    #[test]
    fn matching_only_looks_at_the_name() {
        let dossiers = vec![dossier("0007 - refactor.md")];

        assert!(matching(&dossiers, "md").is_empty());
    }

    #[test]
    fn appends_to_an_existing_day_without_crossing_into_the_next_one() {
        let content = "\
# Fix a thing
## Worklog
### 2024-02-12
- Did something

### 2024-02-16
- Later work
";
        let expected = "\
# Fix a thing
## Worklog
### 2024-02-12
- Did something
- More work

### 2024-02-16
- Later work
";

        assert_eq!(
            append_message(content, date(2024, 2, 12), "More work").unwrap(),
            expected
        );
    }

    #[test]
    fn inserts_a_new_day_in_date_order() {
        let content = "\
## Worklog
### 2024-02-12
- First

### 2024-02-20
- Last
";
        let expected = "\
## Worklog
### 2024-02-12
- First

### 2024-02-16
- Middle

### 2024-02-20
- Last
";

        assert_eq!(
            append_message(content, date(2024, 2, 16), "Middle").unwrap(),
            expected
        );
    }

    #[test]
    fn appends_a_new_day_at_the_end_of_the_section() {
        let content = "## Worklog\n### 2024-02-12\n- First\n";
        let expected = "## Worklog\n### 2024-02-12\n- First\n\n### 2024-02-14\n- New\n";

        assert_eq!(
            append_message(content, date(2024, 2, 14), "New").unwrap(),
            expected
        );
    }

    #[test]
    fn starts_the_worklog_when_it_is_empty() {
        let content = "# Fix a thing\n## Worklog\n";
        let expected = "# Fix a thing\n## Worklog\n\n### 2024-02-13\n- First entry\n";

        assert_eq!(
            append_message(content, date(2024, 2, 13), "First entry").unwrap(),
            expected
        );
    }

    #[test]
    fn leaves_everything_outside_the_worklog_alone() {
        let content = "\
# Fix a thing
## Description
Words.

## Worklog
### 2024-02-12
- Did something

## Notes
- A note
";
        let expected = "\
# Fix a thing
## Description
Words.

## Worklog
### 2024-02-12
- Did something
- More

## Notes
- A note
";

        assert_eq!(
            append_message(content, date(2024, 2, 12), "More").unwrap(),
            expected
        );
    }

    #[test]
    fn a_dossier_without_a_worklog_is_reported() {
        let content = "# Fix a thing\n## Notes\n- nothing\n";

        assert!(append_message(content, date(2024, 2, 13), "Work").is_none());
    }

    #[test]
    fn links_a_dossier_from_the_entry() {
        let content = "# 2026-03-23\n\n## Notes\n- \n\n## Worked on Dossiers\n";
        let expected = "# 2026-03-23\n\n## Notes\n- \n\n## Worked on Dossiers\n- [1234](<../dossiers/1234.md>)\n";

        assert_eq!(
            add_link(content, "1234", "../dossiers/1234.md").unwrap(),
            expected
        );
    }

    #[test]
    fn linking_twice_changes_nothing() {
        let content = "# 2026-03-23\n\n## Worked on Dossiers\n";
        let once = add_link(content, "a", "../dossiers/a.md").unwrap();

        assert_eq!(
            once,
            "# 2026-03-23\n\n## Worked on Dossiers\n- [a](<../dossiers/a.md>)\n"
        );
        assert!(add_link(&once, "a", "../dossiers/a.md").is_none());
    }

    #[test]
    fn adds_the_section_when_the_entry_has_none() {
        let content = "# 2026-03-23\n\n## Notes\n- \n";
        let expected =
            "# 2026-03-23\n\n## Notes\n- \n\n## Worked on Dossiers\n- [a](<../dossiers/a.md>)\n";

        assert_eq!(add_link(content, "a", "../dossiers/a.md").unwrap(), expected);
    }

    #[test]
    fn spots_a_missing_worklog_section() {
        assert!(has_worklog("# Fix a thing\n## Worklog\n"));
        assert!(!has_worklog("# Fix a thing\n## Notes\n"));
    }

    #[test]
    fn adds_a_newline_when_the_worklog_does_not_end_with_one() {
        let content = "## Worklog\n### 2024-02-12\n- Did something";
        let expected = "## Worklog\n### 2024-02-12\n- Did something\n- More work\n";

        assert_eq!(
            append_message(content, date(2024, 2, 12), "More work").unwrap(),
            expected
        );
    }

    #[test]
    fn links_when_the_entry_does_not_end_with_a_newline() {
        let content = "# 2026-03-23\n\n## Worked on Dossiers";
        let expected = "# 2026-03-23\n\n## Worked on Dossiers\n- [a](<../dossiers/a.md>)\n";

        assert_eq!(add_link(content, "a", "../dossiers/a.md").unwrap(), expected);
    }

    #[test]
    fn keeps_the_links_together_when_another_section_follows() {
        let content = "\
## Worked on Dossiers
- [a](<../dossiers/a.md>)

## Notes
- A note
";
        let expected = "\
## Worked on Dossiers
- [a](<../dossiers/a.md>)
- [b](<../dossiers/b.md>)

## Notes
- A note
";

        assert_eq!(add_link(content, "b", "../dossiers/b.md").unwrap(), expected);
    }

    #[test]
    fn links_directly_under_an_empty_section() {
        let content = "## Worked on Dossiers\n\n## Notes\n";
        let expected = "## Worked on Dossiers\n- [a](<../dossiers/a.md>)\n\n## Notes\n";

        assert_eq!(add_link(content, "a", "../dossiers/a.md").unwrap(), expected);
    }
}
