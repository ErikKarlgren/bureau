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
    /// after at most three spaces of indentation. A line that opens a fence is
    /// not itself content.
    #[must_use]
    pub fn opening(line: &str) -> Option<Self> {
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
        (length >= 3).then_some(Self { character, length })
    }

    /// Whether `line` closes this fence.
    ///
    /// The closing fence uses the same character, is at least as long as the
    /// opening one, and has nothing but whitespace after it.
    #[must_use]
    pub fn closes(self, line: &str) -> bool {
        let indent = line
            .len()
            .saturating_sub(line.trim_start_matches(' ').len());
        if indent > 3 {
            return false;
        }

        let Some(rest) = line.get(indent..) else {
            return false;
        };
        let count = rest
            .chars()
            .take_while(|found| *found == self.character)
            .count();
        if count < self.length {
            return false;
        }

        rest.get(count..)
            .is_some_and(|after| after.trim().is_empty())
    }
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
