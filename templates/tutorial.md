# 0000 - welcome to bureau
- Creation date: $CREATION_DATE

## Description
bureau keeps work dossiers and daily entries as plain Markdown in a Git
repository. The files are the interface: open them in your editor, link them
with relative paths, and let Git keep the history. `bureau` only creates,
records, lists and reports; it does not browse or edit for you.

A dossier is one file for one piece of work. A daily entry is one file for one
day, and it links the dossiers that day touched. `bureau worklog` writes both
sides of that link.

## Notes
- The Markdown files are the primary interface. Use your editor to move
  between them.
- Git is the ledger. The commands commit the notes they write, and every file
  name, heading and commit title is part of the record.
- `bureau tasks` lists outstanding work, `bureau report` prints a period of
  work, and `bureau doctor` checks the notes for formatting problems.
- Relative links keep a dossier and its entries connected without a database.
- The task markers are:

| Marker  | Meaning             |
| ------- | ------------------- |
| `- [ ]` | todo, not started   |
| `- [.]` | work in progress    |
| `- [o]` | almost finished     |
| `- [?]` | waiting on somebody |
| `- [x]` | done                |
| `- [-]` | won't do            |

## Commands
```bash
bureau new dossier "My first task"
bureau new entry
bureau worklog welcome
bureau tasks
bureau report
bureau doctor
```

## Pending
- [ ] Read this dossier from top to bottom.
- [ ] Create your first dossier with `bureau new dossier "My first task"`.
- [ ] Log a line to this dossier with `bureau worklog welcome`, or choose another with `bureau worklog --menu`.
- [ ] List the outstanding work with `bureau tasks`.
- [ ] Read the day's record with `bureau report`.
- [ ] Check the notes with `bureau doctor`.
- [ ] Mark these tasks `[x]` as you go, or delete this dossier when you are done.
- If you use Neovim and want automatic folds:
    - [ ] Enable `exrc` so Neovim reads `.nvim.lua`: add `vim.opt.exrc = true` to your `init.lua`, then open Neovim in this repository and trust the file.

## Worklog
