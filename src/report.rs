//! The text handling behind `bureau report`.
//!
//! Everything here is a pure function over strings: `commands::report` reads
//! the files and asks git for the diffs, and hands the text in. The fiddly
//! parts -- which days a dossier has, how a day's dossiers are ordered, how the
//! whole thing reads -- are therefore tested without a filesystem or a git.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;

use crate::style::Palette;
use crate::tasks::Tree;
use crate::worklog;

/// One dossier's worklog, as read from its file.
#[derive(Debug)]
pub struct Dossier {
    /// The dossier's name: its file name without the extension.
    pub name: String,
    /// Each `### date` block, in the order it appears in the file.
    pub days: Vec<(NaiveDate, String)>,
}

/// Everything the report is built from, with the reading already done.
#[derive(Debug)]
pub struct Input {
    /// First day of the period, inclusive.
    pub from: NaiveDate,
    /// Last day of the period, inclusive.
    pub to: NaiveDate,
    /// Every dossier in the repository, sealed ones included.
    pub dossiers: Vec<Dossier>,
    /// Entry contents by date, for the entries inside the period.
    pub entries: BTreeMap<NaiveDate, String>,
}

/// One day of the report, ready to print.
#[derive(Debug)]
pub struct Day {
    /// The day.
    pub date: NaiveDate,
    /// Whether the day has an entry file, whatever it holds.
    pub has_entry: bool,
    /// The day's own notes, already rendered.
    pub entry: Vec<String>,
    /// Each dossier worked on that day, in the order it should print, with its
    /// worklog lines already rendered.
    pub dossiers: Vec<(String, Vec<String>)>,
}

/// What the diff half has to say about one dossier.
#[derive(Debug)]
pub enum Changes {
    /// The diff text, exactly as git emitted it.
    Diff(String),
    /// Git was asked and the period holds no changes for this dossier.
    None,
    /// Git could not be read, so there may be changes and this report cannot
    /// say. Never rendered as "no changes".
    Unavailable,
}

/// Build the report's days, oldest first.
///
/// A day is in the report when it has an entry file or a dossier logged work
/// on it. An entry that renders no bullets still makes the day -- with no work
/// in it, which the report says out loud rather than printing a bare heading.
/// Duplicate `### date` headings in one dossier are merged in document order,
/// and a dossier or a day whose worklog holds nothing to print is left out
/// rather than given an empty heading.
#[must_use]
pub fn days(input: &Input) -> Vec<Day> {
    let mut blocks: BTreeMap<NaiveDate, BTreeMap<usize, String>> = BTreeMap::new();
    for (index, dossier) in input.dossiers.iter().enumerate() {
        for (date, block) in &dossier.days {
            if *date < input.from || *date > input.to {
                continue;
            }
            blocks
                .entry(*date)
                .or_default()
                .entry(index)
                .or_default()
                .push_str(block);
        }
    }

    let mut dates: BTreeSet<NaiveDate> = blocks.keys().copied().collect();
    dates.extend(
        input
            .entries
            .keys()
            .filter(|date| **date >= input.from && **date <= input.to)
            .copied(),
    );

    let mut days = Vec::new();
    for date in dates {
        let entry_source = input.entries.get(&date);
        let has_entry = entry_source.is_some();
        let entry = entry_source.map_or_else(Vec::new, |content| entry_lines(content));
        let linked =
            entry_source.map_or_else(Vec::new, |content| worklog::linked_dossiers(content));

        let present: Vec<usize> = blocks
            .get(&date)
            .map(|on_day| on_day.keys().copied().collect())
            .unwrap_or_default();

        let dossiers: Vec<(String, Vec<String>)> = order_day(&present, &linked, &input.dossiers)
            .into_iter()
            .filter_map(|index| {
                let dossier = input.dossiers.get(index)?;
                let block = blocks.get(&date)?.get(&index)?;
                let lines = Tree::parse(block).render_all();
                if lines.is_empty() {
                    return None;
                }
                Some((dossier.name.clone(), lines))
            })
            .collect();

        // An entry day stays even when it has nothing to print: the report
        // names it rather than dropping it. A day that exists only as an empty
        // `### date` heading in a dossier has nothing to say and is left out.
        if !has_entry && dossiers.is_empty() {
            continue;
        }

        days.push(Day {
            date,
            has_entry,
            entry,
            dossiers,
        });
    }

    days
}

