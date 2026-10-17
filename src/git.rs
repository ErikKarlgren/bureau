//! The little bit of `git` that bureau needs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use anyhow::{Context, bail};
use chrono::NaiveDate;

use crate::Result;

/// The root of the repository bureau was run from.
///
/// # Errors
///
/// Fails when the current directory is not inside a git repository, or when
/// `git` itself cannot be run.
pub fn toplevel() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("could not run git")?;

    if !output.status.success() {
        bail!("not inside a git repository: {}", stderr(&output));
    }

    let path = String::from_utf8(output.stdout).context("git returned a path that is not UTF-8")?;
    Ok(PathBuf::from(path.trim_end()))
}

/// Stage and commit the given files, leaving any other staged changes alone.
///
/// # Errors
///
/// Fails when `git add` or `git commit` exits with an error. The message
/// carries git's own explanation.
pub fn commit(paths: &[&Path], message: &str) -> Result<()> {
    let added = Command::new("git")
        .arg("add")
        .args(paths)
        .output()
        .context("could not run git")?;
    if !added.status.success() {
        bail!("git add failed: {}", stderr(&added));
    }

    let committed = Command::new("git")
        .args(["commit", "--only", "-m", message])
        .args(paths)
        .output()
        .context("could not run git")?;
    if !committed.status.success() {
        bail!("git commit failed: {}", stderr(&committed));
    }

    Ok(())
}

/// Whatever git had to say for itself, for use in an error message.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_owned()
}

/// The settings pinned on every `git diff`, so the bytes a report prints do not
/// depend on a user's or a repository's configuration.
///
/// `-c` has to come before the subcommand, and each of these is a setting that
/// changes the output or runs somebody's tool if it is left alone.
const PINNED: [&str; 12] = [
    "diff.algorithm=histogram",
    "diff.renames=true",
    "diff.noprefix=false",
    "diff.mnemonicPrefix=false",
    "diff.wsErrorHighlight=none",
    "color.moved=false",
    "color.diff.meta=bold",
    "color.diff.frag=bold",
    "color.diff.old=red",
    "color.diff.new=green",
    "color.diff.func=",
    "core.quotepath=false",
];

/// `path` as git names it: relative to the repository root.
///
/// Git echoes a path in a diff header relative to the root whatever form the
/// pathspec took, and [`last_commits`] reports the paths `git log` hands back,
/// which are relative too. A caller that found a dossier through
/// `sources::read_markdown` holds an absolute path, so it needs this to look
/// either one up.
///
/// A path that is already relative, or that lies outside the root, is returned
/// unchanged: git will simply not match it, which is the same answer the old
/// per-path calls gave.
#[must_use]
pub fn relative<'a>(root: &Path, path: &'a Path) -> &'a Path {
    path.strip_prefix(root).unwrap_or(path)
}

/// Whether the repository has a commit to compare against.
///
/// An unborn `HEAD` is not an error: a fresh repository simply has no history,
/// and a report over it has no diffs.
///
/// # Errors
///
/// Fails when `git` cannot be run.
pub fn has_head(root: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .current_dir(root)
        .output()
        .context("could not run git")?;

    Ok(output.status.success())
}

/// The newest commit touching each path strictly before `date`.
///
/// One history walk answers this for every path at once, which is the whole
/// point: `git rev-list -1 --before=` walks the graph from `HEAD` down to the
/// date for *each* path it is asked about, so a report over N dossiers paid
/// that walk N times. `git log --name-only` with the same date limit walks
/// once and reports the paths each commit touched, and the first commit seen
/// for a path is the newest one, because `git log` lists commits newest first.
///
/// A path with no answer is absent from the map, not mapped to `None`; callers
/// treat absence as "did not exist yet".
///
/// # Errors
///
/// Fails when `git` cannot be run, the history cannot be read, or a path git
/// reports is not UTF-8.
pub fn last_commits<'p, I>(
    root: &Path,
    date: NaiveDate,
    paths: I,
) -> Result<HashMap<String, String>>
where
    I: IntoIterator<Item = &'p Path>,
{
    let output = Command::new("git")
        .arg("--no-pager")
        .args(["-c", "core.quotepath=false"])
        .args(["log", "-z", "--format=%H", "--name-only"])
        .arg(year_first_instant(date))
        .arg("HEAD")
        .arg("--")
        .args(paths)
        .current_dir(root)
        .output()
        .context("could not run git")?;

    if !output.status.success() {
        bail!("git log failed: {}", stderr(&output));
    }

    parse_walk(&output.stdout)
}

