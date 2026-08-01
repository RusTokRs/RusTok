# Implementation plan for `rustok-rbac`

## Source of truth

This file is the canonical live RBAC implementation plan. It owns current source state,
remaining priorities, verification commands, and the periodic release handoff.

- `[x]` means present in `main` or the current verification branch.
- `[ ]` means landing or execution evidence remains required.
- Source-ready is not compiled or operationally verified.

Last reconciled with `main`: 2026-08-01.

## Ownership boundary

`rustok-rbac` owns permission decisions, role/permission relation semantics, canonical
built-in role mutation policy, repair, relation integrity, durable generation storage,
and RBAC integration contracts.

`apps/server` owns authenticated adapters, database transaction orchestration, cache
adapters, fast-path delivery, worker supervision, and process telemetry. `rustok-events`
owns sealed payloads and schema digests; `rustok-outbox` owns transactional durable
transport.

Claims, presentation roles, caches, projections, and event consumers are never
permission or role-assignment authority.

## Current state

Merged PR #2867 owns the canonical built-in user-role mutation contract:

- tenant and identity validation;
- actor assign/manage hierarchy;
- last-active-SuperAdmin continuity;
- exact canonical role no-op;
- malformed same-effective-role repair;
- typed `rbac.user_role_replaced` and `rbac.user_role_assignment_repaired` events;
- relation, generation, and event in one caller transaction.

Draft PR #2866 is strictly additive and contains four remaining P1 corrections:

1. remove mutation methods from `PermissionResolver` and
   `RuntimePermissionResolver`, plus `RoleAssignmentStore` and the server direct-store
   adapter;
2. publish sealed artifact role-permission assignment events with mutation and receipt
   in one RBAC owner transaction;
3. compare requested status to the locked user row and skip user-row update,
   generation, session revocation, and fan-out for exact role/status replay;
4. add an incremental PostgreSQL/SQLite migration that cleans malformed artifact grant
   and operation rows and enforces tenant, role, actor, catalog, parent-update, parent-
   delete, and referenced-catalog identity integrity at the database boundary.

Historical drafts #2843, #2847, and #2863 were cross-linked and closed without merge on
2026-08-01 after their unique corrections were reconciled into #2866. They must not be
reopened or merged in addition to #2866.

## FFA/FBA boundary

- FFA: `in_progress`
- FBA: `boundary_ready`
- Provider: `RbacPermissionDecisionPort` / `rbac.permission_decision.v1`
- Promotion remains blocked on composed live-host, degraded-path, and native operator
  parity evidence.

## Implementation phases

### Principal and tenant trust — source complete, execution pending

- [x] Use one typed principal classifier.
- [x] Fail closed for malformed authenticated facts.
- [x] Require direct, session-bound, tenant-matching principals for RBAC control-plane
  operations.
- [x] Keep authoritative and cached relation reads tenant-safe.
- [x] Enforce cross-tenant base `user_roles` and `role_permissions` integrity.
- [ ] Execute focused API/server and live negative transport gates.

### Canonical user-role mutation — merged source-ready in #2867

- [x] Keep canonical role policy in `rustok-rbac`.
- [x] Lock target and continuity facts in the server transaction.
- [x] Distinguish exact no-op, relation repair, and role replacement.
- [x] Publish typed role mutation event with the same durable generation.
- [x] Roll back relation, user row, generation, and event on required publication failure.
- [x] Preserve exact role replay as no relation mutation, generation, or event.
- [x] In draft #2866, also preserve exact status replay as no user-row update,
  generation, session revocation, or fan-out.
- [ ] Execute owner policy, server adapter, Outbox, and effective-status tests.

### Resolver ownership — source complete in draft #2866

- [x] Make `PermissionResolver` and `RuntimePermissionResolver` read-only.
- [x] Remove `RoleAssignmentStore` and the server-owned direct persistence adapter.
- [x] Retain no deprecated alias, compatibility mutation path, or local-only
  invalidation bypass.
- [ ] Land and execute #2866.

