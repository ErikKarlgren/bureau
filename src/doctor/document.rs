/// The parts of one file the checks need.
#[derive(Debug, Default)]
pub(super) struct Document {
    headings: Vec<Heading>,
    bullets: Vec<Bullet>,
    links: Vec<Link>,
    unclosed_fence: Option<usize>,
    line_count: usize,
}

impl Document {
    pub(super) fn parse(contents: &str) -> Self {
        let lines: Vec<&str> = contents.lines().collect();
        let mut document = Self {
            line_count: lines.len(),
            ..Self::default()
        };
        let mut fence: Option<(char, usize, usize)> = None;

        for (index, line) in lines.iter().enumerate() {
            let number = index.saturating_add(1);

            if let Some((character, length, _)) = fence {
                if is_closing_fence(line, character, length) {
                    fence = None;
                }
                continue;
            }

            if let Some((character, length)) = opening_fence(line) {
                fence = Some((character, length, number));
                continue;
            }

            if let Some(heading) = parse_heading(line, number) {
                document.headings.push(heading);
            }
            if let Some(bullet) = parse_bullet(line, number) {
                document.bullets.push(bullet);
            }
            for mut link in parse_links(line) {
                link.line = number;
                document.links.push(link);
            }
        }

        document.unclosed_fence = fence.map(|(_, _, line)| line);
        document
    }

    pub(super) fn exact_sections(&self, raw: &str) -> Vec<&Heading> {
        self.headings
            .iter()
            .filter(|heading| heading.indent == 0 && heading.raw.trim_end() == raw)
            .collect()
    }

    pub(super) fn section_end(&self, heading: &Heading) -> usize {
        self.headings
            .iter()
            .find(|candidate| {
                candidate.line > heading.line
                    && candidate.indent == 0
                    && candidate.level <= heading.level
            })
            .map_or_else(
                || self.line_count.saturating_add(1),
                |candidate| candidate.line,
            )
    }

    pub(super) fn headings_between(&self, start: usize, end: usize, level: usize) -> Vec<&Heading> {
        self.headings
            .iter()
            .filter(|heading| heading.line > start && heading.line < end && heading.level == level)
            .collect()
    }

    pub(super) fn links_between(&self, start: usize, end: usize) -> Vec<&Link> {
        self.links
            .iter()
            .filter(|link| link.line > start && link.line < end)
            .collect()
    }

    pub(super) fn bullets(&self) -> &[Bullet] {
        &self.bullets
    }

    pub(super) fn links(&self) -> &[Link] {
        &self.links
    }

    pub(super) const fn unclosed_fence(&self) -> Option<usize> {
        self.unclosed_fence
    }
}

/// One ATX heading.
#[derive(Debug, Clone)]
pub(super) struct Heading {
    pub(super) line: usize,
    pub(super) level: usize,
    pub(super) indent: usize,
    pub(super) text: String,
    pub(super) raw: String,
}

/// One list item.
#[derive(Debug, Clone)]
pub(super) struct Bullet {
    pub(super) line: usize,
    pub(super) marker: BulletMarker,
    pub(super) text: String,
}

/// The list marker a bullet uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BulletMarker {
    Dash,
    Star,
    Plus,
    Ordered,
}

/// One inline link or image.
#[derive(Debug, Clone)]
pub(super) struct Link {
    pub(super) line: usize,
    pub(super) target: String,
    pub(super) angle: bool,
    pub(super) malformed: bool,
}

/// A heading text that is exactly one inline link, as its label and target.
pub(super) fn whole_link(text: &str) -> Option<(String, String)> {
    let text = text.trim();

    if !text.starts_with('[') || text.starts_with("![") {
        return None;
    }

    let close = text.find("](")?;
    let label = text.get(1..close)?;
    let rest = text.get(close.saturating_add(2)..)?;
    let (target, consumed, _, malformed) = parse_destination(rest);
    let remaining = rest.get(consumed..)?;

    if malformed || !remaining.trim().is_empty() {
        return None;
    }

    Some((label.to_owned(), target))
}

/// Parse one link destination at the start of `rest`.
///
/// Returns the target, how many bytes it consumed, whether it used angle
/// brackets, and whether the syntax was broken.
fn parse_destination(rest: &str) -> (String, usize, bool, bool) {
    if rest.starts_with('<') {
        let Some(close) = rest.find('>') else {
            return (
                rest.get(1..).unwrap_or("").trim().to_owned(),
                rest.len(),
                true,
                true,
            );
        };

        let target = rest.get(1..close).unwrap_or("").trim().to_owned();
        let after = rest.get(close.saturating_add(1)..).unwrap_or("");
        let Some(paren) = after.find(')') else {
            return (target, rest.len(), true, true);
        };

        let consumed = close
            .saturating_add(1)
            .saturating_add(paren)
            .saturating_add(1);
        (target, consumed, true, false)
    } else {
        let Some(close) = rest.find(')') else {
            return (rest.trim().to_owned(), rest.len(), false, true);
        };

        let target = rest.get(..close).unwrap_or("").trim().to_owned();
        let malformed = target.is_empty();

        (target, close.saturating_add(1), false, malformed)
    }
}

