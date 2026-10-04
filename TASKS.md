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
- [ ] `bureau new dossier`: after creating a new dossier, run `bureau worklog`, add the message "created dossier", and link this new dossier to today's entry. If the latter doesn't exist, simply create it then.
      - By "run `bureau worklog`" I mean doing so without shelling out
- [ ] Reading a note in full just to check whether it is sealed: `is_sealed` only
      looks at the leading frontmatter, but `source_of` reads the whole file.
      Reading just the head would save the I/O and the allocation on large
      dossiers; low value while notes are small, so it was left alone.
- [ ] Shell completion for bash and fish
- [ ] Fix commit behavior. Whenever any `bureau` command is run, all files dependant on `bureau` (for now only entries/ and dossiers/) need to be committed. Need to discuss whether to create 1 commit per file, 1 commit per type of file (e.g. 1 commit for all daily entries, another for all dossiers). The problem to fix is that, after manually editing some files already created with `bureau new`, the changes aren't committed automatically, and then the user needs to do so manually, which is undesired.
- [ ] `bureau new (dossier|entry)`: print the name of the created file
- [ ] Maybe fix later: `bureau tasks` treats a task inside a fenced (or indented)
      code block as real work. `Tree::parse` only looks for a leading `-`, so an
      example checklist pasted into a dossier shows up in the listing. No
      current dossier has a fence, so this is latent; if it bites, track fence
      state and skip what is inside. The parser is deliberately looser than
      CommonMark, so this is a judgment call rather than a clear bug.
- [ ] `bureau tasks --quickfix`: emit locations an editor can consume
      (`path:line: text`, or whatever Neovim's quickfix/telescope wants), so the
      listing jumps straight to a task instead of being grepped for. `Node`
      carries no line number today; recording one during `parse` is small, and
      choosing the output shape is the work. General features come first, so
      this is not a priority.

- [ ] `bureau report` diff size: a dossier created in the period diffs as its
      whole file, so `## Git diff` can dwarf the daily work that is the
      report's point. Consider a per-dossier `--numstat` line and a
      `--diff=full|stat|none` switch, or `--no-diff` plus `--stat`.
- [ ] `bureau report --brief`: one line per day, for a standup or a CV, with the
      full report left as the appendix. Also `--group dossier`, a dossier-major
      view that matches the diff half's order.

## Current features
