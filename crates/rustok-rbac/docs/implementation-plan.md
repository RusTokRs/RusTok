# Implementation plan for `rustok-rbac`

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Transport profile: temporary native-only; native/GraphQL admin parity is in progress.
- FBA provider contract: `RbacPermissionDecisionPort` / `rbac.permission_decision.v1` in `crates/rustok-rbac/contracts/rbac-fba-registry.json`.
- Static and runtime evidence: `crates/rustok-rbac/contracts/evidence/rbac-contract-test-static-matrix.json` and `crates/rustok-rbac/contracts/evidence/rbac-provider-runtime-order-smoke.json`.
- Evidence: `scripts/verify/verify-rbac-admin-boundary.mjs` locks the admin boundary guardrail.

## Source of truth

This is the canonical live RBAC implementation plan. It records source state, open
priorities, required evidence, and the current verification handoff.

- `[x]` means source is present in `main`.
- `[ ]` means execution or promotion evidence remains required.
- Source-ready is not compiled, migrated, transport-verified, or operationally verified.

Last reconciled with `main`: 2026-08-18.

- Source baseline PR: #2980, merged as `f4d89c26f1a30079918660280150016930c837a4`.
- Architecture-guard fix PR: #3563, merged as `eedd1954bf0db9920c7557b691863e316a00befa`.
- Runtime-evidence PR: #3570, merged as `9d7a8d4790c66bbcee3479cb880dc2008e5765b4`.
- Redis-restart evidence PR: #3579, merged as `6cb7d26734661b17f9b2ca8fead6e46c552bc3eb`.
- Registered-CLI evidence PR: #3590, merged as
  `67ed475549598720486188f175fcfce0ab826a3b`; merge-SHA RBAC Runtime Evidence run
  `32004633206`, artifact `9280508296`.
- Artifact-event-digest evidence PR: #3617, merged as
  `65a63a33d457ed5ff7c592f2c81b839cbf690d96`; merge-SHA digest run
  `32061362433`, artifact `9298285369`.
- Source-verifier refresh PR: #3629, based on fresh `main`
  `967bbbfbebdf3bfcedef35745029b0149aa07321`; merge and complete exact-head evidence are pending.

## Ownership boundary

`rustok-rbac` owns permission decisions, role/permission relation semantics, built-in
role mutation policy, artifact permission admission and assignment, relation integrity,
repair, durable authorization generation storage, and RBAC integration contracts.

`apps/server` owns authenticated adapters, caller transaction orchestration, cache
adapters, fast-path invalidation delivery, worker supervision, and process telemetry.
`rustok-events` owns sealed event contracts. `rustok-outbox` owns durable transactional
transport. `rustok-migrations` owns the immutable published migration prefix and the
canonical dependency-sorted planner; Migration Compatibility permits only appended
migration IDs relative to the base revision.

Claims, presentation roles, caches, projections, and consumers are never permission or
role-assignment authority.

## Current state

`cycle-001/core-rbac` remains `in_progress`.

Active verification task (2026-08-18): `[in_progress]` finish the exact-head source and
migration gates without advancing the cursor. The previously reported mutation
architecture-guard failure is closed: the current guard excludes only the explicit
`#[cfg(test)] mod tests` block and retained exact-head execution passes it. The fresh
exact-head core packet also passed RBAC compilation, focused Rust tests, and module
validate/test. PostgreSQL runtime diagnostics that ended during linking were classified
as runner pressure because `ld` was killed by signal 9 with `CARGO_PROFILE_TEST_DEBUG=0`,
before any RBAC runtime assertion executed.

The migration-order hypothesis from superseded draft #3627 was rejected. Migration
Compatibility correctly proved that the proposed dependency edges would reorder the
already-published base prefix. Direct source inspection found no Forum, Blog, or RBAC
schema/data reference that requires Forum -> Blog -> RBAC execution order: each migration
owns separate tables. PR #3629 therefore carries only stale source-verifier corrections.
The tenant-integrity guard now verifies the current canonical planner together with the
strict base-prefix compatibility comparator and explicitly forbids resurrecting the
retired `APPEND_ONLY_MIGRATION_TAIL` path.