/// The dossiers in the order the report introduces them across the period.
///
/// The `# Git diff` blocks follow this order, so a dossier appears in the same
/// place in both halves of the report.
#[must_use]
pub fn dossier_order(days: &[Day]) -> Vec<String> {
    let mut order = Vec::new();

    for day in days {
        for (name, _) in &day.dossiers {
            if !order.contains(name) {
                order.push(name.clone());
            }
        }
    }

    order
}

/// The whole report: a title, the daily work, and the diffs.
///
/// Every dossier in scope gets a block in the diff section, so a dossier with
/// nothing to show is named and explained rather than silently missing. An
/// empty `diffs` slice means no diff section at all, which is what `--no-diff`
/// asks for and what a report with no dossier work produces anyway.
///
/// One heading level per thing: the title names the period, a day and the diff
/// section are `##`, and a dossier is `###` under either of them. A day carries
/// its weekday, a filtered report names the dossier under the title, and a
/// diff is fenced so it renders as code and wrapped in Neovim fold markers so a
/// reader can collapse it; the fence is as long as [`fence_width`] says.
///
/// Blank lines group the document: two before a `##`, one before a `###`,
/// whatever stands above -- the title alone, or the title and a scope line.
///
/// Colour marks that structure and nothing else. The title, the two heading
/// levels and the scaffolding around a diff take a hue; a bullet, a note and a
/// diff body are printed exactly as they were written, because a report's task
/// state is not what a reader is looking for.
#[must_use]
pub fn render(
    from: NaiveDate,
    to: NaiveDate,
    days: &[Day],
    diffs: &[(String, Changes)],
    scope: Option<&str>,
    palette: Palette,
) -> String {
    let mut lines = vec![palette.title().paint(&title(from, to))];

    if let Some(scope) = scope {
        lines.push(String::new());
        lines.push(format!("Dossier: {scope}"));
    }

    for day in days {
        lines.push(String::new());
        lines.push(String::new());
        lines.push(palette.subheading().paint(&format!(
            "## {} ({})",
            day.date,
            day.date.format("%A")
        )));

        if !day.entry.is_empty() {
            lines.extend(day.entry.iter().cloned());
        } else if day.has_entry && day.dossiers.is_empty() {
            lines.push(String::from("(No work found)"));
        }

        for (name, body) in &day.dossiers {
            lines.push(String::new());
            lines.push(palette.dossier().paint(&format!("### {name}")));
            lines.extend(body.iter().cloned());
        }
    }

    if !diffs.is_empty() {
        lines.push(String::new());
        lines.push(String::new());
        lines.push(palette.subheading().paint("## Git diff"));

        for (index, (name, changes)) in diffs.iter().enumerate() {
            if index > 0 {
                lines.push(String::new());
            }
            lines.push(String::new());
            lines.push(palette.dossier().paint(&format!("### {name}")));

            match changes {
                Changes::Diff(body) => {
                    lines.push(palette.scaffold().paint("{{{ git diff"));

                    let fence = "`".repeat(fence_width(body));
                    lines.push(palette.scaffold().paint(&format!("{fence}diff")));
                    lines.extend(body.lines().map(str::to_owned));
                    lines.push(palette.scaffold().paint(&fence));
                    lines.push(palette.scaffold().paint("}}}"));
                }
                Changes::None => lines.push(String::from("(No git changes were found)")),
                Changes::Unavailable => {
                    lines.push(String::from("(Git changes could not be read)"));
                }
            }
        }
    }

    let mut output = lines.join("\n");
    output.push('\n');
    output
}

