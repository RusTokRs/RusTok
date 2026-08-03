# Implementation plan for `rustok-rbac`

## Source of truth

This file is the canonical live RBAC implementation plan. It owns current source state,
remaining priorities, verification commands, and the periodic release handoff.

- `[x]` means present in `main` or the current verification branch.
- `[ ]` means landing or execution evidence remains required.
- Source-ready is not compiled or operationally verified.

Last reconciled with `main`: 2026-08-03.

## Ownership boundary

`rustok-rbac` owns permission decisions, role/permission relation semantics, canonical
built-in role mutation policy, artifact permission identity and assignment, repair,
relation integrity, durable authorization generation storage, and RBAC integration
contracts.

`apps/server` owns authenticated adapters, caller transaction orchestration, cache
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

Draft PR #2870 supersedes closed PR #2866 and carries the active `cycle-001/core-rbac`
source work:

1. remove mutation methods from `PermissionResolver` and
   `RuntimePermissionResolver`, plus `RoleAssignmentStore` and the server direct-store
   adapter;
2. publish sealed artifact role-permission assignment events with mutation and receipt
   in one RBAC owner transaction;
3. compare requested status to the locked user row and skip user-row update,
   generation, session revocation, and fan-out for exact role/status replay;
4. replace locale-bearing artifact permission identity rows with one immutable,
   language-neutral definition plus owner translations, normalize locale tags through
   the shared API contract, and reject semantic locale duplicates;
5. bind grants and idempotency receipts to the exact artifact permission definition,
   admitted scope, tenant-composite role, and tenant-composite actor through real
   foreign keys and database checks;
6. reject cross-tenant definition binding, concurrent parent update/delete, definition
   identity rebinding, and nil tenant registration scope;
7. remove the superseded corrective trigger migration and keep the unreleased schema
   correction in the canonical artifact migrations;
8. define account deletion as deactivation/redaction and preserve `RESTRICT` as the
   fail-closed role/user/definition hard-delete contract until an owner-coordinated
   teardown workflow exists;
9. remove legacy compatibility wording from the only new-user role initialization path
   and replace a broad event-contract lint allowance with a narrow documented
   expectation.

Historical drafts #2843, #2847, #2863, and #2866 are superseded and must not be reopened
or merged in parallel with #2870. Technical PR #2879 merged current `main` into the
verification branch as merge commit `a66d5d5c308a05fd5cc4c8ae941e25b68bde358c`
without modifying `main` or force-pushing the RBAC work.

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

### Canonical user-role mutation — source-ready

- [x] Keep canonical role policy in `rustok-rbac`.
- [x] Lock target and continuity facts in the server transaction.
- [x] Distinguish exact no-op, relation repair, and role replacement.
- [x] Publish typed role mutation events with the same durable generation.
- [x] Roll back relation, user row, generation, and event on required publication failure.
- [x] Preserve exact role replay as no relation mutation, generation, or event.
- [x] Preserve exact status replay as no user-row update, generation, session revocation,
  or fan-out.
- [x] Confine initial role assignment to caller-owned user creation; existing-user
  mutations use explicit transactional or committed entrypoints.
- [ ] Execute owner policy, server adapter, Outbox, effective-status, and architecture
  guard tests.

### Resolver ownership — source complete in draft #2870

- [x] Make `PermissionResolver` and `RuntimePermissionResolver` read-only.
- [x] Remove `RoleAssignmentStore` and the server-owned direct persistence adapter.
- [x] Retain no deprecated mutation alias, compatibility path, or local-only
  invalidation bypass.
- [ ] Land and execute #2870.

### Artifact permission identity, mutation, and events — source complete in draft #2870

- [x] Add sealed `rbac.artifact_role_permission.assignment_changed` v1.
- [x] Include exact `artifact_permission_id` in grants, receipts, authorization queries,
  and events.
- [x] Keep command validation, idempotency, state-change detection, and publication in
  the RBAC owner.
- [x] Preserve stable typed role/permission errors by validating before receipt insert.
- [x] Publish mutation, receipt, relation, and event in one transaction.
- [x] Emit no event for exact retry or state no-op.
- [x] Roll back on required Outbox failure.
- [x] Declare the RBAC -> Outbox module dependency consistently.
- [x] Store language-neutral permission definitions separately from localized labels and
  descriptions; use `VARCHAR(32)` locale storage.
- [x] Normalize every locale through `rustok_api::normalize_locale_tag`, store only the
  canonical tag, and reject duplicates after normalization.
- [x] Reject nil tenant scope before opening a registration transaction.
- [x] Keep definition scope, installation, module, release, and permission identity
  immutable after admission while allowing localized translation updates.
- [x] Enforce exact admitted scope, tenant-composite role and actor identity, and exact
  permission parent identity through checks and real foreign keys.
- [x] Prevent concurrent parent update/delete from committing orphan grants or receipts.
- [x] Prevent direct database writes from binding a tenant to another tenant's permission
  definition while preserving explicit platform definitions.
- [x] Remove the trigger-only corrective migration instead of preserving two schema paths.
- [x] Define current account deletion as deactivation/redaction and hard parent deletion
  as fail-closed `RESTRICT` until a single owner-coordinated teardown exists.
- [x] Add SQLite valid-write, normalized-locale, platform-scope, cross-tenant, orphan,
  parent-change, deletion, definition-immutability, translation, and exact-event-identity
  regressions.
- [x] Add fail-closed source verifiers for canonical migration registration, owner
  ordering, exact event identity/scope, canonical locale storage, nil tenant scope,
  teardown semantics, and removed paths.
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

