# `bureau doctor` — diagnostic scope and candidate checks

**Status:** superseded by [`docs/subcommands/doctor.md`](subcommands/doctor.md).
The implemented decisions differ from this proposal where the discussion
settled them: bare worklog day headings are errors and are no longer read by
`worklog`/`report`; a missing `## Worked on Dossiers` is a warning; cross-links
are checked in both directions; sealed dossiers are skipped for now; and
`doctor` prints `path:line: Severity: message`. This file remains as the
discussion record.

The short version: `bureau doctor` should be a read-only linter for the *shape*
of the notes, not for their content. The highest-value checks are broken links,
non-portable file names, a missing or malformed `## Worklog`, task bullets the
task parser cannot see, empty task placeholders, and the worklog/entry link
relationship. The `tmp/` repository shows two of those directly (empty tasks and
the worklog/entry link relationship), plus one Markdown-rendering issue
(one-space task indentation) if that is considered in scope.

The report has three jobs:

1. pin down what doctor is and is not;
2. list every check I think is worth considering, with complexity, value and the
   tradeoff for each;
3. recommend a small v1, a cheap second tier, and the things that should stay
   out of the command.

## 1. What doctor should be

### 1.1 Scope

Doctor reads the working tree, not `HEAD`. It checks the same two directories
every other command reads:

| Directory | Files | Why |
|---|---|---|
| `dossiers/` | `*.md` | dossiers, including sealed ones |
| `entries/` | `YYYY-MM-DD.md` | daily entries |

A file in `entries/` that is not date-named is not an entry; N2 reports it, but
its contents are not linted as a day. Doctor must **not** check `README.md`,
`docs/`, `TASKS.md`, `templates/`, or files outside those two directories. It is
a notes doctor, not a repository-wide Markdown linter.

Sealed dossiers should be checked. Sealing is about hiding a dossier from
*action* (`tasks` and `worklog`), not from the record; `report` reads sealed
dossiers for the same reason. A broken link in a sealed dossier is still a
broken link. Keep in mind "sealed dossiers" haven't been implemented yet.

Doctor is **read-only**:

- no writes, no edits, no formatting;
- no `git add`, `git commit`, or index changes;
- no prompts, no picker, no TTY requirement;
- no network access, no external URL checks;
- no new dependencies.

The output must be useful both to a person and to a pipe. It is therefore plain
text on stdout, with no colour in v1. If colour is ever added, it may only paint
the severity word, and only when stdout is a terminal.

### 1.2 Errors and warnings

Two severities are enough. A third (`info`/`note`) would only make users ask
which one matters.

| Severity | Meaning | Examples |
|---|---|---|
| `error` | Bureau cannot use the note as intended, or the note's shape hides content from a command. | missing `## Worklog`, unclosed code fence, unreadable file, malformed required heading |
| `warning` | Bureau still works, but the note is inconsistent or a normal Markdown reader will see something else. | broken link, empty task, non-portable file name, bare worklog day heading, CRLF |

Recommended exit code:

- `0` when there are no findings or only warnings;
- `1` when there is at least one error;
- `--strict` later, if wanted, promotes warnings to a failing exit.

There is a small implementation wrinkle here. `main` currently turns any
`Err` into `Error: ...` on stderr and exit 1. Doctor must not print a second
error just because it found errors; findings are its normal output. The clean
change is to let `commands::run` (or a doctor-specific arm in `main`) return an
`ExitCode`, not an `Err`, for findings. That is a small cross-cutting change but
it needs to be decided before the command lands. Do not call
`std::process::exit`: the crate denies the `exit` lint.

### 1.3 Output shape

Proposed, subject to discussion:

```text
dossiers/43 fweaaw iuferigber.md:18: warning: empty task; bureau tasks lists it as `- [ ]`
dossiers/wwafwfwefwe.md:16: warning: empty task; bureau tasks lists it as `- [ ]`
entries/2026-09-19.md:8: warning: dossier '1234 - killing goblins was never an option' logs work on 2026-09-19, but this entry does not link it
3 warnings
```

Rules:

- `path:line: severity: message`, one finding per line.
- Paths relative to the repository root, with `/` on every platform, the way
  `bureau new` reports paths.
- Findings sorted by path, then line, then a fixed check order, so two runs on
  the same tree are byte-identical.
- A file-level finding uses line 1, or no line; this is an open question (see
  below).
- Last line: a summary such as `3 warnings` or `1 error, 3 warnings`; exact
  wording to be decided. With no findings, `No formatting issues found` reads
  better than `0 errors, 0 warnings`.
- Findings go to **stdout**, not stderr. This is a deliberate exception to the
  usual "warnings go to stderr" rule: doctor's warnings are the command's data,
  and a pipe must receive them. Only "doctor could not run at all" belongs on
  stderr.

Grouped output (a file heading followed by indented findings) is friendlier to
read but worse to pipe. `path:line:` wins unless the user wants a human-only
default. A `--format` switch can come later.

## 2. What `tmp/` actually shows

I read every dossier and entry in `tmp/` and checked the things the code cares
about. The concrete findings are:

| File:line | What is there | What it means |
|---|---|---|
| `dossiers/43 fweaaw iuferigber.md:18` | `- [ ] ` | Empty task. `bureau tasks` lists it as a phantom `- [ ]`. |
| `dossiers/wwafwfwefwe.md:16` | `- [ ] ` | The same. |
| `entries/2026-09-19.md:7-8` | `## Worked on Tasks` and `- ` | Dossier `1234 - killing goblins was never an option` has a worklog line for this day, but the entry has no `## Worked on Dossiers` and never links it. |
| `dossiers/9820 - mine crypto.md:15,16,18,20,23,28,29` | bullets indented in one-column steps | Bureau nests them because it compares relative indentation, but a CommonMark renderer may not treat a one-space step as a nested list. |
| `dossiers/43 fweaaw iuferigber.md:5`, `9820:5`, `lmao:5`, `wwafwfwefwe:5` | `- ` under `## Affected Systems` | Empty template placeholder. Hidden from every command. |
| `entries/2026-09-19.md:8`, `2026-10-01:4`, `2026-10-02:4`, `2026-10-08:4`, `2026-10-09:4`, `2026-10-17:4` | `- ` under `## Notes` | The same placeholder. |
| many | empty `## Notes`, `## Completed`, `## Worklog`, `## Worked on Dossiers` | Template-shaped empty sections. Normal, and not worth warning about on their own. |

What is **not** wrong in `tmp/`, and should not be flagged:

- all nine internal links resolve; worklog day headings point at the right
  entries, and entry dossier links point at the right dossiers;
- file names contain spaces and hyphens but no Windows-forbidden characters,
  reserved names, trailing dots/spaces or case collisions;
- every file is UTF-8, LF-only, has a final newline and no BOM;
- every dossier has a valid `- Creation date:` line, a title matching its
  sanitized file name, and all six template sections;
- `## Worked on Tasks` is a legitimate user section, not a typo for
  `## Worked on Dossiers`; `report` preserves it as an entry section. Do not
  flag it;
- `### faobgbaoiegiabpibg` under `## Description` in `43 fweaaw iuferigber` is
  a legitimate subsection. Extra sections and subsections are allowed;
- empty template sections are allowed. A fresh dossier has several;
- finished tasks under `## Pending` are allowed. `tasks` deliberately ignores
  section names, and moving a finished child out of its parent would break the
  hierarchy the tree is there to show.

If a v1 doctor implemented only the Tier 1 checks from §4 below, it would
print three warnings on `tmp/`: the two empty tasks and the missing entry link.
That is a nice size: every finding is real and none is a template artifact.
Adding the ten empty plain bullets would make it thirteen; adding the seven
one-column indentation findings would make it twenty. That is the noise
tradeoff in concrete numbers.

## 3. Candidate checks

Each item has a severity, a complexity estimate, a value estimate and the
tradeoff. The estimates are in terms of this crate:

- **Low**: one small pure function plus tests, no new parser.
- **Medium**: needs the shared line scanner, cross-file indexing, or careful
  false-positive control.
- **High**: a new dependency, a full Markdown parser, or a change that risks
  changing the meaning of notes.

The "fixable later" line is not a promise. Doctor is diagnostic first; any
auto-fix is a separate decision and should be explicit (`--fix`), dry-run by
default, and per check.

### 3.1 File and identity checks

#### N1. Non-portable file names (from `TASKS.md`)

- **What it finds:** a dossier or entry file name that Windows cannot check out
  reliably: `< > : " / \ | ? *`, control characters, a trailing dot or space, a
  reserved device name (`CON`, `NUL`, `COM1`, …), or a name over the
  filesystem's length limit.
- **Severity:** warning.
- **Complexity:** Low. The crate already has `dossier::name_to_filename` and
  `sanitise-file-name`; either expose a portability predicate beside it, or
  compare the existing name with `name_to_filename(stem)`. The latter is a
  blunt instrument: it also flags names that are portable but not "bureau
  style" (for example, double spaces). A dedicated predicate is better.
- **Value:** High. The product claims Windows and Linux support, and a file
  that cannot be checked out on one of them breaks the repository for everyone.
- **Tradeoff:** Worth doing in v1. Keep it a warning, not an error: the file is
  fine on the current machine. Do not flag spaces, hyphens, accents, or
  non-ASCII letters; all are valid on both platforms and the crate's tests
  already preserve `Förstudie`. Fixing this is `bureau rename`'s job, not
  doctor's.
- **Extra case to fold in:** case-insensitive collisions (`Foo.md` and
  `foo.md`). They are harmless on Linux and cannot coexist on a case-insensitive
  volume (the default on Windows and macOS). Complexity Low (a lowercase map);
  value Medium; warning. Worth having for the same cross-platform reason.

#### N2. Files bureau will not use

- **What it finds:** a `.md` file in `entries/` whose stem is not a
  `YYYY-MM-DD` date, a file in either notes directory that is not `.md`, or a
  directory whose name ends in `.md`. `tasks` and `report` skip all of these;
  an unreadable one may produce a warning, but it never becomes a note.
- **Severity:** warning.
- **Complexity:** Low. Doctor already lists both directories; this is the
  `.md`/date filter, not a parser.
- **Value:** Medium. A note that is silently invisible is exactly the kind of
  thing doctor should tell you about.
- **Tradeoff:** A scratch file in `entries/` may be intentional. Keep it a
  warning and word it as "this file will not be read by bureau", not "delete
  this file". Consider only warning for `.md` files, and not for `.gitkeep` or
  editor swap files. This is a Tier 2 check.

#### N3. Encoding and line endings

- **What it finds:** a file that is not valid UTF-8 (error); a UTF-8 BOM
  (warning); CRLF line endings or a mixture of CRLF and LF (warning); no final
  newline (warning).
- **Severity:** error for non-UTF-8; warning for the rest.
- **Complexity:** Low. Read bytes, check for `\r\n`, `\n`, `\xEF\xBB\xBF`, and
  `ends_with('\n')`; non-UTF-8 is `String::from_utf8`.
- **Value:** Medium. The product supports Windows, and `bureau worklog` writes
  new bullets with `\n` even into a CRLF file, producing mixed endings. That is
  a real portability bug. A BOM also breaks the frontmatter check and the
  `#` title lookup. Missing final newline is very low value on its own, but it
  costs almost nothing to include.