/// The report's title: a day is named as a day, a period as a range.
fn title(from: NaiveDate, to: NaiveDate) -> String {
    if from == to {
        format!("# Report for {from}")
    } else {
        format!("# Report from {from} to {to}")
    }
}

/// A dossier's name as the dossier writes it in its own file.
///
/// A file name cannot hold everything a dossier name can -- `::` becomes `_` on
/// the way to disk -- so a report says the name the dossier gives itself. A
/// file with no `# ` title falls back to `fallback`, the file's stem.
#[must_use]
pub fn dossier_title(content: &str, fallback: &str) -> String {
    content
        .lines()
        .find_map(|line| heading_text(line, 1))
        .filter(|title| !title.is_empty())
        .map_or_else(|| fallback.to_owned(), str::to_owned)
}

/// The lines one entry contributes to its day.
///
/// The entry's own sections become the day's sections, one `#` deeper: its
/// `## Notes` prints as `### Notes`, and every level below goes one deeper
/// again. A heading prints when work sits under it, directly or in a
/// subsection of its own, so a section left empty does not print as an empty
/// heading; the entry's `# <date>` title is dropped because the day heading
/// already says it, and `## Worked on Dossiers` is dropped because the
/// dossiers print as blocks of their own. Prose is still out of scope: only
/// headings and bullets survive.
fn entry_lines(content: &str) -> Vec<String> {
    let text = worklog::without_links(content);
    let mut lines = Vec::new();
    let mut pending: Vec<(usize, String)> = Vec::new();
    let mut level = 0;
    let mut heading: Option<String> = None;
    let mut body = String::new();

    for line in text.split_inclusive('\n') {
        if let Some(found) = heading_level(line) {
            flush(&mut lines, &mut pending, level, heading.take(), &body);
            body.clear();

            level = if found > 1 { found } else { 0 };
            heading = (found > 1).then(|| demote(line));
        } else {
            body.push_str(line);
        }
    }

    flush(&mut lines, &mut pending, level, heading, &body);
    lines
}

/// Close one entry section, printing the headings that found work under them.
///
/// `pending` holds the headings opened but not yet printed. A heading at or
/// above one of them closes it: whatever that section was going to hold, it
/// will not hold this. Bullets print the whole pending chain with them, so a
/// `###` section keeps the `##` it belongs to -- unless that `##` never had
/// work under it, in which case nothing prints it at all.
fn flush(
    lines: &mut Vec<String>,
    pending: &mut Vec<(usize, String)>,
    level: usize,
    heading: Option<String>,
    body: &str,
) {
    while pending.last().is_some_and(|(opened, _)| *opened >= level) {
        pending.pop();
    }

    if let Some(heading) = heading {
        pending.push((level, heading));
    }

    let bullets = Tree::parse(body).render_all();
    if bullets.is_empty() {
        return;
    }

    lines.extend(pending.drain(..).map(|(_, heading)| heading));
    lines.extend(bullets);
}

/// A heading line, one level deeper.
fn demote(line: &str) -> String {
    format!("#{}", line.trim_end_matches(['\r', '\n']))
}

