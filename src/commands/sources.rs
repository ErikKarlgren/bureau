//! Finding the notes a command can read, and dropping the sealed ones.

use std::cmp::Reverse;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};

use crate::commands::paths::DOSSIERS_DIR;

/// One markdown file in the repository, and whether it is sealed.
#[derive(Debug)]
pub struct Source {
    /// Where the file is.
    pub path: PathBuf,
    /// Whether its frontmatter marks it sealed.
    pub sealed: bool,
}

impl Source {
    /// Whether the file may be listed, which sealed dossiers may not.
    ///
    /// Sealed notes are skipped by every command, `--all` flags included:
    /// sealing is how a dossier is put out of the way, and a listing that can
    /// be talked into showing one is not much of a seal.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        !self.sealed
    }
}

/// Every `.md` file in `directory`, at the repository root.
///
/// A directory that is not there is not an error: a repository with no
/// dossiers yet simply has no dossiers.
///
/// # Errors
///
/// Fails when the directory is there but cannot be read, or when a file
/// cannot be read to check whether it is sealed.
pub fn read_markdown(root: &Path, directory: &str) -> Result<Vec<Source>> {
    let directory = root.join(directory);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read '{}'", directory.display()));
        }
    };

    let mut sources = Vec::new();
    for entry in entries {
        let path = entry
            .with_context(|| format!("could not read '{}'", directory.display()))?
            .path();
        if path.extension().is_some_and(|extension| extension == "md") {
            sources.push(source_of(path)?);
        }
    }

    Ok(sources)
}

/// One file as a [`Source`], reading its frontmatter to see whether it is
/// sealed.
fn source_of(path: PathBuf) -> Result<Source> {
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("could not read '{}'", path.display()))?;

    Ok(Source {
        sealed: is_sealed(&contents),
        path,
    })
}

/// Whether a file's frontmatter marks it sealed.
///
/// Only frontmatter counts: a `sealed:` line further down the file is prose
/// about sealing, not a seal. The check is deliberately literal, because
/// `bureau seal` is not written yet and the field has no other reader.
#[must_use]
pub fn is_sealed(contents: &str) -> bool {
    let mut lines = contents.lines();

    if lines.next().map(str::trim_end) != Some("---") {
        return false;
    }

    lines
        .take_while(|line| line.trim_end() != "---")
        .any(|line| line.trim_end().starts_with("sealed:"))
}

/// The paths of the `sources` that may be listed.
#[must_use]
pub fn open_paths(sources: &[Source]) -> Vec<PathBuf> {
    sources
        .iter()
        .filter(|source| source.is_open())
        .map(|source| source.path.clone())
        .collect()
}

/// Every open dossier in the repository, most recently modified first.
///
/// # Errors
///
/// Fails when the dossiers directory is there but cannot be read, or when one
/// of the files cannot be read to check whether it is sealed.
pub fn read_dossiers(root: &Path) -> Result<Vec<PathBuf>> {
    Ok(by_recency(open_paths(&read_markdown(root, DOSSIERS_DIR)?)))
}

/// Paths sorted most recently modified first, by name within equal times.
///
/// A fresh `git clone` stamps every file with the same time, which is why the
/// name is there to break the tie.
#[must_use]
pub fn by_recency(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort_by_cached_key(|path| (Reverse(modified(path.as_path())), path.clone()));
    paths
}

/// A file's modification time, when the filesystem will tell us one.
fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

/// The paths sharing the newest modification time.
///
/// A fresh `git clone` stamps every file with the same time, so this is often
/// more than one dossier, and the caller asks instead of guessing.
#[must_use]
pub fn newest(paths: &[PathBuf]) -> &[PathBuf] {
    let Some(first) = paths.first() else {
        return &[];
    };

    let stamp = modified(first.as_path());
    let tied = paths
        .iter()
        .take_while(|path| modified(path.as_path()) == stamp)
        .count();

    paths.get(..tied).unwrap_or(paths)
}

/// A path as the user sees it: relative to the repository root when it is
/// inside it.
#[must_use]
pub fn relative<'a>(path: &'a Path, root: &Path) -> &'a Path {
    path.strip_prefix(root).unwrap_or(path)
}

/// A file's name, extension included.
#[must_use]
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_sealed_field_in_frontmatter() {
        assert!(is_sealed("---\nsealed: 2026-09-21\n---\n# Done\n"));
        assert!(is_sealed("---\nsealed:2026-09-21\n---\n"));
    }

    #[test]
    fn a_file_without_frontmatter_is_not_sealed() {
        assert!(!is_sealed("# A dossier\nsealed: not really\n"));
        assert!(!is_sealed(""));
    }

    #[test]
    fn only_frontmatter_counts() {
        let contents = "# A dossier\n\n## Notes\n- the sealed: field is not set yet\n";
        assert!(!is_sealed(contents));
    }

    #[test]
    fn frontmatter_without_the_field_is_not_sealed() {
        assert!(!is_sealed("---\ncreated: 2026-09-21\n---\n# A dossier\n"));
    }

    #[test]
    fn an_unterminated_frontmatter_still_reads() {
        assert!(is_sealed("---\nsealed: 2026-09-21\n"));
    }

    #[test]
    fn sealed_sources_are_not_open() {
        let sealed = Source {
            path: PathBuf::from("dossiers/a.md"),
            sealed: true,
        };
        let open = Source {
            path: PathBuf::from("dossiers/b.md"),
            sealed: false,
        };

        assert!(!sealed.is_open());
        assert_eq!(
            open_paths(&[sealed, open]),
            vec![PathBuf::from("dossiers/b.md")]
        );
    }
}
