# `bureau tasks`

## Purpose

List the tasks in a notes repository as a tree, grouped by what can be done
about them right now. It answers "what should I pick up next" without opening
an editor, and without the user having to keep tasks in any particular
section.

This command is **read-only**: it never writes a file, never stages anything
and never commits.

## Usage

```bash
bureau tasks [--filter [<pattern>]] [--menu] [--all]
```

| Argument | Meaning |
|---|---|
| `--filter` | With no pattern: the most recently modified dossier. With a pattern: the dossier whose name contains it (case-insensitive). If several match, the same picker `worklog` uses opens. Entries are never included in a filtered run. |
| `--menu` | Open that picker over every dossier, filter or not. |
| `--all` | Also print the `FINISHED` section. |

No arguments: every source, `PENDING` and `BLOCKED` only.

### Selection

The filter follows `bureau worklog`'s rule exactly, so the muscle memory
carries over:

| Invocation | Result |
|---|---|
| `bureau tasks` | every entry and every dossier |
| `bureau tasks --filter` | the newest dossier by modification time; the picker when several share that timestamp |
| `bureau tasks --filter auth` | the one dossier matching `auth`; the picker when several match |
| `bureau tasks --filter auth --menu` | the picker over all dossiers |
| `bureau tasks --menu` | the picker over all dossiers |

`--filter <pattern>` that matches nothing is an error (`no dossier matches
'<pattern>'`); `--filter` with no dossiers at all is an error too. Every other
outcome, including "the selection has no tasks", prints a message and exits 0.
An empty result is not a failure.

## Output

```
=== PENDING ===
# 2026-09-21 (entry)
- [ ] ask Tom about the auth split
# 1234 - refactor authentication
- Random comment
  - [ ] Ask Tom how he'd do X
- [o] Split auth.rs into more modules
  - [o] Logic
- [.] Update docs
- [x] Create tests
  - [o] Data structures
    - [ ] "Evil" path: malformed data, etc

=== BLOCKED ===
# 1234 - refactor authentication
- [o] Split auth.rs into more modules
  - [?] API: waiting to talk with John about this
- [x] Create tests
  - [?] API: waiting to talk with John about this
```

With `--all`, the same output plus:

```
=== FINISHED ===
# 1234 - refactor authentication
- [x] Check current auth docs
- [o] Split auth.rs into more modules
  - [x] Data structures
- [x] Create tests
  - [o] Data structures
    - [x] Happy path
  - [x] Logic
- [-] Ask boss about task: nah, he's on vacation the whole month
```

### Shape rules

- Sections print in the order `PENDING`, `BLOCKED`, `FINISHED`. A section with
  nothing in it is not printed at all.
- Each source (entry or dossier) that has something to show is printed once per
  section, as `# <heading>` followed by its tree.
- Entries first, newest date first. Then dossiers, most recently modified
  first, file name ascending as the tiebreak (a fresh `git clone` stamps every
  file with one time, so the tiebreak decides the whole order there). A dossier
  whose modification time cannot be read sorts after the ones that have one.
- Headings: an entry is `# YYYY-MM-DD (entry)`, a dossier is its file name
  without the extension. Entry files never print their own `##` headings.
- Indentation in the output is two spaces per level, regardless of how the
  input was indented.
- Every printed task keeps **its own marker**, even when that marker does not
  match the section it is helping to explain: `- [o] Split auth.rs into more
  modules` appears under `BLOCKED` because it is the ancestor of a blocked
  task, and still reads `[o]`.
- A non-task bullet prints only when a task below it is printed, as
  `- <original text>` with no marker. That applies at any depth, including a
  run of them:
  ```markdown
  - Random message
    - Some context
      - [ ] Random task
  ```
  prints all three lines.
- Nothing is printed for its own sake. Every line is either a task that belongs
  to the section or a link in the chain to one, so a sibling of a printed task
  is never printed just to show that it exists.
- A dossier that only has finished tasks prints nothing without `--all`: its
  non-finished task list is empty, so it has nothing to show. With `--all` it
  prints its closed tasks like any other dossier. This is deliberate: seeing a
  dossier that can be sealed is useful, and it is the pressure that will
  justify `bureau seal` later (see `TASKS.md`).

### Empty state

When every printed section would be empty, the whole run prints exactly:

```
No pending tasks
```

(no trailing full stop). That covers a repository with no notes at all, and a
repository whose dossiers only hold finished tasks. With `--all` the
`FINISHED` section counts, so this line appears only when there is genuinely
nothing anywhere.