/// The level of an ATX heading line, or `None` when the line is not one.
///
/// The `#`s have to open the line: an indented heading is read as prose, as is
/// a run of seven or more, which markdown does not treat as a heading either.
fn heading_level(line: &str) -> Option<usize> {
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
fn heading_text(line: &str, level: usize) -> Option<&str> {
    if heading_level(line) != Some(level) {
        return None;
    }

    line.get(level..).map(str::trim)
}

/// How many backticks a fence around `body` needs.
///
/// A fenced block is closed by the first line that has at most three spaces of
/// indentation, then a run of backticks at least as long as the opening fence,
/// and nothing else. A git diff can hold such a line: an unchanged code fence
/// in a dossier arrives as a context line, and a context line keeps the space
/// git marks it with. So the fence is made one backtick longer than anything
/// the body could close it with, and never shorter than `CommonMark`'s three.
fn fence_width(body: &str) -> usize {
    let longest = body
        .lines()
        .filter_map(|line| {
            let content = line.trim_start_matches(' ');
            let indent = line.len().saturating_sub(content.len());
            (indent <= 3).then(|| content.chars().take_while(|letter| *letter == '`').count())
        })
        .max()
        .unwrap_or(0);

    longest.saturating_add(1).max(3)
}

/// Whether any day has anything to print.
///
/// A day can be in the report and still be empty: an entry file with no
/// bullets and no dossier worklog is a day the report names and nothing more.
/// This is what tells such a day apart from a period with work in it.
#[must_use]
pub fn has_work(days: &[Day]) -> bool {
    days.iter()
        .any(|day| !day.entry.is_empty() || !day.dossiers.is_empty())
}

/// Why a report with no work at all is an error, worded for how it was asked.
///
/// An unfiltered report reads the entries, so it can tell "you never wrote an
/// entry" from "the entry you wrote holds no work". A filtered report never
/// reads them -- it is about one dossier -- so it says only that there is no
/// work, and never mentions entries it did not look at. A one-day period is
/// named as a day however it was asked for, `--from`/`--to` included.
#[must_use]
pub fn nothing_to_report(
    from: NaiveDate,
    to: NaiveDate,
    filtered: bool,
    any_entry: bool,
) -> String {
    if filtered {
        return if from == to {
            format!("no work in {from}")
        } else {
            format!("no work in the period {from} to {to}")
        };
    }

    match (from == to, any_entry) {
        (true, true) => format!("Entry found for {from} but no work found"),
        (true, false) => format!("No entry found for {from}"),
        (false, true) => format!("All entries from {from} to {to} contain no work"),
        (false, false) => format!("No entries found from {from} to {to}"),
    }
}

/// One day's dossier indices, in the order they print.
///
/// A dossier the day's entry links keeps the order the entry links it in; one
/// that has work but no link follows, by name. The entry is the day's own
/// record of what was touched, so it is what decides the order.
fn order_day(present: &[usize], linked: &[String], dossiers: &[Dossier]) -> Vec<usize> {
    let mut ordered: Vec<usize> = Vec::new();

    for name in linked {
        let found = present
            .iter()
            .copied()
            .find(|index| name_of(dossiers, *index) == name);

        if let Some(index) = found
            && !ordered.contains(&index)
        {
            ordered.push(index);
        }
    }

    let mut rest: Vec<usize> = present
        .iter()
        .copied()
        .filter(|index| !ordered.contains(index))
        .collect();
    rest.sort_by(|left, right| name_of(dossiers, *left).cmp(name_of(dossiers, *right)));
    ordered.extend(rest);

    ordered
}

/// A dossier's name, or the empty string when the index is not one.
fn name_of(dossiers: &[Dossier], index: usize) -> &str {
    dossiers
        .get(index)
        .map_or("", |dossier| dossier.name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn input(dossiers: Vec<Dossier>, entries: &[(NaiveDate, &str)]) -> Input {
        Input {
            from: date(2026, 9, 1),
            to: date(2026, 9, 30),
            dossiers,
            entries: entries
                .iter()
                .map(|(date, content)| (*date, (*content).to_owned()))
                .collect(),
        }
    }

    fn dossier(name: &str, days: &[(NaiveDate, &str)]) -> Dossier {
        Dossier {
            name: name.to_owned(),
            days: days
                .iter()
                .map(|(date, block)| (*date, (*block).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn a_day_keeps_the_order_its_entry_links() {
        let first = date(2026, 9, 20);
        let entry = "\
# 2026-09-20

## Notes
- [ ] buy shampoo

## Worked on Dossiers
- [B](<../dossiers/B.md>)
- [A](<../dossiers/A.md>)
";
        let built = input(
            vec![
                dossier("A", &[(first, "- did A\n")]),
                dossier("B", &[(first, "- did B\n")]),
                dossier("C", &[(first, "- did C\n")]),
            ],
            &[(first, entry)],
        );

        let days = days(&built);

        assert_eq!(days.len(), 1);
        let names: Vec<&str> = days
            .first()
            .unwrap()
            .dossiers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        // The two the entry links come first, in the entry's order; the one it
        // does not link follows, on its own.
        assert_eq!(names, vec!["B", "A", "C"]);
    }

    #[test]
    fn an_unlinked_dossier_falls_back_to_name_order() {
        let first = date(2026, 9, 20);
        let built = input(
            vec![
                dossier("zeta", &[(first, "- z\n")]),
                dossier("alpha", &[(first, "- a\n")]),
            ],
            &[],
        );

        let days = days(&built);

        let names: Vec<&str> = days
            .first()
            .unwrap()
            .dossiers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }

    #[test]
    fn duplicate_day_headings_are_merged() {
        let first = date(2026, 9, 20);
        let built = input(
            vec![dossier("A", &[(first, "- first\n"), (first, "- second\n")])],
            &[],
        );

        let days = days(&built);

        assert_eq!(
            days.first().unwrap().dossiers.first().unwrap().1,
            vec!["- first", "- second"]
        );
    }

    #[test]
    fn the_links_section_is_not_part_of_the_entry_bullets() {
        let first = date(2026, 9, 20);
        let entry = "\
# 2026-09-20

## Notes
- [x] filed the ticket

## Worked on Dossiers
- [A](<../dossiers/A.md>)
";
        let built = input(
            vec![dossier("A", &[(first, "- did A\n")])],
            &[(first, entry)],
        );

        let days = days(&built);
        let day = days.first().unwrap();

        assert_eq!(day.entry, vec!["### Notes", "- [x] filed the ticket"]);
        assert_eq!(day.dossiers.len(), 1);
    }

    #[test]
    fn an_entry_prints_its_own_sections_one_level_deeper() {
        let first = date(2026, 9, 20);
        let entry = "\
# 2026-09-20

## Notes
- [ ] buy shampoo

## Worked on Tasks
- filed the ticket

## Worked on Dossiers
- [A](<../dossiers/A.md>)
";
        let built = input(
            vec![dossier("A", &[(first, "- did A\n")])],
            &[(first, entry)],
        );

        let days = days(&built);

        assert_eq!(
            days.first().unwrap().entry,
            vec![
                "### Notes",
                "- [ ] buy shampoo",
                "### Worked on Tasks",
                "- filed the ticket",
            ]
        );
    }

    #[test]
    fn an_entry_section_left_empty_is_not_printed() {
        let first = date(2026, 9, 20);
        let entry = "# 2026-09-20\n\n## Notes\n- [x] a task\n\n## Worked on Tasks\n- \n";
        let built = input(vec![], &[(first, entry)]);

        let days = days(&built);

        assert_eq!(
            days.first().unwrap().entry,
            vec!["### Notes", "- [x] a task"]
        );
    }

    #[test]
    fn a_nested_entry_section_keeps_its_parent() {
        let first = date(2026, 9, 20);
        let entry = "# 2026-09-20\n\n## Notes\n### Sub\n- [x] a task\n";
        let built = input(vec![], &[(first, entry)]);

        let days = days(&built);

        assert_eq!(
            days.first().unwrap().entry,
            vec!["### Notes", "#### Sub", "- [x] a task"]
        );
    }

    #[test]
    fn a_dossier_is_named_by_its_own_title() {
        let content = "# a::b\n- Creation date: 2026-09-19\n\n## Worklog\n";

        assert_eq!(dossier_title(content, "a_b"), "a::b");
    }

    #[test]
    fn a_dossier_without_a_title_falls_back_to_its_file() {
        assert_eq!(dossier_title("## Worklog\n", "a_b"), "a_b");
        assert_eq!(dossier_title("# \n## Worklog\n", "a_b"), "a_b");
    }

    #[test]
    fn an_unfiltered_report_leaves_two_blank_lines_under_the_title() {
        let first = date(2026, 9, 20);
        let built = input(vec![dossier("A", &[(first, "- did A\n")])], &[]);

        let output = render(built.from, built.to, &days(&built), &[], None, Palette::OFF);

        assert!(
            output.starts_with(
                "# Report from 2026-09-01 to 2026-09-30\n\n\n## 2026-09-20 (Sunday)\n"
            ),
            "{output:?}"
        );
    }

    #[test]
    fn a_filtered_report_names_its_dossier_under_the_title() {
        let first = date(2026, 9, 20);
        let built = input(vec![dossier("A", &[(first, "- did A\n")])], &[]);

        let output = render(
            built.from,
            built.to,
            &days(&built),
            &[],
            Some("a::b"),
            Palette::OFF,
        );

        assert!(
            output.starts_with(
                "# Report from 2026-09-01 to 2026-09-30\n\nDossier: a::b\n\n\n## 2026-09-20 (Sunday)\n"
            ),
            "{output:?}"
        );
    }

    #[test]
    fn an_entry_alone_still_makes_a_day() {
        let first = date(2026, 9, 20);
        let built = input(
            vec![dossier("A", &[])],
            &[(first, "# 2026-09-20\n\n## Notes\n- \n")],
        );

        let days = days(&built);

        assert_eq!(days.len(), 1);
        assert_eq!(days.first().unwrap().date, first);
        assert!(days.first().unwrap().has_entry);
        assert!(days.first().unwrap().entry.is_empty());
        assert!(days.first().unwrap().dossiers.is_empty());
        assert!(!has_work(&days));
    }

    #[test]
    fn a_worklog_heading_with_nothing_under_it_makes_no_day() {
        let built = input(vec![dossier("A", &[(date(2026, 9, 20), "")])], &[]);

        assert!(days(&built).is_empty());
    }

    #[test]
    fn an_entry_with_no_work_is_named_in_the_report() {
        let built = input(
            vec![],
            &[(date(2026, 9, 20), "# 2026-09-20\n\n## Notes\n- \n")],
        );
        let days = days(&built);

        let output = render(built.from, built.to, &days, &[], None, Palette::OFF);

        assert!(
            output.contains("## 2026-09-20 (Sunday)\n(No work found)\n"),
            "{output}"
        );
    }

    #[test]
    fn a_worklog_day_without_an_entry_is_left_unremarked() {
        let first = date(2026, 9, 20);
        let built = input(vec![dossier("A", &[(first, "- did A\n")])], &[]);
        let days = days(&built);

        assert!(has_work(&days));
        let output = render(built.from, built.to, &days, &[], None, Palette::OFF);
        assert!(!output.contains("No work found"), "{output}");
    }

    #[test]
    fn names_what_a_report_with_no_work_is_missing() {
        let from = date(2026, 9, 1);
        let to = date(2026, 9, 30);

        assert_eq!(
            nothing_to_report(from, to, false, false),
            "No entries found from 2026-09-01 to 2026-09-30"
        );
        assert_eq!(
            nothing_to_report(from, to, false, true),
            "All entries from 2026-09-01 to 2026-09-30 contain no work"
        );
        assert_eq!(
            nothing_to_report(from, from, false, false),
            "No entry found for 2026-09-01"
        );
        assert_eq!(
            nothing_to_report(from, from, false, true),
            "Entry found for 2026-09-01 but no work found"
        );
    }

    #[test]
    fn a_filtered_report_never_mentions_entries() {
        let from = date(2026, 9, 1);
        let to = date(2026, 9, 30);

        assert_eq!(
            nothing_to_report(from, to, true, true),
            "no work in the period 2026-09-01 to 2026-09-30"
        );
        assert_eq!(
            nothing_to_report(from, from, true, true),
            "no work in 2026-09-01"
        );
    }

    #[test]
    fn a_period_with_no_notes_at_all_has_no_days() {
        let built = input(vec![dossier("A", &[])], &[]);

        assert!(days(&built).is_empty());
    }

    #[test]
    fn days_outside_the_period_are_left_out() {
        let built = input(
            vec![dossier(
                "A",
                &[
                    (date(2026, 8, 31), "- before\n"),
                    (date(2026, 9, 10), "- inside\n"),
                    (date(2026, 10, 1), "- after\n"),
                ],
            )],
            &[],
        );

        let days = days(&built);

        assert_eq!(days.len(), 1);
        assert_eq!(days.first().unwrap().date, date(2026, 9, 10));
    }

    #[test]
    fn the_dossier_order_is_first_appearance() {
        let early = date(2026, 9, 10);
        let late = date(2026, 9, 11);
        let built = input(
            vec![
                dossier("A", &[(late, "- a\n")]),
                dossier("B", &[(early, "- b\n"), (late, "- b\n")]),
            ],
            &[],
        );

        let days = days(&built);

        assert_eq!(dossier_order(&days), vec!["B", "A"]);
    }

    #[test]
    fn renders_the_header_the_days_and_the_diffs() {
        let first = date(2026, 9, 20);
        let built = input(vec![dossier("A", &[(first, "- did A\n")])], &[]);
        let days = days(&built);
        let diffs = vec![(
            "A".to_owned(),
            Changes::Diff("diff --git a/dossiers/A.md b/dossiers/A.md\n@@ -1 +1 @@\n".to_owned()),
        )];

        assert_eq!(
            render(built.from, built.to, &days, &diffs, None, Palette::OFF),
            "\
# Report from 2026-09-01 to 2026-09-30


## 2026-09-20 (Sunday)

### A
- did A


## Git diff

### A
{{{ git diff
```diff
diff --git a/dossiers/A.md b/dossiers/A.md
@@ -1 +1 @@
```
}}}
"
        );
    }

    #[test]
    fn the_fence_outgrows_what_the_diff_could_close_it_with() {
        assert_eq!(fence_width("no backticks here\n"), 3);
        assert_eq!(fence_width("+```\n"), 3);
        assert_eq!(fence_width(" ```\n"), 4);
        assert_eq!(fence_width("   ``````\n"), 7);
        assert_eq!(fence_width("    ```\n"), 3);
        assert_eq!(fence_width("\t```\n"), 3);
    }

    #[test]
    fn a_diffs_own_fence_line_stays_inside_the_block() {
        let built = input(vec![dossier("A", &[(date(2026, 9, 20), "- did A\n")])], &[]);
        let days = days(&built);
        let body = String::from("diff --git a/A.md b/A.md\n@@ -1,3 +1,3 @@\n ```\n-old\n+new\n");
        let diffs = vec![("A".to_owned(), Changes::Diff(body))];

        let output = render(built.from, built.to, &days, &diffs, None, Palette::OFF);

        assert!(output.contains("````diff\n"), "{output}");
        assert!(output.contains("\n````\n}}}\n"), "{output}");
    }

    #[test]
    fn a_dossier_with_no_changes_says_so_under_its_heading() {
        let built = input(vec![dossier("A", &[(date(2026, 9, 20), "- did A\n")])], &[]);
        let days = days(&built);
        let diffs = vec![("A".to_owned(), Changes::None)];

        let output = render(built.from, built.to, &days, &diffs, None, Palette::OFF);

        assert!(
            output.contains("## Git diff\n\n### A\n(No git changes were found)\n"),
            "{output}"
        );
        assert!(!output.contains("{{{ git diff"), "{output}");
    }

    #[test]
    fn an_unreadable_diff_is_never_reported_as_no_changes() {
        let built = input(vec![dossier("A", &[(date(2026, 9, 20), "- did A\n")])], &[]);
        let days = days(&built);
        let diffs = vec![("A".to_owned(), Changes::Unavailable)];

        let output = render(built.from, built.to, &days, &diffs, None, Palette::OFF);

        assert!(
            output.contains("## Git diff\n\n### A\n(Git changes could not be read)\n"),
            "{output}"
        );
        assert!(!output.contains("No git changes were found"), "{output}");
        assert!(!output.contains("{{{ git diff"), "{output}");
    }

    #[test]
    fn an_empty_diff_list_prints_no_section_heading() {
        let built = input(vec![dossier("A", &[(date(2026, 9, 20), "- did A\n")])], &[]);
        let days = days(&built);

        let output = render(built.from, built.to, &days, &[], None, Palette::OFF);

        assert!(!output.contains("## Git diff"), "{output}");
    }

    #[test]
    fn carriage_returns_do_not_reach_the_output() {
        let first = date(2026, 9, 20);
        let entry = "# 2026-09-20\r\n\r\n## Notes\r\n- [ ] a task\r\n";
        let built = input(
            vec![dossier("A", &[(first, "- did A\r\n")])],
            &[(first, entry)],
        );

        let output = render(built.from, built.to, &days(&built), &[], None, Palette::OFF);

        assert!(!output.contains('\r'), "{output:?}");
    }

    #[test]
    fn a_coloured_run_hues_the_structure_and_nothing_else() {
        let first = date(2026, 9, 20);
        let built = input(
            vec![dossier("A", &[(first, "- [ ] todo\n- [?] waiting\n")])],
            &[],
        );
        let days = days(&built);
        let diffs = vec![(
            "A".to_owned(),
            Changes::Diff("diff --git a/A.md b/A.md\n@@ -1 +1 @@\n".to_owned()),
        )];

        let output = render(built.from, built.to, &days, &diffs, None, Palette::ON);

        assert!(
            output.contains("\x1b[1;35m# Report from 2026-09-01 to 2026-09-30\x1b[0m"),
            "{output}"
        );
        assert!(
            output.contains("\x1b[1;36m## 2026-09-20 (Sunday)\x1b[0m"),
            "{output}"
        );
        assert!(output.contains("\x1b[1;36m## Git diff\x1b[0m"), "{output}");
        assert!(output.contains("\x1b[1;34m### A\x1b[0m"), "{output}");
        assert!(output.contains("\x1b[2m{{{ git diff\x1b[0m"), "{output}");
        assert!(output.contains("\x1b[2m```diff\x1b[0m"), "{output}");
        assert!(output.contains("\x1b[2m```\x1b[0m"), "{output}");
        assert!(output.contains("\x1b[2m}}}\x1b[0m"), "{output}");

        // The notes keep their markers exactly as written, and the diff body
        // is git's to paint.
        assert!(output.contains("- [ ] todo"), "{output}");
        assert!(output.contains("- [?] waiting"), "{output}");
        assert!(!output.contains("[34m[ ]"), "{output}");
        assert!(output.contains("diff --git a/A.md b/A.md"), "{output}");
    }

    #[test]
    fn colour_off_draws_the_report_as_plain_text() {
        let built = input(
            vec![dossier("A", &[(date(2026, 9, 20), "- [ ] todo\n")])],
            &[],
        );
        let days = days(&built);
        let diffs = vec![("A".to_owned(), Changes::None)];

        let output = render(built.from, built.to, &days, &diffs, None, Palette::OFF);

        assert!(!output.contains('\x1b'), "{output:?}");
    }
}
