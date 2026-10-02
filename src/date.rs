//! Plain-English dates for `bureau worklog --date` and `bureau report`.
//!
//! Everything here is a pure function: `today` is handed in, so the grammar
//! and the arithmetic are tested without a clock. The vocabulary is
//! deliberately closed -- a fixed set of units and number words -- because the
//! alternative is a natural-language dependency for a date-only tool.

use anyhow::{Context, Result, bail};
use chrono::{Days, Months, NaiveDate};

/// How an ISO date is written.
const DATE_FORMAT: &str = "%Y-%m-%d";

/// The largest number word that is understood; digits have no such limit.
const WORD_LIMIT: u64 = 10;

/// A unit a relative date can be counted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Day,
    Week,
    Month,
    Year,
}

impl Unit {
    /// The unit a word names, singular or plural alike.
    fn from_word(word: &str) -> Option<Self> {
        match word {
            "day" | "days" => Some(Self::Day),
            "week" | "weeks" => Some(Self::Week),
            "month" | "months" => Some(Self::Month),
            "year" | "years" => Some(Self::Year),
            _ => None,
        }
    }
}

/// The date `text` names, relative to `today`.
///
/// # Errors
///
/// Fails when `text` is not `YYYY-MM-DD`, `today`, `yesterday`, or a relative
/// form such as `a week ago` or `3 months ago`. Future forms are refused
/// rather than quietly accepted: a report about the future is empty by
/// construction, and saying so beats printing nothing.
pub fn parse(text: &str, today: NaiveDate) -> Result<NaiveDate> {
    let normalised = normalise(text);

    if let Ok(date) = NaiveDate::parse_from_str(&normalised, DATE_FORMAT) {
        return Ok(date);
    }

    match normalised.as_str() {
        "today" => return Ok(today),
        "yesterday" => return subtract(today, 1, Unit::Day),
        _ => {}
    }

    if let Some((count, unit)) = relative(&normalised) {
        return subtract(today, count, unit);
    }

    bail!(
        "could not read '{text}' as a date; expected YYYY-MM-DD, today, yesterday, \
         or \"<n> days|weeks|months|years ago\""
    )
}

/// The inclusive range the arguments describe, oldest first.
///
/// `<date>` is one day; without it, `--from` and `--to` each default to today.
///
/// # Errors
///
/// Fails when a date cannot be read, or when `--from` is later than `--to`.
pub fn range(
    date: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
    today: NaiveDate,
) -> Result<(NaiveDate, NaiveDate)> {
    if let Some(text) = date {
        let day = parse(text, today)?;
        return Ok((day, day));
    }

    let start = from.map_or(Ok(today), |text| parse(text, today))?;
    let end = to.map_or(Ok(today), |text| parse(text, today))?;

    if start > end {
        bail!("'--from {start}' is after '--to {end}'");
    }

    Ok((start, end))
}

