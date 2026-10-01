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
      - This tool should also work well enough with Obsidian, Joplin, ... right?
- [ ] Add a logo to the readme. I'm thinking of a drawer full of dossiers, maybe showing git-related. I wonder if I should really make it with AI due to ethical concerns, even if most of the code here has been written by deepseek.
- [ ] `bureau rename`: for renaming dossiers and all relevant links, cause right now it'd need to be manually done. This would of course also commit the affected files
- [ ] `bureau doctor`: for checking formatting issues
      - Broken links
      - Anything else?
- [ ] `bureau report [<when>] [--from <date>] [--to <date>] [--no-diff] [--filter] [--menu]`:
      print a reproducible report of the work in a period, built from the notes'
      contents (worklog dates own the period), with committed git diffs as an
      appendix. Design settled in `docs/subcommands/report.md`; implementation
      still to do. Key decisions: files are the source of truth and git is
      evidence; sealed dossiers and daily entries are included; days without
      work are omitted; the period is inclusive and "no work in range" is an
      error; no `--json` for now; no LLM dependency.
- [ ] `bureau report`'s `<when>`, `--from` and `--to`, and `bureau worklog
      --date <date>` (TBD), must accept dates in plain english and share one
      grammar (`src/date.rs`, TBD). The decision is a small hand-rolled closed
      grammar; the evaluated candidate crates are in
      `docs/subcommands/report.md`.
- [ ] `bureau report`'s git diff: `git diff --follow` needs git >= 2.47 and
      exactly one path. Decide the fallback for older git before implementing
      (currently: rename detection only). Tracked in `docs/subcommands/report.md`.
- [ ] `bureau new dossier`: after creating a new dossier, run `bureau worklog`, add the message "created dossier", and link this new dossier to today's entry. If the latter doesn't exist, simply create it then.
      - By "run `bureau worklog`" I mean doing so without shelling out

## Current features
- [ ] Shell completion for bash and fish
- [ ] Fix commit behavior. Whenever any `bureau` command is run, all files dependant on `bureau` (for now only entries/ and dossiers/) need to be committed. Need to discuss whether to create 1 commit per file, 1 commit per type of file (e.g. 1 commit for all daily entries, another for all dossiers). The problem to fix is that, after manually editing some files already created with `bureau new`, the changes aren't committed automatically, and then the user needs to do so manually, which is undesired.