## Grammar

### What counts as a task

A task is a bullet whose text starts with a marker:

```
^ [ \t]* "-" [ \t]* "[" <symbol> "]"
```

- Leading whitespace before the dash is allowed and ignored for this test.
- Any run of spaces and tabs between the dash and the marker is accepted, so
  `- [x]`, `-[x]`, `-  [x]` and `-\t[x]` are the same task. This is more
  permissive than CommonMark on purpose: hand-written notes should not need to
  match a strict renderer to be picked up.
- The marker has to start the bullet text. `- I should check [x] later` is
  prose, not a task.
- A bare `-` with nothing after it is nothing at all, and is ignored like any
  other non-bullet line.

| Symbol | State | Section |
|---|---|---|
| `[ ]` | pending, not started | `PENDING` |
| `[.]` | pending, in progress | `PENDING` |
| `[o]` | pending, almost done | `PENDING` |
| `[?]` | on wait | `PENDING` and `BLOCKED` |
| `[x]` | done | `FINISHED` |
| `[-]` | cancelled | `FINISHED` |

A `[?]` task is the one marker that belongs to two sections, because it is
open (there is something left to do, eventually) and blocked (it cannot be done
yet). It is listed under both, with the same marker.

Any other symbol (`[!]`, `[>]`, `[X]`, …) is not a recognised marker, so the
bullet is treated as prose and ignored. Nothing is reported about it: this is a
listing, not a linter.

### What counts as a parent

Indentation decides nesting, measured in columns:

- A tab advances to the next multiple of 4 (a tab stop).
- A space advances one column.
- The level of a bullet is its column count divided by 2, rounding down.

So two-space and four-space nesting both work, and mixing them in one file is
tolerated rather than reported. The level is relative, not absolute: a bullet
with no indentation is at level 0 whether it sits under `## Notes` or under
nothing at all.

Bullets nest onto the last bullet seen with a lower level. A bullet whose level
jumps more than one past its predecessor — the first bullet at level 0, then
one at level 4 — attaches to that predecessor as if it were one level deeper,
rather than inventing phantom levels or dropping the line. Non-bullet lines are
ignored for nesting, so a continuation line under a task does not disturb it.

For example, with two-space steps:

```markdown
- [.] A
    - [x] B          level 2, child of A
  - [ ] C            level 1, child of A, sibling of B
- [ ] D              level 0, new root
```

### Records

A bullet's text is everything after its marker, with one leading space removed
if there is one. The text may be empty, which is what the dossier template
leaves behind in a fresh file; such a task is listed as `- [ ]` and is
otherwise a task like any other.

Multi-line bullets are **out of scope for v1**, and the spec does not pretend
otherwise: `- [ ] Do the thing` followed by an indented continuation line
prints `- [ ] Do the thing`, and the continuation is dropped. Continuation
lines never disturb the tree, and a continuation whose text looks like another
bullet is a new node, because v1 has no way to tell the two apart.

## Classification

State predicates:

- **open**: `[ ]`, `[.]`, `[o]`, `[?]`
- **finished**: `[x]`, `[-]`

`blocked` is derived rather than read off one marker: a task is blocked when it
or any of its ancestors is `[?]`. A `[?]` task is therefore both open and
blocked, which is why it can appear in two sections at once.

Section predicates. A *match* is what a section is about; the next subsection
turns matches into printed lines:

| Section | A task matches when |
|---|---|
| `PENDING` | it is open, or it has an open descendant |
| `BLOCKED` | it is blocked, or it has a blocked descendant |
| `FINISHED` | it is finished **and** it has no unfinished descendant |

The `FINISHED` row is not "it is finished". A finished task with unfinished work
below it is excluded from `FINISHED` outright, and this is an exception to every
other rule rather than a consequence of one:

> A finished task with at least one unfinished task anywhere below it is never
> printed in `FINISHED`, not even as the ancestor of a match.

So `- [x] Create tests` does not appear in `FINISHED` while
`- [o] Data structures` hangs below it, and `- [x] Data structures`, which would
qualify on its own, disappears along with it. It appears in `PENDING` instead,
as the ancestor of the work that is left. The same goes for a `[-]` task, and
for one whose unfinished descendant is several levels down, not just a direct
child.

