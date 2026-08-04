# Implementation plan for `rustok-rbac`

## Source of truth

This file is the canonical live implementation plan for RBAC. It records the current
source state, open priorities, required evidence, and the periodic verification handoff.

- `[x]` means the source contract is present on `main` or the active verification branch.
- `[ ]` means landing or execution evidence remains required.
- Source-ready is not compiled, migrated, transport-verified, or operationally verified.

Last reconciled with `main`: 2026-08-04 (`c6ae3db0caf64c4578cb76073e9b719e483fb953`).

## Ownership boundary

`rustok-rbac` owns permission decisions, role and permission relation semantics,
canonical built-in role mutation policy, artifact permission admission and assignment,
relation integrity, repair, durable authorization-generation storage, and RBAC
integration contracts.

`apps/server` owns authenticated adapters, caller transaction orchestration, cache
adapters, best-effort invalidation delivery, worker supervision, and process telemetry.
`rustok-events` owns sealed payloads and schema digests. `rustok-outbox` owns durable
transactional transport. `rustok-migrations` owns the immutable global migration prefix
and explicit append-only release tail.

Claims, presentation roles, caches, projections, and event consumers are never
permission or role-assignment authority.

## Current state

The active cycle remains `cycle-001/core-rbac` and stays `in_progress`.

This source slice was reconstructed from the useful product changes in superseded draft
PR #2870 onto current `main`. The temporary workflow used by that branch to diagnose a
stale lock and migration prefix was deliberately excluded because repository policy does
not permit task-specific workflow edits and the workflow encoded an obsolete migration
tail.

The clean source slice provides:

1. read-only `PermissionResolver` and `RuntimePermissionResolver` contracts, with the
   server direct role-assignment store removed;
2. exact role and status replay as no user update, durable generation reservation,
   session revocation, relation mutation, or event;
3. sealed `rbac.artifact_role_permission.assignment_changed` v1 publication in the
   same owner transaction as relation and idempotency receipt;
4. immutable language-neutral artifact permission definitions plus canonical localized
   translations;
5. exact definition identity and admitted scope on grants, receipts, authorization
   reads, and events;
6. tenant-composite role and actor foreign keys, exact-scope definition foreign keys,
   and fail-closed parent behavior;
7. explicit operator scope selection (`platform` or trusted routed `tenant`) without
   preferred-scope fallback or a caller-supplied second tenant identity;
8. one shared assignable permission-key contract across registration, assignment, and
   event publication;
9. an append-only fifth RBAC migration that upgrades legacy catalog, grant, and receipt
   state and fails closed on ambiguous or orphan authority;
10. downgrade validation that refuses to erase exact scope identity when canonical
    state cannot be represented by the legacy selector;
11. explicit SQLite upgrade and downgrade transactions that restore the pre-migration
    schema after validation or destructive DDL failure;
12. SQLite upgrade, rollback, failure-atomicity, scope-shadowing, cross-tenant,
    parent-integrity, localization, immutability, and transactional event regressions;
13. a global migration tail that preserves every entry already published by current
    `main` and appends only `m20260803_000001_canonicalize_artifact_permissions`;
14. fail-closed source guards for owner boundaries, exact scope, migration history,
    release order, event identity, locale normalization, rollback identity, migration
    atomicity, and removed execution paths.

## Findings

- `P0=0`
- `P1=11`
  1. localized copy participated in authorization identity;
  2. locale keys were not canonical and semantic duplicates were possible;
  3. trigger-only parent checks admitted a concurrent parent update or delete race;
  4. grants could bind tenant authority to an incorrect permission scope;
  5. admitted authorization identity remained mutable;
  6. preferred platform-versus-tenant lookup could shadow or fail to revoke an existing
     grant;
  7. an exact generated UUID transport was unusable because no owner read contract
     exposed that identity;
  8. registration admitted permission keys that assignment and event contracts rejected;
  9. rewriting registered migration IDs would leave upgraded databases on legacy schema;
  10. downgrade could erase canonical grant or receipt scope identity;
  11. SQLite cutover failure could leave a partially renamed authorization schema.
- `P2=1`: nil tenant registration scope.
- `P3=2`: compatibility wording and obsolete broad lint handling.

All listed findings are source-fixed in the active slice. None is execution-verified.

## FFA/FBA boundary

- FFA: `in_progress`
- FBA: `boundary_ready`
- Provider: `RbacPermissionDecisionPort` / `rbac.permission_decision.v1`
- Promotion remains blocked on composed live-host, degraded-path, native operator, and
  same-revision runtime evidence.

## Implementation phases

### Principal and tenant trust — source complete, execution pending

