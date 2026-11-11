# bureau
![CI Badge](https://github.com/ErikKarlgren/bureau/actions/workflows/ci.yml/badge.svg)

bureau keeps a traceable record of long investigations as plain Markdown in your git repository: one dossier per task or investigation, one entry per day, and a report that reconstructs any period from your notes and the commits.

- **Tracing, not just notes:** bureau commits every file it writes. `bureau report` then prints a markdown document describing your work for the chosen time period.
- **Your writing stays yours:** bureau creates the scaffolding, links files together and makes the git commits. It never writes your notes, because deciding what's important enough to write down is up to you.
- **Built for investigation work:** Create a dossier for each long debugging session, incident or non-trivial ticket (e.g. legacy systems, migrations, low-level software). <!-- REVIEW WORDING HERE -->
- **No lock-in:** Even if you uninstall bureau, you will still be able to read your notes.
- **One source of truth:** Your notes and the Git history. No database, no sync service.
- **Small, fast and local:** 2.2 MB binary. No network, no account, no telemetry.

## Requirements
- `rustup` (only for installation and updates)
- `git`
- Linux or Windows (macOS should work, but not tested in CI)

## Installation
1. Install `rustup`, either [as the official docs say](https://rust-lang.org/tools/install/) or with your package manager.
2. Run the following command in a terminal:
    ```bash
    cargo install --locked --git https://github.com/ErikKarlgren/bureau
    ```
After a version bump, repeat step 2 every time you want to update bureau.

## License
MIT. See [LICENSE](LICENSE).

<!-- TODO: REVIEW LAST DEEPSEEK'S FEEDBACK. You've done quite better with the intro this time, but there are still some grammar and styling issues -->
