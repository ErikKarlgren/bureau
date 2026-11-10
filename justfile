# `just ci` is the portable gate: what the Windows CI job runs, and what
# anyone without the extra linters installed can run. `just ci-full` adds the
# Lua, spell and workflow checks the Linux CI job runs.
#
# The tool versions are pinned by `rust-toolchain.toml` (Rust), and by the
# install lines in `.github/workflows/ci.yml` and the container/devshell, if
# one is added later.

# The portable gate: Rust format, lint and tests.
ci: fmt lint test

# The full gate: `ci` plus the Lua, spelling and workflow checks.
ci-full: ci lua spell workflows

# Check Rust formatting.
fmt:
    cargo fmt --check

# Lint Rust, denying compiler warnings as well as the denied clippy groups.
lint:
    cargo clippy --all-targets --locked -- -D warnings

# Run the test suite.
test:
    cargo test --locked

# Check the embedded Neovim template's Lua formatting.
lua:
    stylua --check templates/nvim.lua

# Spell-check comments, strings and documentation.
spell:
    typos

# Check the GitHub Actions workflows for correctness and security.
workflows:
    actionlint
    zizmor .
