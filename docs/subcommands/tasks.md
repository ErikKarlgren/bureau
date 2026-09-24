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
bureau tasks [--filter[=<pattern>]] [--menu] [--all]
```

| Argument | Meaning |
|---|---|
| `--filter[=<pattern>]` | With no pattern: the most recently modified dossier. With a pattern: the dossier whose name contains it (case-insensitive). If several match, the same picker `worklog` uses opens. Entries are never included in a filtered run. The pattern is attached with `=`, so `--filter auth` is an error and `--filter=auth` is not: an optional value that can also be a separate argument is how `--filter` with no pattern gets mistaken for `--filter` with somebody else's argument. |
| `--menu` | Open that picker over every dossier, filter or not. |
| `--all` | Widen the listing to the work there is nothing to do about: the `FINISHED` section, for dossiers only, and the tasks a `[?]` is holding up, in `BLOCKED`. |

No arguments: every source, `ACTIONABLE` and `BLOCKED` only, with `BLOCKED`
holding the `[?]` tasks on their own.

### Selection

The filter follows `bureau worklog`'s rule exactly, so the muscle memory
carries over:

| Invocation | Result |
|---|---|
| `bureau tasks` | every entry and every dossier |
| `bureau tasks --filter` | the newest dossier by modification time; the picker when several share that timestamp |
| `bureau tasks --filter=auth` | the one dossier matching `auth`; the picker when several match |
| `bureau tasks --filter=auth --menu` | the picker over all dossiers |
| `bureau tasks --menu` | the picker over all dossiers |

`--filter <pattern>` that matches nothing is an error (`no dossier matches
'<pattern>'`); `--filter` with no dossiers at all is an error too. Every other
outcome, including "the selection has no tasks", prints a message and exits 0.
An empty result is not a failure.

## Output

```
=== ACTIONABLE ===
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

`--all` widens `BLOCKED` at the same time. On its own, a wait is all the section
has to say; with `--all`, the work it is holding up is listed under it:

```
$ bureau tasks
=== BLOCKED ===
- [?] API: waiting to talk with John about this
```

```
$ bureau tasks --all
=== BLOCKED ===
- [?] API: waiting to talk with John about this
  - [ ] Regenerate the client
  - [o] Port the callers
```

### Shape rules

- Sections print in the order `ACTIONABLE`, `BLOCKED`, `FINISHED`. A section with
  nothing in it is not printed at all.
- `--all` widens `BLOCKED` as well as adding `FINISHED`: a `[?]` is listed on
  its own, and the work it is holding up is listed under it. A `[?]` nested
  further down is a wait in its own right, so it is listed either way, together
  with the chain of tasks that reaches it.
- `FINISHED` is a dossiers-only section. A finished task in a daily entry is
  never printed, with or without `--all`; entries appear in `ACTIONABLE` and
  `BLOCKED` and nowhere else. Daily entry tasks are meant to be closed the day
  they are written, so a finished one is a leftover rather than history worth
  keeping a listing of.
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
  modules` appears under `BLOCKED` because it is the ancestor of a waiting
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
No actionable tasks
```

(no trailing full stop). That covers a repository with no notes at all, and a
repository whose dossiers only hold finished tasks. With `--all` the
`FINISHED` section counts, so this line appears only when no dossier has
anything unfinished and no entry has anything at all — an entry holding nothing
but finished tasks prints this line, because those tasks are not shown
anywhere either.

## Colour

Colour is an aid to scanning, never a carrier of information. Every hue repeats
something the line already says, so a run with colour off loses nothing but the
emphasis, and prints byte for byte what it printed before colour existed.

| Line | Drawn in |
|---|---|
| `=== ACTIONABLE ===` | bold blue |
| `=== BLOCKED ===` | bold yellow |
| `=== FINISHED ===` | bold green |
| `[ ]` in `ACTIONABLE` | blue |
| `[.]` and `[o]` in `ACTIONABLE` | cyan |
| `[?]` in `BLOCKED` | yellow |
| `[x]` and `[-]` in `FINISHED` | green |
| `# <heading>`, entry or dossier | bold, no hue |
| every other task line | dim |
| a bullet with no marker | dim |
| `No actionable tasks` | plain |

Three things about that table are worth spelling out.

