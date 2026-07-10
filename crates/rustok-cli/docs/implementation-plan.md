# Implementation Plan for `rustok-cli`

## Current state

`rustok-cli` is the terminal-facing runner outside the production HTTP server.
It aggregates selected providers from `rustok-cli-registry`, normalizes command
arguments into `CommandRequest`, rejects duplicate namespace/name registrations,
and dispatches commands asynchronously. `run_with_environment` already creates
`RuntimeComposition` from settings and an optional database environment.

The built-in `core version` command proves the typed path, but no real
module-local maintenance provider is registered yet.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- The runner owns parsing, listing, output, exit codes, and runtime bootstrap.
  Domain modules own command implementations in their own `cli/` adapters;
  `apps/server` remains a runtime composition root, not a command dump.

## Open results

1. **Register the first real module-local CLI provider.** Done when generated
   registry metadata wires an owning module `cli/` adapter through
   `[provides.cli]` and the provider receives `RuntimeComposition` without a
   server or domain dependency leak.
   **Depends on:** an approved module command and its owner adapter.
   **Verification:** `node scripts/generate/generate-cli-registry.mjs --check`
   and targeted provider/runner tests.
2. **Move the first server task, seed, or migration into that typed command.**
   Done when the former workflow is invoked through the provider contract with
   structured output and correct exit behavior rather than a server-local task
   entrypoint.
   **Depends on:** priority 1 and the selected workflow owner.
   **Verification:** `cargo test -p rustok-cli --quiet` and a focused command
   execution test.

## Verification

- `cargo test -p rustok-cli --quiet`
- `cargo run -p rustok-cli --quiet -- list --json`
- `node scripts/generate/generate-cli-registry.mjs --check`
- `node scripts/verify/verify-api-surface-contract.mjs`

## Change rules

1. Keep terminal UX and runtime construction in `rustok-cli`.
2. Keep provider contracts in `rustok-cli-core` and selected wiring in
   `rustok-cli-registry`.
3. Keep module commands in owner-local adapters and update their documentation
   with a changed command contract.
