# `bureau report`

Implemented. This document is the contract the implementation satisfies; it was
written before the code the way `docs/subcommands/tasks.md` was, and is kept in
step with it.

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
bureau report [<date>] [--from <date>] [--to <date>] [--no-diff]
              [--filter <pattern>] [--menu]
```

| Argument | Meaning |
|---|---|
| `<date>` | One day, in plain English or `YYYY-MM-DD`, e.g. `bureau report yesterday`, `bureau report 2026-09-20`. Expanded to `--from <date> --to <date>`. This is the common case: a single day's report. |
| `--from <date>` | First day of the period, **inclusive**. Defaults to today when neither it nor `<date>` is given. |
| `--to <date>` | Last day of the period, **inclusive**. Defaults to today. |
| `--no-diff` | Omit the whole `## Git diff` section. |
| `--filter <pattern>` | Restrict the report to one dossier. **The pattern is required** (unlike `tasks`), so it can never be mistaken for the `<date>` positional; `--menu` is how the picker is asked for. The matching rule is `bureau worklog`'s. |
| `--menu` | Open the same picker. |

Rules:

- `<date>` and `--from`/`--to` are mutually exclusive; clap enforces it.
- `--from` later than `--to` is an error.
- **A report with no work at all is an error** (exit 1), not an empty report. An
  unfiltered report names what is missing: `No entry found for <date>` /
  `No entries found from <from> to <to>` when the period has no entry file, and
  `Entry found for <date> but no work found` / `All entries from <from> to
  <to> contain no work` when it has entries but nothing in them. A `--filter`ed
  report never reads entries, so it keeps the older wording: `no work in
  <date>` / `no work in the period <from> to <to>`. A range with work but no
  diff is not an error.
- `--filter` matching nothing is an error; matching several opens the picker,
  as it does everywhere else. Reuse `commands/selection.rs` rather than
  re-deciding.
- `--filter` takes its pattern as a separate **required** value, so the
  optional-value ambiguity `tasks` has does not exist here: the pattern can
  never swallow `<date>`. A bare `--filter` is a clap error; `--menu` is how
  the picker is asked for. `tasks` keeps its optional-value form, since it has
  no positional to collide with and bare `--filter` there means "the newest
  dossier".
- A bare `bureau report` (no `<date>`, no `--from`) **defaults to today**, so
  `bureau report` is "today's report". `--from`/`--to` remain for explicit
  ranges.

### Helpers

`<date>` is deliberately the only helper in v1: a bare date **is** the single
day. The shape leaves room for `bureau report week` / `bureau report month`
later without changing what a bare date means. "`day <date>`" as a two-word
form is not needed while there is only one helper; if more land, revisit.

## Dates in plain English