- **Tradeoff:** CRLF is valid Markdown, so this must be a warning. A future fix
  (normalise line endings, strip BOM) is mechanical and safe, unlike most
  checks here. Do not flag lone `\r` in prose; just report the mixed-ending
  case. Tier 2, but I would take it if it costs less than an hour.

#### N4. Dossier title and file name disagree

- **What it finds:** the `#` title, passed through `dossier::name_to_filename`,
  no longer matches the actual file name. For example, a file renamed manually
  or a title edited after creation.
- **Severity:** warning.
- **Complexity:** Low. Reuse `report::dossier_title` and
  `dossier::name_to_filename`.
- **Value:** Low to Medium. `tasks` and the report's day/diff headings print
  the file stem; a filtered report's `Dossier:` line prints the title. A
  mismatch can therefore make the same dossier appear under two names, but a
  deliberate sanitized name (`a::b` in `a_b.md`) must not be flagged.
- **Tradeoff:** The sanitized comparison makes false positives unlikely, but a
  user may intentionally keep a title different from the file name. Tier 3;
  only worth it if it is cheap after N1 is written.

#### N5. Missing or invalid creation date

- **What it finds:** no `- Creation date: YYYY-MM-DD HH:MM:SS` line, or a line
  that does not parse as a date.
- **Severity:** warning.
- **Complexity:** Low.
- **Value:** Low. No current command reads the field; it is a record for the
  user and the template supplies it.
- **Tradeoff:** Not worth v1. A hand-written or old dossier may not have the
  line, and doctor should not nag about metadata no command uses. Revisit when
  `bureau seal` or `bureau rename` starts reading metadata. Tier 3.

### 3.2 Structure checks

#### S1. Missing or malformed `## Worklog` in a dossier (from `TASKS.md`)

- **What it finds:** no `## Worklog` heading at all; or a heading that almost is
  one (`##Worklog` with no space, `### Worklog`, `## worklog`, `## Worklog:`,
  `## Worklog ##`, a `## Worklog` inside a fenced block, or a duplicate).
- **Severity:** error for missing or malformed; warning for a duplicate.
- **Complexity:** Low. `worklog::has_worklog` already tests for the exact
  heading, but it is not fence-aware, so a heading inside a code example would
  fool it; the doctor's scanner should make the final call. The near-miss part
  needs a normalized comparison of candidate headings (trim, lowercase, strip
  trailing hashes/punctuation, compare level), but only against this one known
  heading, so the false-positive surface is small.
- **Value:** High. `bureau worklog` refuses to run without it, and a malformed
  heading is a silent trap: the file looks right to a human and fails only when
  the command is used.
- **Tradeoff:** Worth doing in v1. `## Worklog` is the one section that is
  genuinely required by a command. A duplicate should be a warning, not an
  error: the first section wins today, which is surprising but not fatal.

#### S2. Missing other template sections

- **What it finds:** a dossier missing `## Affected Systems`, `## Description`,
  `## Notes`, `## Pending` or `## Completed`; an entry missing `## Notes`.
- **Severity:** warning.
- **Complexity:** Low. The embedded `template::DOSSIER` and
  `template::DAILY_ENTRY` already list the headings; extract them or hardcode
  the same list.
- **Value:** Medium. These are the sections the templates create; a missing
  one usually means a hand-written note that does not have the shape the rest
  of the repository uses. No command fails because of it.
- **Tradeoff:** This is the most debatable check. Users may delete sections
  they do not use, and `tmp/` would gain a warning for the entry that lacks
  `## Worked on Dossiers` even though no command fails. I would make it a
  warning, exclude `## Worked on Dossiers` (the cross-link check L5 covers the
  case where it matters), and let `--strict` turn it into a failure later. If it
  turns out noisy, demote it to `--strict` only. Tier 2, not v1.

#### S3. Duplicate known sections

- **What it finds:** two `## Worklog` headings, two `## Notes` headings in one
  file, etc.
- **Severity:** warning.
- **Complexity:** Low. A map of heading text to first line.
- **Value:** Medium. For `## Worklog` the first section wins and the second is
  silently ignored by `worklog` and `report`, so the content is on disk but out
  of the record. For cosmetic sections it is only tidiness.
- **Tradeoff:** Fold the `## Worklog` duplicate into S1 and treat the rest as a
  generic warning if the scanner is already there. Otherwise defer. Low noise.

#### S4. Empty sections

- **What it finds:** a heading with nothing under it.
- **Severity:** warning.
- **Complexity:** Low.
- **Value:** Low. Empty `## Notes`, `## Completed` and `## Worklog` are normal;
  `tmp/` is full of them.
- **Tradeoff:** **Do not add.** It would warn on every freshly created dossier
  and every entry that has not been filled in yet, and it tells the user
  nothing the empty bullet checks do not already say. A formatter might tidy
  whitespace later; doctor should not.

#### S5. Section order and heading hierarchy

- **What it finds:** `## Worklog` before `## Pending`, a skipped heading level,
  a `####` under a `##`, and so on.
- **Severity:** warning.
- **Complexity:** Low to Medium.
- **Value:** Low. No command cares, and a normal Markdown reader does not care
  about a skipped level either.
- **Tradeoff:** **Do not add.** It is taste, not correctness, and it would
  generate a lot of noise in notes people write by hand.

### 3.3 Task checks

#### T1. Empty task

- **What it finds:** a task whose text is empty or only whitespace, such as
  `- [ ] `, `- [.]`, `- [x]` and so on. The docs explicitly allow it; the
  template leaves one behind.