/// The `--before` bound for a day: the first instant of `date`.
///
/// `git rev-list --before` compares the committer timestamp against this, so a
/// commit made at any time on the day before `date` is included and one made on
/// `date` itself is not.
fn year_first_instant(date: NaiveDate) -> String {
    format!("--before={date}T00:00:00")
}

/// Read `git log -z --format=%H --name-only` output into path to newest commit.
///
/// `-z` makes every field NUL-terminated, so a path holding any byte but NUL
/// needs no escaping and a path is never confused with a neighbouring field.
/// The only two shapes a non-empty field has are a 40-character commit hash and
/// a path. A path can itself be 40 hex characters, so a hash is only read as a
/// hash when it does not directly follow another hash; in `hash, path, path,
/// hash, path` the second hash that follows a path is unambiguous.
fn parse_walk(bytes: &[u8]) -> Result<HashMap<String, String>> {
    let mut newest = HashMap::new();
    let mut commit: Option<&[u8]> = None;
    let mut previous_was_hash = false;

    for field in bytes.split(|byte| *byte == 0) {
        let field = field.strip_prefix(b"\n").unwrap_or(field);

        if field.is_empty() {
            continue;
        }

        let is_hash =
            field.len() == 40 && field.iter().all(u8::is_ascii_hexdigit) && !previous_was_hash;

        if is_hash {
            commit = Some(field);
            previous_was_hash = true;
            continue;
        }

        previous_was_hash = false;

        let Some(hash) = commit else {
            bail!("git log named a path before any commit");
        };

        let path = std::str::from_utf8(field)
            .context("git returned a path that is not UTF-8")?
            .to_owned();

        // The first commit a path appears under is the newest one that touched
        // it, so a later appearance is older and is left alone.
        if let std::collections::hash_map::Entry::Vacant(slot) = newest.entry(path) {
            let hash = std::str::from_utf8(hash)
                .context("git returned a commit that is not UTF-8")?
                .to_owned();
            slot.insert(hash);
        }
    }

    Ok(newest)
}

/// The hash of the empty tree, in whatever object format the repository uses.
///
/// A dossier created inside the period has no commit before `--from` to diff
/// against, so the empty tree is its base.
///
/// # Errors
///
/// Fails when `git` cannot be run.
pub fn empty_tree(root: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("mktree")
        .stdin(Stdio::null())
        .current_dir(root)
        .output()
        .context("could not run git")?;

    if !output.status.success() {
        bail!("git mktree failed: {}", stderr(&output));
    }

    let hash = String::from_utf8_lossy(&output.stdout);
    Ok(hash.trim().to_owned())
}

