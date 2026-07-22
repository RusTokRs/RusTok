# Implementation Plan for `rustok-cli-registry`

## Current state

`rustok-cli-registry` owns selected-distribution provider aggregation outside
the runner and server. Generated source currently composes the platform
provider and `rustok-media-cli`; the latter exposes the owner-local
`media reconcile` workflow through `RuntimeComposition`. The generator checks
manifest selection and required registry dependencies.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- The registry selects provider adapters. It must not own terminal parsing,
  server runtime, or domain command logic.

## Open results

1. **Register the next approved module-local or platform operations provider.**
   Done when `[provides.cli]`, generated source, and registry dependencies add
   an owner adapter without introducing a runner/server/domain dependency leak.
   **Depends on:** an approved workflow owner and adapter crate.
   **Verification:** `node scripts/generate/generate-cli-registry.mjs --check`
   and `cargo test -p rustok-cli-registry --quiet`.
2. **Collect runtime evidence for the selected media reconciliation command.** Done
   when a database-backed run proves settings parsing, bounded cleanup, typed
   failure output, and structured outcome data through the generated registry.
   **Depends on:** an approved non-production runtime environment and media
   storage configuration. **Verification:** targeted provider integration test
   plus `rustok-cli media reconcile --limit <n>` in that environment.

## Verification

- `node scripts/generate/generate-cli-registry.mjs --check`
- `cargo test -p rustok-cli-registry --quiet`
- `node scripts/verify/verify-api-surface-contract.mjs`

## Change rules

1. Keep provider implementation in the owner-local adapter crate.
2. Regenerate `src/generated.rs`; never hand-edit selected provider wiring.
3. Keep production HTTP builds independent from CLI provider aggregation.
