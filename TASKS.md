# Tasks

## Pending features

- [ ] `bureau seal <dossier>` — mark a dossier as sealed via a `sealed: <YYYY-MM-DD>`
      frontmatter field. Sealed dossiers should be excluded from `worklog` selection
      by default, with an `--all` flag to include them.

## Current tasks

- [ ] Implement `bureau tasks` as specified in `docs/subcommands/tasks.md`:
      read-only listing of task trees from dossiers and entries, `PENDING` /
      `BLOCKED` / `FINISHED` sections, `--all`, `--filter [<pattern>]`,
      `--menu`. New `src/tasks.rs` (pure) and `src/commands/tasks.rs` (IO),
      with the dossier/entry discovery shared out of `commands/worklog.rs`.
- [ ] When `bureau seal` lands, `bureau tasks` must warn about dossiers whose
      tasks are all finished, since those are the ones that can be sealed.
      Until then, `--all` lists them with no hint at all.
- [ ] `bureau tasks` statistics: count pending / blocked / finished tasks per
      dossier and per entry, so a dossier can read as "18 of 20 done" instead
      of only listing what is left. Keep it out of v1: print tasks, no numbers.
      The tree already keeps a marker on every node, so this is a counting pass
      over data the parser has anyway.

