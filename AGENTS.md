# Working on bureau

bureau maintains markdown notes in a git repository as a single synchronous
binary, with everything it needs shipped inside it.

## Validate every change

    cargo fmt --check
    cargo check
    cargo clippy --all-targets
    cargo test

Clippy is a gate here, not advice. The denied lints are in `Cargo.toml`, with
`clippy.toml` relaxing them inside tests: don't weaken the config to fit the
code, and don't `#[allow]` your way around it. Two consequences look strange
until you know why: index arithmetic goes through `saturating_*`/`checked_*`, and
string and slice work goes through `.get()`, `strip_prefix` and `split_inclusive`
rather than indexing.

If you cannot run these commands, say so instead of implying the change was
verified.

## Dependencies

- Ask before adding one, dev-dependencies included.
- Always `default-features = false`, enabling only the features that are used.
- The fewest that do the job: `std` is free, and `git` is already a runtime
  requirement, so shelling out to it beats a crate.
- No async runtime. There is nothing to await.

## Tests

- Every regression gets a test, unless writing one needs a large refactor or a
  new dependency — then say what it would cost and let the user decide.
- A regression test must fail against the code from before the fix. If you can't
  show that, say so.
- Prefer pure functions over strings so behaviour is testable without the
  filesystem. Keep IO at the edges, and inject it when a flow needs testing.
- If a rule can be enforced by a test or a lint, do that instead of writing it
  down here.

## Design

- Simplicity beats an interesting architecture. Prefer the straight line; reach
  for an abstraction once duplication has caused a bug, not in anticipation.
- User-visible output is an interface: messages, file names, headings and link
  formats are covered by tests. Changing one is a decision to report, never a
  side effect.
- Notes files belong to the user. Edits are additive and confined to the part
  being edited, and a failure after content is on disk is a warning rather than
  an error: never lose what the user typed.
- Windows and Linux are both supported; no platform-specific behaviour without a
  gate.
- Ideas for later go in `TASKS.md`, not into half-built code.

## Working with the user

- When a request or a spec contradicts what the code does, stop and ask which is
  canonical. Don't silently pick one.
- Report deliberate deviations from what was asked, where you made them.
- Push back: a simpler option, a plan not worth its cost, or a lint whose
  suggestion is worse than the code it flags — say so before implementing.
- Ask when something is genuinely ambiguous rather than guessing and moving on.
