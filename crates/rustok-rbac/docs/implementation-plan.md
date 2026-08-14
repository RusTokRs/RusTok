# Implementation plan for `rustok-rbac`

## Source of truth

This is the canonical live RBAC implementation plan. It records source state, open
priorities, required evidence, and the current verification handoff.

- `[x]` means source is present in `main`.
- `[ ]` means execution or promotion evidence remains required.
- Source-ready is not compiled, migrated, transport-verified, or operationally verified.

Last reconciled with `main`: 2026-08-14.

- Merge base used by the clean source PR: `fe786a9076f9457ef6564f53957a12a4d355859d`
- Merged source PR: #2980
- Merged source commit: `f4d89c26f1a30079918660280150016930c837a4`
- Current verification PR: #3546, rebased over `main@5f95e475dd0dcb56534419e081fe2c6eba1745e9`

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

The 2026-08-14 verification pass re-reviewed the critical RBAC P0/P1 source paths and did
not find a new semantic P0/P1 in the inspected resolver, artifact-permission, role
mutation, migration, tenant-integrity, generation-invalidation, or server-runtime paths.
This is a bounded static-review statement, not a completion claim.

Exact-head run `31787534422` on the previous verification head retained useful targeted
evidence: RBAC compilation, SQLite/contracts, and RBAC module gates passed. Its remaining
failures were format/event digest, architecture/verifiers, and API/server compilation.
Both `rustok-api` checks passed; `cargo check -p rustok-server --lib` failed with exit 101,
but the retained GitHub annotation does not include the Rust compiler diagnostic.

Formatter job `94726772608` retained the exact Rust 1.97.1 diff: two chained assertions in
`crates/rustok-build/src/module_manifest_contribution.rs` were reformatted at the same
physical source through the direct crate and shared include paths. PR #3546 now replaces
those two chains with semantics-preserving local bindings to remove the line-layout
oscillation. This repair still requires exact-final-head execution.

The current `rbac_permission_resolver_read_only_guard` and inspected resolver/runtime
sources still satisfy the intended static contract: the read-only owner marker is present
and no active `RoleAssignmentStore` mutation path was found. The failed architecture job
is not claimed as passed until a final-head run proves it.

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

All findings above are source-fixed. Execution evidence is partial and does not yet close
the component.

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
- [ ] Compile and execute the focused resolver architecture guard on the final head.

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
  migration compatibility, rollback, and module gates on one final head.

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
  and module validate/test on the final verification head.
- [ ] Resolve the `rustok-server --lib` exit-101 failure using a retained compiler
  diagnostic; do not infer a source cause from the generic annotation.
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

No command is claimed as passed on the final head until retained final-head execution
evidence exists. Historical exact-head evidence is recorded but does not transfer across
source or documentation commits automatically.

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
- Last verified at (UTC): `2026-08-14`
- Scope inspected: `critical RBAC resolver ownership; canonical role mutation/no-op behavior; artifact permission identity, exact scope, locale, tenant integrity and transactional event publication; append-only migration tail; generation invalidation; server RBAC runtime wiring; exact-head formatter, compilation, SQLite/contracts, architecture, verifier, and module-gate boundaries`
- Findings: `P0=0, P1=11, P2=1, P3=2`
- Fixed in this pass: `rebased PR #3546 directly over main@5f95e475dd0dcb56534419e081fe2c6eba1745e9 while preserving #3554; replaced two rustfmt-oscillating shared-manifest assertion chains with semantics-preserving local bindings; no new semantic RBAC P0/P1 was found in the inspected source paths`
- Remaining risks or blockers: `final-head formatting/event digest, rustok-server --lib compiler diagnostic and successful compilation, architecture/verifier execution, final-head module gates, Migration Compatibility, PostgreSQL clean apply/N-1/integrity/locale/concurrency/rollback, watchdog/Redis/CLI/incident packets, live negative transports, native operator parity, and FFA/FBA promotion evidence remain open. Local repository execution is unavailable in the connector environment, so GitHub Actions is the execution source of truth.`
- Evidence: `historical exact-head run 31787534422: RBAC compilation passed, SQLite/contracts passed, module-gate job 94726772566 passed validate and test; format job 94726772608 retained the exact Rust 1.97.1 two-assert diff now repaired in PR #3546; API/server job 94726772686 passed both rustok-api checks and failed rustok-server --lib with exit 101 but no retained compiler text; current resolver/runtime static review found the expected read-only marker and no active RoleAssignmentStore mutation path. No final-head pass is claimed yet.`
- Next action: `run the temporary exact-head workflow against the final PR SHA, consume the formatter/server/architecture diagnostics, fix only reproducible product failures, then execute PostgreSQL integrity and recovery packets before considering core/rbac complete`
- Resume command: `cargo fmt --all -- --check && cargo run -p rustok-events --example event_contract_digests -- --write && cargo check -p rustok-events --all-targets && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-server --lib && cargo test -p rustok-events --test rbac_artifact_permission_contracts && cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite && cargo test -p rustok-rbac --test artifact_permission_tenant_integrity_sqlite && cargo test -p rustok-rbac --test artifact_permission_upgrade_sqlite && cargo test -p rustok-migrations migrator_preserves_append_only_migration_tail && cargo test -p rustok-server --test rbac_permission_resolver_read_only_guard && cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard && cargo test -p rustok-server --test rbac_mutation_api_architecture_guard && node scripts/verify/verify-rbac-owner-role-mutation-contract.mjs && node scripts/verify/verify-rbac-artifact-permission-outbox.mjs && node scripts/verify/verify-rbac-artifact-permission-tenant-integrity.mjs && node scripts/verify/verify-rbac-explicit-principal-kind.mjs && node scripts/verify/verify-rbac-invalidation-observability.mjs && cargo xtask module validate rbac && cargo xtask module test rbac`