Registered-CLI repair propagation (#2862) is also retained through merge. PR #3590
exact-head RBAC Runtime Evidence run `31885429843` at
`06554657c50535204cdca7f7baf87c7b8d55ba65` passed the source contracts, mutation
architecture guard, PostgreSQL concurrency (#2849), independent-process durable
watchdog (#2853), Redis available/outage/restart (#2856), registered-CLI repair
propagation (#2862), artifact archive, and final gate. The merged revision
`67ed475549598720486188f175fcfce0ab826a3b` then passed push-to-main RBAC Runtime
Evidence run `32004633206`; artifact `9280508296` records the same merge SHA,
`CARGO_PROFILE_TEST_DEBUG=0`, PostgreSQL 16, Redis 7.0.15 where applicable, and Rust
1.97.1. The source-only evidence JSON remains `source_ready_unvalidated`; retained
runtime execution is recorded separately.

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
12. a canonical global migration planner whose published base prefix is protected by
    Migration Compatibility and whose dependency descriptors are reserved for real
    schema/data dependencies;
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

All findings above are source-fixed in `main`; broad execution verification remains
incomplete. The migration-order experiment did not add a product finding because no
cross-module schema/data dependency exists and the immutable-prefix gate correctly
rejected the reorder. The #2849 PostgreSQL concurrency, #2853 durable-watchdog, #2856
Redis restart, and #2862 registered-CLI runtime packets are retained on one same-revision
packet and confirmed after merge, and the event-contract digest is generator-current on
its merged revision. These results do not by themselves verify every source finding or
close the component.

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
- [ ] Execute owner policy, server adapter, Outbox, status, and the remaining architecture tests.

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
- [x] Generate and review the exact-head event digest — PR #3617 merge-SHA run
  `32061362433`; artifact `9298285369`; generated diff 0 bytes.
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
- [x] Preserve the immutable published migration prefix and add no fake cross-module
  dependency solely to recreate historical release order.
- [ ] Execute Migration Compatibility, clean PostgreSQL apply, N-1 upgrade, fixture,
  rollback, and schema-contract checks on one exact head.

### Durable invalidation and recovery

- [x] Reserve monotonic database generation in mutation transactions.
- [x] Treat local/Redis publication as best-effort fast paths.
- [x] Recover missed, stale, duplicate, and gapped generations from checkpoints.
- [x] Export bounded lag, generation, worker, and recovery telemetry.
- [x] Retain source packets for #2849, #2853, #2856, and #2862.
- [x] Execute and retain #2849 PostgreSQL concurrency on the PR #3570 exact head.
- [x] Execute and retain #2853 independent-process durable watchdog recovery on the
  PR #3570 exact head.
- [x] Execute and retain #2856 Redis available/outage/restart recovery on the PR #3579
  exact head.
- [x] Execute and retain #2862 registered-CLI repair propagation — PR #3590 exact-head
  run `31885429843`; merge-SHA run `32004633206`; merge artifact `9280508296`.
- [ ] Execute and retain the incident packet from #2846.

## Remaining priorities

### P0 — exact-head verification

- [x] Generate and review the artifact event digest — PR #3617 merge-SHA run
  `32061362433`; artifact `9298285369`; exact merge SHA and zero generated diff retained.
- [ ] Run formatting, Events/RBAC/Admin/server compilation, focused tests, verifiers,
  and module validate/test on the merged revision. — `in_progress`; PR #3629 refreshes
  the stale source guards while preserving the migration plan.
- [ ] Run SQLite proofs and PostgreSQL clean apply, N-1 upgrade, integrity, locale,
  explicit-scope, concurrency, and rollback scenarios.
- [ ] Resolve every product failure before claiming verification.

### P0 — runtime evidence

- [x] Execute #2849 PostgreSQL concurrency — PR #3570 run `31836046621`, 3/3 pass.
- [x] Execute #2853 independent-process watchdog recovery — PR #3570 run
  `31836046621`, 1/1 pass.
- [x] Execute #2856 Redis available/outage/restart recovery — PR #3579 run
  `31842014975`, 1/1 pass; artifact `9235209675`.
- [x] Execute #2862 registered-CLI repair propagation — PR #3590 exact-head run
  `31885429843`, 1/1 pass; merged run `32004633206`; artifact `9280508296`.
- [x] Retain one same-revision result set within documented bounds for all required
  multi-replica packets — PR #3590 run `31885429843` at
  `06554657c50535204cdca7f7baf87c7b8d55ba65`, then confirm the merged revision with
  push-to-main run `32004633206` at `67ed475549598720486188f175fcfce0ab826a3b`.

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
node scripts/verify/verify-migration-plan-compatibility.mjs --self-test
cargo test -p rustok-server --test rbac_permission_resolver_read_only_guard
cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard
CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -p rustok-server --test rbac_mutation_api_architecture_guard -- --nocapture
CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -p rustok-server --test rbac_postgres_concurrency -- --ignored --nocapture --test-threads=1
CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -p rustok-server --test rbac_two_process_durable_recovery separate_process_replica_recovers_missed_local_publication_from_durable_generation -- --ignored --nocapture
CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -p rustok-server --test rbac_two_process_redis_restart separate_process_redis_fast_path_survives_restart_and_recovers_missed_publication -- --ignored --nocapture
CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -p rustok-cli --test rbac_live_repair_propagation live_cli_system_role_repair_reaches_two_running_replicas_without_restart -- --ignored --nocapture
node scripts/verify/verify-rbac-cli-live-repair-propagation-source.mjs
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
- Event review requires generator-produced digest output; PR #3617 merge-SHA run
  `32061362433` satisfies this gate for merged revision
  `65a63a33d457ed5ff7c592f2c81b839cbf690d96`.
- Relation integrity requires retained SQLite and PostgreSQL execution.
- Migration integrity requires immutable-prefix, clean apply, N-1 upgrade, fixture,
  downgrade identity, failure atomicity, and rollback evidence.
- Durable invalidation requires retained PostgreSQL/watchdog/Redis/CLI evidence; the
  four required runtime packets are retained on one exact head and confirmed after
  #3590 merged.
- FBA remains `boundary_ready`; FFA and `core/rbac` remain `in_progress`.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-08-18`.
- Scope inspected: `fresh main 967bbbfbebdf3bfcedef35745029b0149aa07321; closed migration-order experiment #3627; Forum, Blog and RBAC migration bodies; canonical global migration planner and base-prefix compatibility policy; stale RBAC source guards; PR #3629 exact-head source/runtime/migration checks`.
- Findings: `P0=0, P1=11, P2=1, P3=2. No new RBAC product-semantics defect was found. The migration-order hypothesis is not a P1: Forum, Blog and RBAC migrations operate on separate tables and expose no schema/data dependency, while Migration Compatibility correctly rejects reordering of the published base prefix. The PostgreSQL signal-9 link termination is environment/runner pressure, not an RBAC runtime failure.`
- Fixed in this pass: `Closed unmergeable draft #3627. PR #3629 updates stale source guards to current regression names and handoffs, makes formatting-insensitive checks whitespace-stable where layout is non-semantic, and updates the tenant-integrity verifier to require the canonical dependency sorter plus strict append-only base-prefix comparator while forbidding the retired APPEND_ONLY_MIGRATION_TAIL path. No migration IDs, migration dependency edges, or RBAC runtime authorization semantics changed.`
- Remaining risks or blockers: `PR #3629 exact-head completion and merge; broad workspace format contains unrelated drift outside this RBAC diff unless separately corrected on main; SQLite/PostgreSQL migration and rollback proofs; incident/live negative transport evidence; native operator parity; composed-host/degraded-path evidence; and FFA/FBA promotion remain open.`
- Evidence: `The original mutation architecture guard is green on retained exact-head evidence after excluding only the explicit cfg(test) module. Earlier PostgreSQL diagnostics show ld killed by signal 9 with CARGO_PROFILE_TEST_DEBUG=0 before runtime assertions. Migration Compatibility rejected #3627 because its dependency edges changed the already-published prefix. Source inspection of m20260803_000017_add_forum_topic_canonical_resolution, m20260803_000009_add_blog_comments_audit_canonical_handoff and m20260803_000001_canonicalize_artifact_permissions found no cross-module table/schema dependency. PR #3629 started at 1e3b9caf7d7f73470700a5f0c181b8f12e1c0430 from fresh main 967bbbfbebdf3bfcedef35745029b0149aa07321; RBAC Runtime Evidence run 32130014973 already passed its evidence-source-contract job, and Migration Compatibility run 32130014947 passed infrastructure preflight while the append-only plan job proceeds. Complete PR evidence is not yet claimed.`
- Next action: `finish PR #3629 exact-head source and Migration Compatibility checks, classify any remaining failures against the six-file guard diff, merge only with an unchanged migration plan and no attributable product failure, then continue the remaining SQLite/PostgreSQL clean-apply, N-1 upgrade, integrity, locale, explicit-scope and rollback gates. Keep the cursor on cycle-001/core-rbac.`
- Resume command: `node scripts/verify/verify-rbac-artifact-permission-tenant-integrity.mjs && node scripts/verify/verify-rbac-artifact-permission-outbox.mjs && node scripts/verify/verify-rbac-cli-live-repair-propagation-source.mjs && node scripts/verify/verify-rbac-owner-role-mutation-contract.mjs && node scripts/verify/verify-rbac-explicit-principal-kind.mjs && node scripts/verify/verify-rbac-invalidation-observability.mjs && node scripts/verify/verify-migration-plan-compatibility.mjs --self-test`
