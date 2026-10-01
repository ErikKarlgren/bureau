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
- [ ] `bureau report --from "a week ago" [--to "today"]`: Print a deterministic report following the structure of the example below:
      - As you can see, the idea is: 1) Add a worklog section showing the work done each day (what about days without work?) categorized per dossier; 2) Show a git diff of each dossierto show a detailed report of everything that changed. Since dossiers show a good deal of the thought process of each task and subtask, this is very valuable info.
      - The output of this report can be fed to an LLM to ask questions (as a pipe or writing to a file). For example: what have I done in this time period? What might be worth adding to my CV? What should I tell my boss during tomorrow's performance review?
      - Add a `--brief` or `--no-diff` flag to omit the git diff
      - Add a `--json` flag to make it easier to consume by LLMs
      - Don't add an LLM as a direct dependency
      - Example:
            ```markdown
            Report from "a week ago" to "today"
            # Worklog
            ## 2026-09-20

            ### Dossier A
            - Killed 3 goblins
            - Watered my plants

            ### Dossier B
            - Bought 2 burritos


            ## 2026-09-21

            ### Dossier C
            - Walked 10 steps
            - Sprang 10.000 steps

            ### Dossier A
            - Got a magical wand

            [more dates, more worklog entries for each...]



            # Git diff

            ## Dossier A
            ```diff
            diff --git dossiers/Dossier A.md dossiers/Dossier A.md
            index 714c718..dcac756 100644
            --- dossiers/Dossier A.md
            +++ dossiers/Dossier A.md
            @@ -82,8 +82,12 @@ one branch of a dossier is printed, and only what is still outstanding is
             printed: ticking a task off takes it out of the listing rather than leaving it
             there as context. `BLOCKED` lists the `[?]` tasks themselves — what you are
             waiting on — and the work a wait is holding up stays out of the listing until
            -`--all` asks for it. Tasks in a daily entry are listed under their date, before
            -the dossiers, which follow most recently modified first. `--filter [<pattern>]`
            +`--all` asks for it. Output is coloured when it goes to a terminal — blue for
            +what can be picked up, cyan once it has been started, yellow for what is waiting
            +on somebody, green for what is over, and dim for the lines that are only there
            +to give context — and plain text when it is piped or `NO_COLOR` is set. Tasks in
            +a daily entry are listed under their date, before the dossiers, which follow
            +most recently modified first. `--filter [<pattern>]`
             narrows the listing to one dossier the same way `bureau worklog` picks one,
             `--menu` opens the picker, and sealed dossiers are never listed, not even with
             `--all`. The exact rules, output format and examples live in
            ```

            ## Dossier B
            [...]
            ```
- [ ] `bureau new dossier`: after creating a new dossier, run `bureau worklog`, add the message "created dossier", and link this new dossier to today's entry. If the latter doesn't exist, simply create it then.
      - By "run `bureau worklog`" I mean doing so without shelling out

## Current features
- [ ] `bureau tasks` UI: add a space above a "dossier" header name, unless it's the first dossier in its section. This will make `tasks` easier to read in more terminals.
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
