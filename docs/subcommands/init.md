# `bureau init`

## Purpose

Set up a repository for bureau. It creates a Git repository if the current
directory is not already inside one, writes the repository files bureau
recommends, and applies the local Git settings that make a text-only notes
repository behave consistently.

`bureau init` is interactive and takes no arguments. It asks before changing
anything outside the repository and never overwrites a file that already
exists. Re-running it only fills in what is missing.

## Usage

```bash
bureau init
```

Run it from the directory that should hold the notes, or from any directory
inside an existing notes repository. There are no flags yet; a non-interactive
run fails with a clear message instead of choosing defaults.

## What it does

It first reports what it found: whether the current directory is already a Git
repository, the configured Git author, whether `.nvim.lua` is present, whether
the repository already has dossiers, and whether the recommended setup is
complete. It then asks only about things that are missing or optional.

1. **Initialize repository?** Only when the current directory is not inside a
   Git repository.
2. **Your name and email.** If both are already configured, the command prints
   the author and does not ask. If either is missing, it asks for both, with
   the existing value as the default.
3. **Editor configuration.** Only when `.nvim.lua` is missing. `None` is the
   default unless `$EDITOR` names `nvim` or `neovim`. The picker labels say
   that Neovim adds a repository-local `.nvim.lua` with automatic folds, and
   that `None` adds no editor-specific config.
4. **Welcome dossier.** Only when the repository has neither a tutorial
   dossier nor any other dossier. If it already has one, or already has notes,
   the command says so and skips the question.
5. **Recommended setup.** Only when at least one recommended file or local Git
   setting is missing. If everything is already in place, it says so and skips
   the question.

If the repository is already fully configured, the command prints that there
is nothing to do and exits without asking for a confirmation. Otherwise it
prints the files it will add, a short dim explanation under each file, the Git
settings it will change, and the commit it will create when files are added,
then asks for confirmation.

## Files

### `.gitattributes`

```gitattributes
* text=auto eol=lf
*.md text diff=markdown
```

The first line keeps every text file LF in the repository and in every
checkout, which is what `bureau doctor` expects. The second selects Git's
built-in Markdown diff driver, so hunk headers name the nearest heading. That
driver also changes the hunk headers `bureau report` prints; the generated
file says so. Git's diff has no simple override for a repository's
`.gitattributes`, so forcing the driver would mean extracting the two trees or
building a temporary attribute tree; the file is the source of that behavior.

### `.gitignore`

Editor swap and backup files, OS metadata, and Obsidian's per-window state.
It deliberately does **not** ignore `dossiers/`, `entries/`, `.nvim.lua` or
`.exrc`.

### `.editorconfig`

UTF-8, LF, a final newline and four-space indentation, with trailing
whitespace preserved in Markdown because two trailing spaces are a hard line
break. Neovim has built-in EditorConfig support; some other editors need an
extension or plugin.

### `.nvim.lua`

Only written when Neovim is chosen, and only when the file does not already
exist. If it exists, the command reports it and leaves it alone instead of
asking which editor to use. It contains a `foldexpr` that combines Markdown
heading levels with list indentation, applies to Markdown buffers under the
repository, and leaves
other windows' fold settings alone. Folds start open (`foldlevel=99`).

Neovim reads this file only when `exrc` is enabled and the file is trusted.
Add this to the user's `init.lua`:

```lua
vim.opt.exrc = true
```

Then open Neovim in the repository and accept the trust prompt (`:help exrc`,
`:trust`). Neovim looks for `.nvim.lua` when it starts in the repository or one
of its subdirectories; opening a note from an unrelated directory does not
load it. `bureau init` prints a reminder but does not edit the user's global
Neovim configuration.

### `dossiers/0000 - welcome to bureau.md`

An optional tutorial dossier with an open checklist. It demonstrates
`bureau tasks`, `bureau worklog`, `bureau report` and `bureau doctor`, and it
includes the Neovim `exrc` step. It passes `bureau doctor` with no findings.
The tutorial is generated from a template embedded in the binary, with the
current creation date.

## Local Git settings

These are written with `git config --local`, so they live in `.git/config`, not
in a tracked file:

| Setting | Value | Why |
|---|---|---|
| `user.name` | asked | The author of the first and later commits. |
| `user.email` | asked | The author of the first and later commits. |
| `core.quotepath` | `false` | Non-ASCII dossier names display normally. |
| `core.autocrlf` | `false` | Prevents a global Windows setting from turning notes into CRLF. |
| `diff.algorithm` | `histogram` | Matches the algorithm `bureau report` pins. |
| `merge.conflictStyle` | `zdiff3`, or `diff3` on older Git | More informative conflict markers for prose. |
| `fetch.writeCommitGraph` | `true` | Keeps history walks fast after a fetch. |

A repository-root `.gitconfig` is **not** created: Git reads `.git/config`,
not a file called `.gitconfig` in the worktree.

## Output and exit code

Progress is printed as files are created and settings are written. Warnings
go to stderr. The command exits `0` when the setup succeeded, including when
some files were already present and were left alone. It exits `1` when the
repository cannot be created, a file cannot be written, or a Git setting
cannot be applied.

If the files are written but the commit fails, the command warns and still
succeeds: the notes are on disk and can be committed manually, the same rule
`bureau new` follows.

## Tests

- `src/commands/init.rs`: fresh repository, existing files, declined prompts,
  editor choices, ignored `.nvim.lua`, subdirectory discovery, tutorial
  doctor-cleanliness, and the generated Lua loading.
- `src/git.rs`: repository discovery in normal repositories, linked worktrees,
  bare repositories and `.git`; local config read/write; forced commits.
- `src/cli.rs`: `bureau init` takes no arguments.
