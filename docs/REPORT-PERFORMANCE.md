# Making `bureau report`'s git diff fast

`bureau report` used to fork a `git` process several times per dossier. On a
20,000-commit repository with 100 dossiers that was 400 invocations and ~1.9 s,
almost all of it git walking the same history over and over. It is now 13
invocations and ~0.42 s, with byte-identical output.

Nothing about the report's text changed. Every case below was checked with
`cmp` against the binary built from the previous commit.

## What was slow

The cost was never `exec()`. A bare `git --version` costs ~0.8 ms, and on a
20k-commit report the whole spawn budget was ~12% of the time. The cost was this
call, made **once per dossier**:

```sh
git rev-list -1 --before=<from> HEAD -- <dossier>
```

`--before=<date>` forces git to traverse the commit graph from `HEAD` all the
way down to the base date, comparing the path at every commit on the way. It
cannot stop early, so it costs ~1.2 µs per commit *traversed* regardless of how
many commits actually touch the dossier. I confirmed the shape of that cost on
two 20,000-commit repositories that differed only in how often the target file
was touched:

| query | 5 commits touch the file | 20,000 touch it |
|---|---|---|
| `rev-list -1 --before=2024-06-01 HEAD -- target.md` | 24.50 ms | 21.12 ms |

The number of matching commits is irrelevant; the depth of the history is
everything. Running that walk 100 times, for 100 dossiers, is the whole problem.

## The change

### One walk answers every dossier (`src/git.rs`)

`git::last_commit(root, date, path)` became `git::last_commits(root, date, paths)`.
Instead of `rev-list -1` per path, it makes a single call:

```sh
git -c core.quotepath=false log -z --format=%H --name-only \
    --before=<from>T00:00:00 HEAD -- <every dossier path>
```

`git log` lists commits newest first and names the paths each one touched, so
the first time a path appears is the newest commit that touched it. The batch
collects that into a path → commit map in one pass. The pathspec limit means the
same TREESAME history simplification still applies, so a commit that did not
actually change a path is not reported for it.

Two details make the parse exact:

* `-z` NUL-terminates every field, so a path needing quotes, spaces, a newline
  or non-ASCII bytes cannot be confused with a neighbouring field, and the
  pinned `core.quotepath=false` keeps even awkward names raw.
* A path may itself be 40 hex characters, which is exactly the shape of a
  commit id. A hash is therefore only read as a hash when it does not
  immediately follow another hash; in `hash, path, path, hash, path` the second
  hash follows a path and is unambiguous.

### Many diffs in one call (`src/git.rs`)

`git::diff(root, base, head, path, follow, color)` became
`git::diffs(root, base, head, paths, color)`, which asks about every path that
shares a base and head in one invocation and splits the result back apart. The
split uses `--line-prefix` with a single mark byte:

```sh
git ... diff --no-ext-diff --no-textconv --no-color-moved --no-color \
    --line-prefix=$'\x01' <base> <head> -- <paths>
```

`--line-prefix` puts the mark at the front of *every* line, which is what makes
the split exact: a `diff --git` line inside a file's own content is prefixed
too, so it cannot be mistaken for the start of the next file. The mark is then
removed, leaving the bytes a one-path `git diff` would have produced.

That claim is not assumed. `git.rs` now has a test
(`a_batched_diff_is_the_bytes_of_one_alone`) that compares the batched result
for each path against `git diff` run on that path alone, and a colour test that
does the same with `--color=always`.

Colour needed one subtlety: git writes the SGR sequence *after* the line prefix
and the reset *before* the newline, so a coloured line is stripped, inspected
for the `diff --git` header, and re-emitted whole — git's own codes, reset
included. Splitting a coloured diff therefore yields exactly the coloured bytes
of the single-path diff.

### The empty tree is asked for once (`src/commands/report.rs`)

`git::empty_tree` was called from `real_diff`, i.e. **once per dossier**, so a
100-dossier report paid 100 `git mktree` processes. It is now called once at the
start of the batch.

### Grouping by commit pair (`src/commands/report.rs`)

`batch_diffs` collects the base for every dossier from the one walk, groups the
paths by the `(base, head)` pair they share, and makes one `git diff` per
distinct pair. In the common case — a period ending today, so every head is
literally `HEAD` — that is a single diff call for the whole report. A period
that ends in the past gets a second batched walk for the heads.

A pair whose diff fails marks only its own dossiers as unreadable; the rest of
the report still prints, which is stricter per-dossier behaviour than before.

