# Implementation plan for `rustok-rbac`

## Read-only coherent role overview (verification pending; 2026-11-27)

Removed GraphQL read-triggered system-role repair and unified GraphQL/native
role overview on one owner statement. Metadata, grants and localized names
are observed together; empty roles and tenant isolation are preserved.
The regression covers read-only empty listings, unchanged invalidation state,
malformed cross-tenant links and transport parity. Compilation and runtime
verification remain pending. Explicit bootstrap and repair are unchanged.

## GraphQL role-grant durable invalidation (verification pending; 2026-11-02)

The owner GraphQL `update_role` transaction now reserves the existing durable
RBAC invalidation generation with a supplied grant replacement. The server clears
its local permission cache and attempts namespace-wide fan-out after commit.
Metadata-only updates do not reserve a generation; identical supplied grant sets
are still rewritten and advance it. Missing/exhausted generation storage aborts
the edit. PostgreSQL late reservation also rejects a contended table or row
with NOWAIT; reservation retains the typed driver error through the server
write adapter. Standalone committed role replacement retries typed pre-commit
conflicts after explicit rollback and fresh locked facts; no-op and commit
failure never retry. Bootstrap assignment statements now preserve driver errors through the
standalone replacement adapter; begin/commit/rollback diagnostics do not
enter that statement variant. Account deactivation retries only watermark
reservation conflicts with fresh credential admission. Admin creation now
retries typed account insertion/role assignment failures; admin update
retries generation reservation conflicts before event publication. GraphQL role
create/update/delete now retry typed owner SQL failures after explicit rollback
and fresh host authority; update/delete retain the original target UUID.
Host admission/deadline and commit failures are not replayed. An opt-in
PostgreSQL GraphQL fixture holds the real generation row, denies the second
host lock call and verifies rollback of name/grants, unchanged generation and
no notifier call. The adapter is a test double, not real session revocation.
Compilation, execution, actual host credential re-admission and other Auth
retry evidence remain open. An optional legacy host callback still needs its own generation
reconciliation. Added owner/host regressions have not been compiled or executed.

This is a durable watermark plus eventual watchdog reconciliation, not an outbox,
immediate cross-replica or transaction-spanning revocation fence. Cancellation
after commit can prevent the fast path; availability and latency depend on live
watchdogs. PostgreSQL concurrency and real multi-replica recovery remain release
gates. Do not treat cache TTL as a bound on in-flight request snapshots. See the
[decision](../../../../DECISIONS/2026-11-02-rbac-graphql-grant-invalidation.md).


## Cache-resolution consistency continuation (2026-11-02)

The owner now distinguishes a relation-store error from exhaustion of bounded
generation-aware cache-fill retries. A rejected read cannot produce an allowed
or empty successful authorization result. Unit and server-adapter regressions
are added but have not been compiled or executed in this workspace. The error
contract changes `resolve_permissions_with_cache` and the runtime adapter's
conversion bound; external Rust consumers must migrate. Multi-query consistency,
commit-time revocation fences, and multi-replica recovery remain separate gates.
See [the decision](../../../../DECISIONS/2026-11-02-rbac-cache-resolution-consistency.md).

## Coherent static permission reads (2026-11-27)

Cached misses and uncached decisions share one tenant/user-scoped SQL relation
read rather than separately sampling membership and role grants. The relation
store contract now requires a coherent read, including custom implementations;
the old split-load methods are removed. Source regressions cover malformed
cross-tenant grants and membership, cache misses, and caller-transaction
revocation. Compilation, PostgreSQL concurrent execution and distributed cache
recovery remain pending; a statement snapshot is not a revocation fence.

## Transaction-bound management ceiling (2026-11-27)

The control-plane host and GraphQL owner intersect admitted and live effective
grants before evaluating the management OR gate. Independent successful
checks against disjoint grant sets are no longer sufficient. The host rechecks
request scope after lock/read waits. Regressions cover wildcard narrowing,
disjoint management alternatives on create/update/delete, and unchanged state
on denial. Compile and execution evidence remain pending. Artifact purge
authorization and global inter-module fences remain separate work.

