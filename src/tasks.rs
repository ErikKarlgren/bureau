//! The text handling behind `bureau tasks`.
//!
//! Everything here is a pure function over strings, so the fiddly parts --
//! how a bullet is read, what nests under what, and which nodes a section
//! keeps -- are tested without touching the filesystem.
//!
//! One rule decides every section: *print every node that matches the
//! section's predicate, and every ancestor of a match*. That is why rendering
//! is a single filtered walk down the tree rather than three renderers.

use std::fmt;

/// How many spaces one level of nesting is in the *output*.
///
/// Nothing in the input is measured in levels: how deep a bullet sits is
/// decided by comparing its indentation with the bullets above it, so a file
/// indented by two spaces, four, or eight nests the same way.
const LEVEL: usize = 2;

/// How far a tab advances when indentation is being compared.
const TAB: usize = 4;

/// The marker that turns a bullet into a task, and what it means.
///
/// A bullet whose text does not start with one of these is not a task: it may
/// still be printed as the ancestor of one, but it never matches a section on
/// its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Todo,
    Doing,
    Almost,
    Waiting,
    Done,
    Cancelled,
}

impl State {
    /// The state a single marker character stands for, `None` when no marker
    /// does.
    #[must_use]
    pub const fn from_symbol(symbol: char) -> Option<Self> {
        match symbol {
            ' ' => Some(Self::Todo),
            '.' => Some(Self::Doing),
            'o' => Some(Self::Almost),
            '?' => Some(Self::Waiting),
            'x' => Some(Self::Done),
            '-' => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// The character this state is written with.
    #[must_use]
    pub const fn symbol(self) -> char {
        match self {
            Self::Todo => ' ',
            Self::Doing => '.',
            Self::Almost => 'o',
            Self::Waiting => '?',
            Self::Done => 'x',
            Self::Cancelled => '-',
        }
    }

    /// Whether the work is still outstanding.
    ///
    /// `Waiting` is open even though it is also blocked: there is something to
    /// do, it just cannot be done yet. That is why a `[?]` task is listed
    /// under two sections.
    #[must_use]
    pub const fn is_open(self) -> bool {
        matches!(
            self,
            Self::Todo | Self::Doing | Self::Almost | Self::Waiting
        )
    }

    /// Whether the work is over with.
    #[must_use]
    pub const fn is_finished(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }
}

/// One bullet, and the ones nested under it.
#[derive(Debug, Clone)]
pub struct Node {
    /// The marker, or `None` for a bullet that is only prose.
    pub marker: Option<State>,
    /// The bullet's text, with the leading `- [ ] ` removed.
    pub text: String,
    /// The parent's index, `None` at the top level.
    pub parent: Option<usize>,
    /// The indices of the bullets nested directly under this one.
    pub children: Vec<usize>,
}

/// A whole file: every bullet, in document order, as one or more trees.
#[derive(Debug, Default)]
pub struct Tree {
    nodes: Vec<Node>,
    roots: Vec<usize>,
}

/// What a section asks of the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Actionable,
    Blocked,
    Finished,
}

impl Section {
    /// The heading a section prints under.
    #[must_use]
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Actionable => "ACTIONABLE",
            Self::Blocked => "BLOCKED",
            Self::Finished => "FINISHED",
        }
    }

    /// Whether the section is about this node, rather than about something
    /// below it.
    ///
    /// Only a task is ever the subject of a section: a bullet with no marker
    /// matches nothing, whatever sits above it, and neither does a finished
    /// task in `ACTIONABLE` or `BLOCKED`, where a `[?]` above it would otherwise
    /// drag it in. Either way it can still print as the ancestor of a task
    /// that does match.
    fn matches(self, tree: &Tree, node: usize) -> bool {
        if !tree.is_task(node) || (self != Self::Finished && tree.is_finished(node)) {
            return false;
        }

        match self {
            // Work that can be picked up now: open, and not waiting on
            // anything above it. A `[?]` task is open but is exactly what its
            // own section is for, so it is not also a pending one.
            Self::Actionable => tree.is_open(node) && !tree.is_blocked(node),
            // Derived from the marker and from every ancestor's.
            Self::Blocked => tree.is_blocked(node),
            // `is_finished` alone would print a `[x]` that still has work
            // below it. The section claims the work is over, so a node with
            // anything unfinished underneath does not belong in it.
            Self::Finished => tree.is_finished(node) && !tree.any_open_descendant(node),
        }
    }
}

