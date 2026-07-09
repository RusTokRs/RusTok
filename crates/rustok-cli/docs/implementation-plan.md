# Implementation plan for `rustok-cli`

## Status

`in_progress`

## Current State

- User-facing `rustok-cli` binary exists outside the production HTTP server.
- The runner consumes command contracts from `rustok-cli-core`.
- Selected distribution providers come from `rustok-cli-registry`.
- `rustok-cli list`, `rustok-cli list --json` and namespace-scoped discovery are implemented.
- `rustok-cli <namespace> <command>` asynchronously dispatches typed `CommandRequest`
  values to selected providers.
- Command arguments are normalized into JSON `options` and `positionals` before
  provider execution.
- `rustok-cli core version` proves the typed provider execution path without
  server or domain coupling.
- The runner rejects duplicate `namespace/name` registrations before execution.

## Target State

- Runtime/server maintenance tasks, seeds and migrations move out of legacy task execution into typed commands.
- The runner accepts a host-neutral `RuntimeComposition`; module providers are constructed with it
  and can capture DB, settings and typed host handles without importing `apps/server`.
- The binary bootstrap loads `RUSTOK_SETTINGS_JSON` and connects `RUSTOK_DATABASE_URL` or
  `DATABASE_URL` when present; commands that do not need a database remain usable without it.
- Distribution build/check/generate commands use the same provider model.
- Module-specific command implementations stay in module-local `cli/` adapter packages or external integration packages.
- The runner remains terminal-facing only: argument parsing, help/list output, exit codes and runtime construction.

## Next Steps

1. Connect the bootstrapped `RuntimeComposition` to the first real module-local provider crate.
2. Move the first server task or seed into that typed asynchronous command.

## Verification

- `cargo test -p rustok-cli --quiet`
- `cargo run -p rustok-cli --quiet -- list --json`
- `node scripts/generate/generate-cli-registry.mjs --check`
- `node scripts/verify/verify-api-surface-contract.mjs`