## Standalone role lock hierarchy (2026-11-27)

PostgreSQL standalone replacement takes the target user fence, then the
existing target membership, destination and SuperAdmin continuity roles in
canonical UUID order before per-role checks. Bootstrap assignment fences
the user and exact system role before touching shared permission vocabulary;
a late role conflict fails via NOWAIT. Lazy missing-role creation and bulk
repair remain outside a complete cross-owner ordering proof. PostgreSQL
regression evidence is authored but unrun.
The regression holds the lower UUID and checks that a blocked replacement
has not taken the higher UUID, in both former orders. Compilation is pending.

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Transport profile: temporary native-only; native/GraphQL admin parity is in progress.
- Active contract-test profile: `in_process` only. Remote adapters and remote
  profile switching are deferred under the embedded-first platform plan.
- The compatible provider-registry baseline is validated through
  `crates/libs/rustok-fba/contracts/provider-registry-v1.schema.json` before
  RBAC-specific semantic checks run.
- FBA provider contract: `RbacPermissionDecisionPort` / `rbac.permission_decision.v1` in `crates/modules/rustok-rbac/contracts/rbac-fba-registry.json`.
- Static and runtime evidence: `crates/modules/rustok-rbac/contracts/evidence/rbac-contract-test-static-matrix.json` and `crates/modules/rustok-rbac/contracts/evidence/rbac-provider-runtime-order-smoke.json`.
- Evidence: `scripts/verify/verify-rbac-admin-boundary.mjs` locks the admin boundary guardrail.

## Release and Data Rollback Readiness

- Runtime kind: `Core`
- Rollback unit: `Component`
- Data boundary owner: `roles`, `permissions`, `role_permissions`, `user_roles`, `rbac_invalidation_generations`, `rbac_artifact_release_permission_definitions`, `rbac_artifact_release_permission_translations`, `rbac_artifact_permission_installations`, `rbac_artifact_permission_definitions`, `rbac_artifact_permission_translations`, `rbac_artifact_role_permissions`, `rbac_artifact_role_permission_operations`
- Native migrations: 5 canonical migrations (`m20260714_900001`, `m20260714_900002`, `m20260716_000001`, `m20260717_000001`, `m20260803_000001`)
- Supported migration policy: `AdditiveOnly`
- Predecessor standby strategy: `Standby DB + Slot`
- Rollback eligibility: `AutomaticSingleAttempt`
- N/N+1 compatibility: Additive definitions and projections; inert release definitions keyed by `(release_digest, module_slug, permission_key)`; scoped install projects under `(scope, installation_id)`. Unchanged authorization fingerprint retains continuity; fingerprint diff requires explicit operator approval; display text changes are excluded from fingerprint and remain non-breaking.
- External side effects & fences: Monotonic RBAC epoch binding; sealed role and artifact-permission outbox event publication commits in owner transaction; removed grants become dormant; rollback never restores revoked grants.
- Uncertain-outcome recovery: Crash-safe idempotent projections; transaction rollback restores canonical schema.
- Responsible module owner: Platform RBAC Owner

## Current role ownership verification

Role-mutation facts and persisted continuity checks now belong to
`rustok-rbac`. `plan_persisted_user_role_mutation_on` reads the locked target,
effective permissions, exact assignment set, and remaining active administrators
inside the caller transaction. Token permission ceilings and role hierarchy
also use shared owner policy; server guards only extract request evidence.
Ordinary committed replacement delegates to
`replace_persisted_user_role_on`; status-only and removal adapters call
`ensure_user_authority_continuity_on`. The host retains user-field persistence,
commit, outbox composition, post-commit cache delivery, and telemetry.