impl fmt::Display for Section {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.heading())
    }
}

impl Tree {
    /// Read every bullet in `content` into a tree.
    ///
    /// Non-bullet lines are ignored, including `##` headings, so the same
    /// rules work in a dossier and in a daily entry with no section
    /// convention to maintain.
    #[must_use]
    pub fn parse(content: &str) -> Self {
        let mut tree = Self::default();
        // The ancestors a following bullet could still nest into, outermost
        // first, each with the indentation that opened it. Their indentation
        // strictly increases, which is what makes "deeper than the one before
        // it" the only way to nest.
        let mut open: Vec<(usize, usize)> = Vec::new();

        for line in content.lines() {
            let Some((bullet, indent)) = parse_line(line) else {
                continue;
            };

            // A bullet nests into the nearest bullet indented less than it is.
            // One indented *the same* as an earlier bullet is its sibling, and
            // closes it and everything deeper off -- which is what keeps a
            // list where several bullets share a deeper indentation flat
            // instead of staircasing them.
            open.retain(|(opened, _)| *opened < indent);
            let parent = open.last().map(|(_, node)| *node);

            let node = tree.push(bullet, parent);

            match parent {
                Some(parent) => tree.attach(parent, node),
                None => tree.roots.push(node),
            }

            open.push((indent, node));
        }

        tree
    }

    /// Add a node, returning its index.
    fn push(&mut self, bullet: Bullet, parent: Option<usize>) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node {
            marker: bullet.marker,
            text: bullet.text,
            parent,
            children: Vec::new(),
        });
        index
    }

    /// File a bullet under the one it nests into.
    fn attach(&mut self, parent: usize, node: usize) {
        if let Some(parent) = self.nodes.get_mut(parent) {
            parent.children.push(node);
        }
    }

    /// A node's marker.
    #[must_use]
    pub fn marker(&self, node: usize) -> Option<State> {
        self.nodes.get(node)?.marker
    }

    /// Whether the node is a task at all.
    fn is_task(&self, node: usize) -> bool {
        self.marker(node).is_some()
    }

    /// Whether the node is a task, and open.
    fn is_open(&self, node: usize) -> bool {
        self.marker(node).is_some_and(State::is_open)
    }

    /// Whether the node is a task, and over with.
    fn is_finished(&self, node: usize) -> bool {
        self.marker(node).is_some_and(State::is_finished)
    }

    /// Whether the node has any open task anywhere below it.
    fn any_open_descendant(&self, node: usize) -> bool {
        let mut stack: Vec<usize> = self
            .nodes
            .get(node)
            .map(|node| node.children.clone())
            .unwrap_or_default();

        while let Some(index) = stack.pop() {
            let Some(descendant) = self.nodes.get(index) else {
                continue;
            };

            if descendant.marker.is_some_and(State::is_open) {
                return true;
            }

            stack.extend(descendant.children.iter().copied());
        }

        false
    }

    /// Whether the node is a task on wait, or sits anywhere under one.
    fn is_blocked(&self, node: usize) -> bool {
        // Starting at the node itself, so a `[?]` task is blocked for its own
        // sake and not only for the sake of what is below it.
        let mut current = Some(node);

        while let Some(index) = current {
            let Some(candidate) = self.nodes.get(index) else {
                return false;
            };

            if candidate.marker == Some(State::Waiting) {
                return true;
            }

            current = candidate.parent;
        }

        false
    }
}

impl Tree {
    /// The lines one section of this tree prints, at two spaces per level.
    ///
    /// A node prints when it belongs to the section, or when a node below it
    /// does and it is on the way there. Nothing else prints: no sibling, no
    /// closed task, and no bullet that is not a link in such a chain.
    #[must_use]
    pub fn render(&self, section: Section) -> Vec<String> {
        let mut lines = Vec::new();

        for root in &self.roots {
            self.render_node(*root, section, 0, &mut lines);
        }

        lines
    }