- [x] Use one typed principal classifier and fail closed for malformed facts.
- [x] Require direct, session-bound, tenant-matching principals for control-plane writes.
- [x] Keep authoritative and cached relation reads tenant-safe.
- [x] Enforce cross-tenant base role and permission relation integrity.
- [ ] Execute focused API/server checks and live negative transport gates.

### Canonical user-role mutation — source-ready

- [x] Keep canonical role policy in `rustok-rbac`.
- [x] Lock target and continuity facts in the caller transaction.
- [x] Distinguish exact no-op, malformed relation repair, and role replacement.
- [x] Publish relation, generation, and typed event atomically.
- [x] Preserve exact role/status replay as a complete side-effect no-op.
- [x] Confine initial role assignment to caller-owned user creation.
- [ ] Execute owner policy, server adapter, Outbox, status, and architecture tests.

### Resolver ownership — source-ready

- [x] Make resolver contracts read-only.
- [x] Remove `RoleAssignmentStore` and the server direct persistence adapter.
- [x] Retain no deprecated mutation alias, compatibility wrapper, or local-only bypass.
- [ ] Compile and execute the focused resolver architecture guard.

### Artifact permission identity, mutation, and events — source-ready

- [x] Add sealed artifact role-permission assignment event v1.
- [x] Include exact immutable definition identity in durable state and events.
- [x] Keep validation, idempotency, mutation, and publication in owner code and one
  transaction.
- [x] Emit no event for exact retry or durable state no-op.
- [x] Roll back mutation and receipt when required publication fails.
- [x] Store language-neutral definitions separately from localized translations.
- [x] Normalize locale tags, use `VARCHAR(32)`, and reject semantic duplicates.
- [x] Reject nil tenant registration scope before opening a transaction.
- [x] Make scope, installation, module, release, and permission key immutable.
- [x] Use explicit platform/tenant selection derived from trusted routing context.
- [x] Enforce one 256-byte, trimmed, control-free permission-key contract.
- [x] Enforce tenant-composite role/actor and exact definition/scope parents.
- [x] Preserve account deactivation/redaction and fail closed on unsupported hard delete.
- [x] Add SQLite integrity, upgrade, rollback, explicit-scope, and Outbox regressions.
- [x] Add fail-closed source verifiers.
- [ ] Generate and review the exact-head event digest.
- [ ] Execute contract, transaction, SQLite, PostgreSQL, adapter, verifier, migration,
  rollback, and module gates.

### Append-only schema upgrade — source-ready, execution pending

- [x] Preserve historical RBAC migration bodies.
- [x] Register `m20260803_000001_canonicalize_artifact_permissions` as a fifth RBAC
  migration instead of changing an existing migration ID.
- [x] Backfill one immutable definition per exact legacy identity and canonical
  translations without fabricating authority.
- [x] Fail closed when legacy grants or receipts are orphaned or ambiguous.
- [x] Prevent lossy rollback when distinct scoped state collapses to one legacy key.
- [x] Require every grant and operation receipt to retain one representable legacy
  selector before downgrade.
- [x] Wrap SQLite upgrade and downgrade in explicit transactions.
- [x] Register a PostgreSQL backfill fixture in
  `docs/migrations/backfill-contracts.json`.
- [x] Preserve the current `main` migration tail and append only the RBAC cutover.
- [ ] Execute Migration Compatibility preflight, clean PostgreSQL apply, N-1 upgrade,
  fixture assertion, rollback, and schema-contract checks on one exact head.

### Durable invalidation and recovery — source complete, execution pending

