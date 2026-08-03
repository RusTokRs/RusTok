# Implementation Plan for `rustok-cli`

## Current state

`rustok-cli` is the terminal-facing runner outside the production HTTP server.
It aggregates selected providers from `rustok-cli-registry`, normalizes command
arguments into `CommandRequest`, rejects duplicate namespace/name registrations,
and dispatches commands asynchronously. `run_with_environment` already creates
`RuntimeComposition` from settings and an optional database environment.

The generated registry now contains real owner-local providers, including the
complete standalone `rustok-modules-cli` authoring command set: `module init`,
`validate`, `test`, `build`, `package`, `publish`, and `inspect`.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- The runner owns parsing, listing, output, exit codes, and runtime bootstrap.
  Domain modules own command implementations in their own `cli/` adapters;
  `apps/server` remains a runtime composition root, not a command dump.

## Open results

1. **Completed: register real module-local CLI providers.** Generated registry
   metadata wires owner `cli/` adapters through `[provides.cli]`; the current
   module authoring provider exposes the complete authoring command set without
   a server, worker transport, OCI/signing client, AI, or Alloy dependency.
   **Verification:** `node scripts/generate/generate-cli-registry.mjs --check`
   and `node scripts/verify/verify-module-authoring-cli.mjs`.
2. **Move the first server task, seed, or migration into that typed command.**
   Done when the former workflow is invoked through the provider contract with
   structured output and correct exit behavior rather than a server-local task
   entrypoint.
   **Depends on:** priority 1 and the selected workflow owner.
   **Verification:** `cargo test -p rustok-cli --quiet` and a focused command
   execution test.
3. **Completed: module authoring flow parity.** Test, build, package, inspect,
   and publish now use `rustok-modules-cli`. Build and publication policy remain
   in owner controls rather than the terminal runner; author publish creates a
   review request and cannot perform admission or final approval.

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