On 2026-09-13, the focused owner tests pass 6/6 with a native-UUID SQLite
regression for tenant isolation, rejected last-administrator removal/demotion,
exact no-op, malformed-assignment repair, and inactive-target removal. The complete RBAC lib tests pass 73/73 and scoped clippy passes.
Server committed-role tests pass 4/4, covering cache invalidation, exact no-op,
malformed-assignment repair, and last-active-admin rollback. PostgreSQL
concurrency and broader host/Outbox/parity evidence remain to be revalidated.
FFA `in_progress` and FBA `boundary_ready` are unchanged. These transaction
checks do not establish an atomic permission-revocation fence for module purge.

## Source of truth

This is the canonical live RBAC implementation plan. It records source state, open
priorities, required evidence, and the current verification handoff.

- `[x]` means source is present in `main`.
- `[ ]` means execution or promotion evidence remains required.
- Source-ready is not compiled, migrated, transport-verified, or operationally verified.

Last reconciled with `main`: 2026-08-17.

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

## Ownership boundary

`rustok-rbac` owns permission decisions, role/permission relation semantics, built-in
role mutation policy, artifact permission admission and assignment, relation integrity,
repair, durable authorization generation storage, and RBAC integration contracts.

`apps/server` owns authenticated adapters, caller transaction orchestration, cache
adapters, fast-path invalidation delivery, worker supervision, and process telemetry.
`rustok-events` owns sealed event contracts. `rustok-outbox` owns durable transactional
transport. `rustok-migrations` owns the canonical global migration plan. Its Migration
Compatibility workflow exports base and head plans, then accepts only a head that
preserves the complete published base prefix and appends new migrations.

Claims, presentation roles, caches, projections, and consumers are never permission or
role-assignment authority.

The server request-snapshot branches now resolve authenticated, exact tenant/actor
evidence through the module `PermissionResolver` contract and canonical tenant
policy engine for single, any, and all checks. `load_role_user_ids_on` owns role
membership SQL and filters subjects by persisted tenant identity. The owner test
covers malformed cross-tenant membership; the host ceiling test covers policy
engine selection and denied permission restoration. On 2026-09-14, the complete
owner lib test run passes 73/73, including the changed membership fixture.
Fresh host compilation, ceiling-test execution, and scoped clippy remain pending;
the owner test count does not prove those boundaries.

The persisted `SeaOrmRelationPermissionStore` is now owned here and shared by
cached runtime and current decisions. `authorize_current_permission` evaluates
persisted grants through the canonical tenant policy engine without request or
cache snapshots. The server implementation was removed atomically; server
composition uses this reader with its cache adapter. Current policy reads are
not transaction-spanning revocation fences. Modules purge consumes the owner
decision after binding its exact installation/tenant authority context.
FFA/FBA statuses are unchanged; SQLite relation/revocation tests are bounded
evidence and do not prove production fencing or remote execution.
Tenant membership deletion also belongs to `RbacRoleAssignmentDbWriter` and
validates the subject's persisted tenant before mutation. The server persistence
adapter delegates to that writer; transaction commit and post-commit cache
invalidation remain host responsibilities.
The separate server authoritative permission query was also removed.
`resolve_persisted_permissions_on` uses the same owner relation implementation
on a caller-supplied connection or transaction, preserving host serialization
locks without reading cache/request snapshots. The SQLite owner test checks
transaction-local revocation and rollback, without claiming a production fence.

## Current state

`cycle-001/core-rbac` remains `in_progress`.

Active verification task (2026-08-17): `[in_progress]` execute the exact-head formatting,
Events/RBAC/Admin/server compilation, focused-test, source-verifier, and RBAC module
gates, then continue into the remaining migration gates. The artifact event digest is
now retained on the merged revision: PR #3617 merge-SHA run `32061362433` at
`65a63a33d457ed5ff7c592f2c81b839cbf690d96` completed successfully with
`CARGO_PROFILE_TEST_DEBUG=0` and repository-selected Rust 1.97.1. Artifact `9298285369`
records exact-SHA provenance, generator output, generated-file SHA-256
`ca261acbf570615cb1ea180854a2d927aeca385db74917cbb487e0dfd9b730b9`, and a zero-byte
`generated.diff`, proving the committed event-contract digest is generator-current on
the merged revision.

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
12. a global migration planner whose published plan remains an immutable prefix and
    appends `m20260803_000001_canonicalize_artifact_permissions` without rewriting
    earlier migration identities;
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
incomplete. The #2849 PostgreSQL concurrency, #2853 durable-watchdog, #2856 Redis
restart, and #2862 registered-CLI runtime packets are retained on one same-revision
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