- [x] Reserve monotonic database generation in authorization mutation transactions.
- [x] Use local/Redis publication only as best-effort fast paths.
- [x] Recover missed, stale, duplicate, and gapped generations from one checkpoint.
- [x] Export bounded lag, generation, worker, and recovery telemetry.
- [x] Retain source packets for PostgreSQL concurrency (#2849), watchdog recovery
  (#2853), Redis restart/outage (#2856), and CLI repair propagation (#2862).
- [ ] Execute and retain all four packets on one reconciled revision.
- [ ] Execute and retain the incident packet from #2846.

## Remaining work

### P0. Exact-head verification

- [ ] Generate and review the artifact event contract digest.
- [ ] Run formatting, Events/RBAC/Admin/server compilation, Clippy, focused Rust/Node
  tests, and module validate/test on the final exact head.
- [ ] Run canonical SQLite proofs and real PostgreSQL clean apply, N-1 upgrade,
  integrity, canonical-locale, explicit-scope, parent-delete concurrency, and rollback.
- [ ] Resolve every product failure before claiming verification.

### P0. Runtime evidence

- [ ] Execute #2849 PostgreSQL concurrency.
- [ ] Execute #2853 independent-process watchdog recovery.
- [ ] Execute #2856 real Redis available/outage/restart recovery.
- [ ] Execute #2862 registered-CLI repair propagation.
- [ ] Retain one same-revision result set within documented bounds.

### P1. Operator parity and lifecycle

- [ ] Decide whether remote/headless role management is required.
- [ ] Define custom-role and arbitrary permission mutation ownership.
- [ ] Route native operator management through owner policy without parallel relation
  writes.
- [ ] Identify idempotent, non-authoritative event consumers.
- [ ] Complete incident and live negative transport evidence.

### P2. Deferred hard-delete workflow and promotion evidence

- [ ] If hard deletion enters product scope, implement one owner transaction that removes
  receipts, grants, definitions/translations, and parent rows in documented order.
- [ ] Exercise provider, consumer, and degraded paths in a composed host.
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
cargo test -p rustok-events --test rbac_artifact_permission_contracts
cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite
cargo test -p rustok-rbac --test artifact_permission_tenant_integrity_sqlite
cargo test -p rustok-rbac --test artifact_permission_upgrade_sqlite
cargo test -p rustok-migrations migrator_preserves_append_only_migration_tail
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

No command above is claimed as passed until its exact-head workflow or retained execution
packet completes successfully.

## Completion gates

- Source-ready becomes verified only after exact-head commands pass.
- Artifact event review requires generator-produced digest output.
- Artifact relation integrity requires retained SQLite and PostgreSQL execution.
- Migration integrity requires immutable-prefix, clean-apply, N-1 upgrade, fixture,
  downgrade selector identity, failure atomicity, and rollback evidence.
- Durable invalidation requires retained PostgreSQL/watchdog/Redis/CLI evidence.
- FBA remains `boundary_ready`; FFA and `core/rbac` remain `in_progress`.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-08-04`
- Scope inspected: `resolver ownership; canonical user-role mutation; effective status replay; artifact permission identity, locale normalization, explicit scope, database integrity, transactional event publication, append-only schema upgrade, downgrade scope preservation, SQLite failure atomicity, current-main migration release order, and clean reconstruction from superseded draft PR #2870`
- Findings: `P0=0, P1=11, P2=1, P3=2`
- Fixed in this pass: `reconstructed the RBAC product source on current main without the temporary workflow; removed server-owned role mutation paths; preserved exact role/status no-op behavior; introduced immutable language-neutral artifact permission definitions and owner translations; bound grants, receipts, reads, and events to exact scope identity; aligned permission-key contracts; added same-transaction Outbox publication; added a fail-closed append-only cutover migration with SQLite upgrade/rollback coverage; and preserved the complete current-main global migration tail before appending the RBAC cutover`
- Remaining risks or blockers: `formatting, compilation, focused tests, source/module gates, generated event digest, Migration Compatibility, PostgreSQL clean apply and N-1 upgrade, concurrency and rollback evidence, live negative transports, watchdog/Redis/CLI/incident packets, native operator parity, and FFA/FBA evidence remain absent. Issue #2740 remains an infrastructure blocker for the known Rust-host path unless a current run proves otherwise.`
- Evidence: `source inspection and committed regression files only. The clean source tree is based on main c6ae3db0caf64c4578cb76073e9b719e483fb953; the task-specific workflow is absent; the global migration registry differs by one append-only RBAC tail entry. No formatting, compiler, test, verifier, database, Redis, workflow, CI, or production pass is claimed.`
- Next action: `run the exact-head targeted commands, generate the event digest, resolve product failures, then execute PostgreSQL integrity and runtime recovery packets before considering core/rbac complete`
- Resume command: `cargo fmt --all -- --check && cargo run -p rustok-events --example event_contract_digests -- --write && cargo check -p rustok-events --all-targets && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-server --lib && cargo test -p rustok-events --test rbac_artifact_permission_contracts && cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite && cargo test -p rustok-rbac --test artifact_permission_tenant_integrity_sqlite && cargo test -p rustok-rbac --test artifact_permission_upgrade_sqlite && cargo test -p rustok-migrations migrator_preserves_append_only_migration_tail && cargo test -p rustok-server --test rbac_permission_resolver_read_only_guard && cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard && cargo test -p rustok-server --test rbac_mutation_api_architecture_guard && node scripts/verify/verify-rbac-owner-role-mutation-contract.mjs && node scripts/verify/verify-rbac-artifact-permission-outbox.mjs && node scripts/verify/verify-rbac-artifact-permission-tenant-integrity.mjs && cargo xtask module validate rbac && cargo xtask module test rbac`