`FINISHED` is the only section that turns a task away because of what is below
it. In `PENDING` and in `BLOCKED` a finished task is let in for exactly that
reason — it is the context for the work left under it — and its own marker is
printed unchanged. A finished task with nothing unfinished below it has no such
reason, so those sections do not print it at all; that is what `--all` is for.

### Rendering a section

One rule covers all three sections, and decides every case below:

> Print every node that matches the section's predicate, and every ancestor of a
> match. Nothing else.

Ancestors print whether or not they are tasks: a non-task ancestor prints as
`- <its text>` with no marker, which is what keeps a note that groups tasks
readable. A match with no ancestor above it is a line on its own.

The rule never looks sideways and never looks down from an ancestor, and both
halves show: an ancestor that is only context prints as a single chain down to
the match, and nothing that is not on such a chain appears at all.

There is one more thing the rule is not free to do, and it is where an
implementation is most likely to go wrong: `FINISHED` does not walk *through* a
task it has excluded. `- [x] Data structures` under `- [x] Create tests` matches
the predicate on its own, and is still never printed, because every path to it
runs through a task the section refuses to show. So a closed subtree appears
only when the whole chain above it qualifies, and once a `PENDING` or `BLOCKED`
match is taken out, nothing below it can put it back.

Worked through in `PENDING` for the dossier at the top of this document:

- `- [o] Split auth.rs` matches (it is open), so it prints. Its child
  `- [o] Logic` also matches, so it prints too, indented under it. Its other
  child `- [x] Data structures` does **not** match — it is closed — so it is
  not printed, and nothing under it is either.
- `- [x] Create tests` is closed, so it does not match. Its child
  `- [o] Data structures` is open, so it matches, and that makes its parent an
  ancestor of a match. So `- [x] Create tests` prints as a bare `- [x] …` line
  with only the chain below it that leads to open work:
  `- [o] Data structures`, then `- [ ] "Evil" path: …`. The finished
  `- [x] Happy path` next to it is not printed.
- `- Random comment` is not a task and never matches, but it is the ancestor of
  `- [ ] Ask Tom…`, so it prints and brings that one line with it.

Worked through in `BLOCKED`: `- [o] Split auth.rs` is not blocked but is the
ancestor of `- [?] API…`, so both print, and `- [o] Logic` — a sibling of the
blocked task — does not.

Worked through in `FINISHED`, with `--all`: `- [x] Check current auth docs`
matches and prints alone. `- [o] Split auth.rs` is the ancestor of the match
`- [x] Data structures`, so both print; the ancestor is a `[o]`, and it is the
`[x]` underneath that matched. `- [x] Create tests` is not printed at all: it
has unfinished work below it, so the exception takes it out of the section, and
the closed tasks under it go with it. It appears in `PENDING` instead, as the
ancestor of what is left — and there `- [x] Data structures` is missing from
under it, while in `FINISHED` the same task is present. Same task, opposite
treatment, because the two sections ask different questions.

### What is deliberately absent

The rule above means a section can be much shorter than the part of the file it
came from, and the gaps are the design, not a bug:

- No closed task appears in `PENDING` or in `BLOCKED`, at any depth, even when
  the task above it is open or blocked. Finishing `- [x] Data structures` makes
  it vanish from the listing rather than sit there as a tick; `--all` is how you
  ask for closed work.
- No sibling is ever printed to "give context". Every printed line is either
  actionable in this section or a link in the chain to something that is.
- A `[x]` or `[-]` parent of unfinished work prints as an ancestor in `PENDING`
  and `BLOCKED` with its `[x]` marker intact. That is not an oversight: the
  marker is kept (see the shape rules) precisely so a line that looks out of
  place can be told apart from a mistake.
- In `FINISHED`, a closed task with unfinished work below it is skipped, and so
  is every closed task below *it*. A closed subtree inside an unfinished one is
  therefore never shown anywhere, which is the one place this command loses
  information rather than filtering it: the section would have to contradict
  its own name to show it.
- A non-task bullet is printed only when a task below it is printed. An
  annotation under a task, with no task of its own below it, is not printed.

## Sources

| Directory | Heading | Sort key |
|---|---|---|
| `entries/*.md` | `YYYY-MM-DD (entry)` | the date in the file name, newest first |
| `dossiers/*.md` | file stem | modification time, newest first, then file name |

Both directories are optional; neither existing is simply "no sources". A file
whose name is not a date in `entries/`, or not `.md`, is skipped. Nothing is
read outside those two directories.