    /// Print one node and whatever below it belongs to `section`, returning
    /// whether anything was printed.
    ///
    /// A node prints when it belongs to the section, or when it is on the way
    /// down to a node that does.
    fn render_node(
        &self,
        node: usize,
        section: Section,
        depth: usize,
        lines: &mut Vec<String>,
    ) -> bool {
        let Some(current) = self.nodes.get(node) else {
            return false;
        };

        if !self.leads_to_a_match(node, section) {
            return false;
        }

        // Whatever the children print goes under this line, so note where
        // they start and put this line in front of them once they are done.
        let first = lines.len();
        for child in &current.children {
            self.render_node(*child, section, depth.saturating_add(1), lines);
        }
        lines.insert(first, indent(&render_bullet(current), depth));

        true
    }

    /// Whether `node` prints in `section`: either it belongs there, or it is
    /// an ancestor of something that does.
    ///
    /// A closed task is the context for unfinished work under it -- the
    /// `- [x] Create tests` whose last subtask is still open -- and nothing
    /// else. It is not the context for a closed task, so a closed subtree with
    /// no open work in it is left out rather than listed under another
    /// heading, and it is not the context for a section that walks through
    /// closed tasks only, which is `FINISHED` itself.
    fn leads_to_a_match(&self, node: usize, section: Section) -> bool {
        if section.matches(self, node) {
            return true;
        }

        // Nothing under a block can be pending, so a pending listing does not
        // walk into one and does not print the chain above it either.
        if section == Section::Actionable && self.is_blocked(node) {
            return false;
        }

        if self.is_finished(node) {
            if section == Section::Finished {
                return false;
            }

            let Some(current) = self.nodes.get(node) else {
                return false;
            };

            return current
                .children
                .iter()
                .any(|child| self.is_open(*child) || self.leads_to_a_match(*child, section));
        }

        let Some(current) = self.nodes.get(node) else {
            return false;
        };

        current
            .children
            .iter()
            .any(|child| self.leads_to_a_match(*child, section))
    }
}

/// One bullet's marker and text, as read off a line.
#[derive(Debug)]
struct Bullet {
    marker: Option<State>,
    text: String,
}

/// The bullet a line is, and how far it is indented, or `None` when the line
/// is not a bullet at all.
fn parse_line(line: &str) -> Option<(Bullet, usize)> {
    let (indent, rest) = split_whitespace(line);
    let tail = rest.strip_prefix('-')?;
    Some((parse_bullet(tail), columns(indent)))
}

/// How many columns of indentation `whitespace` is, a tab advancing to the
/// next tab stop.
fn columns(whitespace: &str) -> usize {
    let mut columns = 0;

    for character in whitespace.chars() {
        columns = match character {
            '\t' => next_tab_stop(columns),
            _ => columns.saturating_add(1),
        };
    }

    columns
}

/// The column `column` lands on when a tab is written there.
const fn next_tab_stop(column: usize) -> usize {
    column
        .saturating_div(TAB)
        .saturating_add(1)
        .saturating_mul(TAB)
}

/// A bullet's marker and text.
///
/// Any run of whitespace between the dash and the marker is accepted, so
/// `- [x]`, `-[x]` and `-  [x]` are the same task. This is looser than
/// `CommonMark` on purpose: a note that a stricter reader would render as a
/// paragraph is still a task here, because the user wrote a marker on it.
fn parse_bullet(tail: &str) -> Bullet {
    let (_, rest) = split_whitespace(tail);

    match marker(rest) {
        Some((state, text)) => Bullet {
            marker: Some(state),
            text: bullet_text(text),
        },
        None => Bullet {
            marker: None,
            text: rest.trim_end_matches('\n').to_owned(),
        },
    }
}