### P1. Explicit actor-kind contract

- [x] Add an explicit actor/principal kind
- [x] Preserve fail-closed compatibility
- [x] Add boundary tests proving

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

- Source runtime admission now requires an admitted direct session, using
  the host credential transaction with tenant/user/session and ordered role
  locks. One RBAC query returns exact scope/installation/module/release keys
  for UI projection, execution and audit; policy admission is shared across
  those paths and repeated after idempotency claim waits, including replay.
  OAuth principals fail closed pending a dynamic delegation vocabulary.
  This is an admission check, not a transaction across the sandbox or a
  lock of all module runtime policy inputs. Exact key regressions unrun.

- [x] Add sealed assignment event v1 with exact immutable definition identity.
- [x] Keep validation, idempotency, mutation, and publication in one owner transaction.
- [x] Emit no event for exact retry or durable no-op.
- [x] Source-fence actual artifact grant changes with the existing durable RBAC
  generation in the receipt/event transaction; never heal a negative generation.
  Protect the PostgreSQL actor FK before role locks and the exact immutable
  permission parent before receipt lookup. SQLite already holds its writer fence.
  Regression coverage for generation advance, no-op, rollback and missing state
  is authored, unrun; PostgreSQL contention and complete runtime entitlement
  remain open.
- [x] Roll back mutation and receipt when required publication fails.
- [x] Separate language-neutral definitions from canonical localized translations.
- [x] Normalize locale tags, use safe locale width, and reject semantic duplicates.
- [x] Reject nil tenant scope and mutable admitted identity.
- [x] Use trusted explicit platform/tenant selection.
- [x] Enforce one trimmed, bounded, control-free permission-key contract.
- [x] Enforce tenant-composite role/actor and exact definition/scope parents.
- [x] Add SQLite integrity, upgrade, rollback, explicit-scope, and Outbox regressions.
- [x] Add fail-closed source verifiers.
- [x] Retain the executable explicit-scope owner regression. The SQLite fixture
  creates parallel platform and tenant definitions for the same admitted
  installation/permission key, grants both explicitly, then proves that each
  scoped revoke removes only its own definition and preserves the other grant.
  `verify-rbac-artifact-permission-outbox.mjs` and the focused scenario passed
  on 2026-08-22.
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
- [x] Preserve the complete published migration-plan prefix and append only the RBAC
  cutover; the CI-owned base/head export check is the canonical enforcement point.
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
  and module validate/test on the merged revision. — `in_progress`
- [ ] Run SQLite proofs and PostgreSQL clean apply, N-1 upgrade, integrity, locale,
  explicit-scope, concurrency, and rollback scenarios.
- [ ] Resolve every product failure before claiming verification.

### P0 — runtime evidence

- [x] Execute #2849 PostgreSQL concurrency — PR #3570 run `31836046621`, 3/3 pass.
- [x] Execute #2853 independent-process watchdog recovery — PR #3570 run
  `31836046621`, 1/1 pass.
- [x] Execute #2856 Redis available/outage/restart recovery — PR #3579 run
  `31842014975`, 1/1 pass; artifact `9235209675`.
- [x] Execute #2862 registered-CLI repair propagation — PR #3590 run
  `31885429843`, 1/1 pass; merged run `32004633206`; artifact `9280508296`.
- [x] Retain one same-revision result set within documented bounds for all required
  multi-replica packets — PR #3590 run `31885429843` at
  `06554657c50535204cdca7f7baf87c7b8d55ba65`, then confirm the merged revision with
  push-to-main run `32004633206` at `67ed475549598720486188f175fcfce0ab826a3b`.

### P1. Invalidation observability and incident operations

Status: `in_progress`