/// `text` lowercased, with every run of whitespace collapsed to one space.
///
/// This is what lets `"  A   Week   Ago "` and `"a week ago"` be the same
/// string, and it is why the whole expression has to be one argument.
fn normalise(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The count and unit a `<n> <unit> ago` string carries.
fn relative(text: &str) -> Option<(u64, Unit)> {
    let rest = text.strip_suffix(" ago")?;
    let mut words = rest.split(' ');
    let count = count(words.next()?)?;
    let unit = Unit::from_word(words.next()?)?;

    // A third word means this was not the two-word form, so it is not a date.
    if words.next().is_some() {
        return None;
    }

    Some((count, unit))
}

/// The number a count word or a run of digits names.
///
/// Words stop at [`WORD_LIMIT`], digits do not, and zero is refused so that
/// `0 days ago` is a mistake rather than a silent alias for today.
fn count(word: &str) -> Option<u64> {
    let named = match word {
        "a" | "an" | "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(WORD_LIMIT),
        _ => None,
    };

    named.or_else(|| word.parse::<u64>().ok().filter(|count| *count > 0))
}

/// `today` moved back by `count` of `unit`.
fn subtract(today: NaiveDate, count: u64, unit: Unit) -> Result<NaiveDate> {
    let shifted = match unit {
        Unit::Day => today.checked_sub_days(Days::new(count)),
        Unit::Week => {
            let days = count
                .checked_mul(7)
                .context("the date is too far in the past")?;
            today.checked_sub_days(Days::new(days))
        }
        Unit::Month => today.checked_sub_months(months(count)?),
        Unit::Year => {
            let months = count
                .checked_mul(12)
                .context("the date is too far in the past")?;
            today.checked_sub_months(months_from(months)?)
        }
    };

    shifted.context("the date is outside the range this tool can represent")
}

/// `count` months, as the [`Months`] chrono's calendar arithmetic wants.
fn months(count: u64) -> Result<Months> {
    months_from(count)
}

/// The same, for a count that is already in months.
fn months_from(count: u64) -> Result<Months> {
    let count = u32::try_from(count).context("too many months for a date")?;
    Ok(Months::new(count))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    /// The fixed "today" every test works from.
    fn today() -> NaiveDate {
        date(2026, 10, 2)
    }

    /// `parse` against [`today`].
    fn parsed(text: &str) -> NaiveDate {
        parse(text, today()).unwrap()
    }

    #[test]
    fn reads_iso_dates() {
        assert_eq!(parsed("2026-09-20"), date(2026, 9, 20));
        assert_eq!(parsed("2024-02-29"), date(2024, 2, 29));
    }

    #[test]
    fn reads_today_and_yesterday() {
        assert_eq!(parsed("today"), today());
        assert_eq!(parsed("yesterday"), date(2026, 10, 1));
    }

    #[test]
    fn reads_every_number_form() {
        let cases = [
            ("a day ago", date(2026, 10, 1)),
            ("an day ago", date(2026, 10, 1)),
            ("one day ago", date(2026, 10, 1)),
            ("1 day ago", date(2026, 10, 1)),
            ("two days ago", date(2026, 9, 30)),
            ("2 days ago", date(2026, 9, 30)),
            ("ten days ago", date(2026, 9, 22)),
            ("3 weeks ago", date(2026, 9, 11)),
            ("a week ago", date(2026, 9, 25)),
            ("two weeks ago", date(2026, 9, 18)),
            ("3 months ago", date(2026, 7, 2)),
            ("1 year ago", date(2025, 10, 2)),
        ];

        for (text, expected) in cases {
            assert_eq!(parsed(text), expected, "{text}");
        }
    }

    #[test]
    fn digits_are_unbounded_while_words_stop_at_ten() {
        assert!(parse("400 days ago", today()).is_ok());
        assert_eq!(parsed("23 months ago"), date(2024, 11, 2));
        assert!(parse("eleven days ago", today()).is_err());
    }

    #[test]
    fn singular_and_plural_do_not_have_to_agree() {
        assert_eq!(parsed("1 days ago"), date(2026, 10, 1));
        assert_eq!(parsed("2 day ago"), date(2026, 9, 30));
        assert_eq!(parsed("1 week ago"), date(2026, 9, 25));
    }

    #[test]
    fn case_and_extra_whitespace_are_ignored() {
        assert_eq!(
            parse("  A   Week   Ago  ", today()).unwrap(),
            date(2026, 9, 25)
        );
        assert_eq!(parse("YESTERDAY", today()).unwrap(), date(2026, 10, 1));
    }

    #[test]
    fn months_clamp_to_the_shorter_month() {
        assert_eq!(
            parse("1 month ago", date(2024, 3, 31)).unwrap(),
            date(2024, 2, 29)
        );
        assert_eq!(
            parse("1 month ago", date(2023, 3, 31)).unwrap(),
            date(2023, 2, 28)
        );
        assert_eq!(
            parse("1 year ago", date(2024, 2, 29)).unwrap(),
            date(2023, 2, 28)
        );
    }

    #[test]
    fn refuses_future_and_malformed_dates() {
        for text in [
            "",
            "tomorrow",
            "in a week",
            "2 days",
            "2 days from now",
            "0 days ago",
            "eleven days ago",
            "-1 day ago",
            "yesterdcay",
            "2026-13-45",
        ] {
            assert!(parse(text, today()).is_err(), "{text} should be refused");
        }
    }

    #[test]
    fn a_single_date_is_a_one_day_range() {
        assert_eq!(
            range(Some("2026-09-20"), None, None, today()).unwrap(),
            (date(2026, 9, 20), date(2026, 9, 20))
        );
    }

    #[test]
    fn a_default_range_is_today_alone() {
        assert_eq!(
            range(None, None, None, today()).unwrap(),
            (today(), today())
        );
    }

    #[test]
    fn from_and_to_default_to_today() {
        assert_eq!(
            range(None, Some("yesterday"), None, today()).unwrap(),
            (date(2026, 10, 1), today())
        );
        assert_eq!(
            range(None, Some("a week ago"), Some("yesterday"), today()).unwrap(),
            (date(2026, 9, 25), date(2026, 10, 1))
        );
    }

    #[test]
    fn a_backwards_range_is_an_error() {
        let error = range(None, Some("today"), Some("yesterday"), today()).unwrap_err();
        assert!(error.to_string().contains("is after"), "{error}");
    }
}