/// The marker a bullet's text starts with, and the text after it.
fn marker(rest: &str) -> Option<(State, &str)> {
    let mut characters = rest.chars();
    if characters.next()? != '[' {
        return None;
    }

    let state = State::from_symbol(characters.next()?)?;
    if characters.next()? != ']' {
        return None;
    }

    // The marker is three characters wide; if all three were read, the text
    // after it starts at the fourth.
    let text = rest
        .char_indices()
        .nth(3)
        .map_or("", |(offset, _)| rest.get(offset..).unwrap_or(""));

    Some((state, text))
}

/// `text` split into its leading whitespace and the rest.
fn split_whitespace(text: &str) -> (&str, &str) {
    text.find(|character: char| !character.is_ascii_whitespace())
        .map_or((text, ""), |start| text.split_at(start))
}

/// Trim a marker's text: a single separating space goes, the rest stays.
fn bullet_text(text: &str) -> String {
    text.strip_prefix(' ').unwrap_or(text).to_owned()
}

/// One node as a bullet line, with no indentation.
fn render_bullet(node: &Node) -> String {
    match node.marker {
        Some(state) if node.text.is_empty() => format!("- [{}]", state.symbol()),
        Some(state) => format!("- [{}] {}", state.symbol(), node.text),
        None => format!("- {}", node.text),
    }
}