- **Severity:** warning.
- **Complexity:** Low. The `Tree` parser already knows the marker and text; the
  only new part is a line number. Either add a line field to `tasks::Node` (which
  also unblocks the future `tasks --quickfix`) or scan the raw lines with the
  same marker rule.
- **Value:** High. It is not invisible: `bureau tasks` lists it as a real
  `- [ ]`, so a fresh dossier shows up as work that is not there. `tmp/` has two.
- **Tradeoff:** Every newly created dossier has one, so doctor will warn
  immediately after `bureau new dossier`. That is arguably the point: the
  template placeholder is a "fill me in" marker. Keep it a warning and word it
  as a placeholder, not a corruption. Worth v1.

#### T2. Unrecognised task marker

- **What it finds:** a bullet whose text starts with a short bracket group that
  is not exactly one of `[ ]`, `[.]`, `[o]`, `[?]`, `[x]` or `[-]`. Examples:
  `[X]`, `[!]`, `[>]`, `[  ]`, `[x ]`.
- **Severity:** warning.
- **Complexity:** Low. The parser's `State::from_symbol` already defines the
  valid set; scan bullet starts and compare.
- **Value:** High. `bureau tasks` treats these as prose and silently omits
  them. `[X]` is a very common typo for `[x]`, and `[!]` is a common way to
  mark urgency.
- **Tradeoff:** A bullet like `- [note] see above` is prose, not a task. Only
  flag the short bracket shape, and phrase the message as "this looks like a
  task but is not a recognised marker" rather than "invalid syntax".
  Worth v1. The `tasks` docs say "this is a listing, not a linter"; doctor is
  the linter they were implicitly leaving room for.

#### T3. Task bullet that does not start with `-`

- **What it finds:** `* [ ]`, `+ [ ]`, `1. [ ]`, `1) [x]`, or a task marker on
  a bullet that uses another Markdown list marker.
- **Severity:** warning.
- **Complexity:** Low. A line scanner for `*`/`+`/ordered-list prefixes plus
  the marker.
- **Value:** High. These are valid Markdown task lists and completely invisible
  to `bureau tasks`, because `Tree::parse` only looks for a leading `-`.
- **Tradeoff:** A `*` bullet can be ordinary prose; only flag it when the text
  starts with a recognised task marker. Worth v1.

#### T4. Empty plain bullet

- **What it finds:** a bare `- `, or a dash followed only by whitespace, with
  no marker and no text, under any section.
- **Severity:** warning.
- **Complexity:** Low.
- **Value:** Medium. It is hidden from `tasks` and `report`, so it is only a
  template placeholder. `tmp/` has ten.
- **Tradeoff:** This is the noisiest check in the set. If T1 is already
  reporting the placeholder tasks, empty plain bullets add ten more findings
  to `tmp/` with little new information. I would leave it out of v1, or put it
  behind `--strict`, and reconsider if users ask for it. If it is included,
  group per file or summarise "N empty placeholders" rather than one line each.

#### T5. Task-like bullets inside fenced code

- **What it finds:** a `- [ ]` line inside a fenced code block. `tasks` does
  not track fences, so it lists the example as real work.
- **Severity:** warning.
- **Complexity:** Medium. It needs the fence-aware scanner (see F1), but once
  that exists the check itself is easy.
- **Value:** Medium. This is latent today: `tmp/` has no fences. It becomes
  valuable as soon as a dossier contains a pasted example or a shell snippet
  with a checklist.
- **Tradeoff:** A code fence is allowed to contain a checklist; the warning is
  about the mismatch with `tasks`, not about the Markdown. Keep it a warning,
  and only when the fenced content really looks like a task. Tier 2/3, after
  the scanner. The alternative — fixing `Tree::parse` to understand fences — is
  a separate `tasks` bug (already in `TASKS.md`) and should not be hidden inside
  doctor.

#### T6. Indentation that a Markdown reader may not nest

- **What it finds:** a child bullet indented by only one column relative to its
  parent, or another step that bureau treats as nesting but CommonMark does
  not. `tmp/dossiers/9820 - mine crypto.md` is the example: bullets at 1, 2
  and 3 spaces, reached in one-column steps.
- **Severity:** warning.
- **Complexity:** Medium. It needs the parent relationship (the `Tree` parser
  already computes it) and the raw indentation columns (tabs count as four, as
  in `tasks`). It also needs the fence-aware scanner to avoid code examples.
- **Value:** Medium. Bureau reads the file one way and a normal Markdown reader
  reads it another; that is exactly the class of bug doctor should surface.
  The one-space steps in `tmp/` are a real formatting defect.
- **Tradeoff:** This is the most controversial check. The tasks spec says, on
  purpose, that any deeper indentation nests and that width is not assumed.
  A doctor warning would be advisory, not a bug report: "bureau nests this, but
  CommonMark may not; use at least two spaces if you want both readers to
  agree." I would only flag a step of **less than two columns**, not every odd
  width, and keep it out of v1 until the user decides whether the project wants
  to bless or discourage its own loose dialect. Tier 3.

#### T7. Marker spacing variants (`-[x]`, `-  [x]`, `- [x]text`)

- **What it finds:** a task whose Markdown spacing is loose, even though
  `tasks` accepts it.
- **Severity:** warning, if it were added.
- **Complexity:** Low.
- **Value:** Low to Medium. It matters if the notes are also read by a strict
  Markdown checklist renderer; it does not matter to bureau.
- **Tradeoff:** **Do not add.** The parser accepts these forms on purpose:
  "hand-written notes should not need to match a strict renderer to be picked
  up." Doctor should not contradict that. If strict Markdown is ever wanted,
  it is a separate `--strict-markdown` mode.

