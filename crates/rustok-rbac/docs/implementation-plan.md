# Implementation plan for `rustok-rbac`

Status: transition to single-engine tenant policy runtime is complete; the module is maintained in
steady-state hardening and drift-prevention mode.

## Execution checkpoint

- Current phase: phase_b_in_progress
- Last checkpoint: RBAC admin FFA guardrail added fast boundary verifier `scripts/verify/verify-rbac-admin-boundary.mjs` and fixture suite `scripts/verify/verify-rbac-admin-boundary.test.mjs` for canonical split, legacy `api.rs`, Leptos-specific core, raw adapter calls, package-local GraphQL selected path and misplaced `#[server]` endpoints without long Rust compilation.
- Next step: Expand operator flows/verification for role and permission management surfaces; add GraphQL/REST secondary path only if such a remote/headless admin contract is approved, and keep the current native-only overview with fast boundary guardrails.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block and the central FFA/FBA readiness board.
- Last updated at (UTC): 2026-06-19T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - batch no-compile gate `scripts/verify/verify-owner-fba-runtime-order.mjs` checks `crates/rustok-rbac/contracts/evidence/rbac-provider-runtime-order-smoke.json`: read policy precedes request validation and claims evaluation, and fallback/degraded metadata remains in sync with registry; status remains `in_progress` until live host execution;
  - admin package split introduced `admin/src/core.rs` for Leptos-free overview view-model/error formatting, `admin/src/transport/` for the native server-function bootstrap facade, and `admin/src/ui/leptos.rs` as the only render adapter;
  - current admin bootstrap is an intentional temporary native-only single-adapter state because `rustok-rbac` had no legacy GraphQL/REST operator contract for this overview;
  - central FFA/FBA readiness board is synchronized in `docs/modules/registry.md`;
  - FBA provider slice: `crates/rustok-rbac/src/ports.rs` declares `RbacPermissionDecisionPort` / `rbac.permission_decision.v1` for admin permission-decision consumers with typed `PortContext`/`PortError`, read deadline semantics, claims-scope preservation and serializable DTOs; `crates/rustok-rbac/contracts/rbac-fba-registry.json` plus `crates/rustok-rbac/contracts/evidence/rbac-contract-test-static-matrix.json` lock planned contract cases and fallback profiles under `npm run verify:rbac:fba` while runtime fallback smoke remains pending before `boundary_ready`;
  - `scripts/verify/verify-rbac-admin-boundary.mjs` and `scripts/verify/verify-rbac-admin-boundary.test.mjs` enforce Leptos-free core, facade-only UI transport calls, native-only overview exception, typed transport error envelope and server-function adapter placement without full Rust compilation.

## Scope of work

- maintain `rustok-rbac` as the single canonical RBAC runtime boundary;
- synchronize permission contracts, integration events and server adapters;
- prevent reversion to shadow-runtime, rollout-mode or server-owned policy logic.

## Current state

- relation-store remains the source of truth for role/permission assignments;
- live authorization executes only through the tenant policy evaluator;
- `RuntimePermissionResolver` and related contracts already live in the module, while `apps/server` only holds adapters and observability;
- operator-facing admin overview is already published through `rustok-rbac-admin` and split across FFA layers (`core`, native-only `transport`, `ui/leptos`);
- local docs, root `README.md` and manifest metadata are part of the scoped audit path.

## Stages

### 1. Contract stability

- [x] lock single-engine runtime contract;
- [x] move policy/evaluator semantics and resolver APIs into the module;
- [x] standardize integration events for role-assignment changes;
- [ ] maintain sync between runtime contracts, server adapters and module metadata (tenant module adapters aligned: `module_registry`/`tenant_modules` and tenant admin bootstrap now check tenant-scoped read/list/manage permissions);
- [ ] contract tests cover all public use-cases for permission resolution, authorization decisions, cache semantics and integration events.

### 2. Drift prevention

- [ ] keep periodic verification green for RBAC/server integration;
- [ ] continue cleaning up presentation-only role inference outside primary authorization path;
- [~] expand guardrails as new RBAC-managed surfaces appear; current admin overview already shows live permission snapshot and module-declared catalog through FFA native-only transport.

### 3. Operability

- [ ] keep decision/cache/latency telemetry as part of the live contract;
- [ ] document runbooks and adapter expectations together with runtime surface changes;
- [ ] cover new event contracts and resolver paths with targeted integration tests.

## Verification

- `cargo xtask module validate rbac`
- `cargo xtask module test rbac`
- targeted tests for permission resolution, authorization decisions, cache semantics and integration events

## Update rules

1. When changing RBAC runtime contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, dependency graph or verification expectations, synchronize `rustok-module.toml` and relevant verification docs.
4. When changing live contract, also update `apps/server/docs/README.md`.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