/// A line at `depth`, two spaces per level.
fn indent(line: &str, depth: usize) -> String {
    let mut indented =
        String::with_capacity(line.len().saturating_add(depth.saturating_mul(LEVEL)));
    for _ in 0..depth {
        indented.push_str("  ");
    }
    indented.push_str(line);
    indented
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dossier the spec works through, task for task.
    const DOSSIER: &str = "\
# 1234 - refactor authentication
- Some context
- Random comment
  - [ ] Ask Tom how he'd do X
- [x] Check current auth docs
- [o] Split auth.rs into more modules
  - [x] Data structures
  - [o] Logic
    - I only need to fix a few more bugs
  - [?] API: waiting to talk with John about this
- [.] Update docs
- [x] Create tests
  - [o] Data structures
    - [x] Happy path
    - [ ] \"Evil\" path: malformed data, etc
  - [x] Logic
  - [?] API: waiting to talk with John about this
- [-] Ask boss about task: nah, he's on vacation the whole month
";

    /// The lines `section` prints for `content`.
    fn rendered(content: &str, section: Section) -> Vec<String> {
        Tree::parse(content).render(section)
    }

    /// The lines `section` prints, as owned strings.
    fn lines(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| (*line).to_owned()).collect()
    }

    #[test]
    fn reads_every_marker() {
        assert_eq!(State::from_symbol(' '), Some(State::Todo));
        assert_eq!(State::from_symbol('.'), Some(State::Doing));
        assert_eq!(State::from_symbol('o'), Some(State::Almost));
        assert_eq!(State::from_symbol('?'), Some(State::Waiting));
        assert_eq!(State::from_symbol('x'), Some(State::Done));
        assert_eq!(State::from_symbol('-'), Some(State::Cancelled));
        assert_eq!(State::from_symbol('X'), None);
        assert_eq!(State::from_symbol('!'), None);
    }

    #[test]
    fn every_marker_round_trips() {
        for state in [
            State::Todo,
            State::Doing,
            State::Almost,
            State::Waiting,
            State::Done,
            State::Cancelled,
        ] {
            assert_eq!(State::from_symbol(state.symbol()), Some(state));
        }
    }

    #[test]
    fn accepts_any_run_of_whitespace_after_the_dash() {
        let expected = lines(&["- [x] Done"]);

        assert_eq!(rendered("- [x] Done\n", Section::Finished), expected);
        assert_eq!(rendered("-[x] Done\n", Section::Finished), expected);
        assert_eq!(rendered("-  [x] Done\n", Section::Finished), expected);
        assert_eq!(rendered("-\t[x] Done\n", Section::Finished), expected);
    }

    #[test]
    fn a_marker_only_starts_a_task() {
        assert!(rendered("- I should check [x] later\n", Section::Finished).is_empty());
    }

    #[test]
    fn an_unknown_symbol_is_prose() {
        assert!(rendered("- [X] Done\n", Section::Finished).is_empty());
        assert!(rendered("- [!] Urgent\n", Section::Actionable).is_empty());
    }

    #[test]
    fn a_bare_dash_is_nothing() {
        assert!(rendered("-\n", Section::Actionable).is_empty());
    }

    #[test]
    fn an_empty_marker_is_a_task() {
        assert_eq!(rendered("- [ ]\n", Section::Actionable), lines(&["- [ ]"]));
        assert_eq!(rendered("- [o]\n", Section::Actionable), lines(&["- [o]"]));
    }

    #[test]
    fn indentation_width_is_not_assumed() {
        // Two, four and eight spaces all nest one level.
        for indent in ["  ", "    ", "        "] {
            let content = format!("- [o] A\n{indent}- [ ] B\n");
            let expected = lines(&["- [o] A", "  - [ ] B"]);

            assert_eq!(rendered(&content, Section::Actionable), expected);
        }
    }

    #[test]
    fn sibling_indentation_makes_siblings_at_any_width() {
        // Four spaces is one level here, so the two deeper bullets are
        // siblings and neither is the other's child.
        let content = "- [ ] a\n- [ ] b\n    - [ ] c\n    - [ ] d\n";
        let expected = lines(&["- [ ] a", "- [ ] b", "  - [ ] c", "  - [ ] d"]);

        assert_eq!(rendered(content, Section::Actionable), expected);
    }

    #[test]
    fn the_same_indentation_at_a_greater_depth_makes_siblings() {
        // The reported bug: `c` must not push `d` a level deeper each time.
        let content = "- [o] A\n      - [ ] B\n      - [ ] C\n";
        let expected = lines(&["- [o] A", "  - [ ] B", "  - [ ] C"]);

        assert_eq!(rendered(content, Section::Actionable), expected);
    }

    #[test]
    fn a_tab_nests_one_level_deep() {
        let content = "- [o] A\n\t- [ ] B\n";
        let expected = lines(&["- [o] A", "  - [ ] B"]);

        assert_eq!(rendered(content, Section::Actionable), expected);
    }

    #[test]
    fn a_deeper_indentation_that_is_not_a_multiple_still_nests() {
        // Odd widths are not rounded to a level: deeper is deeper.
        let content = "- [o] A\n   - [ ] B\n       - [ ] C\n";
        let expected = lines(&["- [o] A", "  - [ ] B", "    - [ ] C"]);

        assert_eq!(rendered(content, Section::Actionable), expected);
    }

    #[test]
    fn a_file_starting_deep_keeps_its_depth() {
        // Nothing above it is shallower, so it is a root -- but a bullet under
        // it is still one level deeper rather than level with it.
        let content = "        - [ ] Deep\n        - [ ] Sibling\n";
        let expected = lines(&["- [ ] Deep", "- [ ] Sibling"]);

        assert_eq!(rendered(content, Section::Actionable), expected);

        let nested = "        - [ ] Deep\n            - [ ] Deeper\n";
        let expected = lines(&["- [ ] Deep", "  - [ ] Deeper"]);

        assert_eq!(rendered(nested, Section::Actionable), expected);
    }

    #[test]
    fn blank_lines_and_continuations_do_not_disturb_the_tree() {
        let content = "\
- [o] A

  - [ ] B
    a continuation line
  - [ ] C
";
        let expected = lines(&["- [o] A", "  - [ ] B", "  - [ ] C"]);

        assert_eq!(rendered(content, Section::Actionable), expected);
    }

    #[test]
    fn headings_are_ignored() {
        let content = "## Pending\n- [ ] A\n\n## Completed\n- [x] B\n";

        assert_eq!(rendered(content, Section::Actionable), lines(&["- [ ] A"]));
        assert_eq!(rendered(content, Section::Finished), lines(&["- [x] B"]));
    }

    #[test]
    fn a_finished_parent_with_unfinished_work_prints_its_open_chain() {
        let content = "\
- [x] Create tests
  - [x] Logic
  - [o] Data structures
    - [ ] Evil path
";
        let expected = lines(&[
            "- [x] Create tests",
            "  - [o] Data structures",
            "    - [ ] Evil path",
        ]);

        assert_eq!(rendered(content, Section::Actionable), expected);
    }

    #[test]
    fn a_closed_child_of_an_open_task_never_prints() {
        let content = "- [o] Split auth.rs\n  - [x] Data structures\n  - [o] Logic\n";
        let expected = lines(&["- [o] Split auth.rs", "  - [o] Logic"]);

        assert_eq!(rendered(content, Section::Actionable), expected);
    }

    #[test]
    fn an_outdented_bullet_nests_under_the_last_shallower_one() {
        let content = "- [.] A\n    - [ ] B\n  - [ ] C\n- [ ] D\n";
        let expected = lines(&["- [.] A", "  - [ ] B", "  - [ ] C", "- [ ] D"]);

        assert_eq!(rendered(content, Section::Actionable), expected);
    }

    #[test]
    fn a_non_task_ancestor_prints_its_chain() {
        let content = "- Random message\n  - Some context\n    - [ ] Random task\n";
        let expected = lines(&[
            "- Random message",
            "  - Some context",
            "    - [ ] Random task",
        ]);

        assert_eq!(rendered(content, Section::Actionable), expected);
    }

    #[test]
    fn a_non_task_annotation_does_not_print() {
        let content = "- [o] Ask about the call\n  - he is back on Monday\n";

        // The task is pending, so it prints; the note under it is not a task
        // and is not on the way to one, so it does not.
        assert_eq!(
            rendered(content, Section::Actionable),
            lines(&["- [o] Ask about the call"])
        );

        let waiting = "- [?] Waiting on John\n  - note about the call\n";
        assert_eq!(
            rendered(waiting, Section::Blocked),
            lines(&["- [?] Waiting on John"])
        );
    }

    #[test]
    fn a_blocked_child_leaves_its_open_parent_actionable() {
        let content = "- [o] Split auth.rs\n  - [?] API: waiting\n";
        let blocked = lines(&["- [o] Split auth.rs", "  - [?] API: waiting"]);

        assert_eq!(rendered(content, Section::Blocked), blocked);
        // The parent is in progress on its own account, so it stays workable
        // even when the one thing left under it is not. Only the waiting child
        // is missing from the section.
        assert_eq!(
            rendered(content, Section::Actionable),
            lines(&["- [o] Split auth.rs"])
        );
    }

    #[test]
    fn a_finished_child_leaves_its_open_parent_actionable() {
        // A parent can be more than the sum of its children: `A` is not done
        // just because `X` and `Y` are.
        let content = "- [.] Implement feat A\n  - [x] Do X\n  - [x] Do Y\n";

        assert_eq!(
            rendered(content, Section::Actionable),
            lines(&["- [.] Implement feat A"])
        );
    }

    #[test]
    fn a_waiting_task_is_not_pending_whatever_is_under_it() {
        let content = "- [?] Waiting on John\n  - [ ] A\n";
        let blocked = lines(&["- [?] Waiting on John", "  - [ ] A"]);

        assert_eq!(rendered(content, Section::Blocked), blocked);
        assert!(rendered(content, Section::Actionable).is_empty());
    }

    #[test]
    fn an_open_sibling_of_a_waiting_task_is_still_pending() {
        let content = "- [o] Split auth.rs\n  - [?] API: waiting\n  - [ ] Logic\n";
        let pending = lines(&["- [o] Split auth.rs", "  - [ ] Logic"]);

        assert_eq!(rendered(content, Section::Actionable), pending);
    }

    #[test]
    fn a_block_covers_the_branch_below_it() {
        let content = "- [?] Waiting on John\n  - [ ] A\n    - [ ] B\n";
        let expected = lines(&["- [?] Waiting on John", "  - [ ] A", "    - [ ] B"]);

        assert_eq!(rendered(content, Section::Blocked), expected);
    }

    #[test]
    fn a_finished_task_under_a_blocked_one_is_not_listed_as_blocked() {
        let content = "- [?] Waiting on John\n  - [x] Filed the ticket\n";

        assert_eq!(
            rendered(content, Section::Blocked),
            lines(&["- [?] Waiting on John"])
        );
    }

    #[test]
    fn a_finished_task_with_unfinished_work_is_not_finished() {
        let content = "- [x] Create tests\n  - [o] Data structures\n";

        assert!(
            rendered(content, Section::Finished).is_empty(),
            "a closed task with work left under it is not over"
        );
    }

    #[test]
    fn a_closed_subtree_under_an_excluded_one_stays_out() {
        let content = "\
- [x] Create tests
  - [o] Data structures
    - [x] Happy path
";
        assert!(
            rendered(content, Section::Finished).is_empty(),
            "the path to a closed task runs through an excluded one"
        );
    }

    #[test]
    fn a_cancelled_task_with_unfinished_work_is_not_finished() {
        assert!(rendered("- [-] Dropped\n  - [ ] Still here\n", Section::Finished).is_empty());
    }

    #[test]
    fn a_deep_unfinished_task_excludes_its_whole_chain() {
        assert!(rendered("- [x] A\n  - [x] B\n    - [ ] C\n", Section::Finished).is_empty());
    }

    #[test]
    fn a_closed_task_with_a_closed_child_is_finished() {
        let content = "- [x] A\n  - [x] B\n  - [-] C\n";
        let expected = lines(&["- [x] A", "  - [x] B", "  - [-] C"]);

        assert_eq!(rendered(content, Section::Finished), expected);
    }

    #[test]
    fn an_unfinished_ancestor_of_a_closed_task_prints_with_its_own_marker() {
        let content = "- [o] Split auth.rs\n  - [x] Data structures\n";
        let expected = lines(&["- [o] Split auth.rs", "  - [x] Data structures"]);

        assert_eq!(rendered(content, Section::Finished), expected);
    }

    #[test]
    fn a_sibling_of_a_match_never_prints() {
        let content = "- [o] Split auth.rs\n  - [x] Data structures\n  - [ ] Logic\n";
        let expected = lines(&["- [o] Split auth.rs", "  - [ ] Logic"]);

        assert_eq!(rendered(content, Section::Actionable), expected);
    }

    #[test]
    fn a_dossier_with_no_tasks_prints_nothing() {
        let content = "# Notes\n- just a note\n- Another comment\n";

        assert!(rendered(content, Section::Actionable).is_empty());
        assert!(rendered(content, Section::Blocked).is_empty());
        assert!(rendered(content, Section::Finished).is_empty());
    }

    #[test]
    fn renders_the_worked_example_section_by_section() {
        // `- Some context` and `- [x] Check current auth docs` are the two
        // bullets of the example that print nowhere: the first has no task
        // under it, and the second is finished with nothing left below it.
        let pending = lines(&[
            "- Random comment",
            "  - [ ] Ask Tom how he'd do X",
            "- [o] Split auth.rs into more modules",
            "  - [o] Logic",
            "- [.] Update docs",
            "- [x] Create tests",
            "  - [o] Data structures",
            "    - [ ] \"Evil\" path: malformed data, etc",
        ]);
        let blocked = lines(&[
            "- [o] Split auth.rs into more modules",
            "  - [?] API: waiting to talk with John about this",
            "- [x] Create tests",
            "  - [?] API: waiting to talk with John about this",
        ]);
        let finished = lines(&[
            "- [x] Check current auth docs",
            "- [o] Split auth.rs into more modules",
            "  - [x] Data structures",
            "- [-] Ask boss about task: nah, he's on vacation the whole month",
        ]);

        assert_eq!(rendered(DOSSIER, Section::Actionable), pending);
        assert_eq!(rendered(DOSSIER, Section::Blocked), blocked);
        assert_eq!(rendered(DOSSIER, Section::Finished), finished);
    }

    #[test]
    fn markers_are_read_off_the_tree() {
        let tree = Tree::parse("- [ ] A\n  - [x] B\n- plain\n");

        assert_eq!(tree.marker(0), Some(State::Todo));
        assert_eq!(tree.marker(1), Some(State::Done));
        assert_eq!(tree.marker(2), None);
    }
}