/// Every inline link in a line, in document order.
fn parse_links(line: &str) -> Vec<Link> {
    let mut links = Vec::new();
    let mut search = 0usize;

    while let Some(found) = line.get(search..).and_then(|rest| rest.find("](")) {
        let close = search.saturating_add(found);
        if line
            .get(..close)
            .and_then(|before| before.rfind('['))
            .is_none()
        {
            search = close.saturating_add(2);
            continue;
        }

        let after = close.saturating_add(2);
        let rest = line.get(after..).unwrap_or("");
        let (target, consumed, angle, malformed) = parse_destination(rest);
        let end = after.saturating_add(consumed);

        links.push(Link {
            line: 0,
            target,
            angle,
            malformed,
        });

        search = end.max(after.saturating_add(1));
    }

    links
}

/// Parse a bullet, outside a fenced block.
fn parse_bullet(line: &str, number: usize) -> Option<Bullet> {
    let (_, rest) = split_indent(line);

    let (marker, text) = if let Some(after) = rest.strip_prefix('-') {
        let starts_task = after.trim_start().starts_with('[');
        if !after.is_empty() && !after.starts_with(char::is_whitespace) && !starts_task {
            return None;
        }
        (BulletMarker::Dash, after)
    } else if let Some(after) = rest.strip_prefix('*') {
        if !after.is_empty() && !after.starts_with(char::is_whitespace) {
            return None;
        }
        (BulletMarker::Star, after)
    } else if let Some(after) = rest.strip_prefix('+') {
        if !after.is_empty() && !after.starts_with(char::is_whitespace) {
            return None;
        }
        (BulletMarker::Plus, after)
    } else {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }

        let after_digits = rest.get(digits..)?;
        let after_marker = after_digits
            .strip_prefix('.')
            .or_else(|| after_digits.strip_prefix(')'))?;

        if !after_marker.starts_with(char::is_whitespace) {
            return None;
        }

        (BulletMarker::Ordered, after_marker)
    };

    Some(Bullet {
        line: number,
        marker,
        text: text.to_owned(),
    })
}

/// `text` split into its leading whitespace and the rest.
fn split_indent(text: &str) -> (&str, &str) {
    text.find(|character: char| !character.is_ascii_whitespace())
        .map_or((text, ""), |start| text.split_at(start))
}

/// Parse an ATX heading at the start of a line, up to three spaces in.
fn parse_heading(line: &str, number: usize) -> Option<Heading> {
    let indent = line
        .len()
        .saturating_sub(line.trim_start_matches(' ').len());
    if indent > 3 {
        return None;
    }

    let rest = line.get(indent..)?;
    let hashes = rest
        .len()
        .saturating_sub(rest.trim_start_matches('#').len());
    if hashes == 0 || hashes > 6 {
        return None;
    }

    let after = rest.get(hashes..)?;
    if !(after.is_empty() || after.starts_with(' ') || after.starts_with('\t')) {
        return None;
    }

    Some(Heading {
        line: number,
        level: hashes,
        indent,
        text: after.trim().to_owned(),
        raw: line.to_owned(),
    })
}

/// The opening fence a line starts, and its character and length.
fn opening_fence(line: &str) -> Option<(char, usize)> {
    let indent = line
        .len()
        .saturating_sub(line.trim_start_matches(' ').len());
    if indent > 3 {
        return None;
    }

    let rest = line.get(indent..)?;
    let character = rest.chars().next()?;
    if character != '`' && character != '~' {
        return None;
    }

    let length = rest.chars().take_while(|found| *found == character).count();
    (length >= 3).then_some((character, length))
}

/// Whether a line closes the fence opened by `character`/`length`.
fn is_closing_fence(line: &str, character: char, length: usize) -> bool {
    let indent = line
        .len()
        .saturating_sub(line.trim_start_matches(' ').len());
    if indent > 3 {
        return false;
    }

    let Some(rest) = line.get(indent..) else {
        return false;
    };
    let count = rest.chars().take_while(|found| *found == character).count();
    if count < length {
        return false;
    }

    rest.get(count..)
        .is_some_and(|after| after.trim().is_empty())
}
