# Implementation plan for `rustok-rbac`

## Source of truth

This is the canonical live RBAC implementation plan. It records source state, open
priorities, required evidence, and the current verification handoff.

- `[x]` means source is present in `main`.
- `[ ]` means execution or promotion evidence remains required.
- Source-ready is not compiled, migrated, transport-verified, or operationally verified.

Last reconciled with `main`: 2026-08-04.

- Merge base used by the clean PR: `fe786a9076f9457ef6564f53957a12a4d355859d`
- Merged PR: #2980
- Merged commit: `f4d89c26f1a30079918660280150016930c837a4`

## Ownership boundary

`rustok-rbac` owns permission decisions, role/permission relation semantics, built-in
role mutation policy, artifact permission admission and assignment, relation integrity,
repair, durable authorization generation storage, and RBAC integration contracts.

`apps/server` owns authenticated adapters, caller transaction orchestration, cache
adapters, fast-path invalidation delivery, worker supervision, and process telemetry.
`rustok-events` owns sealed event contracts. `rustok-outbox` owns durable transactional
transport. `rustok-migrations` owns the immutable global migration prefix and explicit
append-only release tail.

Claims, presentation roles, caches, projections, and consumers are never permission or
role-assignment authority.

## Current state

`cycle-001/core-rbac` remains `in_progress`.

Active verification task (2026-08-14): `[in_progress]` fix the RBAC mutation API
architecture guard so production call-site counting excludes only `#[cfg(test)]` test
modules, then classify PostgreSQL evidence from workflow run `31808510809` without
moving the verification cursor.

PR #2980 reconstructed the useful product changes from superseded draft #2870 on current
`main`. The task-specific workflow and obsolete migration-tail repair path were excluded.
The superseded draft is closed and must not be merged.

Merged source provides:

1. read-only permission resolver contracts and no server-owned role mutation store;
2. exact role/status replay as no row update, generation reservation, session
   revocation, relation mutation, invalidation fan-out, or event;
3. sealed `rbac.artifact_role_permission.assignment_changed` v1 publication in the
   same owner transaction as relation and idempotency receipt;
4. immutable language-neutral permission definitions plus canonical translations;
5. exact definition identity and admitted scope on grants, receipts, reads, and events;
6. tenant-composite role/actor foreign keys and exact-scope definition parents;
7. explicit `platform|tenant` mutation scope derived from trusted routing context;
8. one shared assignable permission-key contract across registration, assignment, and
   event publication;
9. a fifth append-only RBAC migration that fails closed on ambiguous or orphan legacy
   authority;
10. downgrade checks that preserve exact scope identity or refuse the downgrade;
11. explicit SQLite upgrade/downgrade transactions and failure-atomicity regressions;
12. a global migration registry that preserves every current-main tail entry and appends
    only `m20260803_000001_canonicalize_artifact_permissions`;
13. source guards and SQLite regressions for owner boundaries, exact scope, locale,
    tenant integrity, event atomicity, upgrade, rollback, and removed execution paths.

## Findings

- `P0=0`
- `P1=11`
  1. localized copy participated in authorization identity;
  2. locale keys were non-canonical;
  3. trigger-only parent checks admitted concurrency races;
  4. grants could bind authority to the wrong scope;
  5. admitted authorization identity remained mutable;
  6. preferred platform/tenant lookup could shadow an existing grant;
  7. generated UUID mutation input lacked a usable owner discovery contract;
  8. registration and assignment permission-key contracts diverged;
  9. rewriting registered migration IDs would not upgrade already-migrated databases;
  10. downgrade could erase canonical grant or receipt scope identity;
  11. SQLite cutover failure could leave a partial schema.
- `P2=1`: nil tenant registration scope.
- `P3=2`: compatibility wording and obsolete broad lint handling.

All findings above are source-fixed in `main`; none is execution-verified.

## FFA/FBA boundary

- FFA: `in_progress`
- FBA: `boundary_ready`
- Provider: `RbacPermissionDecisionPort` / `rbac.permission_decision.v1`
- Promotion is blocked on composed-host, degraded-path, native operator, and same-SHA
  runtime evidence.