#### T8. Open tasks under `## Completed`, or finished tasks under `## Pending`

- **What it finds:** a task marker whose state contradicts its section.
- **Severity:** warning.
- **Complexity:** Low to Medium (needs a section-to-task mapping).
- **Value:** Low. `tasks` deliberately ignores section names; a finished child
  under an in-progress parent is a normal way to keep the tree together.
  `tmp/` has several `[x]` tasks under `## Pending`, and moving them would
  break the hierarchy.
- **Tradeoff:** **Do not add.** It would impose a convention the parser
  explicitly does not have. If the user wants to clean up finished work, that
  is a content decision, not a formatting check.

### 3.4 Link and cross-file checks

#### L1. Broken local links (from `TASKS.md`)

- **What it finds:** an inline link or image whose target is a relative path
  that does not exist, resolved from the file's own directory. This includes
  the worklog day-to-entry links and the entry dossier links, but the check is
  generic.
- **Severity:** warning.
- **Complexity:** Medium. This is the main parser work:
  - scan inline links and images, not only bullets;
  - support `<...>` destinations (the form bureau writes, and the only sane
    form for paths with spaces);
  - strip a `#fragment` before checking existence, and decide what to do with
    `?query`;
  - ignore URL schemes (`http:`, `https:`, `mailto:`, `ftp:`, `data:`), and
    anchor-only links;
  - ignore fenced code blocks; ideally inline code spans too;
  - handle percent-encoded paths (`%20`) or say that v1 does not;
  - resolve relative to the containing file, not the repository root;
  - report a line number per link.
