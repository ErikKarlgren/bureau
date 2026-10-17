# `bureau doctor`

## Purpose

Check the notes for formatting problems and report them. Doctor is a
**diagnostic** command: it never edits a note, never stages anything and never
commits. It is the linter the rest of the command set deliberately does not
have.

The checks cover the shape of the notes, not their content. In particular,
doctor does not judge whether a task is done, whether a dossier is stale, or
whether prose is good.

## Usage

```bash
bureau doctor
```

No arguments. It checks the whole repository, from the repository root.

## What is checked

| Source | What is read |
|---|---|
| `dossiers/*.md` | every dossier, except sealed ones |
| `entries/YYYY-MM-DD.md` | every date-named entry |

A file in `entries/` whose stem is not a date is not an entry and is not
checked. A file outside those two directories is never checked.

Sealed dossiers are skipped **for now**. Sealing is not implemented yet; when
it is, doctor should probably check sealed dossiers too, because a seal hides a
dossier from action but not from the record.

## Output

```text
dossiers/43 fweaaw iuferigber.md:18: Warning: empty task; bureau tasks lists it as '- [ ]'
dossiers/wwafwfwefwe.md:16: Warning: empty task; bureau tasks lists it as '- [ ]'
entries/2026-09-19.md:1: Warning: missing '## Worked on Dossiers' section; 'bureau worklog' will create it the next time it links a dossier, or you can add it yourself
3 warnings
```

Each finding is one line:

```text
path:line: Severity: message
```

- `path` is relative to the repository root and uses `/` on every platform.
- `line` is the line that should change, or `1` for a whole-file finding and a
  missing section. Line `1` is the portable choice: Vim, Emacs compilation
  mode and editor problem matchers all expect a positive line number, and
  omitting it breaks the common `%f:%l:%m` quickfix patterns.
- `Severity` is `Warning` or `Error`. Uppercase is used because it matches the
  existing command output and because Vim's `%t` captures the first letter;
  Vim itself treats `W`/`w` and `E`/`e` the same, but uppercase is the safest
  conventional form.
- Findings are sorted by path, then line, then a fixed check order, so a run is
  deterministic.
- The last line is a summary: `3 warnings`, `1 error, 2 warnings`,
  `No formatting issues found`.

On a terminal, `path:line:` is cyan (the existing path colour), `Warning: …` is
bold yellow and `Error: …` is bold red. Colour is only an aid: when stdout is
not a terminal, and when `NO_COLOR` is set to anything non-empty or `TERM=dumb`,
the output is plain text with no escapes. Findings go to **stdout**, because
they are the command's output; only a failure to run doctor at all goes to
stderr.

## Exit code

- `0`: no findings, or warnings only.
- `1`: at least one error.

A future `--strict` can make warnings fail as well. A future `--fix` must be
explicit and dry-run by default; doctor is read-only today.

## Checks

### File-level

- **Non-portable file name** (warning): a name Windows cannot check out
  reliably: `< > : " / \ | ? *`, control characters, a trailing dot or space,
  a reserved device name (`CON`, `NUL`, `COM1`, …), or a name over 255 bytes.
- **Case-insensitive collision** (warning): two names in one directory that
  differ only by case cannot coexist on a case-insensitive filesystem.
- **Encoding and line endings** (warning): a UTF-8 BOM, CRLF line endings, a
  mixture of CRLF and LF, or no final newline. A file that cannot be read as
  UTF-8 is an error.
- **Unclosed fenced code block** (error): a backtick or tilde fence with no
  matching closing fence.

### Dossiers

- **Missing `## Worklog`** (error): `bureau worklog` has nowhere to write.
- **Duplicate `## Worklog`** (error): only the first section is used, so the
  second is silently ignored by `worklog` and `report`.