### `git --version` and `--follow` are gone (`src/git.rs`)

The code used to run `git --version` and warn when git was older than 2.47,
because `git diff --follow` needed it. Chasing that down showed `--follow` could
never have changed a byte of this report:

* `git rev-list --follow` does not exist — the walk never followed renames, so
  the base for a dossier is always a commit that holds the path under its
  current name;
* a one-path `git diff` over a path present in both trees never performs rename
  detection, so `--follow` had nothing to act on.

Verified directly: with a dossier renamed inside the period, `diff --follow` and
`diff` between that base and head are byte-identical. So the flag, the
`git --version` call, `follows_renames` and the version warning are all gone.
`a_rename_is_not_followed_into_a_diff` pins the reasoning.

### Repository-relative paths (`src/git.rs`)

`git::relative` is new. Git names a path relative to the repository root in a
diff header whatever form the pathspec took, and `git log --name-only` only ever
reports root-relative paths — but the report holds paths from
`sources::read_markdown`, which are absolute. Every lookup now goes through
`git::relative`. This was the one real bug the refactor introduced and the tests
caught it; the new end-to-end test deliberately passes absolute paths so it
cannot come back.

## Numbers

Best of 5-7 warm runs, same machine, `git 2.55.0`. The baseline is the previous
commit's source built with the same profile.

| case | before | after | speedup |
|---|---|---|---|
| 20k commits, 100 dossiers, period to today | 1,861 ms | 418 ms | **4.5×** |
| 5k commits, 100 dossiers, period to today | 1,514 ms | 302 ms | **5.0×** |
| 801 commits, 40 dossiers, period to today | 112 ms | 13 ms | **8.6×** |
| 801 commits, 40 dossiers, period in the past | 247 ms | 14 ms | **17.6×** |
| bureau's own repo, 3 dossiers | 7 ms | 4 ms | 1.8× |

The win is smaller on the large repositories because git's real work starts to
dominate once the redundant walks are gone. On the 20k-commit report the
remaining ~420 ms is roughly 125 ms of walk (irreducible), ~137 ms of diff and
the rest producing and rendering 7.2 MB of report text.

The process count, which is what the original `exec()` concern was about, drops
much further than the wall time, because `execvp` retries six failing `$PATH`
probes per spawn:

| case | git `execve` before | after |
|---|---|---|
| 20k commits, 100 dossiers | 2,121 | 728 |
| 801 commits, 40 dossiers | 1,141 | 42 |

## Why not a libgit2/gitoxide crate instead

I evaluated both, benchmarking `git2 0.20.4`/libgit2 1.9.7 directly. Both lack
the one primitive this access pattern needs — a pathspec-limited revwalk with
git's TREESAME history simplification — so the base-commit search has to be
reimplemented by hand, and it came out **slower** than shelling out (3,998 ms vs
3,707 ms on the same 20k-commit repo). libgit2 also cannot produce the pinned
`diff.algorithm=histogram` bytes (its output is byte-identical to
`--diff-algorithm=myers`), ignores `core.quotepath`, has no `--follow`, and runs
no hooks. The evaluation is in `tmp/libgit-eval.md`; the conclusion was that
batching the existing calls was the real fix, and these numbers confirm it.

## Verifying

```
cargo fmt --check
cargo check
cargo clippy --all-targets
cargo test
```

All clean; 199 tests pass. The tests that carry the change are:

* `git::tests::one_walk_answers_every_path` — the batched walk agrees with
  `rev-list -1 --before` per path, including paths with spaces and non-ASCII
  characters, and a date before the repository existed.
* `git::tests::a_batched_diff_is_the_bytes_of_one_alone` — the batched split is
  byte-identical to a single-path diff.
* `git::tests::a_coloured_diff_uses_only_the_pinned_slots` — the split survives
  colour, and the pinned colour slots still hold.
* `git::tests::a_hostile_configuration_cannot_change_the_diff` — a repository
  configured to run an external diff still cannot influence the output.
* `git::tests::a_rename_is_not_followed_into_a_diff` — the `--follow` removal is
  a no-op.
* `commands::report::tests::a_batch_answers_each_dossier_what_git_would_alone` —
  end-to-end: the batch's base, head and bytes for each dossier match what git
  says when asked one dossier at a time, with the absolute paths the report
  really holds.

Output equivalence was also checked by hand on four repositories (5k and 20k
commits with 100 dossiers, and an 801-commit repo over both a recent and a past
period): `cmp` reports identical stdout and identical stderr in every case.