### Artifact permission mutation and events — source complete in draft #2866

- [x] Add sealed `rbac.artifact_role_permission.assignment_changed` v1.
- [x] Keep command validation, idempotency, state-change detection, and publication in
  the RBAC owner.
- [x] Preserve stable typed role/catalog errors by validating before receipt insert.
- [x] Publish mutation, receipt, relation, and event in one transaction.
- [x] Emit no event for exact retry or state no-op.
- [x] Roll back on required Outbox failure.
- [x] Declare the RBAC -> Outbox module dependency consistently.
- [x] Clean legacy malformed grant and receipt rows during incremental migration.
- [x] Enforce role/actor tenant and admitted catalog scope on grant and receipt writes.
- [x] Guard parent role/user tenant changes and deletes that would orphan artifact state.
- [x] Keep referenced catalog identity immutable while allowing label/description upsert.
- [x] Add SQLite cleanup, valid-write, cross-tenant, parent-change, and catalog guards.
- [x] Add a fail-closed source verifier for migration registration and owner ordering.
- [ ] Generate and review the exact-head event digest.
- [ ] Execute contract, owner transaction, SQLite, PostgreSQL, adapter, source verifier,
  migration rollback, and module gates.

### Durable invalidation and recovery — source complete, execution pending

- [x] Reserve monotonic database generation in authorization mutation transactions.
- [x] Use local/Redis publication as best-effort fast paths.
- [x] Recover missed, stale, duplicate, and gapped generations through one checkpoint.
- [x] Export bounded lag, generation, worker, and recovery telemetry.
- [x] Add source packets for PostgreSQL concurrency (#2849), watchdog recovery (#2853),
  Redis restart/outage (#2856), and full registered-CLI repair propagation (#2862).
- [ ] Execute and retain all four packets on one reconciled revision.
- [ ] Execute and retain the incident packet from #2846.

## Remaining work

### P0. Exact-head verification

- [x] Rebuild #2866 as one commit on the latest `main` after the migration packet.
- [ ] Generate and review the artifact event contract digest.
- [ ] Run formatting, Events/RBAC/Admin/server compilation, Clippy, focused Rust/Node
  tests, and module validate/test on that exact head.
- [ ] Run the new SQLite migration proof and a real PostgreSQL apply/integrity/rollback
  target on the same revision.
- [ ] Resolve every failure before claiming verification.

### P0. Runtime evidence

- [ ] Execute #2849 PostgreSQL concurrency.
- [ ] Execute #2853 independent-process watchdog recovery.
- [ ] Execute #2856 real Redis available/outage/restart recovery.
- [ ] Execute #2862 full registered-CLI repair propagation.
- [ ] Retain one same-revision result set within documented bounds.

### P1. Operator parity and lifecycle

- [x] Cross-link and close #2843/#2847/#2863 as superseded without merge.
- [ ] Land #2866 without restoring the superseded branches.
- [ ] Define cleanup ordering for hard role/user/tenant deletion before referenced
  artifact grants or operation receipts can be removed.
- [ ] Decide whether remote/headless role management is required.
- [ ] Define custom-role and arbitrary permission mutation ownership.
- [ ] Route native operator management through owner policy without host-owned relation
  writes or a parallel `/roles` implementation.
- [ ] Identify idempotent, non-authoritative consumers for RBAC integration events.
- [ ] Complete incident and live negative transport evidence.

### P2. FBA/FFA promotion evidence

- [ ] Exercise provider/consumer/degraded paths in a composed host.
- [ ] Prove the RBAC evaluator remains the only decision engine.
- [ ] Complete native operator parity before FFA promotion.

## Verification commands

```bash
cargo fmt --all -- --check
cargo run -p rustok-events --example event_contract_digests -- --write
cargo check -p rustok-api
cargo check -p rustok-api --features server
cargo check -p rustok-events --all-targets
cargo check -p rustok-rbac --all-features
cargo check -p rustok-rbac-admin --features ssr
cargo check -p rustok-server --lib
cargo test -p rustok-events rbac_role_mutation
cargo test -p rustok-events --test rbac_artifact_permission_contracts
cargo test -p rustok-rbac role_mutation
cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite
cargo test -p rustok-rbac --test artifact_permission_tenant_integrity_sqlite
cargo test -p rustok-server auth_admin_mutation_provider
cargo test -p rustok-server status_effective_change_ignores_exact_replay
cargo test -p rustok-server --test rbac_permission_resolver_read_only_guard
cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard
cargo test -p rustok-server --test rbac_mutation_api_architecture_guard
node scripts/verify/verify-rbac-owner-role-mutation-contract.mjs
node scripts/verify/verify-rbac-artifact-permission-outbox.mjs
node scripts/verify/verify-rbac-artifact-permission-tenant-integrity.mjs
node scripts/verify/verify-rbac-explicit-principal-kind.mjs
node scripts/verify/verify-rbac-invalidation-observability.mjs
cargo xtask module validate rbac
cargo xtask module test rbac
```

No command above was executed in this connector-only source slice.

## Completion gates

- Source-ready becomes verified only after exact-head commands pass.
- Artifact event review requires the generated digest.
- Artifact relation integrity requires retained SQLite and PostgreSQL execution.
- Durable invalidation requires retained PostgreSQL/watchdog/Redis/CLI evidence.
- FBA remains `boundary_ready`; FFA and `core/rbac` remain `in_progress`.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-08-01`
- Scope inspected: `merged owner role mutation policy/events; effective status replay; read-only resolver ownership; artifact permission transaction event; artifact relation database integrity; durable invalidation and recovery packets`
- Findings: `P0=1, P1=4, P2=0, P3=0`
- Fixed in this pass: `draft PR #2866 is reconciled atop merged #2867. It removes resolver mutation composition, adds transactional artifact role-permission events, prevents exact status or role/status replay from updating the user row or advancing authorization state, and adds an incremental PostgreSQL/SQLite integrity migration for artifact grants and operation receipts. The migration cleans legacy malformed rows, enforces tenant-bound role/actor/catalog identity, protects parent changes, and keeps typed role/catalog errors by validating before receipt insertion. Historical drafts #2843, #2847, and #2863 were closed without merge. The branch is reconstructed as one commit on the latest main.`
- Remaining risks or blockers: `#2866 is source-only and lacks the generated artifact event digest. Formatting, compilation, focused tests, SQLite/PostgreSQL migration execution, Node/module gates, live transports, retained #2849/#2853/#2856/#2862 execution, incident evidence, native operator parity, and FFA/FBA evidence remain absent. Hard-deletion cleanup ordering for referenced artifact receipts must be defined. Issue #2740 remains the known Rust-host PostgreSQL fixture blocker.`
- Evidence: `source review confirms the new migration is registered after artifact tables, cleanup precedes trigger creation, SQLite up/down inventories are symmetric, validation precedes receipt insertion, DB triggers cover grant/receipt writes plus role/user/catalog parent changes, and the branch is one commit zero behind current main. No execution evidence is claimed.`
- Next action: `generate and review the artifact event digest, then run exact-head compile/test/verifier/module and SQLite/PostgreSQL migration gates`
- Resume command: `cargo fmt --all -- --check && cargo run -p rustok-events --example event_contract_digests -- --write && cargo check -p rustok-events --all-targets && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-server --lib && cargo test -p rustok-events --test rbac_artifact_permission_contracts && cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite && cargo test -p rustok-rbac --test artifact_permission_tenant_integrity_sqlite && cargo test -p rustok-server --test rbac_permission_resolver_read_only_guard && cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard && node scripts/verify/verify-rbac-owner-role-mutation-contract.mjs && node scripts/verify/verify-rbac-artifact-permission-outbox.mjs && node scripts/verify/verify-rbac-artifact-permission-tenant-integrity.mjs && cargo xtask module validate rbac && cargo xtask module test rbac`
