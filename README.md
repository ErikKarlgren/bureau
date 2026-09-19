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
```

The dossier is written to `dossiers/<name>.md` at the root of that repository
and committed. `bureau new` will also create daily entries in the future.
