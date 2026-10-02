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
    /// The day's own notes, already rendered.
    pub entry: Vec<String>,
    /// Each dossier worked on that day, in the order it should print, with its
    /// worklog lines already rendered.
    pub dossiers: Vec<(String, Vec<String>)>,
}

/// Build the report's days, oldest first.
///
/// A day is in the report when a dossier logged work on it or its entry has
/// bullets; a day with neither is dropped. Duplicate `### date` headings in one
/// dossier are merged in document order, and a dossier whose day has nothing to
/// print is left out rather than given an empty heading.
#[must_use]
pub fn days(input: &Input, palette: Palette) -> Vec<Day> {
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
        let entry = entry_source.map_or_else(Vec::new, |content| {
            Tree::parse(&worklog::without_links(content)).render_all(palette)
        });
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
                let lines = Tree::parse(block).render_all(palette);
                if lines.is_empty() {
                    return None;
                }
                Some((dossier.name.clone(), lines))
            })
            .collect();

        if entry.is_empty() && dossiers.is_empty() {
            continue;
        }

        days.push(Day {
            date,
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

/// The whole report: a header, the daily work, and the diffs.
///
/// Empty diffs mean no `# Git diff` section at all, which is what `--no-diff`
/// asks for and what a period with no commits would produce anyway.
#[must_use]
pub fn render(from: NaiveDate, to: NaiveDate, days: &[Day], diffs: &[(String, String)]) -> String {
    let mut lines = vec![format!("Report from {from} to {to}")];
    lines.push(String::new());
    lines.push(String::from("# Daily work"));

    for day in days {
        lines.push(String::new());
        lines.push(format!("## {}", day.date));

        if !day.entry.is_empty() {
            lines.push(String::new());
            lines.extend(day.entry.iter().cloned());
        }

        for (name, body) in &day.dossiers {
            lines.push(String::new());
            lines.push(format!("### {name}"));
            lines.extend(body.iter().cloned());
        }
    }

    if !diffs.is_empty() {
        lines.push(String::new());
        lines.push(String::from("# Git diff"));

        for (name, body) in diffs {
            lines.push(String::new());
            lines.push(["{{{", name.as_str()].join(" "));
            lines.extend(body.lines().map(str::to_owned));
            lines.push(String::from("}}}"));
        }
    }

    let mut output = lines.join("\n");
    output.push('\n');
    output
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

        let days = days(&built, Palette::OFF);

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

        let days = days(&built, Palette::OFF);

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

        let days = days(&built, Palette::OFF);

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

        let days = days(&built, Palette::OFF);
        let day = days.first().unwrap();

        assert_eq!(day.entry, vec!["- [x] filed the ticket"]);
        assert_eq!(day.dossiers.len(), 1);
    }

    #[test]
    fn a_day_with_nothing_to_print_is_dropped() {
        let built = input(
            vec![dossier("A", &[])],
            &[(date(2026, 9, 20), "# 2026-09-20\n")],
        );

        assert!(days(&built, Palette::OFF).is_empty());
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

        let days = days(&built, Palette::OFF);

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

        let days = days(&built, Palette::OFF);

        assert_eq!(dossier_order(&days), vec!["B", "A"]);
    }

    #[test]
    fn renders_the_header_the_days_and_the_diffs() {
        let first = date(2026, 9, 20);
        let built = input(vec![dossier("A", &[(first, "- did A\n")])], &[]);
        let days = days(&built, Palette::OFF);
        let diffs = vec![(
            "A".to_owned(),
            "diff --git a/dossiers/A.md b/dossiers/A.md\n@@ -1 +1 @@\n".to_owned(),
        )];

        assert_eq!(
            render(built.from, built.to, &days, &diffs),
            "\
Report from 2026-09-01 to 2026-09-30

# Daily work

## 2026-09-20

### A
- did A

# Git diff

{{{ A
diff --git a/dossiers/A.md b/dossiers/A.md
@@ -1 +1 @@
}}}
"
        );
    }

    #[test]
    fn an_empty_diff_list_prints_no_section_heading() {
        let built = input(vec![dossier("A", &[(date(2026, 9, 20), "- did A\n")])], &[]);
        let days = days(&built, Palette::OFF);

        let output = render(built.from, built.to, &days, &[]);

        assert!(!output.contains("# Git diff"), "{output}");
    }

    #[test]
    fn carriage_returns_do_not_reach_the_output() {
        let first = date(2026, 9, 20);
        let entry = "# 2026-09-20\r\n\r\n## Notes\r\n- [ ] a task\r\n";
        let built = input(
            vec![dossier("A", &[(first, "- did A\r\n")])],
            &[(first, entry)],
        );

        let output = render(built.from, built.to, &days(&built, Palette::OFF), &[]);

        assert!(!output.contains('\r'), "{output:?}");
    }

    #[test]
    fn a_coloured_run_hues_the_markers_by_state() {
        let first = date(2026, 9, 20);
        let built = input(
            vec![dossier("A", &[(first, "- [ ] todo\n- [?] waiting\n")])],
            &[],
        );

        let days = days(&built, Palette::ON);
        let body = &days.first().unwrap().dossiers.first().unwrap().1;

        assert_eq!(body.len(), 2);
        assert_eq!(body.first().unwrap(), "- \x1b[34m[ ]\x1b[0m todo");
        assert_eq!(body.get(1).unwrap(), "- \x1b[35m[?]\x1b[0m waiting");
    }
}
