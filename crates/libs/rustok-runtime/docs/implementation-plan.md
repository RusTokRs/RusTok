# Implementation plan for `rustok-runtime`

## Current state

`rustok-runtime` owns small executable, host-neutral runtime helpers used by
backend adapters: `RuntimeComposition`, typed shared-handle lookup,
host-neutral settings snapshots, and explicit DB access. It re-exports the
neutral `HostRuntimeContext` contract from `rustok-api` but does not own stable
API contracts. It also owns the canonical portable instance layout and its
restart-safe placement/preparation functions; installer re-exports these types
instead of maintaining a second path model.

This crate does not own domain services, HTTP response mapping, CLI contracts,
FBA metadata, or UI transport. It exists to prevent repeated framework-specific
runtime lookup code while server composition stays host-neutral.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This backend-helper crate has no module-owned UI or FBA provider port.

## Open results

1. **Complete adoption of `InstanceLayout`.** Replace remaining executable
   storage, release materialization, worker-cache, service-data, log, and run
   directory derivations with the canonical layout. External object stores and
   explicitly external services remain typed provider configuration, not local
   paths. Read-only runtime inspection reuses an existing marker but never
   claims an unprepared directory; only installer bind/prepare may create
   durable ownership. Digest-addressed methods validate canonical
   `sha256:<hex>` and store the portable hex component without a
   Windows-invalid colon.
   `materialize_role` is only a digest-verified, restart-resumable filesystem
   primitive: the separately authenticated deployment agent composes it with
   the owner-issued lease, process supervision, health evidence, and traffic
   activation; this crate never performs those control-plane actions.
   Existing path prefixes are canonicalized before ownership is bound, and
   managed descendants reject symbolic links and Windows reparse-point
   junctions so a selected root cannot redirect writes into another instance.
   **Depends on:** deployment-agent and service-runtime composition.
   **Done when:** installer, server, CLI, workers, and recovery code derive all
   instance-owned paths from the same layout and cross-platform tests cover two
   independent roots on one host.

2. **Use RuntimeComposition in the first DB-backed module CLI provider.** Pass
   host-created composition into a module-local CLI adapter without depending on
   `apps/server` configuration or the server crate.
   **Depends on:** the first runtime-aware module-local CLI provider.
   **Done when:** provider construction receives DB, settings, and typed handles
   through `RuntimeComposition` and can run outside the production HTTP runtime.

3. **Add focused runtime-helper evidence with real consumers.** Cover
   composition, missing typed-handle errors, settings snapshot, and DB cloning
   after a non-trivial production consumer migrates.
   **Depends on:** a migrated server/module/CLI adapter.
   **Done when:** targeted tests and source guardrails prevent copied handle
   lookup or framework-specific runtime access in that consumer.

4. **Re-evaluate API/runtime ownership after bootstrap decoupling.** Keep stable
   contracts in `rustok-api` and executable helpers here; revisit the boundary
   only once server bootstrap no longer relies on its remaining bridge.
   **Depends on:** server bootstrap/runtime composition progress.
   **Done when:** dependency direction and the ownership of each runtime helper
   are explicit, documented, and free of server-type coupling.

## Verification

- Targeted tests for `RuntimeComposition`, `InstanceLayout`, restart-safe root
  preparation, overlap rejection, settings snapshots, and DB handle cloning
  when a consumer changes.
- `npm run verify:api:surface-contract` for neutral runtime ownership.

## Change rules

1. Keep stable API contracts in `rustok-api` and executable helper code here.
2. Do not add domain services, response mapping, CLI contracts, FBA metadata, or
   UI transport.
3. Update runtime, API, server, and consumer documentation with a changed
   composition contract.