**Sealed dossiers are skipped, always.** A dossier whose YAML frontmatter has a
`sealed:` field is never read, not even with `--all`. `bureau seal` does not
exist yet; the check is written now, in one predicate at the file-discovery
layer, so that adding the command later cannot accidentally expose sealed
notes.

## Modules

| File | Change |
|---|---|
| `src/tasks.rs` (new) | markers, indentation, the tree, the predicates, the renderer. Pure functions over `&str`, in the style of `src/worklog.rs` |
| `src/commands/tasks.rs` (new) | root, file discovery, reading, selection, printing. The `run` / `run_in` split from `src/commands/worklog.rs` |
| `src/cli.rs` | `Command::Tasks(TasksArgs)` and the args struct |
| `src/commands/mod.rs` | one `match` arm |
| `src/commands/sources.rs` (new) | `read_markdown(dir)`, `by_recency`, `relative`, `file_name` and the sealed check, moved out of `src/commands/worklog.rs` and shared |
| `src/commands/paths.rs` | nothing new unless a heading constant is needed |
| `README.md` | the `bureau tasks` paragraph, matching this spec |

The split is the one the crate already uses: `tasks.rs` holds the fiddly parts
as pure functions so they can be tested without a filesystem, and
`commands/tasks.rs` holds the IO at the edge. `src/tasks.rs` does not read
files and does not print.

Data model, as a sketch rather than an interface to freeze:

```rust
enum State { Todo, Doing, Almost, Waiting, Done, Cancelled }

struct Node {
    marker: Option<State>,   // None for a non-task bullet
    text: String,
    children: Vec<usize>,    // indices into the arena
}
```

An arena with indices sidesteps the borrow checker for a tree that is built
top-down and read bottom-up, and it keeps every rule a plain function of
`(tree, predicate)`. The marker is kept on every node, including the ones that
are only printed as ancestors, so the model already carries what a per-source
count of each state would need; nothing counts anything in v1
(`TASKS.md`).

## Tests

### Pure (`src/tasks.rs`)

- Markers: all six, plus the spacing variants `-[x]`, `-  [x]`, `-\t[x]`, plus
  `- I should check [x] later` staying prose and `[X]` staying prose.
- Nesting: two-space and four-space input, one level in the output; tabs; a
  level jump from 0 to 4 landing one level deep; blank lines and continuation
  lines not disturbing the tree; a root-level task in a file with no headings.
- Empty text: `- [ ]` is a task that prints as `- [ ]`.
- Classification, one test per section predicate, including the cases the spec
  calls out by hand: a `[x]` with unfinished work below it staying out of
  `FINISHED` and printing in `PENDING` as an ancestor, a closed task under that
  excluded one staying out of `FINISHED` too, a closed task with a closed child
  matching normally, a closed child of an open task not printing at all, a
  chain of non-task ancestors printing, and a non-task annotation under a task
  not printing.
- Rendering: a fixture holding every branch of the example dossier, compared
  section by section. It is the regression test for the format, and it is the
  test that would have caught printing `- [x] Create tests` under `FINISHED`.
- Determinism: two dossiers with one modification time sort by name.

### Command (`src/commands/tasks.rs`)

- A missing `dossiers/` or `entries/` directory is not an error.
- A dossier with `sealed:` frontmatter is absent, with and without `--all`.
- The filter, the menu and the newest-dossier rule, driven the way
  `worklog.rs` drives them today (the picker is not exercised).
- Empty state prints `No pending tasks` and exits 0.
- A dossier with only finished tasks appears with `--all` and not without.

`run_in` takes the root, the args and the prompt, so these run against a
scratch directory. The `Scratch` helper currently lives in
`src/commands/worklog.rs`'s test module and should move to a shared test
module rather than be copied.

## Manual check

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets
cargo test
cargo run -- tasks
cargo run -- tasks --all
cargo run -- tasks --filter auth
```

## Out of scope

- Writing, ticking, reordering or editing tasks. `bureau tasks` only reports.
- An `## Pending` / `## Completed` section convention. Sections are ignored on
  purpose: entries have tasks too, and the same rule has to work in both.
- Sealing dossiers (`TASKS.md`).
- Colour. The output is plain text; styling the whole CLI is a separate
  decision.
- Filters over entries, over task text, or over states (`--pending`,
  `--blocked`, `--finished`).
- Statistics. "18 of 20 done" per dossier is a nice idea and nothing in the
  design blocks it later, but v1 prints tasks and no numbers (`TASKS.md`).