## Implementation phases

### Principal and tenant trust

- [x] Use one typed principal classifier and fail closed for malformed facts.
- [x] Require direct, session-bound, tenant-matching principals for control-plane writes.
- [x] Keep authoritative and cached relation reads tenant-safe.
- [x] Enforce tenant-composite relation integrity.
- [ ] Execute focused API/server and live negative transport gates.

### Canonical user-role mutation

- [x] Keep canonical role policy in `rustok-rbac`.
- [x] Lock target and continuity facts in the caller transaction.
- [x] Distinguish exact no-op, malformed-assignment repair, and replacement.
- [x] Publish relation, generation, and typed event atomically.
- [x] Preserve exact role/status replay as a complete side-effect no-op.
- [ ] Execute owner policy, server adapter, Outbox, status, and architecture tests.

### Resolver ownership

- [x] Make resolver contracts read-only.
- [x] Remove `RoleAssignmentStore` and the server direct persistence adapter.
- [x] Retain no compatibility wrapper or local-only mutation bypass.
- [ ] Compile and execute the focused resolver architecture guard.

### Artifact permission identity, mutation, and events

- [x] Add sealed assignment event v1 with exact immutable definition identity.
- [x] Keep validation, idempotency, mutation, and publication in one owner transaction.
- [x] Emit no event for exact retry or durable no-op.
- [x] Roll back mutation and receipt when required publication fails.
- [x] Separate language-neutral definitions from canonical localized translations.
- [x] Normalize locale tags, use safe locale width, and reject semantic duplicates.
- [x] Reject nil tenant scope and mutable admitted identity.
- [x] Use trusted explicit platform/tenant selection.
- [x] Enforce one trimmed, bounded, control-free permission-key contract.
- [x] Enforce tenant-composite role/actor and exact definition/scope parents.
- [x] Add SQLite integrity, upgrade, rollback, explicit-scope, and Outbox regressions.
- [x] Add fail-closed source verifiers.
- [ ] Generate and review the exact-head event digest.
- [ ] Execute contract, owner transaction, SQLite, PostgreSQL, adapter, verifier,
  migration compatibility, rollback, and module gates.

### Append-only schema upgrade

- [x] Preserve historical RBAC migration bodies.
- [x] Register `m20260803_000001_canonicalize_artifact_permissions` as a fifth migration.
- [x] Backfill exact legacy identity and translations without fabricating authority.
- [x] Fail closed on orphan or ambiguous legacy grants and receipts.
- [x] Refuse lossy downgrade of exact scope identity.
- [x] Wrap SQLite upgrade and downgrade in explicit transactions.
- [x] Register the PostgreSQL backfill fixture.
- [x] Preserve the complete current-main tail and append only the RBAC cutover.
- [ ] Execute Migration Compatibility, clean PostgreSQL apply, N-1 upgrade, fixture,
  rollback, and schema-contract checks on one exact head.

### Durable invalidation and recovery

- [x] Reserve monotonic database generation in mutation transactions.
- [x] Treat local/Redis publication as best-effort fast paths.
- [x] Recover missed, stale, duplicate, and gapped generations from checkpoints.
- [x] Export bounded lag, generation, worker, and recovery telemetry.
- [x] Retain source packets for #2849, #2853, #2856, and #2862.
- [ ] Execute and retain all packets on one reconciled revision.
- [ ] Execute and retain the incident packet from #2846.

## Remaining priorities

### P0 — exact-head verification

- [ ] Generate and review the artifact event digest.
- [ ] Run formatting, Events/RBAC/Admin/server compilation, focused tests, verifiers,
  and module validate/test on the merged revision.
- [ ] Run SQLite proofs and PostgreSQL clean apply, N-1 upgrade, integrity, locale,
  explicit-scope, concurrency, and rollback scenarios.
- [ ] Resolve every product failure before claiming verification.

### P0 — runtime evidence

