# Implementation plan for `rustok-runtime`

## Current state

`rustok-runtime` owns small executable, host-neutral runtime helpers used by
backend adapters: `RuntimeComposition`, typed shared-handle lookup,
host-neutral settings snapshots, and explicit DB access. It re-exports the
neutral `HostRuntimeContext` contract from `rustok-api` but does not own stable
API contracts.

This crate does not own domain services, HTTP response mapping, CLI contracts,
FBA metadata, or UI transport. It exists to prevent repeated framework-specific
runtime lookup code while server composition is decoupled from Loco.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This backend-helper crate has no module-owned UI or FBA provider port.

## Open results

1. **Use RuntimeComposition in the first DB-backed module CLI provider.** Pass
   host-created composition into a module-local CLI adapter without depending on
   `apps/server` configuration or the server crate.
   **Depends on:** the first runtime-aware module-local CLI provider.
   **Done when:** provider construction receives DB, settings, and typed handles
   through `RuntimeComposition` and can run outside the production HTTP runtime.

2. **Add focused runtime-helper evidence with real consumers.** Cover
   composition, missing typed-handle errors, settings snapshot, and DB cloning
   after a non-trivial production consumer migrates.
   **Depends on:** a migrated server/module/CLI adapter.
   **Done when:** targeted tests and source guardrails prevent copied handle
   lookup or framework-specific runtime access in that consumer.

3. **Re-evaluate API/runtime ownership after bootstrap decoupling.** Keep stable
   contracts in `rustok-api` and executable helpers here; revisit the boundary
   only once server bootstrap no longer relies on its remaining Loco bridge.
   **Depends on:** server bootstrap/runtime composition progress.
   **Done when:** dependency direction and the ownership of each runtime helper
   are explicit, documented, and free of server-type coupling.

## Verification

- Targeted tests for `RuntimeComposition`, `require_shared`, settings snapshots,
  and DB handle cloning when a consumer changes.
- `npm run verify:api:surface-contract` for neutral runtime ownership.

## Change rules

1. Keep stable API contracts in `rustok-api` and executable helper code here.
2. Do not add domain services, response mapping, CLI contracts, FBA metadata, or
   UI transport.
3. Update runtime, API, server, and consumer documentation with a changed
   composition contract.
