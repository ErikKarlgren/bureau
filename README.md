# bureau
Git-based Work Dossiers for Legacy Systems

## Requirements
- `git`
- Linux or Windows

## Installation
```bash
cargo install --path .
```

## Usage
Run it from inside the git repository where you keep your notes:

```bash
bureau new dossier Some dossier name
bureau new entry
bureau worklog refactor
```

Dossiers are written to `dossiers/<name>.md` and daily entries to
`entries/<YYYY-MM-DD>.md`, both at the root of that repository, and both are
committed.

`bureau worklog [<filter>] [--date YYYY-MM-DD] [--menu]` appends one line to a
dossier's `## Worklog` section and links that dossier from the same day's entry.
A filter matching several dossiers opens a picker; with no filter the most
recently modified dossier wins, or the picker opens when several share the
newest timestamp. `--date` logs against another day instead of today, which is
how you backfill; it is the only way to do so.

## Workflow
Keep a git repository for your notes and run bureau from a terminal inside it.
Your editor's integrated terminal is the intended place, since switching between
markdown files is something editors are already good at.

A dossier is one file per task. `bureau new dossier` asks for a description and
an optional link, then commits the file. A daily entry is one file per day,
created with `bureau new entry`.

While you work, `bureau worklog` records a line against a dossier. The line goes
under that day's heading in the dossier's `## Worklog` section, and the dossier
is linked from that day's entry, which is created if it does not exist yet. Both
files are then committed. The two link to each other, so a dossier shows which
days were spent on it and an entry shows which dossiers that day touched.

New files start from [`templates/dossier.md`](templates/dossier.md) and
[`templates/daily-entry.md`](templates/daily-entry.md). Those templates are
embedded in the binary at build time, so changing them takes effect after a
reinstall.

Tasks are generally added to a dossier, although they could be added to a daily
entry if they don't really belong to an existing dossier. They follow this
custom format:
```markdown
- Just some text
- [ ] todo task, not started yet
- [.] work in progress
- [o] presumably almost finished
- [x] done
- [-] won't do, cancelled
- [?] on wait
```

Tasks can be put inside other tasks in a hierarchical fashion, like this:
```markdown
- [.] Refactor billing system
    - [x] Check what files to refactor
        - Files: billing.rs, billing.html
    - Follow the guidelines doc Tom shared with you
    - [?] Decide whether to rewrite the `find_client()` function
        - Waiting for John for feedback
    - [.] Add more checks
```
`bureau tasks` will show all pending tasks you have. With `--all` it will also
show closed ones.

## License
MIT. See [LICENSE](LICENSE).
