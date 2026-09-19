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
```

Dossiers are written to `dossiers/<name>.md` and daily entries to
`entries/<YYYY-MM-DD>.md`, both at the root of that repository, and both are
committed.

