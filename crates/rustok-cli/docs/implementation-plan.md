# Implementation plan for `rustok-cli`

## Status

`in_progress`

## Current State

- User-facing `rustok-cli` binary exists outside the production HTTP server.
- The runner consumes command contracts from `rustok-cli-core`.
- Selected distribution providers come from `rustok-cli-registry`.
- `rustok-cli list`, `rustok-cli list --json` and namespace-scoped discovery are implemented.
- `rustok-cli <namespace> <command>` dispatches typed `CommandRequest` values to
  selected providers.
- Command arguments are normalized into JSON `options` and `positionals` before
  provider execution.
- `rustok-cli core version` proves the typed provider execution path without
  server or domain coupling.
- The runner rejects duplicate `namespace/name` registrations before execution.

## Target State

- Runtime/server maintenance tasks, seeds and migrations move out of legacy task execution into typed commands.
- Distribution build/check/generate commands use the same provider model.
- Module-specific command implementations stay in module-local `cli/` adapter packages or external integration packages.
- The runner remains terminal-facing only: argument parsing, help/list output, exit codes and runtime construction.

## Next Steps

1. Connect the generated selected-distribution registry to real module-local provider crates.
2. Move the first server task or seed into a typed command.
3. Reuse normalized argument input when moving the first server task or seed into
   a typed provider command.

## Verification

- `cargo test -p rustok-cli --quiet`
- `cargo run -p rustok-cli --quiet -- list --json`
- `node scripts/generate/generate-cli-registry.mjs --check`
- `node scripts/verify/verify-api-surface-contract.mjs`
