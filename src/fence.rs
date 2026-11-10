//! Fenced code blocks, read the same way by every command.
//!
//! Both `bureau tasks` and `bureau doctor` have to know where a fenced block
//! starts and ends: a bullet inside a fence is an example, not work. Keeping
//! the rule here is what stops the two commands from disagreeing about it.

/// An open fenced code block: the fence character and how many of it opened it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fence {
    character: char,
    length: usize,
}

impl Fence {
    /// The fence `line` opens, if it opens one.
    ///
    /// A fence is three or more backticks or tildes at the start of a line,
    /// after at most three spaces of indentation. A backtick fence's info
    /// string may not contain a backtick, so a line such as `` ``` `x` ``` ``
    /// is prose, not an opening fence. A line that opens a fence is not
    /// itself content.
    #[must_use]
    pub fn opening(line: &str) -> Option<Self> {
        let (character, length, info) = split(line)?;

        if character == '`' && info.contains('`') {
            return None;
        }

        Some(Self { character, length })
    }

    /// The fence run `line` starts with, whether or not it opens a fence.
    ///
    /// This is the indentation and character rule without the info-string
    /// rule, for a caller that only needs to know how long a run could be,
    /// such as the report's fence sizing.
    #[must_use]
    pub fn run(line: &str) -> Option<(char, usize)> {
        split(line).map(|(character, length, _)| (character, length))
    }

    /// Whether `line` closes this fence.
    ///
    /// The closing fence uses the same character, is at least as long as the
    /// opening one, and has nothing but whitespace after it.
    #[must_use]
    pub fn closes(self, line: &str) -> bool {
        let Some((character, length, after)) = split(line) else {
            return false;
        };

        character == self.character && length >= self.length && after.trim().is_empty()
    }
}

/// The fence character, run length and text after the run, for a line that
/// starts at most three spaces in with three or more fence characters.
fn split(line: &str) -> Option<(char, usize, &str)> {
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
    if length < 3 {
        return None;
    }

    // Every counted character is the ASCII fence character, so the character
    // count is also a byte offset.
    let after = rest.get(length..)?;

    Some((character, length, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_on_three_or_more_backticks_or_tildes() {
        assert!(Fence::opening("```").is_some());
        assert!(Fence::opening("````").is_some());
        assert!(Fence::opening("~~~").is_some());
        assert!(Fence::opening("   ```").is_some());
        assert!(Fence::opening("```lua").is_some());

        assert!(Fence::opening("``").is_none());
        assert!(Fence::opening("    ```").is_none());
        assert!(Fence::opening("text ```").is_none());
    }

    #[test]
    fn a_backtick_info_string_may_not_contain_a_backtick() {
        assert!(Fence::opening("``` `x` ```").is_none());
        assert!(Fence::opening("~~~ `x`").is_some());

        // `run` still sees the backtick run; it is the opening rule that
        // rejects the line.
        assert_eq!(Fence::run("``` `x` ```"), Some(('`', 3)));
    }

    #[test]
    fn closes_only_on_the_same_character_and_at_least_the_same_length() {
        let backticks = Fence::opening("```").unwrap();

        assert!(backticks.closes("```"));
        assert!(backticks.closes("````"));
        assert!(backticks.closes("   ```  "));
        assert!(!backticks.closes("``"));
        assert!(!backticks.closes("~~~"));
        assert!(!backticks.closes("```lua"));
        assert!(!backticks.closes("    ```"));
    }
}