- [x] Export metrics for database generation
- [x] Define alert thresholds
- [x] Add an operator runbook
- [ ] Make one policy incident traceable

The canonical watchdog and bounded metric registry are wired; one retained policy
incident trace still requires owner execution evidence.

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

## Verification

- Contract tests cover every public use case.

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
- Last verified at (UTC): `2026-09-06`.
- Scope inspected: `embedded-first FBA contract metadata, RBAC permission-decision port, and current local/central readiness records`.
- Findings: `source baseline remains P0=0, P1=11, P2=1, P3=2 and is source-fixed. The RBAC contract matrix incorrectly retained a deferred remote profile although remote adapters and profile switching are explicitly deferred. No new RBAC product-semantics defect was found.`
- Fixed in this pass: `removed the deferred remote placeholder from the RBAC registry and static matrix. The sole active contract-test profile is in_process; embedded fallback metadata, port behavior, and boundary status are unchanged.`
- Remaining risks or blockers: `broad exact-head formatting, Events/RBAC/Admin/server compilation, focused tests, source verifiers and module gates; SQLite/PostgreSQL migration and rollback proofs; incident/live negative transport evidence; native operator parity; composed-host/degraded-path evidence; and FFA/FBA promotion remain open.`
- Evidence: `node scripts/verify/verify-rbac-fba.mjs, node scripts/verify/verify-owner-fba-runtime-order.mjs, and node scripts/verify/verify-rbac-admin-boundary.mjs passed after the registry correction. Retained CI evidence remains recorded above; no broad local Cargo pass is claimed.`
- Next action: `execute and retain the exact-head formatting, Events/RBAC/Admin/server compilation, focused tests, source verifiers, and cargo xtask module validate/test rbac gates; classify every failure against the RBAC diff before moving into migration compatibility and PostgreSQL rollback evidence. Keep the verification cursor on cycle-001/core-rbac.`
- Resume command: `cargo fmt --all -- --check`

- Global lock hierarchy continuation: migration 000012 protects the RBAC
  watermark from ordinary SQL reset/removal/replacement; user-admin response
  reads precede reservation. Generation-bearing outbox and final credential
  deadline remain in the transaction tail. Authored SQLite and PostgreSQL
  regressions, including actual generation contention. Compilation, migration
  execution, direct relation invalidation coupling and global lock evidence
  remain pending.

- Direct relation mutation continuation: validate every persisted cache-hit
  candidate against an exact current grant set, then recheck local token.
  This closes stale unscoped cache use on direct SQL or missed notifications
  without introducing an early global generation lock. A hit now requires a
  read query. Source regressions authored; compile, PostgreSQL query parity
  and performance evidence remain pending. Raw-DML generation coupling,
  request-snapshot revocation and relation incarnation continuity remain open.
  Added an opt-in PostgreSQL exact-set regression covering both directions of
  comparison, tenant isolation and FK cascades, and extended the SQLite
  regression for raw static-grant deletion/reinsertion. Tests remain unrun.

- Static authority UUID continuity: migration 000013 preserves user, role and
  permission birth witnesses without a global row lock; same-ID replacement,
  deletion/recreation and direct witness reset fail closed. SQLite alternate
  UUID encodings share one witness. Existing identity history before migration,
  different-ID natural-key replacement, other credential/tenant identities and
  revision-based cache optimization remain open. Source regressions authored;
  migration and PostgreSQL contention evidence unrun.
  Additional regression exercises tenant FK cascades, rollback and rejection
  of UUID reuse with the same revision in another tenant. The PostgreSQL
  contention assertion requires the exact witness-busy error. Owner migration
  registration is guarded; execution remains pending.

- GraphQL role locator integrity: update and delete revalidate the exact tenant,
  UUID and slug under the role lock; stale locators cannot bypass break-glass
  protection. SQLite transport regressions authored, unrun.

