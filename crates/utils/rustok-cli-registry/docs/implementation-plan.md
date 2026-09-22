# Implementation Plan for `rustok-cli-registry`

## Current state

`rustok-cli-registry` owns selected-distribution provider aggregation outside
the runner and server. Generated source composes the selected platform,
installer, auth, Media, Profiles, RBAC, Social Graph, and module-authoring
providers. The generator checks manifest selection and required registry
dependencies.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- The registry selects provider adapters. It must not own terminal parsing,
  server runtime, or domain command logic.

## Open results

1. **Complete the selected module authoring provider.** Keep init/validate
   registered while test/build/package/inspect/publish are added through the
   same owner adapter without introducing runner/server policy.
   **Verification:** `node scripts/generate/generate-cli-registry.mjs --check`
   and `node scripts/verify/verify-module-authoring-cli.mjs`.
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
