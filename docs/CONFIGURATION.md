# Bureau Configuration

Bureau supports local repository configuration via `.bureau/config` or `.bureau.toml` in your repository root. If neither file exists, sensible defaults are used.

## Precedence

Configuration settings follow a strict hierarchy:

1. **CLI Arguments**: explicit flags passed on the command line always take highest precedence (e.g. `--type minimal` or `--no-diff`).
2. **Configuration File**: settings specified in `.bureau/config` or `.bureau.toml`.
3. **Built-in Defaults**: fallback values hardcoded in Bureau.

## Configuration File Locations

Bureau searches for local configuration in the following order:

1. `.bureau/config`
2. `.bureau.toml`

## Supported Sections and Keys

```toml
# Configuration for 'bureau report'
[report]
# Default report format: "minimal", "worklog", or "full" (default: "full")
type = "worklog"
# Omit git diff by default (default: false)
no_diff = true

# Configuration for 'bureau tasks'
[tasks]
# Always list finished tasks by default (equivalent to --all)
all = true
# Days without activity before a dossier is considered stale (default: 14)
staleness_days = 21

# Custom templates for new dossiers and daily entries
[templates]
# Path to a custom dossier template relative to the repository root
dossier = "templates/my-dossier.md"
# Path to a custom daily entry template relative to the repository root
entry = "templates/my-entry.md"
```

### `[report]` Section
- `type`: sets the default output type when `bureau report` is run without `--type`. Supported values are `minimal`, `worklog`, and `full`.
- `no_diff`: sets whether `bureau report` omits git diffs by default.

### `[tasks]` Section
- `all`: boolean flag (`true`/`false`). When `true`, `bureau tasks` includes finished tasks by default as if `--all` were passed.
- `staleness_days`: integer threshold in days for staleness calculations.

### `[templates]` Section
- `dossier`: path relative to repository root containing the template for `bureau new dossier`. Supports placeholders `$DOSSIER_NAME`, `$CREATION_DATE`, `$DESCRIPTION`, and `$DOSSIER_LINK`.
- `entry`: path relative to repository root containing the template for `bureau new entry`. Supports placeholder `$DATE`.