- Artifact catalog lock closure: admission orders permission and normalized
  locale keys; projection orders persisted keys. Migration 000016 covers
  installation and translation FK parent waits and relation admission for
  existing artifact grant/receipt guards. Opt-in PostgreSQL regression covers
  parent row/table contention, retry, wrong tuple and metadata-only update.
  Compilation and runtime evidence remain pending; arbitrary bulk DML,
  generation coupling for direct relation writes and cross-owner edges remain open.
  Extended the 000016 PostgreSQL regression to install the actual 000006
  predecessor and exercise both artifact grant and operation receipt guards
  against all three FK parents (row and relation contention), exact tenant
  rejection and reentrant owner locks. No migration SQL change; unrun.

- Artifact grant generation coupling: migration 000017 advances the existing
  watermark on actual grant row changes, including direct SQL. PostgreSQL
  uses deferred constraint triggers and NOWAIT for late watermark/role locks;
  SQLite uses its serialized writer. Remove explicit duplicate owner
  reservation; cutover advances once. Regression sources cover direct SQL,
  rollback, no-ops, exhaustion and PostgreSQL commit/role contention. Drain
  old writers. Static direct relation coupling, bulk ordering, retry and
  execution evidence remain open.

- Request snapshot temporal admission: matching credential-bearing scopes
  terminally expire on the original deadline or an unhealthy Auth local clock
  sample, including captured artifact scopes and GraphQL root admission.
  Source regressions authored, unrun; cross-task, cross-replica and durable
  direct-SQL revocation remain open.

- Process-local request snapshot revocation: bind JWT admission and HTTP/WS
  scopes to the existing cache epoch, reject pre-entry and cross-task changes,
  and retain the original token for captured leases. Authenticated stripe
  collisions are conservative denials. Source regressions unrun; durable
  revocation and unscoped snapshot consumers remain open.

- Durable static request authority: retain the admitted membership/role grant
  revision in CurrentUser and scope, validate at scope-aware async service and
  GraphQL root boundaries, compare it on WS revalidation. Source regression
  covers membership and raw grant ABA, missing evidence and new admission;
  unrun. Synchronous consumers, service OAuth, module/credential revocation,
  commit-time fencing and cross-replica runtime evidence remain open.

- Fallible scope lookup and synchronous REST admission: invalid matching
  scopes cannot be interpreted as absent by service/resolver fallback;
  CurrentUser single/any/all gates check their deadline and local epoch
  and any matching scope ceiling. Source regressions authored, unrun.
  Other immutable AuthContext/SecurityContext consumers and durable
  credential, module-policy and nested field checks remain open.

- GraphQL immutable context binding: require the original HTTP/WS admission
  marker to match live task-local scope, AuthContext ceiling and credential
  facts. Nested query fields check local validity; selected mutation
  payloads retain committed receipt serialization. Source regressions cover
  absent/substituted authority, widened context, aliases and operation
  selection. Unrun. Fresh protected reads nested inside mutation payloads
  and durable nested read authority remain open for unmigrated consumers.

- Fresh nested read authority: require host provider and original scope
  binding, before/after durable static revision checks for Blog moderation
  and User live role/can/tenant name/custom fields. Blog preserves the
  actor ceiling and requires both moderate and list grants. Owner-issued
  User mutation role/tenant name are receipt data, not new reads. Source
  regressions authored, unrun. Other protected nested fields, full receipt
  materialization and continuous credential/module authority remain open.

- Durable request credential observations: matching authenticated scope
  rechecks tenant, human account/recovery revision, direct session or exact
  OAuth app/access registration/consent lineage; service app grants must
  cover original scoped permissions. Terminal failure without grant fallback;
  shared registration predicate with JWT admission. Tests cover revoked
  session, service grant loss/revocation and delegated consent replacement.
  Source tests unrun. Continuous module policy, signing-key authority,
  synchronous-only consumers and atomic credential/effect fencing remain open.

- Revision-bound permission cache: bracket production fills with durable
  revision reads and recheck the local epoch after publication. Bound hits
  compare exact grants and original revision in one statement. Legacy entries
  retain exact validation; missing/corrupt evidence and storage failure do not
  authorize stale grants. Source regression covers raw grant ABA and mutation
  during publication, plus transient subject visibility between absent
  revision observations and bounded rejection of repeated contradictions.
  Tests and performance evidence pending; revision-only
  optimization requires a trustworthy database incarnation/restore contract.