- **Value:** High. Links are part of the record (`AGENTS.md`: "File names,
  relative links, headings and commit titles are part of the record"), and a
  broken link is the classic thing a user does not notice until much later.
  `tmp/` has none, but that is the normal state of a healthy repository.
- **Tradeoff:** Worth v1, but scope it: local inline links and images only, no
  external URLs, no reference-style links, no anchor-existence checks. Those
  are L7 and later. A full CommonMark parser is not worth a dependency; a small
  scanner over the notes dialect is enough.
- **Fixable later:** No. Doctor can say "target missing", but only a human can
  decide whether the target was renamed, deleted, or the link was always wrong.

#### L2. Malformed link destinations

- **What it finds:** a destination containing spaces without `<...>`, a
  backslash path, an absolute filesystem path, or a link to a directory rather
  than a file.
- **Severity:** warning.
- **Complexity:** Medium. The parser has to distinguish "space is part of the
  target" from "the closing parenthesis is the end"; broken syntax is exactly
  where a simple scanner is weakest.
- **Value:** Medium. The first case can silently truncate a link; the others
  are portability issues.
- **Tradeoff:** Defer. L1 already catches the failure that matters (target does
  not resolve), and malformed-syntax diagnostics are easy to get wrong. A
  link with spaces and no angle brackets is worth a warning if it is cheap, but
  do not let it delay L1. Tier 2/3.

#### L3. Worklog day heading has no entry link

- **What it finds:** a `### 2026-09-19` heading in `## Worklog` instead of
  `### [2026-09-19](<../entries/2026-09-19.md>)`.
- **Severity:** warning.
- **Complexity:** Low. `worklog::worklog_days` already accepts both forms; the
  check is whether the raw heading is bare, so it needs the heading line from
  the scanner. The fix is the link `append_message` already knows how to add.
- **Value:** Medium. The bare form works today and is upgraded the next time
  the day is logged, so it is not an error. But it is a missing link in the
  record, and a reader browsing the dossier cannot click through.
- **Tradeoff:** Worth including with L4. Keep it a warning; the code
  deliberately supports the bare form for old dossiers.

#### L4. Worklog day heading points at the wrong entry

- **What it finds:** a day heading whose label date does not match its target
  (`### [2026-09-19](<../entries/2026-09-20.md>)`), whose target is not
  `../entries/<date>.md`, or whose date does not parse / is at the wrong
  heading level. A target that simply does not exist is L1's finding, not
  L4's.
- **Severity:** warning. A wrong date quietly files work under the wrong day,
  which matters, but every command still runs.
- **Complexity:** Low. The heading parser and `worklog::heading_date` already
  do most of it; the new part is comparing label, target and path shape. Let L1
  own "the target does not exist"; L4 should not duplicate it, or the same
  broken link becomes two findings.
- **Value:** High. A wrong date quietly files work under the wrong day in
  `report`, and a wrong path breaks the link. This is one of the few checks
  that can corrupt the meaning of the record rather than just its appearance.
- **Tradeoff:** Worth v1. Only apply it inside `## Worklog`; a `###` heading
  under `## Description` is a normal subsection and must not be flagged.
- **Fixable later:** Partly. If the target entry exists, rewriting the link is
  mechanical; if not, the user has to decide whether to create the entry.

#### L5. Dossier and entry disagree about the day's work

- **What it finds:** two directions:
  1. a dossier has a worklog day for `2026-09-19`, but `entries/2026-09-19.md`
     does not list that dossier under `## Worked on Dossiers`;
  2. an entry links a dossier under `## Worked on Dossiers`, but the dossier
     has no worklog heading for that day.
- **Severity:** warning.
- **Complexity:** Medium. Doctor has to read all dossiers and entries, build an
  index of `(date, dossier)`, and match the entry's links to dossier stems.
  `worklog::worklog_days` and `worklog::linked_dossiers` already provide the
  raw data, but neither is fence-aware; the scanner should decide what is a
  real heading or link first. The new part is the index and the right message.
- **Value:** High. `tmp/` has exactly one: `entries/2026-09-19.md` never links
  `1234 - killing goblins was never an option`, even though the dossier logs
  work that day. `bureau worklog` writes both halves together, so a missing
  half is a real inconsistency, and it is invisible unless someone compares
  files.
- **Tradeoff:** Worth v1. Keep it a warning: a hand-written worklog may
  intentionally not link an entry, and an entry may link a dossier for a day
  whose work was logged elsewhere. The first direction is stronger than the
  second; if the noise is a problem, ship only direction 1. The missing
  `## Worked on Dossiers` section in `tmp/` is the same finding as this one, so
  do not also report S2 for that heading.
- **Fixable later:** Direction 1 is mechanically fixable when both files exist
  (add the link), which makes it a good candidate for a future `--fix`. Direction
  2 may need a worklog line the user has to write.

#### L6. Entry link label and target disagree

- **What it finds:** `- [Some old name](<../dossiers/current name.md>)`; the
  target stem and the label differ.
- **Severity:** warning.
- **Complexity:** Low.
- **Value:** Low. `report` uses the target stem for ordering and the file stem
  for its dossier headings; the label is mostly cosmetic. A mismatch usually
  means the dossier was renamed and the label was not, which is a `bureau
  rename` problem. (A filtered report's `Dossier:` line uses the title, but the
  label is still not authoritative.)
- **Tradeoff:** Defer until `bureau rename` exists. If it is included, only
  flag exact stem/label mismatches, and remember that sanitized names (`a::b`
  vs `a_b.md`) legitimately differ. Tier 3.

#### L7. Anchors and reference-style links

- **What it finds:** a `#heading` link whose heading does not exist, or a
  `[label][ref]` whose definition is missing.
- **Severity:** warning.
- **Complexity:** Medium to High. Anchor checking needs a heading slugifier
  that matches the user's editor/renderer (GitHub, Obsidian and Vim differ);
  reference links need a second pass over definitions and indentation.
- **Value:** Low. Notes in this project mostly link files, not headings, and
  the worklog/entry links are handled separately.
- **Tradeoff:** Defer. A wrong anchor is annoying, but a slugifier that does
  not match the user's renderer produces false positives. Tier 3.

#### L8. External URLs

- **What it finds:** a link to `https://…` that no longer resolves.
- **Severity:** warning.
- **Complexity:** High (network, timeouts, rate limits, auth, flaky CI).
- **Value:** Medium in theory, but doctor is a local notes tool and the
  report's contract already rules out network/LLM dependencies.
- **Tradeoff:** **Do not add.** If URL checking is ever wanted, it should be an
  explicit `--check-urls` opt-in or a separate command, never the default.

#### L9. Absolute or backslash paths

- **What it finds:** `[x](/home/user/notes/a.md)` or `[x](..\dossiers\a.md)`.
- **Severity:** warning.
- **Complexity:** Low.
- **Value:** Medium. Absolute paths break when the repository moves; backslash
  paths break outside Windows.
- **Tradeoff:** Fold this into L2 or leave it for later. A link that resolves
  on the current machine but not on another is exactly the kind of thing
  doctor should catch, but it is rare in notes written by the tool. Tier 3.

### 3.5 Fence and scanner checks

#### F1. Unclosed fenced code block

- **What it finds:** an opening backtick or tilde fence (three or more) with no
  matching closing fence.
- **Severity:** error.
- **Complexity:** Low to Medium, once the scanner tracks fences. The scanner is
  needed anyway: without it, L1 would report links in code examples and the
  task checks (T1–T3) would report tasks in code examples.
- **Value:** High. Everything after an unclosed fence is code to a Markdown
  reader, but `tasks` and `report` still parse headings and bullets inside it;
  the two views diverge, and the user may not see the closing half of the file.
- **Tradeoff:** Worth v1 as the parser's integrity check. It is also the
  cheapest way to make the other checks trustworthy. The scanner should handle
  backtick and tilde fences, up to three leading spaces, and a closing fence at
  least as long as the opener. Indented code blocks are a separate problem; do
  not try to solve both at once.

#### F2. Task-like bullets inside fenced code

- Already covered as T5. The scanner requirement is listed here so the
  dependency is explicit: F1 and L1 share it.

#### F3. Inline code spans (scanner exclusion, not a finding)

- **What it finds:** a link or task marker inside `` `backticks` ``. This is
  not a defect; it is content the scanner must not treat as a link or a task.
- **Severity:** none. It is an exclusion rule, not a user-facing finding.
- **Complexity:** Medium. Tracking backtick runs and matching lengths is
  fiddly but small.
- **Value:** Low to Medium. Notes can contain code snippets, but the common
  case is a fenced block, which F1 already handles.
- **Tradeoff:** For v1, ignore fenced blocks and accept that an inline
  `` `[x](y)` `` may be reported. If users hit false positives, add inline
  code tracking rather than a dependency. Tier 3.

## 4. Recommendation

### Tier 1 — v1

These are the checks I would ship first. They are the ones with concrete
evidence, high value, or a command that fails without them. Together they need
one shared line/fence scanner and one cross-file index.

| ID | Check | Severity | Complexity | Value | Why now |
|---|---|---|---|---|---|
| F1 | Unclosed fenced code block | error | Low–Medium | High | Makes every other scan trustworthy |
| N1 | Non-portable file names (incl. case collisions) | warning | Low | High | Product claims Windows + Linux |
| S1 | Missing / malformed / duplicate `## Worklog` | error (duplicate: warning) | Low | High | `bureau worklog` fails without it |
| T1 | Empty task | warning | Low | High | Shows up as phantom work in `tasks`; `tmp/` has two |
| T2 | Unrecognised task marker | warning | Low | High | Task silently invisible |
| T3 | Task bullet that is not `-` | warning | Low | High | Task silently invisible |
| L1 | Broken local link | warning | Medium | High | Links are part of the record |
| L3/L4 | Worklog day heading bare or wrong | warning | Low | High | Wrong day in `report`, broken navigation |
| L5 | Dossier ↔ entry disagreement | warning | Medium | High | `tmp/` has one; `worklog` writes both halves |

L3 (bare day heading) is part of L4's family; implement it with L4. L1 and F1
share the scanner; do them together.

Expected v1 output on `tmp/`: three warnings (the two empty tasks and the
missing entry link), no errors. That is a good first-run experience: every
finding is actionable. The two empty tasks are the template placeholders the
warning exists to nudge, and the third is a real missing link.

### Tier 2 — cheap and useful, but decide

| ID | Check | Complexity | Value | Why it can wait |
|---|---|---|---|---|
| N2 | Files bureau will not use | Low | Medium | Real but rare; scratch files may be intentional |
| N3 | Encoding / line endings | Low | Medium | Windows value is real; CRLF is valid Markdown, so warning only |
| S2 | Missing other template sections | Low | Medium | Useful nudge, but noisy for hand-written notes; `--strict` candidate |
| T4 | Empty plain bullets | Low | Medium | Ten more warnings in `tmp/` with little new information |
| T5/F2 | Task-like bullets in fences | Medium | Medium | Latent; needs the scanner first |
| L2 | Malformed link destinations | Medium | Medium | L1 catches the failure that matters |
| L6 | Entry label/target mismatch | Low | Low | Mostly cosmetic until `bureau rename` |
| N4 | Title/filename mismatch | Low | Low–Medium | Only cheap after N1; otherwise defer |
| N5 | Creation date missing/invalid | Low | Low | No command reads it |

### Tier 3 — leave for later

| ID | Check | Complexity | Value | Why later |
|---|---|---|---|---|
| T6 | Indentation a Markdown reader may not nest | Medium | Medium | The one Markdown-rendering issue in `tmp/`, but it contradicts the parser's deliberate "any deeper nests" rule. Decide first. |
| L7 | Anchors and reference links | Medium–High | Low | Slugifiers differ by editor; false positives are likely. |
| L9 | Absolute/backslash paths | Low | Medium | Fold into L2 if it is ever added. |
| F3 | Inline code spans (scanner exclusion) | Medium | Low–Medium | Ignore fenced blocks; accept the inline-code false positive until it bites. |
| S3 | Duplicate known sections (other than Worklog) | Low | Low | The Worklog duplicate is part of S1; the rest is tidiness. |
| — | Entry/worklog prose, empty worklog day blocks | Low | Low | The report drops prose by design; do not warn yet. |
| — | `--format json`, `--strict`, `--filter`, `--fix` switches | Low (`--fix`: High) | High eventually | Useful, but none is needed for the first diagnostic. |

### Not recommended at all

| ID | Item | Complexity | Value | Why not |
|---|---|---|---|---|
| L8 | External URL checking | High | Medium | Network, timeouts, auth, flakiness; if ever wanted, an explicit opt-in or separate command. |
| — | Full CommonMark validation / Markdown parser dependency | High | Low | The notes dialect is deliberately looser than CommonMark; a dependency would need justification. A small scanner is enough. |
| — | Prose, spelling and grammar | High | Low | Not formatting in the sense that matters here. |
| — | Task statistics, stale dossiers, ready-to-seal dossiers | Low–Medium | High for `tasks` | Belongs to `bureau tasks` per `TASKS.md`; doctor should not grow into a second task command. |
| — | Git state, uncommitted changes, commit behaviour | Medium | Medium | A separate task. |
| S4 | Empty sections | Low | Low | Normal notes; noise. |
| S5 | Section order and heading hierarchy | Low | Low | Normal notes; noise. |
| T7 | Marker spacing variants | Low | Low | The parser accepts them on purpose. |
| T8 | Section-state mismatch | Low–Medium | Low | The parser ignores sections deliberately; nested finished tasks are legitimate. |
| — | `## Worked on Tasks` as a typo | Low | None | It is a legitimate section. |
| — | Spaces in file names, non-ASCII names | Low | Negative | Valid on both platforms; the project already tests `Förstudie`. |
| — | Auto-fixing anything in v1 | Medium–High | High eventually | Doctor is read-only. A future `--fix` must be explicit, per check, and dry-run by default. |

## 5. Open questions

These are the decisions I would make explicitly before writing code:

1. **Exit code.** Is `0` for warnings and `1` only for errors right, or should
   any finding fail? I prefer the former, with `--strict` later. This decides
   the `main`/`commands::run` shape.
2. **Output format.** `path:line: severity: message` or grouped per file? The
   former is pipeable and closer to the future `tasks --quickfix`; the latter
   is friendlier to read.
3. **Missing sections.** Only `## Worklog` (error), or the other template
   sections too (warnings, with `--strict`)? I recommend `## Worklog` as an
   error and the rest as warnings, excluding `## Worked on Dossiers` because L5
   covers the case that matters.
4. **Empty plain bullets.** Include them (noisy) or only empty tasks (quiet)?
   I would start with empty tasks only.
5. **Indentation.** Is T6 a real formatting issue worth a warning, given that
   `tasks` deliberately accepts any deeper indentation? I would defer until
   after v1.
6. **Cross-link consistency.** Ship both directions of L5, or only "dossier
   logs work, entry does not link it"? The first direction is stronger.
7. **Sealed dossiers.** Check them (my recommendation) or skip them like
   `tasks`? Skipping would hide formatting problems in the record.
8. **Warnings on stderr?** My recommendation is stdout, because findings are
   the command's output. This is a deliberate exception to the usual rule.
9. **Line-only output.** For a missing section or file-level issue, is line 1
   acceptable, or should the format allow a finding without a line?
10. **Future fixes.** Which checks should be designed with a `--fix` in mind
    from day one? My shortlist: L3/L4 (add/repair links), L5 direction 1 (add
    the entry link), N3 (normalise line endings), and maybe N1 via
    `bureau rename`. The rest need human judgement.
11. **Arguments.** No arguments (scan everything) for v1, or a `--filter`/path
    to check one dossier? I would start with no arguments: doctor is a
    repository-level check, and the filter can come later if it is missed.

## 6. Implementation sketch, if the scope is agreed

This is not a commit plan; it is the shape that keeps the code consistent with
the rest of the crate.

| File | Change |
|---|---|
| `src/doctor.rs` (new) | `Severity`, `Finding`, the shared note scanner (headings, fences, bullets, links), and one pure function per check. No filesystem, no printing. |
| `src/commands/doctor.rs` (new) | Repository root, discovery, reading, invoking checks, rendering findings, deciding the exit code. The `run` / `run_in` split used by the other commands. |
| `src/main.rs` | Use the `ExitCode` from `commands::run`; doctor findings must not go through the `Err` path. |
| `src/cli.rs` | `Command::Doctor(DoctorArgs)`. v1 likely has no arguments; `--strict` can be added later. |
| `src/commands/mod.rs` | One match arm, plus the `ExitCode` change if findings need to fail. |
| `src/commands/sources.rs` | Maybe a `read_notes` helper that returns path + contents; the current `read_markdown` reads a file only to check sealed and then drops the contents. |
| `src/dossier.rs` | A `is_portable_name` / `name_issues` predicate next to `name_to_filename`. |
| `src/tasks.rs` | Optional: record a line number on `Node`, which doctor can reuse and which also unblocks `tasks --quickfix`. |
| `src/worklog.rs` | If needed, expose `heading_date` or a small day-heading parser for L3/L4. |
| `docs/subcommands/doctor.md` (new) | The final contract after this discussion. |
| `README.md` | One usage line and a paragraph. |
| `TASKS.md` | Replace the doctor bullet's open questions with the agreed scope, or split the deferred checks into separate items. |

No new dependency is needed. `sanitise-file-name` is already there for N1;
`chrono` is already there for dates; `anyhow` is already there for IO errors.
Do not add a Markdown parser, a regex crate, a directory walker or an HTTP
client.

The scanner is the only substantial new piece. It should be a small,
line-oriented state machine:

- split on `\n`, keep the line number and strip a trailing `\r`;
- track fenced code blocks (backtick or tilde, up to three spaces of indent,
  closing fence at least as long as the opener);
- expose ATX headings (level, text, line);
- expose bullets (indent columns with tab stops of 4, list marker, rest, line);
- expose inline links and images (label, target, line), ignoring fenced blocks
  and, eventually, inline code;
- expose the raw lines for checks that need them (line endings, empty
  placeholder, indentation).

That is deliberately narrower than CommonMark. It is the subset the notes use
and the subset the commands understand. The existing `tasks` parser stays the
source of truth for task state; doctor can either reuse `Tree` (with a line
number added) or duplicate only the *marker* rule and test the two against the
same fixtures. `Tree` is not fence-aware, so doctor must feed it only the
bullets the scanner accepted; the two parsers must not disagree about fences.
Duplicating the whole tree logic would be a mistake: doctor and `tasks` must not
disagree about what a task is.

## 7. Tests

The usual crate rules apply: pure logic in `src/doctor.rs`, IO at the edge, and
a regression test for every finding that corresponds to a real bug or a real
sample.

- **Scanner:** fences (backtick and tilde, nested-looking content, unclosed),
  headings (with and without space, levels 1–6, indented headings), bullets
  (dash/star/plus/ordered, tabs, 1/2/3/4-space indentation), links (angle
  brackets, spaces, fragments, external schemes, images).
- **Each check:** one positive and one negative fixture. Negative cases matter
  more here: a doctor that cries wolf is worse than one that misses a rare
  issue. Include `## Worked on Tasks` as a legitimate section, `###` under
  `## Description` as a legitimate subsection, and a dossier named with spaces
  as a legitimate file.
- **Sample regression:** a fixture matching the `tmp/` findings — two empty
  tasks, one missing entry link, and (if T6 ships) one 1-space indentation
  block — so the concrete cases that motivated the report stay covered.
- **Command:** a scratch repository with dossiers and entries; output, order,
  summary, exit code with and without errors; no writes and no commits; a
  missing `dossiers/` or `entries/` directory is not an error; sealed dossiers
  are still checked.
- **Determinism:** two runs on the same tree are byte-identical; findings sort
  by path/line/check.
- **No network:** no test should need one; there is no URL check.

The `Scratch` helper already does what a command test needs. If doctor gets a
line-number field on `tasks::Node`, a test should pin that the line points at
the task's own line, not at a continuation.

## 8. The one-paragraph recommendation

If the goal is the smallest doctor that is genuinely useful, build Tier 1: a
fence-aware scanner, the name check, the `## Worklog` check, the three task
syntax checks, broken local links, worklog day links, and the dossier/entry
cross-link check. Leave missing non-required sections, empty plain bullets,
line endings and title/creation-date checks for a second pass. Do not add
external URL checks, a Markdown parser dependency, section-order rules or task
state semantics. Keep every check read-only; any fix is a later, explicit
`--fix` feature, and the output should be plain, deterministic and pipeable.