/// The diffs of many paths between the same two trees, in one `git diff`.
///
/// A `base` of `None` is the empty tree, which is what a dossier created inside
/// the period is diffed against; `git diff` spells that as the empty tree's own
/// hash, so the caller never has to know it.
///
/// Every path in `paths` is answered by a single invocation, and the bytes are
/// split back apart by [`FILE_MARKER`]. The paths are used as the pathspec, so
/// a path with no changes is simply absent from the map.
///
/// `--follow` is deliberately not passed. It only ever affected a path that the
/// base tree does not hold under the same name, and the report's base for a
/// dossier always comes from a walk that name-limited to that dossier's own
/// current path -- so the walk never follows the rename either, and the base it
/// finds is a commit where the path already exists. A one-path `git diff` over
/// a path present in both trees never performs rename detection, so `--follow`
/// could not change a byte here. Removing it also removes the `git --version`
/// call and the version warning that used to guard it.
///
/// # Errors
///
/// Fails when `git` cannot be run or the diff cannot be produced.
pub fn diffs<'p, I>(
    root: &Path,
    base: Option<&str>,
    head: &str,
    paths: I,
    color: bool,
) -> Result<HashMap<String, String>>
where
    I: IntoIterator<Item = &'p Path>,
{
    let paths: Vec<&Path> = paths.into_iter().collect();

    let mut command = Command::new("git");
    command.arg("--no-pager").current_dir(root);

    for setting in PINNED {
        command.args(["-c", setting]);
    }

    command
        .arg("diff")
        .args(["--no-ext-diff", "--no-textconv", "--no-color-moved"])
        .arg(if color {
            "--color=always"
        } else {
            "--no-color"
        })
        .args(["--line-prefix", FILE_MARKER]);

    clear_injected_config(&mut command);

    let output = command
        .arg(base.unwrap_or(EMPTY_TREE))
        .arg(head)
        .arg("--")
        .args(&paths)
        .env_remove("GIT_EXTERNAL_DIFF")
        .env_remove("GIT_DIFF_OPTS")
        .output()
        .context("could not run git")?;

    if !output.status.success() {
        bail!("git diff failed: {}", stderr(&output));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut diffs = parse_diff(&text);

    // Every path asked about gets an answer, so a caller can tell "no changes"
    // from "not asked" without consulting the input again.
    for path in paths {
        let key = path.to_str().unwrap_or_default().to_owned();
        diffs.entry(key).or_default();
    }

    Ok(diffs)
}

/// The marker that starts each file's diff in the batched output.
///
/// `--line-prefix` puts it at the front of *every* line, which is what makes
/// the split exact: a `diff --git` line inside a file's own content is prefixed
/// too, so it cannot be mistaken for the start of the next file. The marker is
/// removed again before the bytes are handed back, and it is one byte for the
/// same reason. Colour codes are emitted after the prefix, so a coloured diff
/// is split -- and reassembled -- exactly like a plain one.
const FILE_MARKER: &str = "\u{1}";

/// The sequence git ends every coloured line with.
const RESET: &str = "\u{1b}[m";

/// The empty tree's object id in a SHA-1 repository, which is every repository
/// bureau is likely to meet. A SHA-256 repository has a different one, which is
/// why [`empty_tree`] asks the repository rather than assuming this.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// Split batched `git diff` output back into one diff per file.
///
/// A path git produced no diff for gets an empty string rather than being
/// absent, because the caller's question is "what is this path's diff" and "no
/// changes" is an answer, not a missing one. A path the caller asked about and
/// the walk did not report never reaches here; the caller leaves those out.
fn parse_diff(text: &str) -> HashMap<String, String> {
    let mut diffs: HashMap<String, String> = HashMap::new();
    let mut current: Option<String> = None;
    let mut body = String::new();

    for line in text.split_inclusive('\n') {
        let Some(rest) = line.strip_prefix(FILE_MARKER) else {
            continue;
        };

        // A coloured line opens with the SGR sequence git paints this kind of
        // line with, so which kind it is has to be read past that. The line
        // itself is kept whole: git's own codes, the reset that ends each
        // coloured line included, are the bytes a one-path diff would carry.
        if let Some(pair) = unpaint(rest).strip_prefix("diff --git ") {
            if let Some(previous) = current.replace(diff_path(pair)) {
                diffs.insert(previous, std::mem::take(&mut body));
            }

            body.push_str(rest);
            continue;
        }

        if current.is_some() {
            body.push_str(rest);
        }
    }

    if let Some(previous) = current {
        diffs.insert(previous, body);
    }

    diffs
}

/// The text of a line with any leading SGR colour sequence removed.
///
/// A coloured line opens with `ESC [ ... m`; the bytes that follow are the
/// bytes the same line has when it is not coloured.
fn unpaint(line: &str) -> &str {
    let Some(rest) = line.strip_prefix('\u{1b}') else {
        return line;
    };

    let Some(rest) = rest.strip_prefix('[') else {
        return line;
    };

    rest.find('m')
        .map_or(line, |end| rest.get(end.saturating_add(1)..).unwrap_or(""))
}

/// The new path out of a `diff --git a/<old> b/<new>` line.
///
/// The two paths are separated by ` b/`, and the first one cannot contain that
/// sequence, so the last occurrence starts the second path. A path git had to
/// quote for reasons `core.quotepath` does not control is unquoted here.
fn diff_path(pair: &str) -> String {
    // A coloured `diff --git` line resets its colour at the end, which lands
    // between the path and the newline.
    let pair = pair.strip_suffix('\n').unwrap_or(pair);
    let pair = pair.strip_suffix(RESET).unwrap_or(pair);

    let new = pair.rsplit_once(" b/").map_or(pair, |(_, new)| new);

    unquote(new)
}

/// Undo git's C-style quoting of a path, if this one is quoted.
///
/// `core.quotepath=false` is pinned on the diff, so a quoted path here means
/// only that it holds a character git always quotes -- a quote, a backslash or
/// a control character. Octal escapes are read back the way git writes them,
/// and anything unrecognised is left as it came.
fn unquote(path: &str) -> String {
    let Some(inner) = path
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return path.to_owned();
    };

    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();

    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }

        let Some(escaped) = chars.next() else {
            out.push('\\');
            break;
        };

        if let Some(octal) = escaped.to_digit(8) {
            // Git writes one byte per escape, so the value is assembled from up
            // to three octal digits and pushed as the UTF-8 encoding of that
            // byte, which is what the original file name held.
            let mut value = octal;
            let mut digits: u32 = 1;

            while digits < 3 {
                let Some(next) = chars.clone().next().and_then(|c| c.to_digit(8)) else {
                    break;
                };
                chars.next();
                let Some(scaled) = value.checked_mul(8) else {
                    break;
                };
                let Some(sum) = scaled.checked_add(next) else {
                    break;
                };
                value = sum;
                digits = digits.saturating_add(1);
            }

            out.push(char::from(u8::try_from(value).unwrap_or(b'?')));
            continue;
        }

        match escaped {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            other => out.push(other),
        }
    }

    out
}

