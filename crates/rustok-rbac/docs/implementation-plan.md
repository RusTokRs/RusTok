# Implementation plan for `rustok-rbac`

## Source of truth

This file is the canonical live RBAC implementation plan. It owns current source state,
remaining priorities, verification commands, and the periodic release handoff.

- `[x]` means present in `main` or the current verification branch.
- `[ ]` means landing or execution evidence remains required.
- Source-ready is not compiled, migrated, or operationally verified.

Last reconciled with `main`: 2026-08-03.

## Ownership boundary

`rustok-rbac` owns permission decisions, role/permission relation semantics, canonical
built-in role mutation policy, artifact permission admission and assignment, repair,
relation integrity, durable authorization generation storage, and RBAC integration
contracts.

`apps/server` owns authenticated adapters, caller transaction orchestration, cache
adapters, fast-path delivery, worker supervision, and process telemetry. `rustok-events`
owns sealed payloads and schema digests. `rustok-outbox` owns transactional durable
transport. `rustok-migrations` owns the immutable global migration prefix and explicit
release-order append-only tail.

Claims, presentation roles, caches, projections, and event consumers are never
permission or role-assignment authority.

## Current state

Merged PR #2867 owns the canonical built-in user-role mutation contract. Draft PR #2870
supersedes closed PR #2866 and carries the active `cycle-001/core-rbac` verification
slice.

The branch is reconciled with current `main` through technical PRs #2879, #2891, and
#2909. Every reconciliation targeted the verification branch, used a normal merge, and
did not write to `main` or force-push the RBAC history.

Draft #2870 currently provides:

1. read-only `PermissionResolver` and `RuntimePermissionResolver` contracts, with the
   server direct role-assignment store removed;
2. exact role/status replay as no user update, durable generation reservation, session
   revocation, relation mutation, or event;
3. sealed `rbac.artifact_role_permission.assignment_changed` v1 publication in the
   same owner transaction as relation and idempotency receipt;
4. immutable language-neutral artifact permission definitions plus canonical localized
   translations;
5. exact definition ID and admitted scope on grants, receipts, authorization reads, and
   events;
6. tenant-composite role/user foreign keys, exact-scope definition foreign keys, and
   fail-closed `RESTRICT` parent behavior;
7. explicit operator scope selection (`platform` or trusted routed `tenant`) without
   preferred-scope fallback or a caller-supplied second tenant identity;
8. one shared assignable permission-key contract across registration, assignment, and
   event publication;
9. an append-only fifth RBAC migration that upgrades legacy catalog/grant/receipt state
   and fails closed on ambiguous or orphan authority;
10. SQLite upgrade, rollback, scope-shadowing, cross-tenant, parent-integrity,
    localization, immutability, and transactional event regressions;
11. a global migration release-order tail that preserves current `main` Forum and Blog
    cutovers before the RBAC cutover;
12. fail-closed source guards for owner boundaries, exact scope, migration history,
    release order, event identity, locale normalization, and removed execution paths.

Historical drafts #2843, #2847, #2863, and #2866 are superseded and must not be reopened
or merged in parallel with #2870.

## Findings

- `P0=0`
- `P1=9`
  1. localized copy was mixed into authorization identity;
  2. locale keys were not canonical and semantic duplicates were possible;
  3. trigger-only parent checks admitted a concurrent parent update/delete race;
  4. grants could bind tenant authority to an incorrect permission scope;
  5. admitted authorization identity remained mutable;
  6. platform-versus-tenant preferred lookup could shadow or fail to revoke an existing
     grant;
  7. an exact internal UUID transport was unusable because no owner read/list contract
     exposed that generated identity;
  8. registration admitted permission keys that assignment/event contracts rejected;
  9. rewriting already registered migration IDs would leave an upgraded database on the
     legacy schema and violate the immutable migration prefix.
- `P2=1`: nil tenant registration scope.
- `P3=2`: compatibility wording and obsolete broad lint handling.

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
- [ ] Execute focused API/server and live negative transport gates.

### Canonical user-role mutation — source-ready

- [x] Keep canonical role policy in `rustok-rbac`.
- [x] Lock target and continuity facts in the caller transaction.
- [x] Distinguish exact no-op, malformed relation repair, and role replacement.
- [x] Publish relation, generation, and typed event atomically.
- [x] Preserve exact role/status replay as a complete side-effect no-op.
- [x] Confine initial role assignment to caller-owned user creation.
- [ ] Execute owner policy, server adapter, Outbox, status, and architecture tests.

### Resolver ownership — source complete in draft #2870

- [x] Make resolver contracts read-only.
- [x] Remove `RoleAssignmentStore` and the server direct persistence adapter.
- [x] Retain no deprecated mutation alias, compatibility wrapper, or local-only bypass.
- [ ] Land and execute #2870.

### Artifact permission identity, mutation, and events — source-ready

- [x] Add sealed artifact role-permission assignment event v1.
- [x] Include exact immutable definition ID in durable state and events.
- [x] Keep validation, idempotency, no-op detection, mutation, and publication in owner
  code and one transaction.
- [x] Emit no event for exact retry or durable state no-op.
- [x] Roll back mutation and receipt when required publication fails.
- [x] Store language-neutral definitions separately from localized translations.
- [x] Normalize locales through `rustok_api::normalize_locale_tag`, use `VARCHAR(32)`,
  and reject semantic duplicates.