- [ ] Execute #2849 PostgreSQL concurrency.
- [ ] Execute #2853 independent-process watchdog recovery.
- [ ] Execute #2856 Redis available/outage/restart recovery.
- [ ] Execute #2862 registered-CLI repair propagation.
- [ ] Retain one same-revision result set within documented bounds.

### P1 — operator parity and lifecycle

- [ ] Decide whether remote/headless role management is required.
- [ ] Define custom-role and arbitrary permission mutation ownership.
- [ ] Route native operator management through owner policy without parallel writes.
- [ ] Identify idempotent, non-authoritative event consumers.
- [ ] Complete incident and live negative transport evidence.

### P2 — deferred hard delete and promotion

- [ ] If hard deletion enters scope, implement one owner transaction for receipts,
  grants, definitions/translations, and parent rows.
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

No command is claimed as passed until exact-head output or retained execution evidence
exists.

## Completion gates

- Source-ready becomes verified only after exact-head commands pass.
- Event review requires generator-produced digest output.
- Relation integrity requires retained SQLite and PostgreSQL execution.
- Migration integrity requires immutable-prefix, clean apply, N-1 upgrade, fixture,
  downgrade identity, failure atomicity, and rollback evidence.
- Durable invalidation requires retained PostgreSQL/watchdog/Redis/CLI evidence.
- FBA remains `boundary_ready`; FFA and `core/rbac` remain `in_progress`.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-08-04`
- Scope inspected: `resolver ownership; canonical user-role mutation; exact status replay; artifact permission identity, locale normalization, explicit scope, tenant integrity, transactional event publication, append-only upgrade, downgrade identity, SQLite failure atomicity, current-main migration order, and clean reconstruction/merge through PR #2980`
- Findings: `P0=0, P1=11, P2=1, P3=2`
- Fixed in this pass: `merged PR #2980 as f4d89c26f1a30079918660280150016930c837a4; removed server-owned role mutation paths; preserved exact role/status no-op behavior; introduced immutable permission identity and owner translations; bound grants, receipts, reads, and events to exact scope identity; aligned permission-key contracts; added owner-transaction Outbox publication; added fail-closed append-only cutover and SQLite regression coverage; preserved the complete current-main migration tail; excluded the task-specific workflow and obsolete repair path`
- Remaining risks or blockers: `formatting, compilation, focused tests, verifiers, module gates, generated event digest, Migration Compatibility, PostgreSQL clean apply/N-1 upgrade/integrity/concurrency/rollback, live negative transports, watchdog/Redis/CLI/incident packets, native operator parity, and FFA/FBA evidence remain absent. Issue #2740 remains an infrastructure classification unless a current exact-head run proves otherwise.`
- Evidence: `source inspection, merged commit f4d89c26f1a30079918660280150016930c837a4, and committed regression files only. PR #2980 changed exactly 38 expected files and no workflow. The global migration registry adds one RBAC tail entry. No formatting, compiler, test, database, Redis, workflow, CI, or production pass is claimed.`
- Next action: `run the exact-head targeted commands on merged main, generate the event digest, resolve product failures, then execute PostgreSQL integrity and recovery packets before considering core/rbac complete`
- Resume command: `cargo fmt --all -- --check && cargo run -p rustok-events --example event_contract_digests -- --write && cargo check -p rustok-events --all-targets && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-server --lib && cargo test -p rustok-events --test rbac_artifact_permission_contracts && cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite && cargo test -p rustok-rbac --test artifact_permission_tenant_integrity_sqlite && cargo test -p rustok-rbac --test artifact_permission_upgrade_sqlite && cargo test -p rustok-migrations migrator_preserves_append_only_migration_tail && cargo test -p rustok-server --test rbac_permission_resolver_read_only_guard && cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard && cargo test -p rustok-server --test rbac_mutation_api_architecture_guard && node scripts/verify/verify-rbac-owner-role-mutation-contract.mjs && node scripts/verify/verify-rbac-artifact-permission-outbox.mjs && node scripts/verify/verify-rbac-artifact-permission-tenant-integrity.mjs && cargo xtask module validate rbac && cargo xtask module test rbac`