- [x] Reconcile #2870 with the current `main` merge base without dropping either the
  RBAC fixes or intervening repository commits.
- [ ] Generate and review the artifact event contract digest.
- [ ] Run formatting, Events/RBAC/Admin/server compilation, Clippy, focused Rust/Node
  tests, and module validate/test on the final exact head.
- [ ] Run the canonical SQLite proof and a real PostgreSQL apply/integrity/concurrency/
  rollback target on the same revision.
- [ ] Resolve every product failure before claiming verification.

### P0. Runtime evidence

- [ ] Execute #2849 PostgreSQL concurrency.
- [ ] Execute #2853 independent-process watchdog recovery.
- [ ] Execute #2856 real Redis available/outage/restart recovery.
- [ ] Execute #2862 full registered-CLI repair propagation.
- [ ] Retain one same-revision result set within documented bounds.

### P1. Operator parity and lifecycle

- [x] Close superseded draft #2866 without merge and continue only in #2870.
- [x] Define current account deletion and fail-closed role/user/definition teardown
  behavior; do not add cascade or compatibility fallback.
- [ ] Decide whether remote/headless role management is required.
- [ ] Define custom-role and arbitrary permission mutation ownership.
- [ ] Route native operator management through owner policy without host-owned relation
  writes or a parallel `/roles` implementation.
- [ ] Identify idempotent, non-authoritative consumers for RBAC integration events.
- [ ] Complete incident and live negative transport evidence.

### P2. Deferred hard-delete workflow and FBA/FFA promotion evidence

- [ ] If product scope introduces role, user, tenant, or installation hard deletion,
  implement one owner-coordinated transaction that removes operation receipts, grants,
  admitted definitions/translations, and then parent rows in documented order.
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

No command above was executed in the connector-only source slice. GitHub workflow
results are recorded only after the corresponding exact-head jobs finish.

## Completion gates

- Source-ready becomes verified only after exact-head commands pass.
- Artifact event review requires the generated digest.
- Artifact relation integrity requires retained SQLite and PostgreSQL execution,
  including canonical locale, cross-tenant scope, and parent-delete concurrency probes.
- Durable invalidation requires retained PostgreSQL/watchdog/Redis/CLI evidence.
- FBA remains `boundary_ready`; FFA and `core/rbac` remain `in_progress`.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-08-03`
- Scope inspected: `resolver ownership; canonical user-role mutation entrypoints; effective status replay; artifact permission multilingual storage, locale normalization and immutable identity; tenant/scope/concurrency integrity; transactional Outbox event identity; account teardown semantics; event-contract lint policy`
- Findings: `P0=0, P1=5, P2=1, P3=2`
- Fixed in this pass: `draft PR #2870 replaces closed #2866. The pass separates language-neutral artifact permission definitions from owner translations with VARCHAR(32) locale storage; normalizes locale tags through the shared API contract and rejects semantic duplicates; replaces trigger-only parent existence checks with tenant-composite role/user and exact permission foreign keys; binds every grant and receipt to the admitted platform-or-exact-tenant scope; prevents definition identity rebinding; rejects nil tenant scope; removes the superseded corrective migration; propagates artifact_permission_id through authorization and the sealed event; defines soft account deactivation plus fail-closed RESTRICT teardown; adds SQLite and source regressions; removes compatibility wording from new-user role initialization; replaces a broad lint allowance with a narrow documented expectation; and merges current main into the verification branch through technical PR #2879 without force-push.`
- Remaining risks or blockers: `#2870 remains draft and source-only. The generated event digest, formatting, compilation, focused tests, canonical SQLite execution, real PostgreSQL apply/integrity/locale/scope/parent-delete concurrency/rollback evidence, Node/module gates, live negative transports, retained #2849/#2853/#2856/#2862 execution, incident evidence, native operator parity, and FFA/FBA evidence remain absent. Issue #2740 remains the known Rust-host PostgreSQL fixture blocker until a current workflow proves otherwise.`
- Evidence: `static source inspection confirms one immutable language-neutral definition row per scope/installation/key, canonical shared locale normalization, owner-local translations, exact artifact_permission_id and permission_scope_key propagation, tenant-composite role/user foreign keys, exact-scope permission foreign keys and checks, fail-closed parent RESTRICT behavior, no corrective migration registration, transaction-local event publication with rollback on failure, soft account deactivation, and matching regression/verifier coverage. GitHub reports #2870 synchronized with current main through merge commit a66d5d5c308a05fd5cc4c8ae941e25b68bde358c. No local or database execution evidence is claimed.`
- Next action: `review current exact-head workflow failures, generate and commit the event digest, execute targeted compile/test/verifier/module gates, then run real PostgreSQL integrity and concurrency evidence before considering merge`
- Resume command: `cargo fmt --all -- --check && cargo run -p rustok-events --example event_contract_digests -- --write && cargo check -p rustok-events --all-targets && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-server --lib && cargo test -p rustok-events --test rbac_artifact_permission_contracts && cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite && cargo test -p rustok-rbac --test artifact_permission_tenant_integrity_sqlite && cargo test -p rustok-server --test rbac_permission_resolver_read_only_guard && cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard && cargo test -p rustok-server --test rbac_mutation_api_architecture_guard && node scripts/verify/verify-rbac-owner-role-mutation-contract.mjs && node scripts/verify/verify-rbac-artifact-permission-outbox.mjs && node scripts/verify/verify-rbac-artifact-permission-tenant-integrity.mjs && cargo xtask module validate rbac && cargo xtask module test rbac`