### Tenant-bound role labels and administrator continuity

Persisted role membership reads join the subject's tenant and the role's
tenant, excluding malformed cross-tenant links. The persisted role planner
distinguishes effective hierarchy rank from actual canonical system
`super_admin` membership: only the latter participates in last-active-admin
continuity. Custom-role replacement and status/removal checks use the same
membership predicate. Existing hierarchy and permission-ceiling checks remain.
Source regressions cover a malformed foreign membership and the distinction
between inferred rank and canonical membership; compilation and execution
remain pending.

## Artifact operation receipt contention (source only)

PostgreSQL owner assignment acquires a nonwaiting transaction-scoped advisory
fence over the exact tenant/idempotency-key namespace before receipt lookup or
insert; different actors or target roles do not change that key. Contention
returns a distinct HTTP 409 in-progress response after rollback. Definition
row and relation admission and receipt-table admission are also nonwaiting.
The unique receipt constraint is still authoritative; raw SQL writers that
bypass the owner advisory fence, receipt retention after destructive cleanup,
commit outcome recovery and full cross-owner ordering remain open. SQLite
continues to serialize writers. The opt-in PostgreSQL fence test and REST
mapping regression are authored, not executed.

### Administrator continuity under snapshot isolation (source only)

The continuity role is now a write fence, not just a row lock: the canonical
super_admin row receives an exact no-op update before counting remaining
active members. PostgreSQL REPEATABLE READ/SERIALIZABLE transactions whose
snapshot predates a committed continuity check cannot both authorize removals
from the old active set; serialization failure requires whole-transaction
retry. The no-op does not change grant_revision or the role's timestamp.
SQLite retains its existing writer fence. Persisted role planners and the
standalone status/removal continuity boundary reserve the existing target
membership, destination and continuity roles in canonical UUID order before
reading hierarchy or membership facts. Missing custom destination fails
closed. Callers must retain the transaction through the effect. Raw user
status DML bypassing the owner, privileged database maintenance, and complete
cross-owner lock ordering are not covered by this change. PostgreSQL
regression source authored; compilation and execution remain pending.

## Continuous static module-policy observation (2026-11-27)

JWT admission retains the tenant-bound effective-policy content identity,
composition revision and availability used to filter the original grants.
The final admission compares that evidence with the independently resolved
current owner policy. Authenticated scope-aware async checks re-observe the
same evidence through the uncached Modules owner, after live credential and
static RBAC observations. A difference (even if the grant set is unchanged),
missing composition or read failure terminally invalidates the scope and its
captured clones; it never refreshes the original ceiling. WebSocket admission
compares the same evidence through the existing scope equality. Uncomposed
hosts and trusted grant-only scopes retain their existing contracts.

The recheck does not initialize missing composition, take writer fences or
introduce new migrations. This is an optimistic observation, not an atomic
revocation/effect fence. The effective-policy digest may repeat after a
lifecycle ABA; the durable tenant lifecycle sequence and raw-DML continuity
are separate work. Sync-only and unscoped consumers are not covered. Source
regression covers policy drift without a notification and terminal invalidation
after restoration; compilation and execution remain pending.

### Durable lifecycle cursor in request module authority (2026-11-27)

The host retains the Modules owner's exact optional `module.lifecycle`
cursor state (digest and monotonic sequence), plus the active composition
revision and verified manifest hash. An uncached policy observation brackets
its evaluation with composition and cursor reads; drift or unreadable evidence
fails closed. The read-only cursor port sets transaction-local tenant scope,
never inserts a cursor or locks a writer row, and distinguishes missing from
issued zero state. The original authenticated scope, including WebSocket lease
and captured clones, compares the complete owner observation. A canonical
lifecycle transition/resync ABA is detected even if the final policy digest and
grants equal those admitted earlier. Conservative invalidation is intentional
for unrelated tenant lifecycle transitions and first cursor creation.