- **Worklog day heading** (error):
  - it must be a `###` heading that is exactly one link to an entry;
  - its label must be a `YYYY-MM-DD` date;
  - its target must be `../entries/<date>.md`;
  - the target must exist.
  A bare `### 2026-09-19` is not supported any more. `bureau worklog` and
  `bureau report` now read only linked day headings. For a link whose label
  and target disagree, the user has to decide which date is the source of
  truth; doctor reports the mismatch rather than guessing.
- **Broken relative link** (warning): an inline link or image to a local file
  that does not exist, is not a file, has a malformed target, or uses spaces
  without `<...>`.

### Entries

- **Missing `## Worked on Dossiers`** (warning): `bureau worklog` will create
  it the next time it links a dossier, and the user can add it themselves. This
  is a warning even though the command can create it, because an entry without
  the section is not shaped like the other entries.
- **Broken relative link** (warning): as for dossiers.

### Tasks

- **Empty task** (warning): a dash task with no text, such as `- [ ] `. The
  template leaves one behind, and `bureau tasks` lists it as real work.
- **Unrecognised marker** (warning): `- [X]`, `- [!]`, `- [  ]`, `- [x ]` and
  similar. `bureau tasks` treats these as prose.
- **Task on a non-dash bullet** (warning): `* [ ]`, `+ [x]`, `1. [ ]` and
  similar. `bureau tasks` only reads bullets that start with `-`.

### Cross-file consistency

Both directions are warnings:

- a dossier logs work on a day, but that day's entry does not link the dossier
  under `## Worked on Dossiers`;
- an entry links a dossier under `## Worked on Dossiers`, but that dossier has
  no worklog heading for the entry's date.

An entry with no `## Worked on Dossiers` section at all gets the missing-section
warning instead of the first cross-link warning.

## What is deliberately not checked

- Indentation width. `bureau tasks` deliberately treats any deeper indentation
  as nesting; a Markdown renderer may disagree, but that is a separate decision.
- Empty plain bullets (`- ` with no marker). They are invisible to the commands
  and would add a lot of template noise.
- Missing template sections other than `## Worklog` and
  `## Worked on Dossiers`. They are optional for now.
- Empty sections, section order and heading hierarchy.
- External URLs. Doctor is local and makes no network requests.
- Anchors and reference-style links.
- Non-date files in `entries/`.
- Task statistics, stale dossiers and ready-to-seal dossiers; those belong to
  `bureau tasks`.
- Git state, uncommitted changes and commit behaviour.
- Prose, spelling and grammar.
- Full CommonMark validation. The notes dialect is deliberately looser.

## Modules

| File | Contents |
|---|---|
| `src/doctor.rs` | public finding/input types, `check`, ordering, and the submodule wiring |
| `src/doctor/document.rs` | the line scanner: fences, headings, bullets, links and destinations |
| `src/doctor/checks.rs` | names, encoding, fences, tasks and case collisions |
| `src/doctor/links.rs` | broken relative links and dossier-link targets |
| `src/doctor/worklog.rs` | `## Worklog` shape and the dossier/entry cross-links |
| `src/doctor/tests.rs` | pure check tests |
| `src/commands/doctor.rs` | root, file discovery, reading, rendering and exit code |

The scanner is deliberately narrower than CommonMark. It is the subset the
notes use and the subset the commands understand.

## Tests

- `src/doctor/tests.rs`: each check has a positive and a negative case, plus
  scanner cases for fences, headings, bullets, links and line endings.
- `src/commands/doctor.rs`: the command against a scratch repository, including
  colours, summary, exit code, unreadable files and sealed dossiers.
- `src/cli.rs`: `bureau doctor` takes no arguments.
- `src/worklog.rs` and `src/commands/report.rs`: bare `### date` headings are no
  longer read by `worklog` or `report`.

## Manual check

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets
cargo test
cargo run -- doctor
cargo run -- doctor | cat    # plain: no escapes reach a pipe
NO_COLOR=1 cargo run -- doctor
```
