# Tasks

## Pending features
- [ ] `bureau seal <dossier>` — mark a dossier as sealed via a `sealed: <YYYY-MM-DD>`
      frontmatter field. Sealed dossiers should be excluded from `worklog` selection
      by default, with an `--all` flag to include them.
- [ ] When `bureau seal` lands, `bureau tasks` must warn about dossiers whose
      tasks are all finished, since those are the ones that can be sealed.
      Until then, `--all` lists them with no hint at all. The check for a
      `sealed:` frontmatter field is already written and already skips those
      dossiers, so `bureau seal` only has to write the field.
- [ ] `bureau tasks` statistics: count pending / blocked / finished tasks per
      dossier and per entry, so a dossier can read as "18 of 20 done" instead
      of only listing what is left. Keep it out of v1: print tasks, no numbers.
      The tree already keeps a marker on every node, so this is a counting pass
      over data the parser has anyway.
- [ ] `bureau tasks`: show "ready-to-close" dossiers. After `bureau seal` has been implemented, `bureau tasks` must check if there are any "open" dossiers that only contain "finished" tasks (i.e. `[x]` or `[-]`). For all such dossiers, print a warning after printing all found tasks. The goal is reminding the user to clean the output of `bureau tasks --all` by sealing ready-to-close dossiers.
- [ ] `bureau tasks`: warn about "stale dossiers", which means dossiers that haven't been sealed, still have non-finished tasks, and haven't been worked on in the last X days/weeks. This should provide a lot of value in the future if the user ends up having dozens of dossiers to remind them to take care of old non-finished work. Checking how long they haven't been worked on would depend on the last date they have in the `Worklog` section and compare it to their mtime. In fact, we could maybe extend this to also warn about dossiers that don't have any worklogs and which already are X days old, all to remind the user of working on them, but I'm slightly less sure about how to implement this.
- [ ] Rework the `README.md` to make it easier to understand to other people how `bureau` is meant to be used. Mention how the repo owner uses it with neovim+telescope, and how it should also work fine with vscode (ctrl-p toquickly switch between files, etc) and other code editors.
      - Mention too how the tool has been mostly vibecoded with Deepseek (because the implementation doesn't interest me thaaaat much in comparison to other personal projects, although it does a bit), but how at the same time it's meant for manual workflows
      - The repo owner shall write the readme themselves and manually, so the LLM must simply help them brainstorm, structure the file and fix typos and wording issues
- [ ] `bureau rename`: for renaming dossiers and all relevant links, cause right now it'd need to be manually done. This would of course also commit the affected files

## Current features
- [ ] Add colors to `bureau tasks`. They must carry semantic meaning and not simply be for decoration. Need to discuss this in depth with repo owner before doing anything. Colors must be turned off when stdout isn't a shell.
- [ ] Shell completion for bash and fish
- [ ] Fix commit behavior. Whenever any `bureau` command is run, all files dependant on `bureau` (for now only entries/ and dossiers/) need to be committed. Need to discuss whether to create 1 commit per file, 1 commit per type of file (e.g. 1 commit for all daily entries, another for all dossiers). The problem to fix is that, after manually editing some files already created with `bureau new`, the changes aren't committed automatically, and then the user needs to do so manually, which is undesired.
- [ ] `bureau tasks` prints dossiers in `read_dir` order instead of newest first.
      The spec asks for "most recently modified first, file name ascending as the
      tiebreak", and `read_dossiers`/`by_recency` already do exactly that for
      `worklog`, but `render_all` walks `sources::read_markdown` straight through
      and never sorts. The order is whatever the filesystem hands back, so it can
      differ between two runs on the same repository: the same two dossiers came
      out `1234` then `9820` in one run and `9820` then `1234` in another. Only the
      unfiltered listing is affected — `--filter` and `--menu` already sort through
      `by_recency`. Sorting the dossiers the listing prints is the fix, and
      `by_recency` needs a sibling that takes `Source`s rather than paths, since the
      sealed check travels with the path. The spec's "two dossiers with one
      modification time sort by name" test is missing along with it.