**A section colours its own markers.** That is a question about the marker, not
about the section's match predicate. Only `[?]` is yellow in `BLOCKED`: the
branch `--all` adds is dim, even though the section's predicate calls every task
in it a match. Since the rule never asks whether a line matched, it never has to
look at the tree, and a task that appears in two sections can legitimately be
coloured in one and dim in the other.

**The hue goes on the marker alone.** A task reads as a coloured marker rather
than a coloured sentence, and its text keeps the terminal's own foreground, so
colour survives long task text and copy-paste. The exception is the dim
register, which is about a line's place in the listing rather than about its
marker, and covers the whole line.

**The palette is the eight basic `ANSI` colours**, plus the bold and dim
attributes: no 256-colour, no truecolor, and no bright slots, so the terminal's
theme decides the shades. Only blue, cyan, yellow and green are ever used.
`[.]` and `[o]` share a hue on purpose: both mean the work has been started, and
the marker itself is what tells them apart.

### When colour is off

Colour is for a person reading a terminal, so it is left out entirely — no hue,
no bold, no dim — when standard output is not a terminal, when `NO_COLOR` is set
to anything non-empty, and when `TERM=dumb`. Nothing prints differently: the
text is exactly the text of a plain run.

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
| `[ ]` | not started | `ACTIONABLE` |
| `[.]` | in progress | `ACTIONABLE` |
| `[o]` | almost done | `ACTIONABLE` |
| `[?]` | on wait | `BLOCKED` |
| `[x]` | done | `FINISHED` |
| `[-]` | cancelled | `FINISHED` |

A `[?]` task belongs to `BLOCKED` alone, even though there is still work to do
on it: `ACTIONABLE` is the list of things that can be picked up, and something
waiting on somebody else is not one of them. Neither is anything under it,
which is why that branch takes `--all` to be listed at all.

Any other symbol (`[!]`, `[>]`, `[X]`, …) is not a recognised marker, so the
bullet is treated as prose and ignored. Nothing is reported about it: this is a
listing, not a linter.

### What counts as a parent

Nesting comes from comparing indentation, never from a width:

> A bullet nests into the nearest bullet above it that is indented **less**
> than it is. A bullet indented **the same** as one above it is that bullet's
> sibling, and closes off everything deeper.

No width is assumed and none is rounded to a level. Two spaces, four, six, a
tab or an odd number of columns all mean the same thing — deeper than the last
one, or level with it — so a file indented four spaces and a file indented two
nest identically, and a file that uses both is read as it looks. Indentation is
counted in columns, with a tab advancing to the next multiple of 4 so that tabs
at least compare sensibly against spaces.

Depth is relative, not absolute: the shallowest bullet in a file is a root
whatever its indentation, whether it sits under `## Notes` or under nothing at
all. Non-bullet lines are ignored for nesting, so a continuation line under a
task does not disturb it.

The comparison is what keeps a family of bullets at one indentation flat. Four
spaces is not "two levels"; it is simply deeper than the bullets above, so both
of these print the same way:

```markdown
- [ ] a
- [ ] b
    - [ ] c
    - [ ] d
```

```markdown
- [ ] a
- [ ] b
  - [ ] c
  - [ ] d
```

Were each bullet's depth counted off in fixed steps, the second bullet at a
given indentation would find itself one level deeper than the first, and the
list would staircase.

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
- **waiting**: the marker is `[?]`
- **blocked**: waiting, or under a task that is

The first two are read off a marker and the last two are not. `waiting` is
exactly the `[?]` marker. `blocked` is derived: a task is blocked when it is
`[?]`, or when any of its ancestors is. A `[?]` task is therefore open *and*
blocked, which is what keeps it out of `ACTIONABLE`, while `BLOCKED` is where
the wait itself is listed.

`waiting` and `blocked` are kept apart because `BLOCKED` is about the waits you
can chase, not about the work they are holding up. The two differ only for a
task nested under a `[?]`, and that difference is exactly what `--all` asks
for.

Section predicates. A *match* is what a section is about; the next subsection
turns matches into printed lines:

| Section | A task matches when |
|---|---|
| `ACTIONABLE` | it is open and not blocked |
| `BLOCKED` | it is waiting; with `--all`, it is blocked |
| `FINISHED` | it is finished **and** it has no unfinished descendant |

