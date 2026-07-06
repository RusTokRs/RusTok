# Implementation plan for `rustok-outbox`

Status: core outbox baseline is locked; the module has been aligned to a unified
manifest/doc contract.

## Execution checkpoint

- Current phase: fba_write_policy_alignment
- Last checkpoint: `OutboxRelayPort` uses canonical `rustok_api::ports` primitives; outbox-specific Loco helper moved to owner crate under feature `loco-adapter`, so the dependency graph remains directed and without a cycle.
- Next step: Expand relay/backlog/DLQ evidence without long full-workspace compilation and then add targeted runtime contract/fallback smoke when compilation is allowed again.
- Open blockers: None.
- Hand-off notes for next agent: Keep read-only admin UI over the module-owned transport facade; do not move relay/runtime ownership to host UI.
- Last updated at (UTC): 2026-07-01T00:00:00Z

## FFA/FBA status block

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence / notes:
  - Admin native server-function transport no longer imports `loco_rs::app::AppContext`; it consumes host-provided `rustok_api::HostRuntimeContext`, and this is guarded by `scripts/verify/verify-api-surface-contract.mjs`;
  - batch owner gate `scripts/verify/verify-owner-fba-runtime-order.mjs` checks `crates/rustok-outbox/contracts/evidence/outbox-provider-runtime-order-smoke.json`: canonical `rustok_api::ports` write policy helper, deadline/idempotency error mapping, relay invocation before metrics projection and fallback/degraded parity; registry/manifest metadata migrated from old `rustok_api::ports::*` to the single `rustok_api::ports::*` contract, status remains `in_progress` until live relay execution;
  - admin UI has explicit FFA split: `admin/src/lib.rs` only wiring/re-export, `admin/src/core.rs` contains Leptos-free DTO/view-model helpers, `admin/src/transport/` owns the native server-function facade, `admin/src/ui/leptos.rs` owns Leptos rendering;
  - GraphQL/REST fallback was not added in this slice because the legacy outbox admin surface was a native-only read-only bootstrap; this is a temporary single-adapter state until a headless parity requirement appears for the operator UI;
  - fast evidence: `cargo check -p rustok-outbox-admin --lib` (25.04s, without full-workspace build), `node scripts/verify/verify-outbox-admin-boundary.mjs`, `node scripts/verify/verify-outbox-admin-boundary.test.mjs`;
  - fast evidence: `cargo check -p rustok-outbox-admin --lib` (25.04s, without full-workspace build);
  - compile-free FFA evidence: `npm run verify:outbox:admin-boundary` validates that UI uses only the module-owned transport facade, `core.rs` remains Leptos/server-function free, generated native server functions stay private to `transport/native_server_adapter.rs`, and host-provided `UiRouteContext.locale` remains the locale source;
  - FBA provider slice: `crates/rustok-outbox/contracts/outbox-fba-registry.json` + `crates/rustok-outbox/src/ports.rs` declare `OutboxRelayPort` / `outbox.relay_control.v1` for relay worker control with canonical `rustok_api::ports::PortContext`/`PortError`, `PortCallPolicy::write()` deadline/idempotency semantics and static evidence packet `crates/rustok-outbox/contracts/evidence/outbox-contract-test-static-matrix.json` verified by `npm run verify:outbox:fba`; status remains below `boundary_ready` until executable runtime contract/fallback smoke lands.

## Scope of work

- keep `rustok-outbox` as a bounded-context module for transactional publishing;
- synchronize relay/runtime contract, local docs and manifest metadata;
- evolve operational guarantees without spreading event runtime contract across the host layer.

## Current state

- write-side transactional publishing contract is already implemented;
- relay/retry/DLQ semantics are already part of the base runtime surface;
- module publishes admin visibility through `rustok-outbox-admin`, where UI split is aligned to `core/transport/ui`;
- root README, local docs and manifest contract are part of the scoped audit path.

## Stages

### 1. Contract stability

- [x] align root README, local docs and manifest metadata under a unified standard path;
- [x] lock transactional publishing as the main bounded-context contract;
- [x] separate FFA `core/transport/ui` boundary for read-only admin visibility surface;
- [x] add compile-free FFA boundary verifier for read-only admin visibility surface;
- [ ] maintain sync between public crate API and server event-runtime tests;
- [ ] contract tests cover all public use-cases for transactional publishing, relay, retry and DLQ semantics.

### 2. Runtime hardening

- [x] add no-compile FFA boundary verifier for read-only admin split and fixture regression suite;
- [ ] expand automated tests around relay/backlog/DLQ boundary behavior;
- [ ] document new runtime guarantees together with event transport contract changes;
- [ ] keep observability and operability as part of delivery readiness, not an afterthought.

### 3. Productionization

- [ ] clarify rollout and migration strategy for incremental adoption;
- [ ] complete security/tenancy/rbac checks that actually belong to the module;
- [ ] keep incident runbook in sync with operational semantics.

## Verification

- `npm run verify:outbox:admin-boundary`
- `npm run test:verify:outbox:admin-boundary`
- `npm run verify:outbox:fba`
- `cargo xtask module validate outbox`
- `cargo xtask module test outbox`
- `node scripts/verify/verify-outbox-admin-boundary.mjs`
- `node scripts/verify/verify-outbox-admin-boundary.test.mjs`
- `npm run verify:outbox:fba`
- targeted event-runtime tests for transactional publish, relay, retry and DLQ semantics

## Update rules

1. When changing transactional publishing or relay contract, update this file first.
2. When changing public/runtime contract, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata and UI wiring, synchronize `rustok-module.toml`.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and currency of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
