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