A task's marker is a statement about *that task*, and each section takes it at
face value. `- [.] Implement feat A` with `- [x] Do X` and `- [x] Do Y` under it
is still work in progress, because `A` is more than `X` plus `Y`, and it is
still listed even when the only thing left under it is blocked. What a marker
does not do is decide what is below it: a closed child never prints in
`ACTIONABLE`, and neither does anything under a `[?]`.

The `FINISHED` row is the one place that is not "it is finished". A finished
task with unfinished work below it is excluded from `FINISHED` outright, and
this is an exception to every other rule rather than a consequence of one:

> A finished task with at least one unfinished task anywhere below it is never
> printed in `FINISHED`, not even as the ancestor of a match.

So `- [x] Create tests` does not appear in `FINISHED` while
`- [o] Data structures` hangs below it, and `- [x] Data structures`, which would
qualify on its own, disappears along with it. It appears in `ACTIONABLE` instead,
as the ancestor of the work that is left. The same goes for a `[-]` task, and
for one whose unfinished descendant is several levels down, not just a direct
child.

`FINISHED` is the only section that turns a task away because of what is below
it. In `ACTIONABLE` and in `BLOCKED` a finished task is let in for exactly that
reason — it is the context for the work left under it — and its own marker is
printed unchanged. A finished task with nothing unfinished below it has no such
reason, so those sections do not print it at all; `--all` is how closed work is
asked for, and `FINISHED` is where it is printed.

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
only when the whole chain above it qualifies, and once an `ACTIONABLE` or `BLOCKED`
match is taken out, nothing below it can put it back.

Worked through in `ACTIONABLE` for the dossier at the top of this document:

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
- `- [?] API: waiting to talk with John about this` is missing from all of the
  above. It is open, but it cannot be worked on, and `ACTIONABLE` is not the
  list of everything unfinished: it is the list of what can be picked up. Its
  parent stays, because the marker that is blocked is the child's and not the
  parent's.

Worked through in `BLOCKED`: `- [o] Split auth.rs` is not blocked but is the
ancestor of `- [?] API…`, so both print, and `- [o] Logic` — a sibling of the
wait — does not. So `Split auth.rs` is the one task in the example that
is listed in every section it could be: `ACTIONABLE` for its own sake, `BLOCKED`
for its waiting child, and `FINISHED` as the ancestor of a closed one.

Worked through in `FINISHED`, with `--all`: `- [x] Check current auth docs`
matches and prints alone. `- [o] Split auth.rs` is the ancestor of the match
`- [x] Data structures`, so both print; the ancestor is a `[o]`, and it is the
`[x]` underneath that matched. `- [x] Create tests` is not printed at all: it
has unfinished work below it, so the exception takes it out of the section, and
the closed tasks under it go with it. It appears in `ACTIONABLE` instead, as the
ancestor of what is left — and there `- [x] Data structures` is missing from
under it, while in `FINISHED` the same task is present. Same task, opposite
treatment, because the two sections ask different questions.

### What is deliberately absent

The rule above means a section can be much shorter than the part of the file it
came from, and the gaps are the design, not a bug:

- No closed task appears in `ACTIONABLE` or in `BLOCKED`, at any depth, even when
  the task above it is open or blocked. Finishing `- [x] Data structures` makes
  it vanish from the listing rather than sit there as a tick; `--all` is how you
  ask for closed work.
- Nothing below a `[?]` appears in `ACTIONABLE`, at any depth, because being
  under a block is what blocked means. It is not listed under `BLOCKED` either:
  that section is about the waits themselves, so the work a wait is holding up
  is hidden until `--all` asks for it. The task *above* the block is a
  different matter: it is open, it is not blocked, and it prints as a line of
  its own, since the only thing that could print under it is the blocked work.
- `--all` does not put finished work into `BLOCKED`, even when it sits inside a
  blocked branch: a closed task is listed in `FINISHED` or nowhere, at either
  width.
- A finished task in an entry is not printed. `FINISHED` covers dossiers only:
  the section exists to show work that is worth reviewing later, and a ticked
  line in a day's notes has already served its purpose. Nothing is lost by it
  either way, since the entry itself is the record.