- [x] Reject nil tenant registration scope before opening a transaction.
- [x] Make scope, installation, module, release, and permission key immutable.
- [x] Use explicit platform/tenant selection with tenant identity derived from trusted
  routing context.
- [x] Enforce the same 256-byte, trimmed, control-free, duplicate-free permission-key
  contract at registration and assignment/event boundaries.
- [x] Enforce tenant-composite role/actor and exact definition/scope parents with real
  foreign keys and database checks.
- [x] Preserve current account deletion as deactivation/redaction and unsupported hard
  deletion as fail-closed `RESTRICT`.
- [x] Add SQLite integrity, upgrade, rollback, explicit-scope, and Outbox regressions.
- [x] Add fail-closed source verifiers.
- [ ] Generate and review the exact-head event digest.
- [ ] Execute contract, owner transaction, SQLite, PostgreSQL, adapter, verifier,
  migration compatibility, rollback, and module gates.

### Append-only schema upgrade — source-ready, execution pending

- [x] Restore the two historical RBAC migration bodies to the `main` definitions.
- [x] Register `m20260803_000001_canonicalize_artifact_permissions` as a fifth RBAC
  migration instead of changing an existing migration ID.
- [x] Backfill one immutable definition per exact legacy identity and canonical
  translations without fabricating authority.
- [x] Fail closed when legacy grants/receipts are orphaned or have both platform and
  tenant candidates.
- [x] Prevent lossy rollback when distinct scoped grants collapse to one legacy key.
- [x] Register a PostgreSQL backfill fixture in
  `docs/migrations/backfill-contracts.json`.
- [x] Append current-main Forum and Blog migrations, then the RBAC cutover, to the global
  release-order migration tail.
- [x] Guard exact tail order in the RBAC source verifier.
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

- [x] Reconcile #2870 with current `main` without dropping either side.
- [x] Preserve migration history and append the canonical cutover in release order.
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

- [x] Close superseded draft #2866 and continue only in #2870.
- [x] Define deactivation and fail-closed hard-parent teardown behavior.
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
- Migration integrity requires immutable-prefix, clean-apply, N-1 upgrade, fixture, and
  rollback evidence.
- Durable invalidation requires retained PostgreSQL/watchdog/Redis/CLI evidence.
- FBA remains `boundary_ready`; FFA and `core/rbac` remain `in_progress`.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-08-03`
- Scope inspected: `resolver ownership; canonical user-role mutation; effective status replay; artifact permission identity, locale normalization, explicit scope, database integrity, transactional event publication, append-only schema upgrade, and global migration release order`
- Findings: `P0=0, P1=9, P2=1, P3=2`
- Fixed in this pass: `draft #2870 removes server-owned role mutation paths; preserves exact role/status no-op behavior; introduces canonical immutable artifact permission definitions and owner translations; binds grants, receipts, authorization reads, and events to exact definition identity and admitted scope; removes platform/tenant preferred lookup; exposes usable explicit scope selection; aligns permission-key contracts; adds transactional Outbox publication; restores historical migration bodies; adds a fail-closed append-only cutover migration with SQLite upgrade/rollback and PostgreSQL fixture coverage; and extends the current global migration tail in release order through Forum, Blog, and RBAC. Technical PR #2909 reconciled current main into the verification branch without changing main or force-pushing.`
- Remaining risks or blockers: `#2870 remains draft. Generator-produced event digest, exact-head formatting, compilation, focused tests, source/module gates, Migration Compatibility, PostgreSQL clean apply and N-1 upgrade, concurrency/rollback evidence, live negative transports, retained #2849/#2853/#2856/#2862 execution, incident evidence, native operator parity, and FFA/FBA evidence remain absent. Rust-host issue #2740 remains an infrastructure blocker unless a current exact-head run proves otherwise.`
- Evidence: `source inspection and committed regressions only. The branch contains main through merge commit 39ab7c4d9bae2953781511dc0eeec4dfb546ffb7. The global tail change is isolated to three added migration names. No exact-head command, database, runtime, or production pass is claimed.`
- Next action: `review exact-head Migration Compatibility and compiler failures, generate the event digest through the repository generator, then execute PostgreSQL integrity and runtime packets before considering merge`
- Resume command: `cargo fmt --all -- --check && cargo run -p rustok-events --example event_contract_digests -- --write && cargo check -p rustok-events --all-targets && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-server --lib && cargo test -p rustok-events --test rbac_artifact_permission_contracts && cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite && cargo test -p rustok-rbac --test artifact_permission_tenant_integrity_sqlite && cargo test -p rustok-rbac --test artifact_permission_upgrade_sqlite && cargo test -p rustok-migrations migrator_preserves_append_only_migration_tail && cargo test -p rustok-server --test rbac_permission_resolver_read_only_guard && cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard && cargo test -p rustok-server --test rbac_mutation_api_architecture_guard && node scripts/verify/verify-rbac-owner-role-mutation-contract.mjs && node scripts/verify/verify-rbac-artifact-permission-outbox.mjs && node scripts/verify/verify-rbac-artifact-permission-tenant-integrity.mjs && cargo xtask module validate rbac && cargo xtask module test rbac`
