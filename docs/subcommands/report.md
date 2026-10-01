# `bureau report`

Design note. Not implemented yet. This document is the contract the
implementation has to satisfy, written before the code the way
`docs/subcommands/tasks.md` was.

## Purpose

Print a reproducible report of the work recorded in the notes over a period:
what was logged each day, grouped per dossier, followed by the git diff of the
dossiers that period touched.

Two principles decide everything below:

- **The notes' contents are the source of truth; git is evidence appended to
  them.** The days and the lines come from the `## Worklog` sections and the
  daily entries. The diff is a supporting annex, not a second opinion about
  what happened.
- **The report is data.** It is meant to be read by a person and piped into an
  LLM ("what did I do this week?", "what belongs on my CV?", "what do I tell my
  boss tomorrow?"). Nothing here talks to a model: **no LLM dependency, no
  network, no API key, no TTY, no prompts.** Plain text on stdout, always.

This command is **read-only**: it never writes a file, never stages anything,
never commits.

## Usage

```
bureau report [<when>] [--from <date>] [--to <date>] [--no-diff]
              [--filter [<pattern>]] [--menu]
```

| Argument | Meaning |
|---|---|
| `<when>` | One day, in plain English or `YYYY-MM-DD`, e.g. `bureau report yesterday`, `bureau report 2026-09-20`. Expanded to `--from <when> --to <when>`. This is the common case: a single day's report. |
| `--from <date>` | First day of the period, **inclusive**. Defaults to today when neither it nor `<when>` is given. |
| `--to <date>` | Last day of the period, **inclusive**. Defaults to today. |
| `--no-diff` | Omit the whole `# Git diff` section. |
| `--filter [<pattern>]` | Restrict the report to one dossier, with exactly the rule `bureau worklog` and `bureau tasks` use. |
| `--menu` | Open the same picker. |

Rules:

- `<when>` and `--from`/`--to` are mutually exclusive; clap enforces it.
- `--from` later than `--to` is an error.
- **No work in the period is an error** (exit 1), not an empty report. A
  period with work logs but no diff is not an error.
- `--filter` matching nothing is an error; matching several opens the picker,
  as it does everywhere else. Reuse `commands/selection.rs` rather than
  re-deciding.
- **Open point, deliberately unsettled:** `--filter` takes an optional separate
  value and report also has the positional `<when>`, so
  `bureau report --filter yesterday` would read `yesterday` as the pattern
  rather than the day. Decide when the CLI lands: pair `--filter` with
  `--from`/`--to`, make report's filter flag-only, or give the pattern another
  spelling. Do not guess it now.
- A bare `bureau report` (no `<when>`, no `--from`) **defaults to today**, so
  `bureau report` is "today's report". `--from`/`--to` remain for explicit
  ranges.

### Helpers

`<when>` is deliberately the only helper in v1: a bare date **is** the single
day. The shape leaves room for `bureau report week` / `bureau report month`
later without changing what a bare date means. "`day <date>`" as a two-word
form is not needed while there is only one helper; if more land, revisit.

## Dates in plain English

`<when>`, `--from` and `--to` accept:

- `YYYY-MM-DD`;
- `today`, `yesterday`, `tomorrow`;
- `N days ago`, `N weeks ago`, `N months ago`;
- `a day ago`, `a week ago`, `a month ago`,
  and the same forms without `ago` meaning "in the future" (`in a week` is a
  possible extension, not required).

Parsing is a **pure function** `parse_date(text, today) -> Result<NaiveDate>`
(and a range wrapper). `today` is injected, so tests never depend on the
clock. The **resolved ISO dates are what the report prints**, never the raw
input, so a report stays auditable after the fact.

This grammar must also be given to `bureau worklog --date <date>` (TBD), so the
CLI is consistent about what a date is. That work is tracked in `TASKS.md`.

### Dependency evaluation

The candidates, evaluated as of 2026-10:

| Crate | Latest / health | Footprint | Handles `a week ago`? | Verdict |
|---|---|---|---|---|
| `chrono-english` | 0.2.1 (Aug 2026); lib.rs flags it **unmaintained** | ~18K SLoC, ~1 MB; deps `chrono`, `scanlex` | Partly: relative day/month intervals (`2 days`, `next week`, `last Friday`), but the README advertises no `ago` suffix and no article `a`; returns a `DateTime` with a time and a US/UK `Dialect` | More than a date, a dialect we do not want, and flagged unmaintained |
| `parse_datetime` (uutils) | 0.16.0 (Aug 2026), active, ~560k downloads/month | ~88K SLoC, ~5 MB; deps `jiff` (+tzdb), `num-traits`, `winnow` | Partly: `1 week ago` (integer required), `next week`, `yesterday`; not `a week ago` | Well maintained, but drags a second date library (jiff) and timezones into a chrono, date-only crate |
| `dateparser` | 0.3.1 (Mar 2026) | ~75K SLoC, ~3–5 MB; deps `chrono`, `regex`, `anyhow` | No: the accepted-format list is absolute datetimes only | Wrong tool |
| `humantime` | mature | small, no deps | No: durations and RFC timestamps, not calendar-relative | Wrong tool |
| `jiff` | 0.2.x, active | ~5 MB with tzdb | Has friendly relative parsing in its `fmt` module | A whole date library; would duplicate or replace `chrono` |

**Decision: a small hand-rolled, closed grammar** (roughly 60 lines plus
tests) in a pure module, shared by `report` and `worklog --date`. Reasons:
the required vocabulary is tiny and date-only; no maintained general-purpose
crate accepts the literal `a week ago` (they want `1 week ago`); a dependency
here buys a dialect/datetime/timezone surface the tool does not want; and
`AGENTS.md` asks for the fewest dependencies, with `std` free. The table above
is kept so the decision is auditable; revisit `parse_datetime` if the
vocabulary ever outgrows a closed grammar.

## Period model: worklog dates own the period

The period is a set of **calendar days**. A day is in the report when

- a dossier's `## Worklog` has a `### <date>` heading in the period, or
- that day's entry has content.

Git time does not define the period. This matters because
`bureau worklog --date` exists precisely to **backfill**: a line dated three
weeks ago is committed today, so a commit-date range would file it in the
wrong period or miss it. The files are the source of truth, so the days come
from the files.

### Why not backdate commits

Considered and rejected: `git commit --date=<date>` sets the **author** date;
`git log --since/--until` filters on the **commit** date, so backdating would
not even produce the ranges it appears to promise. It would also break the
assumption that history is monotonic (making `git log` order, `--since` and
reflogs harder to reason about), fabricate history on push, and complicate the
planned "commit every dependent file" behaviour. The worklog date already
records the logical day. **Bureau never backdates a commit.**

## Output

```
Report from 2026-09-20 to 2026-10-01

# Daily work

## 2026-09-20

- [ ] buy shampoo
  - the green one

### 1234 - killing goblins was never an option
- killed 23 goblins
- cast fireball

### 9820 - mine crypto
- bought a GPU

## 2026-09-21

- [x] filed the ticket

### 1234 - killing goblins was never an option
- got a magical wand

# Git diff

{{{ 1234 - killing goblins was never an option
diff --git a/dossiers/1234 - killing goblins was never an option.md b/dossiers/1234 - killing goblins was never an option.md
...
}}}

{{{ 9820 - mine crypto
diff --git a/dossiers/9820 - mine crypto.md b/dossiers/9820 - mine crypto.md
...
}}}
```

Shape rules:

- The header prints the **resolved** ISO range. The raw `a week ago` is not
  echoed.
- **Days ascending**, oldest first.
- Under a day: the day's **entry content first**, then one `### <dossier>`
  block per dossier that has a worklog line on that day.
- **Dossier order is the order of appearance in that day's entry**
  (`## Worked on Dossiers`, top to bottom). A dossier that has a worklog line
  for the day but is not linked from the entry follows the linked ones, name
  ascending. A day with no entry file at all (a backfilled worklog) is
  name ascending throughout. This reproduces a chronological-ish order without
  inventing a second authority.
- Days with **no work are omitted** entirely; the report is about work, not
  absence.
- `# Git diff` is last, so `--no-diff` leaves a complete, self-contained daily
  document. Its blocks follow **first appearance in the period** (the order the
  dossiers first show up in `# Daily work`), ties broken by name.

### Entry content

For each day the entry contributes its **bullets** (its tasks and notes),
indentation normalised to two spaces per level the same way `bureau tasks`
normalises it, markers preserved. The entry's `# <date>` title and its
`## Worked on Dossiers` section are not reprinted (the latter is the index the
ordering comes from, and the `###` blocks already carry the dossiers).

v1 includes **bullets only**: non-bullet prose inside an entry is out of
scope. The dossier template and every entry seen so far are bullet lists, so
this loses nothing yet; if prose turns out to matter, extend the extractor
rather than the tree renderer.

With `--filter`, the report is dossier-centric: only the chosen dossier's
`###` blocks appear, and the day's entry content is left out.

## The git diff

- **Committed history only.** Uncommitted working-tree changes are ignored, so
  the report is a function of the repository state at HEAD.
- For every dossier in scope:
  - `base` = the newest commit **strictly before** `--from` that touched the
    path (the empty tree when there is none);
  - `head` = `HEAD`, or the newest commit not after `--to` when `--to` is in
    the past;
  - `git diff --follow <base> <head> -- <path>`.
- **`--follow` requires exactly one path and git ≥ 2.47.** Our diff is already
  one path per dossier, so that fits. On older git, fall back to rename
  detection (`-M`/`diff.renames`) and accept that history before a rename may
  be missed. Browser-wide rename handling (a redirect or a rename map) is out
  of scope until `bureau rename` exists.
- **Every flag that affects bytes is pinned**, because `git diff` otherwise
  inherits user config (`diff.algorithm`, `diff.renames`, `diff.noprefix`,
  `diff.context`, `color.ui`, `core.autocrlf`, external diffs, textconv
  filters). The call sets, at minimum:
  `--no-color --no-ext-diff --no-textconv --src-prefix=a/ --dst-prefix=b/
  -U3` plus `-c diff.noprefix=false -c diff.renames=true
  -c diff.algorithm=histogram -c color.ui=false`, with `GIT_EXTERNAL_DIFF` and
  `GIT_DIFF_OPTS` cleared from the child environment. The exact list is pinned
  by a test so a later change is a decision, not a drift.
- A git failure while collecting one dossier's diff is a **warning inside the
  `# Git diff` section**, not a lost report: the daily work is already
  assembled and the report's core is the notes.
- A dossier that has worklog lines in the period but has since been deleted
  cannot be read, so it is absent from `# Daily work`; its diff cannot be
  attached to a current path. Known v1 gap: the report looks at files that are
  there.

### Fence markers

Diff blocks are delimited by Neovim fold markers:

```
{{{ <dossier name>
...diff...
}}}
```

They are **not** markdown code fences: a rendered markdown view shows plain
text. That is acceptable because the consumers are a terminal, Neovim
(`foldmethod=marker`) and an LLM, and the label makes each block fold by
dossier. Rejected alternatives: backtick fences break the moment a dossier
contains a code block (and dossiers are markdown, so they will); `~~~` fences
are valid markdown but do not fold in Neovim.

Safety: every line of a `git diff` body carries a leading ` `, `+`, `-` or
`@`, and header lines are `diff`/`index`/`---`/`+++`, so a line that is
exactly `}}}` **cannot** occur inside a diff. Entry and worklog lines are
bullets and start with `-`, so they cannot collide either. If a future section
ever emits free-form lines, the marker must be lengthened.

## Determinism / reproducibility

"Deterministic" is the goal, stated precisely: **the same repository state plus
the same arguments produce byte-identical output.** The only time-dependent
input is "today" when `--from`/`--to` are relative, and the resolved dates are
printed. To keep that true:

- no colour unless a terminal asked for it, and none in a pipe; no
  locale-dependent collation, no hash-map iteration order;
- no filesystem modification times anywhere in the ordering;
- the git diff is pinned by the flags above, so the user's git config cannot
  change the bytes;
- if `--json` is ever added, it is a second renderer over the same model, not
  a second code path.

## Edge cases

- **CRLF:** reading a file yields `\r`; strip it when parsing lines. Writing
  uses `writeln!`, which emits `\n` on every platform, so the output has no
  `\r` regardless of host.
- **Duplicate `### <date>` headings** in one dossier: merged, in document
  order.
- **A date with dossier worklogs but no entry file** (backfill): dossier order
  falls back to name ascending.
- **Malformed or undated `###` headings**, and dates outside the period: not
  read, not printed.
- **A worklog line for a day the entry does not link:** still shown, after the
  linked dossiers.
- **Sealed dossiers are included.** Sealing puts a dossier out of the way of
  *action*, not out of *history*: a dossier sealed during the period is exactly
  what the report should show. `sources::read_dossiers` drops sealed files, so
  the report reads `read_markdown` and keeps sealed sources.
- **No work in the period:** error, per Usage.
- **`--no-diff`:** the `# Git diff` heading is not printed at all (not an
  empty heading).

## Colour

Colour reuses `bureau tasks`' palette and its switch: it is drawn only when
stdout is a terminal, `NO_COLOR` is unset or empty, and `TERM` is not `dumb`.
It is a redundant encoding, so the plain run loses nothing, and the report is
usually piped to a file or an LLM, where colour is off automatically.

| Line | Drawn in |
|---|---|
| `# Daily work`, `# Git diff` | bold, no hue |
| `## <date>`, `### <dossier>` | bold, no hue |
| a task marker in entry or worklog content | the `tasks` hue for its state: blue `[ ]`, cyan `[.]`/`[o]`, yellow `[?]`, green `[x]`/`[-]` |
| `{{{ <dossier>` fold marker | bold, no hue |
| `Report from …` header and everything else | plain |
| diff body | never coloured |

The marker rule is the `tasks` one keyed by **state**, not by a listing section:
`style::Palette::marker` currently takes a `Section`, and report needs the
section-free form, so `style.rs` gains that mapping. The diff is passed through
git already uncoloured and is never repainted.

## Modules

The split is the one the crate already uses: pure logic in a `report.rs`,
IO at the edge in `commands/report.rs`.

| File | Change |
|---|---|
| `src/report.rs` (new) | Date grammar, range, worklog/entry extraction, grouping, ordering, rendering. Pure functions over `&str` and paths, in the style of `src/tasks.rs` and `src/worklog.rs`. Takes `today` and the diffs as inputs. |
| `src/date.rs` (new) | `parse_date(text, today)` and the range, shared with `worklog --date` (TBD). Could live in `report.rs`; a separate module is right once `worklog` uses it too. |
| `src/commands/report.rs` (new) | Repository root, source discovery (sealed included), git diffs, selection, printing. The `run`/`run_in` split from `commands/tasks.rs`. |
| `src/git.rs` | Add a read-side `diff` (and the commit resolution it needs) with the pinned flags; reuse `toplevel`. |
| `src/cli.rs` | `Command::Report(ReportArgs)`. |
| `src/commands/mod.rs` | One `match` arm. |
| `src/tasks.rs` | Possibly a "render every bullet" path for entry content, or a small extractor in `report.rs`. |
| `src/style.rs` | The section-free marker hue report needs (see Colour). |
| `README.md` | The `bureau report` paragraph, once it exists. |

## Tests

Pure (`src/report.rs`, `src/date.rs`):

- The date grammar: every accepted form, the boundaries of a range, an
  invalid string, and an injected `today` so the tests never read the clock.
- Grouping by day, dossier order taken from an entry, the name-ascending
  fallback when the entry is missing, duplicate day headings, sealed sources
  included, `--no-diff`, the fold markers, the first-appearance order of the
  diff blocks, and CRLF.
- Colour: a terminal run hues the headings and each marker by state, a pipe /
  `NO_COLOR` / `TERM=dumb` prints byte-for-byte the plain report, and no escape
  ever reaches the diff body.
- A fixture's **full report**, compared byte for byte. This is the format
  regression test, the counterpart of `tasks`' worked example.

Command (`src/commands/report.rs`):

- No work in the period is an error; a missing `entries/` or `dossiers/`
  directory is not.
- Git: the diff tests need a real temporary repository (`Scratch` plus
  `git init` and commits) or an injected differ. `git.rs` is **not**
  injectable today, unlike the picker and the prompt, so this is a signature
  decision to make before coding, not after.
- A rename: `git diff --follow` on a single path still finds the pre-rename
  history (git ≥ 2.47).
- The pinned git flags: a repository with a `diff.algorithm` set to something
  else still produces the same bytes.

## Out of scope

- **No LLM dependency.** No network, no API key, no model calls; the report is
  text an LLM can be pointed at, nothing more.
- **No `--json`** for now. Deferred until a program (not an LLM) consumes the
  report. If it lands, it is `--format markdown|json` over one data model, with
  a documented schema, and JSON never contains ANSI escapes.
- No statistics ("18 of 20 done"), no sealing, no writing, no task ticking.
- No natural-language parsing beyond the documented grammar.
- No uncommitted working-tree diff.
- No `--stat`/size caps yet; see how a real report looks with real data first.