This is not a transaction-spanning revocation fence. Raw SQL bypass of owner
transition writers or deletion/recreation of owner generations is not covered
by the sequence contract; database-level enforcement and PostgreSQL contention
verification remain open. SQLite source regressions cover read-only absence,
rollback and cursor ABA without notification; execution is pending.

PostgreSQL source regression `lifecycle_cursor_observation_is_read_only_and_detects_committed_aba` uses independent pools and an uncommitted writer row lock. It asserts that the reader observes the previous committed state without blocking, rollback preserves the admitted state, a committed digest ABA advances the sequence, and another tenant remains isolated. The ignored integration test is authored but unrun.

- Permanent artifact operation key witnesses (m20261127_000206_preserve_artifact_operation_witnesses): retain exact tenant/key
  across receipt cleanup; backfill existing receipts, guard raw DML, reject
  retired-key replay, and retain the existing live receipt contract. Source
  regressions cover SQLite cleanup, UUID aliases, conflict algorithms and
  rollback; PostgreSQL execution and full uncertain-commit recovery remain open.

### Lifecycle cursor continuity integrity

The tenant `module.lifecycle` cursor is durable security authority, not
disposable consumer progress. The Modules migration prevents ordinary DML
from deleting, replacing, moving or resetting that cursor, including SQLite
implicit REPLACE deletes through rowid or alternate UUID representations.
New lifecycle cursors start at zero with no digest; existing legacy
sequence-zero digests are preserved. An advance requires the exact next
integer sequence and a different valid digest. Exact no-op writes remain
legal. PostgreSQL TRUNCATE is rejected for the cursor table because it would
bypass per-row history; other consumers retain normal row mutation semantics.
Install with old writers drained, validate existing rows and reject
inconsistent identity/sequence evidence rather than reconstruct history.
Lifecycle cursors persist even after tenant removal; do not reuse tenant IDs.
The owner cursor remains the source of truth. No new generation table.
This does not couple direct tenant_modules edits to the cursor or prove that
a manually supplied successor digest describes the current policy; the
existing content observation remains necessary. Privileged DDL, restores
and pre-cutover lost history are not covered. Source regressions unrun.

  PostgreSQL source regression now checks the raw receipt INSERT/DELETE key
  fence, cleanup, witness immutability/TRUNCATE and rollback reuse; owner
  witness reads acquire a nonwaiting relation fence. Unrun.

### Static intent physical change witness (2026-11-27)

Migration `m20261127_000105_static_intent_authority_generation` installs a permanent per-tenant generation maintained transactionally by `tenant_modules` DML triggers. It complements, rather than changes, the lifecycle event cursor: INSERT/DELETE and changed enablement advance the witness; settings-only updates do not. SQLite additionally advances on override ID changes and rejects cross-tenant implicit REPLACE, including rowid conflicts. PostgreSQL forbids TRUNCATE of intent or witness history; both backends reject witness reset, deletion, identity replacement and overflow. Zero is the cutover baseline, not reconstructed history. No tenant FK removes witness history on deletion. RBAC uncached observations bracket policy evaluation with this owner-issued generation in addition to the lifecycle cursor and composition identity. A completed raw-DML ABA invalidates admitted scope and WebSocket leases, without event delivery. The witness is not a policy engine or event producer. DDL/privileged trigger bypass, database restore, composition raw-DML ABA and commit-time authorization fences remain distinct boundaries. Deployment must apply migrations before the new auth reader. Regression sources authored, not executed.

PostgreSQL integration regression `static_intent_generation_observes_committed_sql_aba_and_rejects_history_reset` covers the registered migration, two independent pools, settings-only updates, reader visibility during an uncommitted write, rollback, committed direct-SQL ABA, tenant isolation and history reset/TRUNCATE denial. It is authored and ignored pending maintainer execution. The deployed application-role RLS/privilege matrix also requires verification; migrations run before auth reader cutover.