/// Drop the environment's own `-c` injections, so the pinned settings are the
/// only ones in play.
fn clear_injected_config(command: &mut Command) {
    for (key, _) in std::env::vars_os() {
        let Some(key) = key.to_str() else {
            continue;
        };

        if key == "GIT_CONFIG_COUNT"
            || key.starts_with("GIT_CONFIG_KEY_")
            || key.starts_with("GIT_CONFIG_VALUE_")
        {
            command.env_remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::commands::tests::Scratch;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    /// Run git in `root`, failing the test when it does.
    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "bureau")
            .env("GIT_AUTHOR_EMAIL", "bureau@example.invalid")
            .env("GIT_COMMITTER_NAME", "bureau")
            .env("GIT_COMMITTER_EMAIL", "bureau@example.invalid")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// A repository with one committed file and a second commit changing it.
    fn repository(name: &str) -> Scratch {
        let scratch = Scratch::new(name).unwrap();
        let root = scratch.path();
        git(root, &["init", "-q"]);
        scratch.write("f.md", "one\ntwo\n").unwrap();
        git(root, &["add", "f.md"]);
        git(root, &["commit", "-qm", "one"]);
        scratch.write("f.md", "one\nTWO\n").unwrap();
        git(root, &["commit", "-qam", "two"]);
        scratch
    }

    /// The one path's diff, or empty text when the batch had none for it.
    fn diff_of(root: &Path, base: Option<&str>, head: &str, path: &str, color: bool) -> String {
        diffs(root, base, head, [Path::new(path)], color)
            .unwrap()
            .remove(path)
            .unwrap_or_default()
    }

    /// What `git rev-list -1 --before=... HEAD -- <path>` says, one path at a
    /// time, which is what the batched walk has to agree with.
    fn rev_list(root: &Path, date: NaiveDate, path: &str) -> Option<String> {
        let output = Command::new("git")
            .args([
                "rev-list",
                "-1",
                &format!("--before={date}T00:00:00"),
                "HEAD",
                "--",
                path,
            ])
            .current_dir(root)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "git rev-list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let hash = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (!hash.is_empty()).then_some(hash)
    }

    #[test]
    fn history_helpers_read_the_repository() {
        let scratch = repository("git-history");
        let root = scratch.path();

        assert!(has_head(root).unwrap());
        assert_eq!(
            empty_tree(root).unwrap(),
            "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
        );

        let unborn = Scratch::new("git-unborn").unwrap();
        git(unborn.path(), &["init", "-q"]);
        assert!(!has_head(unborn.path()).unwrap());
    }

    #[test]
    fn one_walk_answers_every_path() {
        let scratch = repository("git-walk");
        let root = scratch.path();
        scratch.write("other.md", "one\n").unwrap();
        git(root, &["add", "other.md"]);
        git(root, &["commit", "-qm", "other"]);
        scratch.write("other.md", "one\nTWO\n").unwrap();
        git(root, &["commit", "-qam", "other again"]);
        // A path with a space and a path with a non-ASCII character, because
        // both have to survive the walk's encoding.
        scratch.write("with space.md", "x\n").unwrap();
        scratch.write("kött.md", "y\n").unwrap();
        git(root, &["add", "--", "with space.md", "kött.md"]);
        git(root, &["commit", "-qm", "two more"]);

        let paths = [
            Path::new("f.md"),
            Path::new("other.md"),
            Path::new("with space.md"),
            Path::new("kött.md"),
            Path::new("never.md"),
        ];

        let walked = last_commits(root, date(2030, 1, 1), paths).unwrap();

        for path in paths {
            let path = path.to_str().unwrap();
            assert_eq!(
                walked.get(path).map(String::as_str),
                rev_list(root, date(2030, 1, 1), path).as_deref(),
                "{path}"
            );
        }

        // A date before the repository existed has no answer for anything.
        let none = last_commits(root, date(2000, 1, 1), paths).unwrap();
        assert!(none.is_empty(), "{none:?}");
    }

    #[test]
    fn a_batched_diff_is_the_bytes_of_one_alone() {
        let scratch = repository("git-batched");
        let root = scratch.path();
        scratch.write("second.md", "alpha\n").unwrap();
        git(root, &["add", "second.md"]);
        git(root, &["commit", "-qm", "second"]);
        scratch.write("f.md", "one\nTWO\nTHREE\n").unwrap();
        scratch.write("second.md", "alpha\nBETA\n").unwrap();
        git(root, &["commit", "-qam", "both again"]);

        let paths = [
            Path::new("f.md"),
            Path::new("second.md"),
            Path::new("absent.md"),
        ];
        let batched = diffs(root, Some("HEAD~1"), "HEAD", paths, false).unwrap();

        for path in paths {
            let path = path.to_str().unwrap();

            // `git diff` on this path alone, with the same pinned settings.
            let alone = Command::new("git")
                .args(["-c", "diff.algorithm=histogram"])
                .args(["diff", "--no-color", "HEAD~1", "HEAD", "--", path])
                .current_dir(root)
                .output()
                .unwrap();

            assert_eq!(
                batched.get(path).map(String::as_str),
                Some(String::from_utf8_lossy(&alone.stdout).into_owned().as_str()),
                "{path}"
            );
        }
    }

    #[test]
    fn a_rename_is_not_followed_into_a_diff() {
        // The report's base for a path comes from a walk that is limited to
        // that path and does not follow renames, so the base always holds the
        // path under its current name -- and a one-path diff over a path in
        // both trees never runs rename detection. The batched diff therefore
        // matches git with and without `--follow` here.
        let scratch = repository("git-rename");
        let root = scratch.path();
        git(root, &["mv", "f.md", "g.md"]);
        git(root, &["add", "-A"]);
        git(root, &["commit", "-qm", "renamed"]);
        // A commit after the rename, so the base is the rename itself and the
        // path exists under its current name on both sides of the diff.
        scratch.write("g.md", "one\nTWO\nTHREE\n").unwrap();
        git(root, &["commit", "-qam", "changed after the rename"]);

        // The rename commit, which holds `g.md`, against the commit after it:
        // the same shape the report diffs, with the path on both sides.
        let rename = rev_list(root, date(2030, 1, 1), "g.md").unwrap();
        let base = {
            let out = Command::new("git")
                .args(["rev-parse", &format!("{rename}~1")])
                .current_dir(root)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        };
        assert_ne!(base, rename, "the fixture did not rename anything");

        let batched = diff_of(root, Some(&base), "HEAD", "g.md", false);

        let followed = Command::new("git")
            .args(["-c", "diff.algorithm=histogram"])
            .args([
                "diff",
                "--no-color",
                "--follow",
                &base,
                "HEAD",
                "--",
                "g.md",
            ])
            .current_dir(root)
            .output()
            .unwrap();

        assert_eq!(
            batched,
            String::from_utf8_lossy(&followed.stdout).into_owned(),
            "base={base} batched={batched:?} followed={:?}",
            String::from_utf8_lossy(&followed.stdout)
        );
        assert!(
            batched.contains("+THREE"),
            "base={base} batched={batched:?} followed={:?}",
            String::from_utf8_lossy(&followed.stdout)
        );
    }

    #[test]
    fn a_hostile_configuration_cannot_change_the_diff() {
        let scratch = repository("git-pinned");
        let root = scratch.path();
        git(root, &["config", "diff.algorithm", "myers"]);
        git(root, &["config", "diff.noprefix", "true"]);
        git(root, &["config", "color.diff.meta", "yellow"]);
        git(root, &["config", "color.diff.frag", "magenta"]);
        git(
            root,
            &["config", "diff.external", "sh -c 'echo EXTERNAL_RAN'"],
        );
        git(
            root,
            &["config", "diff.bin.textconv", "sh -c 'echo TEXTCONV_RAN'"],
        );
        scratch.write(".gitattributes", "*.md diff=bin\n").unwrap();

        // Control: unpinned, git really does run the configured tool.
        let raw = Command::new("git")
            .args(["diff", "HEAD~1", "HEAD", "--", "f.md"])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&raw.stdout).contains("EXTERNAL_RAN"),
            "the control did not run the external diff"
        );

        let diff = diff_of(root, Some("HEAD~1"), "HEAD", "f.md", false);

        assert!(diff.contains("-two"), "{diff}");
        assert!(diff.contains("+TWO"), "{diff}");
        assert!(!diff.contains('\x1b'), "{diff:?}");
        assert!(!diff.contains("EXTERNAL_RAN"), "{diff}");
        assert!(!diff.contains("TEXTCONV_RAN"), "{diff}");
    }

    #[test]
    fn a_coloured_diff_uses_only_the_pinned_slots() {
        let scratch = repository("git-colour");
        let root = scratch.path();
        git(root, &["config", "color.diff.meta", "yellow"]);
        git(root, &["config", "color.diff.frag", "magenta"]);
        git(root, &["config", "color.moved", "true"]);

        // Control: unpinned, the hostile slots leak into the output.
        let raw = Command::new("git")
            .args([
                "-c",
                "color.ui=always",
                "diff",
                "HEAD~1",
                "HEAD",
                "--",
                "f.md",
            ])
            .current_dir(root)
            .output()
            .unwrap();
        let raw = String::from_utf8_lossy(&raw.stdout).into_owned();
        assert!(raw.contains("33m"), "the control lost its yellow: {raw:?}");

        let diff = diff_of(root, Some("HEAD~1"), "HEAD", "f.md", true);

        assert!(diff.contains("31m"), "no red: {diff:?}");
        assert!(diff.contains("32m"), "no green: {diff:?}");
        for leaked in ["33m", "34m", "35m", "36m"] {
            assert!(!diff.contains(leaked), "{leaked} leaked: {diff:?}");
        }
    }

    #[test]
    fn a_missing_path_is_not_a_source_of_bytes() {
        let scratch = repository("git-missing");
        let root = scratch.path();
        fs::write(root.join("other.md"), "x\n").unwrap();

        let diffs = diffs(root, Some("HEAD~1"), "HEAD", [Path::new("other.md")], false).unwrap();

        assert_eq!(
            diffs.get("other.md").map(String::as_str),
            Some(""),
            "{diffs:?}"
        );
    }
}