`<date>`, `--from` and `--to` accept **one argv element** (quote it, the way
systemd's `--since` expects), case-insensitively, with surrounding and repeated
whitespace collapsed:

- `YYYY-MM-DD`;
- `today`, `yesterday`;
- `[a|an|one…ten|<digits>] day[s] | week[s] | month[s] | year[s] ago`.

So `a day ago`, `one day ago`, `1 day ago`, `two weeks ago`, `23 months ago`
and `3 years ago` all parse. Singular and plural are both accepted whatever the
number (`1 days ago` is not an error), matching how permissive the tasks
grammar is. Human words stop at `ten`; digits are unbounded.

**Future forms are not accepted** (`tomorrow`, `in a week`). A report about the
future is empty by construction, and an error saying so is better than a blank
report.

Parsing is a **pure function** `parse_date(text, today) -> Result<NaiveDate>`
(and a range wrapper). `today` is injected, so tests never depend on the clock.
The **resolved ISO dates are what the report prints**, never the raw input, so
a report stays auditable after the fact.

Arithmetic uses `chrono` and needs no new dependency:
`NaiveDate::checked_sub_days(Days::new(7 * n))` for days and weeks,
`checked_sub_months(Months::new(n))` for months, `Months::new(12 * n)` for
years. `checked_sub_months` **clamps the day to the target month's length**, so
`Jan 31` minus a month is `Feb 28/29` — the calendar-correct answer a "30 days
a month" hack gets wrong. `None` becomes an error, never a panic. The base is
`Local::now().date_naive()`, so local time decides which day "today" is;
everything after that is `NaiveDate` arithmetic. `chrono::Days` and
`chrono::Months` are already exported by the version in the lockfile.

This grammar is shared with `bureau worklog --date <date>`; both live in
`src/date.rs`, and the parity is tracked in `TASKS.md`.

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

- a dossier's `## Worklog` has a `### <date>` heading in the period whose block
  holds at least one line, or
- that day has an entry file.

A day's **work** is what it renders: the entry's bullets, or the `###` blocks of
the dossiers that logged work that day. A day can be in the report and have no
work -- an entry holding nothing but prose, or nothing at all -- and the report
names that case rather than dropping the day.

The dossier worklog can be a day's only source. `bureau worklog --date <date>`
creates the day's entry when it is missing, so a day that was logged through the
tool always has one to inspect; but a worklog written by hand, or one whose
entry was deleted afterwards, still reports on its own, and is never an error
about a missing entry.

An entry that renders nothing still makes the day. In a period that has work in
it, such a day prints its heading followed by `(No work found)` rather than a
bare heading. A day whose only trace is an empty `### date` heading -- no
entry, no worklog lines -- is left out entirely.

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

````
# Report from 2026-09-20 to 2026-10-01


## 2026-09-20 (Sunday)
### Notes
- [ ] buy shampoo
  - the green one

### 1234 - killing goblins was never an option
- killed 23 goblins
- cast fireball

### 9820 - mine crypto
- bought a GPU


## 2026-09-21 (Monday)
### Notes
- [x] filed the ticket

### 1234 - killing goblins was never an option
- got a magical wand


## 2026-09-22 (Tuesday)
(No work found)


## Git diff

### 1234 - killing goblins was never an option
{{{ git diff
```diff
diff --git a/dossiers/1234 - killing goblins was never an option.md b/dossiers/1234 - killing goblins was never an option.md
...
```
}}}


### 9820 - mine crypto
{{{ git diff
```diff
diff --git a/dossiers/9820 - mine crypto.md b/dossiers/9820 - mine crypto.md
...
```
}}}


### 5501 - a dossier with nothing committed this period
(No git changes were found)
````

A `--filter`ed report carries its scope under the title instead of the day's
own notes:

````
# Report for 2026-09-20

Dossier: 1234 - killing goblins was never an option


## 2026-09-20 (Sunday)

### 1234 - killing goblins was never an option
- killed 23 goblins
````

(The outer fences here are four backticks because the examples contain three:
the same rule the report itself follows, see below.)

Shape rules:

- The title prints the **resolved** ISO dates: `# Report for <date>` for a
  single day, `# Report from <from> to <to>` for a range. The raw `a week ago`
  is not echoed, and a one-day report is never written as a range.
- A filtered report prints `Dossier: <title>` under the title, where the title
  is the name the dossier writes at the top of its own file rather than the one
  its file name had to take: a dossier called `a::b` lives in `a_b.md` and is
  named `a::b` here. A dossier with no `# ` title falls back to its file stem.
- One heading level per thing: the title is the only `#`, a day and the diff
  section are `##`, and a dossier is `###` under either of them. A day heading
  carries its weekday, `## 2027-01-02 (Monday)`, so a reader does not have to
  work it out.
- A heading is followed immediately by its content. Two blank lines precede a
  `##` and one precedes a `###`, which groups a day and its dossiers into one
  block while keeping consecutive days apart. That holds for the first day too,
  whether the title is alone above it or a scope line stands between.
- **Days ascending**, oldest first.
- Under a day: the day's **entry content first** (its own sections, see below),
  then one `### <dossier>` block per dossier that has a worklog line on that
  day.
- **Dossier order is the order of appearance in that day's entry**
  (`## Worked on Dossiers`, top to bottom). A dossier that has a worklog line
  for the day but is not linked from the entry follows the linked ones, name
  ascending. A day with no entry file at all (a backfilled worklog) is
  name ascending throughout. This reproduces a chronological-ish order without
  inventing a second authority.
- A day with no worklog lines and no entry file is not in the report at all. A
  day whose entry renders nothing prints its heading and then `(No work found)`.
  The note is entry-only: a day that is in the report for its dossier worklogs
  alone never carries it.
- `## Git diff` is last, so `--no-diff` leaves a complete, self-contained daily
  document. Its blocks follow **first appearance in the period** (the order the
  dossiers first show up among the days), ties broken by name.
- Every dossier in scope gets a block there. A dossier git reports no changes
  for prints its heading and `(No git changes were found)`; one whose diff
  could not be read prints `(Git changes could not be read)` instead. The
  section therefore appears whenever the period mentions a dossier at all, and
  `--no-diff` is the only way to leave it out.

### Entry content

An entry contributes its **headings and bullets**, and its own sections become
the day's sections one `#` deeper: `## Notes` prints as `### Notes`, `### Sub`
as `#### Sub`, and so on. The section a bullet sits under is therefore visible
in the report, which is what tells a note from a task reference.

- Indentation is normalised to two spaces per level the same way `bureau tasks`
  normalises it, and markers are preserved as written.
- The entry's `# <date>` title is dropped: the day heading already says it.
- `## Worked on Dossiers` is dropped: it is the index the ordering comes from,
  and the `###` blocks carry the dossiers themselves.
- A heading prints only when work sits under it, directly or in a subsection of
  its own. A section left empty is dropped rather than printed as an empty
  heading, which is what keeps a template entry with nothing in it down to the
  `(No work found)` note.

Non-bullet prose inside an entry is still out of scope: only headings and
bullets survive. The dossier template and every entry seen so far are bullet
lists, so this loses nothing yet; if prose turns out to matter, extend the
extractor rather than the tree renderer.

With `--filter`, the report is dossier-centric: only the chosen dossier's
`###` blocks appear, the day's entry content is left out, and the scope line
names the dossier the report is about.

## The git diff

- **Committed history only.** Uncommitted working-tree changes are ignored, so
  the report is a function of the repository state at HEAD.
- For every dossier in scope:
  - `base` = the newest commit **strictly before** `--from` that touched the
    path (the empty tree when there is none);
  - `head` = `HEAD`, or the newest commit not after `--to` when `--to` is in
    the past;
  - `git diff --follow <base> <head> -- <path>`.
- **`--follow` requires exactly one path and git ≥ 2.47.** Our diff is one path
  per dossier, so that fits. Support is probed once by parsing `git --version`
  (locale-independent), never by matching git's error text, which is localised.
  When unsupported, print **one warning on stderr** and run the whole section
  without `--follow`, accepting that history before a rename is missed.
  Broader rename handling is out of scope until `bureau rename` exists.
- **User config must not change the bytes or run user tools.** The call pins,
  explicitly: `--no-ext-diff` (blocks `diff.external`, `GIT_EXTERNAL_DIFF` and
  external diff drivers — verified against a scratch repo), `--no-textconv`
  (blocks textconv filters; this repo has `diff.bin.textconv = hexdump -v -C`),
  `--no-color-moved` (this repo has `diff.colormoved = true`, i.e. zebra move
  detection, whose moved-line slots are magenta/cyan/blue/yellow),
  `--ws-error-highlight=none`, `--no-pager`, `-c diff.algorithm=histogram`,
  `-c diff.renames=true`, `-c diff.noprefix=false`,
  `-c diff.mnemonicPrefix=false`, and `-c core.quotepath=false` so non-ASCII
  dossier names are not octal-escaped. `GIT_EXTERNAL_DIFF`, `GIT_DIFF_OPTS` and
  any `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_*`/`GIT_CONFIG_VALUE_*` pair are
  cleared from the child environment. The exact list is pinned by a test, so a
  later change is a decision rather than drift.
- A dossier with worklog lines but no commit in the period has nothing to show,
  and says so: `(No git changes were found)` under its heading. The daily work
  is the report's core, so an empty diff is never an error and never a reason
  to drop the block.
- A git failure while collecting one dossier's diff is a **warning on stderr**
  and `(Git changes could not be read)` in the document. The two are separate
  on purpose: a redirected report loses stderr, and an unread diff must never
  read as "no changes", because the changes may well be there. The daily work
  is already assembled either way, so a broken diff still leaves a report.
- A dossier that has worklog lines in the period but has since been deleted
  cannot be read, so it is absent from the daily work; its diff cannot be
  attached to a current path. Known v1 gap: the report looks at files that are
  there.

### The fenced, folded diff

Each diff is one markdown code block and one Neovim fold:

````
### <dossier>
{{{ git diff
```diff
...diff...
```
}}}
````

`{{{ git diff` and `}}}` are **not** markdown. They are the markers
`foldmethod=marker` looks for, and the label is constant because the `###`
heading immediately above already names the block: a fold that repeated a long
dossier name would be noise, and the heading stays visible when the fold is
closed. `foldmethod=markdown`, or a treesitter fold, needs none of this and
folds by the headings alone.

The backtick fence is **computed**, never fixed: `report::fence_width` measures
the longest run of backticks at the start of a line (after at most three
spaces) in the diff body and opens with one more, never fewer than three. A
diff is not only `+` and `-`: unchanged lines arrive as context lines, each
keeping the space git marks it with, so an unchanged code fence in a dossier
arrives as a line `CommonMark` would accept as a closing fence. A fixed
three-backtick fence would end the block in the middle of the diff and spill
the rest as prose. Lengthening the fence fixes it without touching a byte of
the diff, so the block stays a real patch that `git apply` accepts.

Rejected alternatives: `-U0` (safe, and simpler, but it throws away the
unchanged lines that make a diff readable); `--output-indicator-context` (safe,
and it keeps the context, but git then calls its own output a corrupt patch and
syntax highlighting loses the context lines); escaping or rewriting the body
(it is evidence, and rewriting it is the one thing this section must not do).

The fold markers are the weaker half of the pair: Neovim looks for `{{{` and
`}}}` **anywhere** in a line, so a note or a dossier containing either string
can open or close a fold early. That needs those characters literally in the
notes, which is rare, unlike a code fence in a dossier, which is ordinary --
which is why only the fence is computed.

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
- **No work at all in the range:** an error, named by what is missing. An
  unfiltered report says `No entry found for <date>` / `No entries found from
  <from> to <to>` when there is no entry file in the period, and `Entry found
  for <date> but no work found` / `All entries from <from> to <to> contain no
  work` when there are entries but nothing in them. A `--filter`ed report says
  `no work in <date>` / `no work in the period <from> to <to>`, because it
  never reads entries. A one-day period is named as a day even when it was
  asked for with `--from`/`--to`.
- **A period with some work in it:** its workless entry days are printed with
  `(No work found)`, not dropped and not an error.
- **`--no-diff`:** the `## Git diff` heading is not printed at all
  (not an empty heading).

## Colour

Colour reuses the run's palette and its switch: it is drawn only when stdout is
a terminal, `NO_COLOR` is unset or empty, and `TERM` is not `dumb`. It is a
redundant encoding, so the plain run loses nothing, and the report is usually
piped to a file or an LLM, where colour is off automatically.

**The report paints its structure, not its content.** `bureau tasks` hues a
task's marker by its state, because that state is what the listing is about. A
report is not a listing: the reader is looking for the day and the dossier, so
the headings carry the colour and every bullet is printed exactly as it was
written -- marker, prose and all.

| Line | Drawn in |
|---|---|
| `# Report …` title | bold magenta |
| `## <date>`, `## Git diff` | bold cyan |
| `### <dossier>`, in either half | bold blue |
| `{{{ git diff`, `}}}`, and the backtick fence | dim |
| every bullet, `(No work found)`, `(No git changes were found)`, `(Git changes could not be read)` | plain |
| diff body | git's own colours, see below |

A dossier heading takes the same hue in both halves, which is the point: the
`### 1234 - …` under a day and the `### 1234 - …` above its diff are visibly
the same thing. All three heading levels are bold, so what separates them is
the hue alone -- which is still enough, because the `#`s and the nesting say
the same thing in the text. The four lines around a diff, its fold markers and
its backtick fence, are the only machinery left in the document, and dim is
what says so. The notes are content, so nothing in them is painted: a coloured
`[x]` here would say the report is about task state, which is `tasks`' claim to
make, not this command's.

The diff is the one part the report does not paint itself. With colour on it is
requested from git with `--color=always`; with colour off, `--no-color`. That
gives the diff red/green, which no other part of the report uses for meaning,
and git's own slots are pinned so a user's `color.diff.*` cannot add hues:
`meta` and `frag` are bold with no hue, move detection is off, and the diff
body is passed through exactly as git emits it. This is the one place two
colour systems meet, and it is deliberate: re-encoding a diff ourselves would
mean parsing it.

Warnings (`--follow` unsupported, a git diff that failed) go to **stderr** in
yellow when colour is on, never to stdout: stdout is the report, and it is fed
to an LLM.

Yellow is therefore free for warnings: `tasks` now draws the `BLOCKED` rule and
the `[?]` marker in magenta, so a yellow line in a report is always a warning.

## Modules

The split is the one the crate already uses: pure logic in a `report.rs`,
IO at the edge in `commands/report.rs`.

| File | Change |
|---|---|
| `src/report.rs` | The `Input`/`Day` model, grouping, ordering and rendering: pure functions over `&str`, in the style of `src/tasks.rs`. `days` is colour-free; `render` takes the palette and paints only the structure. |
| `src/date.rs` | `parse` and `range`: the plain-English grammar and the calendar arithmetic, shared by report and `worklog --date`. |
| `src/commands/report.rs` | Repository root, source discovery (sealed included), entries, the injected differ and picker, printing. The `run`/`run_in` split from `commands/tasks.rs`. |
| `src/git.rs` | `version`/`follows_renames`, `has_head`, `last_commit`, `empty_tree` and the pinned `diff`. |
| `src/cli.rs` | `Command::Report(ReportArgs)`. |
| `src/commands/mod.rs` | One `match` arm. |
| `src/tasks.rs` | `Tree::render_all`, for printing a day's bullets without a section and without paint. |
| `src/style.rs` | The report's four structural slots (`title`, `subheading`, `dossier`, `scaffold`), all bold hues but the scaffolding, which is dim. `Palette::state` is gone: the report no longer hues a marker by its state, so nothing wanted it. |
| `src/worklog.rs` | `worklog_days`, `without_links` and `linked_dossiers`, the readers the report is built from. |
| `README.md` | The `bureau report` paragraph. |

## Tests

Pure (`src/report.rs`, `src/date.rs`):

- The date grammar: every accepted form, the boundaries of a range, an
  invalid string, and an injected `today` so the tests never read the clock.
- Grouping by day, dossier order taken from an entry, the name-ascending
  fallback when the entry is missing, duplicate day headings, sealed sources
  included, `--no-diff`, the fold markers, the computed fence (a diff holding a
  fence line of its own keeps it inside the block), the first-appearance order
  of the diff blocks, and CRLF.
- The entry extractor: its sections demoted one level, a nested section keeping
  its parent, an empty section dropped, the `# <date>` title and the
  `## Worked on Dossiers` index dropped, and no `\r` reaching the output.
- The day heading's weekday, two blank lines before every `##` (the first day
  and a filtered report's day included), the scope line's presence
  when filtered and absence when not, and a dossier named by its own title
  with the file stem as the fallback.
- Colour: a terminal run hues the title, the `##` headings, the dossier
  headings and the scaffolding around each diff, and leaves every bullet, note
  and diff body alone; `Palette::OFF` (a pipe, `NO_COLOR`, `TERM=dumb`) prints
  byte-for-byte the plain report, with no escape anywhere in it. The diff body
  is passed through exactly as git emits it, so with colour on it carries git's
  red and green and with colour off it carries none.
- A fixture's **full report**, compared byte for byte. This is the format
  regression test, the counterpart of `tasks`' worked example.

Command (`src/commands/report.rs`):

- The four no-work messages, one test each, for a single day and for a range:
  no entry file, and entry files holding no work. Also that a one-day
  `--from`/`--to` period is named as a day, and that a `--filter`ed report
  keeps the `no work` wording.
- A worklog day with no entry reports on its own; a workless entry prints the
  note beside a day that has work; a worklog heading with nothing under it
  makes no day at all. A missing `entries/` or `dossiers/` directory is not an
  error.
- The differ is **injected into `run_in`**, the way the picker and the prompt
  are, so command tests supply placeholders instead of calling git and do not
  depend on the installed git version. Useful fakes, as plain closures: one
  returning the dossier name as the diff (placement and block order), one
  returning `Err` (the stderr warning and the `(Git changes could not be read)`
  note), one returning empty (the `(No git changes were found)` note, and that
  no fold markers or fence are printed for it), one recording the requests
  (which dossiers and which range were asked for, and that `--no-diff` never
  asks).
- The real git call is tested in `git.rs` against a scratch repository carrying
  hostile config (`diff.colormoved`, a `textconv`, `diff.algorithm`,
  `color.diff.*`) so the pinned flags are proven, and a rename exercises
  `--follow` where git supports it.

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