- No sibling is ever printed to "give context". Every printed line is either
  actionable in this section or a link in the chain to something that is.
- A `[x]` or `[-]` parent of unfinished work prints as an ancestor in `ACTIONABLE`
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
| `src/style.rs` (new) | the palette, the marker rule and the decision to draw at all. Pure except for reading the environment and `stdout` |
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
- Nesting: two, four, six, eight and odd-width indentation all nesting one
  level; tabs; bullets sharing an indentation staying siblings at any width
  (the staircase regression); blank lines and continuation lines not
  disturbing the tree; a file whose shallowest bullet is indented still
  starting at the root.
- Empty text: `- [ ]` is a task that prints as `- [ ]`.
- Classification, one test per section predicate, including the cases the spec
  calls out by hand: a `[x]` with unfinished work below it staying out of
  `FINISHED` and printing in `ACTIONABLE` as an ancestor, a closed task under that
  excluded one staying out of `FINISHED` too, a closed task with a closed child
  matching normally, a closed child of an open task not printing at all, a
  chain of non-task ancestors printing, and a non-task annotation under a task
  not printing.
- The breadth of `BLOCKED`: a `[?]` printing on its own while `--all` prints the
  branch it holds up, a `[?]` nested under a `[?]` printing with the chain that
  reaches it at either width, and a closed task inside a blocked branch staying
  out of `BLOCKED` at either width.
- Rendering: a fixture holding every branch of the example dossier, compared
  section by section. It is the regression test for the format, and it is the
  test that would have caught printing `- [x] Create tests` under `FINISHED`.
- Determinism: two dossiers with one modification time sort by name.
- Colour in the renderer: a coloured run paints the marker and leaves the text
  alone, dims a line that is only there for context, dims a bullet with no
  marker, and with colour off produces exactly the plain lines.

### Colour (`src/style.rs`)

- Which markers each section hues, and that a section's other marker states take
  no hue, so `--all`'s branch of `BLOCKED` is context rather than a wall of
  yellow.
- That a pipe, `NO_COLOR`, and `TERM=dumb` each turn colour off, as a plain
  function of those three facts.
- That the palette draws with the basic `ANSI` slots and the two attributes,
  with no bright slot and no truecolor in it.

### Command (`src/commands/tasks.rs`)

- A missing `dossiers/` or `entries/` directory is not an error.
- A dossier with `sealed:` frontmatter is absent, with and without `--all`.
- The filter, the menu and the newest-dossier rule, driven the way
  `worklog.rs` drives them today (the picker is not exercised).
- Empty state prints `No actionable tasks` and exits 0.
- A dossier with only finished tasks appears with `--all` and not without.
- A `[ ]` under a `[?]` is absent from the whole run without `--all`, and listed
  under `BLOCKED` with it.
- An entry with finished tasks in it prints nothing for them, with `--all` set,
  while its unfinished tasks still print.
- A coloured run puts the section hue on the section rule, bold on the source
  heading, the hue on the marker alone, and dim on the line above a match.

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
cargo run -- tasks | cat            # plain: no escapes reach a pipe
NO_COLOR=1 cargo run -- tasks       # plain, in a terminal
```

## Out of scope

- Writing, ticking, reordering or editing tasks. `bureau tasks` only reports.
- An `## Pending` / `## Completed` section convention. Sections are ignored on
  purpose: entries have tasks too, and the same rule has to work in both.
- Sealing dossiers (`TASKS.md`).
- Styling the rest of the CLI. `bureau tasks` has its own palette (see
  [Colour](#colour)); the other commands print plain text, and a shared
  convention is a separate decision.
- Forcing colour through a pipe: `bureau tasks | less -R` gets plain text,
  because there is no `--color` flag and no `CLICOLOR_FORCE` yet.
- Windows consoles that do not understand `ANSI` escapes. Colour is written
  wherever a terminal is attached; enabling virtual terminal processing on a
  legacy `conhost` needs a platform call that `std` does not make.
- Filters over entries, over task text, or over states (`--pending`,
  `--blocked`, `--finished`).
- Statistics. "18 of 20 done" per dossier is a nice idea and nothing in the
  design blocks it later, but v1 prints tasks and no numbers (`TASKS.md`).
