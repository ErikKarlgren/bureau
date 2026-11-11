# README skeleton

A working document for rewriting `README.md`. It is an outline, not the README:
each section says what it has to do and which facts are true today, and leaves
the prose to the repo owner.

Two rules for the rewrite:

- The README is the pitch and the tutorial. The deep reference already lives in
  `docs/subcommands/*.md`; link to it instead of duplicating it.
- Nothing that is not true today goes in the README, not even as "coming soon".
  Planned work belongs in `TASKS.md` or in a clearly-labelled Roadmap section.

Target: eight sections, about 160–200 lines.

## Why this shape

Four empirical studies inform the structure:

- **Prana et al. (EMSE 2019)** — 393 READMEs, 4,226 sections
  ([paper](https://arxiv.org/abs/1802.06997)): median **7 sections** per README,
  half between 5 and 12. Files containing What 97%, How 88.5%, References
  60.8%, Who 52.9%, Contribution 27.8%, Why 25.7%, When 21.4%. How alone is
  58.4% of all sections.
- **Liu, Noei & Lyons (Information and Software Technology 2022)** — 14,901 Java repos
  ([PDF](http://individual.utoronto.ca/klyons/files/2022-ReadMe.pdf)): most
  READMEs ignore GitHub's guidelines, but guideline-aligned ones get more
  stars; READMEs with project name, usage, installation, license, code snippets
  or image links get more stars; the most common structures belong to less
  popular repos.
- **Wang, Wang & Chen (JSS preprint 2023)** — 5,000 repos
  ([PDF](http://petertsehsun.github.io/papers/jss_2023_readme.pdf)): after
  controlling for topics and license, the **number of lists** and the
  **frequency of updates** are the most important discriminators; popular repos
  update references, unpopular ones update how-to-use text.
- **Venigalla & Chimalakonda (arXiv 2022)** — 1,950 READMEs
  ([paper](https://arxiv.org/abs/2206.10772)): popular READMEs are organized
  with lists and images and link outward; contribution guidelines and
  references correlate with popularity.

The convergent picture: name + what it is + install/usage, bullet lists,
images, outward links, license, and a development/contributing section. All of
it is correlational (stars are a proxy), and the samples are libraries and
applications rather than CLIs, so treat this as a shape, not a promise.

What it means here: eight sections instead of seventeen; What and How dominant;
the report excerpt first, the dossier second; a GIF last to produce but first
to display once it exists; `docs/subcommands` as the References; license and
development visible.

## The pitch the README must convey

The differentiator is **tracing**: a traceable record of long, messy work. One
dossier per investigation, one entry per day, every command writes and commits,
and `bureau report` makes the trail visible.

The second half is **deliberately manual**: bureau writes the scaffolding and
never the notes. It creates the file, files the worklog line under the right
day, links both sides, commits what it wrote and assembles the report — but
every description, note, task and worklog line is yours. There is no
`bureau note`; to write a note you open the file. Writing is how you decide what
deserves attention.

The sales points, in order of importance:

1. **Tracing is the point.** File names, headings and commit titles are stable
   parts of the record; `git log -p` shows how a thought evolved, and the
   repository becomes a durable memory of the work.
2. **`bureau report` is the flagship.** One plain-text document for a period:
   each day's notes, that day's dossier worklogs, then a `git diff` per dossier.
3. **Investigation-first.** A dossier is an investigation folder: multi-week
   debugging, incidents, migrations, legacy archaeology, customer-specific
   issues. Tasks are the checklist inside it, not the product.
4. **Manual by design.** No auto-summary, no generated worklog, no inferred
   task list. The writing is the thinking.
5. **The notes outlive bureau.** Plain Markdown, relative links, a git repo;
   delete the binary and everything still reads.
6. **Notes and git are the source of truth.** No database, no app state, no
   sync service.
7. **No editor lock-in.** Any editor that edits Markdown; plugins optional.
8. **The bookkeeping is automatic.** Worklog links both ways and commits; there
   is no index or dashboard to maintain.
9. **Local-first and quiet.** No network, no account, no telemetry; where the
   repository is pushed is your decision.
10. **Made to be read by other tools.** Plain stdout, warnings on stderr, colour
    never the only signal.
11. **Nested tasks with real states.** In progress, almost finished, waiting,
    cancelled, plus subtasks.
12. **Small and fast.** Five direct dependencies, 68 crates, a 2.2 MB binary.
13. **Linux and Windows**, both tested in CI.

**The division of labour** (belongs in the README, section 2 or 5; do not put
it and the pain table in the same section):

| bureau does | you do |
|---|---|
| creates the right file in the right directory | decide what deserves a dossier |
| asks for one worklog line and files it under the right day | write the line |
| links the entry and the dossier both ways | decide which dossier the work belongs to |
| commits what it wrote, with a consistent title | commit your own edits |
| parses dates, lists tasks, assembles the report | name tasks, set priorities, judge what is stale |
| checks links, names and formatting (`doctor`) | read, think, decide |

Tagline candidates (pick one, then reuse it as the GitHub "About" line):

- `Work notes that keep the trail.`
- `A traceable record of long work, in plain Markdown and git.`
- `Keep the trail: notes, tasks and history in git.`
- `Trace your work, not your time.`
- `Write the notes. bureau keeps the trail.`
- `The notes are yours. The bookkeeping is bureau's.`
- `A dossier for every investigation.`
- `Investigation notes that keep the trail.`

The H1 and the repository description both currently say "Legacy Systems", which
under-scopes the tool. Whatever is chosen, they change together.

## Visuals: three assets, three jobs

They are not substitutes for each other.

| asset | what it proves | first cost | regeneration | risks |
|---|---|---|---|---|
| report excerpt (text) | what you get out | an hour: demo script + trim | free, from the script | a wall of text and diff; must be trimmed |
| dossier excerpt (text) | what you write | the same script | free, from the script | long; trim to a month at most |
| GIF | how it feels to use | VHS script, fonts, timing, a stable demo | ~10 minutes once the script is stable | unreadable if small, heavy, not copyable, no automatic drift check |

**Order to produce:** report, dossier, GIF. The text samples fall out of the
demo script; the GIF needs that script plus VHS, ttyd and ffmpeg, and a flow
that will not change next week. "Cheap to regenerate" is true only once the
expensive part — the script — exists.

**Order to place:** the GIF, once it exists, is the strongest thing above the
fold, so it goes first. The report excerpt is the first *static* visual; the
dossier excerpt belongs in "How it works". Without a GIF, the report excerpt is
the first thing a reader sees.

**What the GIF can and cannot do:** it can show the whole loop in one take —
open a dossier, log a line, run the report — which no static sample can. It
cannot replace the report excerpt: terminal text in a GIF is not readable at
GitHub width, and cannot be copied, searched or fed to an LLM. Use the GIF to
show that the loop happens; use the text to show what it produces. Keep it
10–15 seconds, large font, small terminal, under ~10 MB, with alt text.

## Decided

| Decision | Outcome |
|---|---|
| Installation | Tag → binaries for Linux and Windows, checksums, and `sh` + PowerShell install scripts linked from the README. Tracked in `TASKS.md` under `release`. |
| Lead audience | The long-session debugger and the legacy-system maintainer. Consultants are a secondary "also works for". |
| Editor claim | "Any editor that edits Markdown." Neovim and VS Code get concrete examples; Obsidian and Joplin are not claimed. |
| Disclosure | 0rwx: code `AI:X` with the guardrails spelled out; README `AI:R` (owner writes the prose, AI reviews and fact-checks). |

## 1. What it is (≈10 lines)

Absorbs: the old title, tagline and one-paragraph section.

**Must include**

- The H1, the chosen tagline, and one paragraph saying what bureau is and what
  makes it different.
- What it is not: not a task-tracker service, not a notes app, not a TUI.
- Optionally, one sentence on the name: a bureau is a chest of drawers full of
  dossiers, which is also the logo idea.
- Badges: CI status
  (`https://github.com/ErikKarlgren/bureau/actions/workflows/ci.yml/badge.svg`),
  license, and optionally the 0rwx `AI:X` badge.

**Facts**

- Runtime: `git`, on Linux or Windows. No database, no daemon, no account.
- Five direct dependencies, 68 crates in `Cargo.lock`.
- `report` and `tasks` carry the tracing story.

## 2. Why bureau, and who it is for (≈30 lines)

Absorbs: the old problem, audience, differences and design-principles sections,
plus the origin story.

**Where it came from** — four sentences at most, not a history

- One growing Markdown file: simple and local, but debugging notes, pending
  work and daily notes shared a file that became hard to navigate.
- Logseq: one page per topic was better, but the same information had to be
  written twice (daily journal and project page), and the history lived in the
  application rather than in git.
- bureau: one file per investigation, one file per day, a two-way worklog, and
  git as the timeline.

**Pain → design answer** — this table is the point of the origin; the story is
only the wrapper. If both tables feel heavy, keep this one here and move the
division of labour to section 5.

| pain | design answer |
|---|---|
| one file grew unwieldy | one dossier per investigation |
| notes, pending work and days in one place | entries separate from dossiers |
| writing the same thing twice | the worklog writes both sides |
| history trapped in an application | git is the timeline |
| the tool heavier than the editor | plain files and a small CLI |
| app-owned notes | Markdown files you own |
| bookkeeping friction | commands link and commit for you |
| writing is the thinking | bureau never writes the notes |

**Must include**

- The manual stance in a short paragraph (from the pitch above) and the
  division-of-labour table.
- **For**: multi-week debugging and incident investigations; migrations and
  legacy-system archaeology; customer-specific issues; work that is a tree of
  tasks; also consultants, because the report shows which days went where.
- **Not for**: teams (no server, sync or assignment); billing-grade timesheets
  (days, not hours); people who want a TUI; anyone who wants the tool or an LLM
  to write the notes.
- One short "how it differs" paragraph: traceability, ownership, relative links,
  reporting from the notes rather than a second system.

**Keep it short.** Why content appears in only 25.7% of READMEs (Prana), so it
differentiates — as long as it stays a page, not a memoir. If the history grows,
move it to `docs/design-notes.md` and keep four sentences here.

## 3. Install (≈15 lines)

Absorbs: the old requirements and installation sections.

**Must include**

- The one-line install once the release exists: download the binary or run the
  script; state where it installs and that it verifies a checksum.
- The PowerShell equivalent for Windows.
- The source fallback: `cargo install --locked --path .` (verified; `--frozen`
  fails on a fresh machine because it implies offline mode).
- One requirements line: `git`, Linux or Windows. Say macOS is untested; CI runs
  Ubuntu and Windows only.

**Blocked on** the release workflow and scripts in `TASKS.md` (`release`).

## 4. Quick start (≈25 lines)

Absorbs: the old quick-start section.

A real transcript, run and pasted verbatim:

```
bureau init
bureau new dossier Fix the billing export
bureau new entry
bureau worklog billing
bureau tasks
bureau report
bureau doctor
```

**Facts about `bureau init`**

- Creates a repository if there is none, asking only for missing pieces.
- Writes `.gitattributes`, `.gitignore`, `.editorconfig`, an optional
  `.nvim.lua`, and an optional welcome dossier.
- Applies a few repository-local git settings and commits only what it created.
- Re-running it is safe; existing files are left alone.
- bureau works from any subdirectory of the repository.

## 5. How it works (≈45 lines)

Absorbs: the old two-kinds-of-file, daily-loop, tasks, one-repository and
finished-dossier material.

**Must include**

- `dossiers/<name>.md`: one file per piece of work, with sections for affected
  systems, description, notes, pending and completed tasks, and a worklog.
- `entries/<YYYY-MM-DD>.md`: one file per day, with that day's notes and links
  to the dossiers it touched.
- One dossier **after a week or a month**, not the empty template: a
  description, several worklog days linking to their entries, nested tasks with
  different markers, one finished. Generated by the demo script as
  `docs/sample-dossier.md`. Keep the excerpt short; this is the second *static*
  visual, after the report.
- The daily loop: `worklog` appends under today's heading, links the entry, and
  commits both as `worklog: <dossier>`; an entry that had to be created is
  committed first as `new entry: <date>`; commit titles are
  `<action>: <file stem>`.
- bureau commits only what it writes. Your own edits stay uncommitted until you
  commit them; `TASKS.md` records that committing them automatically is the
  intended fix.
- There is no streak to keep: an entry exists for a day that has something.
  `--date` backfills another day, in plain English or `YYYY-MM-DD`.
- Task markers: `[ ]` todo, `[.]` in progress, `[o]` almost finished, `[x]` done,
  `[-]` cancelled, `[?]` waiting; `-` is prose. Indentation nests, and a task
  prints with the parents that lead to it.
- `bureau tasks` lists actionable and blocked work; `--all` adds finished and
  cancelled dossier tasks; `--filter` and `--menu` narrow to one dossier.
- One repository is the unit: no global state, so one repo with a dossier per
  client works, and separate repos when clients must not mix.
- A finished dossier is, today, one you stop touching. `bureau seal` is planned.

## 6. Report (≈25 lines)

Absorbs: the old report section.

**Must include**

- The synopsis: `bureau report [<date>] [--from <date>] [--to <date>]
  [--no-diff] [--filter <pattern>] [--menu]`.
- A short excerpt generated by the demo script, verbatim in a fenced block:
  the `# Report from …` header, one or two days, one small diff. **This is the
  first static visual in the README.** A GIF, once it exists, may sit above it;
  if only one static sample is above the fold, it is this one, not the dossier.
- A link to the full generated report at `docs/sample-report.md`.
- The sentence that makes it useful: it reconstructs a period from the notes and
  git, so it is a trace, not a summary written afterwards.

**Facts**

- Dates accept plain English (`yesterday`, `a week ago`, `3 months ago`) or
  `YYYY-MM-DD`; ranges are inclusive and default to today.
- Output: each day's entry notes and that day's dossier worklogs, oldest first,
  then one `git diff` per dossier the period touched.
- Plain text on stdout, warnings on stderr, meant to be piped into a file or an
  LLM.
- **Limit to state**: it shows which days were spent on what. It is not a timer
  and does not total hours.

**How the sample is generated**

- The same scripted demo drives the VHS recording and writes the sample, so the
  GIF and the README excerpt show the same session.
- Pin `GIT_AUTHOR_DATE` and `GIT_COMMITTER_DATE`, backfill with `--date`, and
  report a fixed `--from`/`--to`; otherwise the README churns on every run and
  the example is always "today".
- `init`, `new dossier` and `worklog` prompt, so a headless run needs a pty
  (`script -qec`, `expect`, or VHS) or a non-interactive flag that does not
  exist yet. Test this before promising a CI-generated report.
- Split the jobs: the report needs only `bureau` and git; VHS needs `vhs`,
  `ttyd` and ffmpeg, so keep it manual or release-only until it stabilizes.
- Once stable, a CI step can regenerate and `git diff --exit-code` the sample.

## 7. Everyday use (≈15 lines)

Absorbs: the old doctor and editors sections.

**Doctor**

- Read-only; one `path:line: Severity: message` per finding on stdout; exits 1
  only on errors, warnings do not fail the run.
- Checks broken relative links, portable file names, worklogs and their link
  back, empty tasks, unknown markers, line endings, duplicate dates, case
  collisions.
- Exact rules: `docs/subcommands/doctor.md`.

**Editors**

- Any editor that edits Markdown; no plugin required.
- Neovim: the generated `.nvim.lua` gives automatic folds; Telescope or fzf
  jumps between dossiers.
- VS Code: Ctrl-P.
- Do not claim Obsidian or Joplin; they are untested.

## 8. Development, credits and license (≈20 lines)

Absorbs: the old development, status/roadmap and 0rwx sections.

**Development**

- `just ci` runs format, clippy (warnings denied) and tests; needs only cargo.
- `just ci-full` adds stylua, typos, actionlint and zizmor.
- Rust is pinned in `rust-toolchain.toml` (1.98.1, rustfmt and clippy).
- CI runs `ci-full` on Ubuntu and `ci` on Windows, both with the same pinned
  Neovim (0.12.6) for the embedded `.nvim.lua` tests.
- 317 tests (315 on Windows; two are Unix-only).

**Status**

- Version 0.1.0; expect breaking changes. Plans live in `TASKS.md` and are
  intentions, not promises: stale-work detection, sealing, renaming, statistics.

**How this was made (0rwx)**

- The code is `AI:X`: largely LLM-written, curated through the guardrails.
- The guardrails are the argument, so name them: denied clippy groups
  (`pedantic`, `nursery`, `unwrap_used`, `expect_used`, `indexing_slicing`,
  `arithmetic_side_effects`, `panic` and friends), 300+ tests, the `.nvim.lua`
  executed in real Neovim on both platforms, a pinned toolchain, CI on Ubuntu
  and Windows, `doctor` checking its own tutorial, and human review.
- The README is `AI:R`: the owner writes every sentence; the AI reads the draft
  to fact-check claims and suggest wording. That is read-only influence.
- Credit No Boilerplate twice: the clippy lint set came from their write-up, and
  0rwx is by the same author.
- MIT, with a link to `LICENSE`.

## Claims inventory

Every factual claim the README can make today, with its source. If a number is
not here, measure it before writing it down.

| Claim | Today's value | Source | Caveat |
|---|---|---|---|
| Direct dependencies | 5: anyhow, chrono, clap, dialoguer, sanitise-file-name | `Cargo.toml` | Say "few", never "zero" |
| Locked crates | 68 | `Cargo.lock` | Rises with clap/dialoguer |
| Cold release build | ~3 s on a 24-core laptop | measured | Say "seconds", not a number |
| Binary size | 2.2 MB, release build | measured | Not static; still needs git |
| Tests | 317 (315 on Windows) | `cargo test` | Two are `#[cfg(unix)]` |
| CI checks | fmt, clippy `-D warnings`, tests, stylua, typos, actionlint, zizmor | `.github/workflows/ci.yml` | Ubuntu runs `ci-full`; Windows runs `ci` |
| Runtime requirements | git; Linux or Windows | README, CI | macOS untested |
| Async runtime | none | `Cargo.toml` | Everything is synchronous |
| Commands | `init`, `new dossier`, `new entry`, `worklog`, `tasks`, `report`, `doctor` | `src/cli.rs` | `seal` and `rename` do not exist |
| Files | `dossiers/<name>.md`, `entries/<YYYY-MM-DD>.md` | templates | Custom templates planned |
| Commit titles | `<action>: <file stem>` | `src/git.rs` | `new dossier`, `new entry`, `update entry`, `worklog` |
| Doctor exit codes | 1 on errors, 0 with warnings | `src/commands/doctor.rs` | `--strict` planned |
| Date grammar | plain English plus `YYYY-MM-DD` | `src/date.rs` | Shared by report and worklog |
| Report shape | entries + worklogs per day, then diffs | `docs/subcommands/report.md` | Not a time tracker |

## Do not claim yet

- Stale-task or stale-dossier detection (planned; it is a selling point later).
- `bureau seal` or sealed dossiers as a working feature.
- `bureau rename`.
- Task statistics, shell completions, custom templates.
- Billing-grade time tracking, or hours per client.
- Prebuilt binaries or install scripts until the release workflow exists.
- crates.io installs, `cargo binstall`.
- macOS support.
- Obsidian or Joplin compatibility.
- "Zero dependencies", "static binary", "no runtime requirements" (git is one).
- A demo GIF; the VHS pipeline comes after the README.
- Team or "institutional" memory as a collaboration feature: bureau is single
  user, with no server, sync, assignment or shared board. Say "a durable memory
  of the work", not "institutional memory", unless it is clearly about a shared
  repository read by people.
- Incident management: there are no incident timelines, postmortems or on-call
  integrations. Investigations are the audience, not a feature set.

## Open questions

1. The tagline: candidates are in the pitch; decide when the narrative is
   drafted.
2. How much of the generated report to embed, and whether its drift check gates
   PRs or only the demo job.
3. Whether the first visual is the GIF (once it exists), a report excerpt as a
   teaser, or neither; otherwise the report sits in section 6.
4. Whether a logo is worth making at all (see below).

## To do in this branch, around the README

- [ ] Add the release workflow (tag → Linux and Windows binaries + checksums).
      Tracked in `TASKS.md`.
- [ ] Add the `sh` and PowerShell install scripts once assets exist. Tracked in
      `TASKS.md`.
- [ ] Add `description` and `repository` to `Cargo.toml`; `cargo package` warns
      about both today.
- [ ] Update the GitHub "About" line with the chosen tagline, in the same change
      as the H1.
- [ ] Write the demo script that generates the samples, in this order: the
      report first (`docs/sample-report.md`), the filled dossier second
      (`docs/sample-dossier.md`), the VHS recording last. Produce the GIF last,
      but place it first once it exists.
- [ ] Add the report excerpt and the linked dossier example to the README, in
      that order.
- [ ] Optional: a hand-made wordmark or drawer mark for the README header and
      the GitHub social preview. The binary never displays it, so it is branding
      for the repository, not the tool — low priority.
