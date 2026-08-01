# Implementation plan for `rustok-rbac`

## Source of truth

This file is the canonical live implementation plan for RBAC. It owns the
current implementation state, completed source phases, remaining priorities and
targeted verification.

- `[x]` means the capability is present in `main` or the current verification
  branch and protected by source-level tests or architecture guards.
- `[ ]` means implementation or verification is still required.
- A source-complete item is not considered compiled or operationally verified
  until the corresponding Rust and live-service checks have passed.
- `docs/modules/implementation-plans-registry.md` contains only the current
  status and nearest priority; it must not duplicate this backlog.
- `docs/verification/rbac-server-modules-verification-plan.md` remains the
  cross-platform verification checklist, not a second RBAC implementation plan.

Last reconciled with `main`: 2026-08-01.

## Current state

`rustok-rbac` is the single tenant-policy owner for permission decisions,
role/permission relations, canonical built-in role repair and authorization
policy. The relation store is the assignment source of truth. No shadow policy
engine or presentation-only role inference may participate in live
authorization.

The ownership boundary is:

- `rustok-rbac` owns permission evaluation, relation persistence primitives,
  transaction-typed repair APIs, relation-integrity and durable invalidation
  generation migrations, and integration contracts;
- `apps/server` owns authenticated host adapters, transaction orchestration,
  request/process cache adapters, distributed invalidation delivery, runtime
  supervision and process-level invalidation telemetry;
- `rustok-api` owns the host-neutral `AuthPrincipalKind` contract and the
  server-only typed request carrier; it does not own RBAC admission policy;
- `rustok-telemetry` owns the bounded Prometheus collectors registered in the
  canonical process registry; it does not own RBAC recovery decisions;
- `RbacRoleAssignmentDbWriter` is an idempotent bootstrap/test persistence
  primitive, while existing-user mutations use explicit transaction-owned or
  committed entry points;
- Redis/local PubSub is a best-effort fast path; the database-backed monotonic
  generation is the recovery source of truth;
- the admin overview remains an intentional native-only module-owned surface.
  A GraphQL/REST management path requires an approved remote or headless
  operator contract.

PR #2747 merged as `75b67f877eb405abe4e6761a16d6b7ece98bc103` and
made principal admission one owner-defined contract across GraphQL, REST and
native RBAC Admin. PR #2837 added bounded invalidation observability, PR #2842
made the access-token resolver the only principal-kind classifier, and PR #2846
added a source-ready incident trace packet. Those source results remain
unvalidated until the exact-head Rust, Node and runtime gates pass.

Draft PR #2843 removes the obsolete resolver mutation/local-only invalidation
bypass. Draft PR #2847 adds a sealed artifact-permission event and publishes it
through the canonical Outbox in the same owner transaction; its repository-
generated contract digest is still absent. The two drafts remain separate and
must be reconciled additively in landing order.

Merged PR #2849 adds a real-PostgreSQL concurrency harness for same-target role
replacement, last-active-super-admin continuity and unique durable generation
allocation. Merged PR #2853 adds an independent observer/mutator process harness
for an intentionally missed local publication and five-second database-watchdog
recovery. Draft PR #2857 adds the complementary Redis available/restart source
harness. None of those harnesses has retained execution evidence yet.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `RbacPermissionDecisionPort` /
  `rbac.permission_decision.v1` in
  `crates/rustok-rbac/contracts/rbac-fba-registry.json`.
- Static and runtime-order evidence:
  `crates/rustok-rbac/contracts/evidence/rbac-contract-test-static-matrix.json`
  and `crates/rustok-rbac/contracts/evidence/rbac-provider-runtime-order-smoke.json`.
- `scripts/verify/verify-rbac-admin-boundary.mjs` and `npm run verify:rbac:fba`
  lock the native-only boundary, provider metadata and authorization order.
- The in-process provider is `RbacPermissionDecisionProvider`: it resolves the
  UUID tenant and authenticated user actor from `PortContext`, then delegates
  to the authoritative `PermissionResolver`. Request claims are not a second
  authorization source.
- Do not promote FBA to `transport_verified` until composed live-host and
  degraded-path evidence is recorded.

## Consolidated implementation phases

### Phase 1. Principal classification and control-plane boundary — source complete

- [x] Distinguish direct sessions, OAuth authorization-code users and OAuth
  client-credentials principals.
- [x] Fail closed for malformed or ambiguous subject/grant combinations.
- [x] Intersect OAuth-granted permissions with token scopes before deriving the
  effective role used by hierarchy checks.
- [x] Prevent service principals and OAuth delegated users from entering direct
  role-assignment or role-metadata control-plane operations.
- [x] Require a direct grant, a valid session and matching tenant context for
  GraphQL, REST and native RBAC control-plane access.
- [x] Keep authorization decisions on typed permissions rather than inferred
  presentation roles.

### Phase 2. Tenant isolation and persistence integrity — source complete

- [x] Require authoritative permission resolution to confirm that the user
  belongs to the requested tenant.
- [x] Filter cached relation loading through the user's actual tenant before
  resolving requested-tenant roles.
- [x] Enforce cross-tenant `user_roles` and `role_permissions` integrity at the
  database boundary.
- [x] Keep authoritative and cached resolvers fail-closed when foreign or
  malformed relations are encountered.
- [x] Align regression fixtures with database-level rejection of cross-tenant
  relation corruption.

### Phase 3. Transactional role and user mutation safety — source complete

- [x] Separate transaction-owned role replacement from the committed public
  mutation entry point.
- [x] Keep the legacy low-level role alias crate-private and confined to new-user
  creation inside a caller-owned transaction.
- [x] Lock the target user before committed role replacement and serialize
  concurrent changes for the same identity.
- [x] Lock/check the built-in super-admin role and reject demotion, disabling or
  deletion of the last active super administrator.
- [x] Treat an exact single-system-role replacement as a no-op without advancing
  the global invalidation generation.
- [x] Repair multiple or malformed assignments even when one assignment already
  matches the requested role.
- [x] Revoke active sessions when a user is disabled, banned, deleted or has a
  password change that requires revocation.
- [x] Reserve invalidation generations only for mutations that can change an
  existing authorization snapshot.
- [ ] Merge draft #2843 so the public permission resolver cannot expose or
  compose role-assignment mutations.
- [ ] Merge and validate draft #2847 so committed artifact-permission changes
  publish one sealed event in the same owner transaction.

### Phase 4. Durable cache invalidation and replica recovery — source complete

- [x] Add the singleton `rbac_invalidation_state` migration with an idempotent
  seed that preserves an already advanced generation.
- [x] Reserve the next permission invalidation generation inside the same
  database transaction as the authorization mutation.
- [x] Publish the committed database generation directly through the local and
  Redis invalidation fast paths; do not maintain a separate Redis counter.
- [x] Treat post-commit PubSub/Redis delivery failures as recoverable and never
  return a false mutation failure after a successful database commit.
- [x] Share durable generation storage through transaction-typed
  `rustok-rbac` APIs so server and operational tools use one implementation.
- [x] Reconcile missed, unverified, duplicate, stale and gapped invalidation
  events through a shared applied-generation checkpoint.
- [x] Clear all permission snapshots when durable recovery proves a missed
  generation or listener lag.
- [x] Supervise the database watchdog and local/Redis/reconciliation workers;
  replace terminal runtimes and restart workers after panic or unexpected exit.
- [x] Allow pre-install startup before the generation table exists and activate
  reconciliation after migrations become visible.

### Phase 5. Canonical role repair and operational tooling — source complete

- [x] Split the public repair surface into a read-only plan API and a
  `DatabaseTransaction`-typed apply API.
- [x] Apply system-role repair and durable generation reservation in one
  transaction in the server host.
- [x] Make `rbac repair-system-roles --apply` commit repair plus generation
  atomically and roll back explicitly on failure.
- [x] Report the committed durable generation from the CLI and remove the old
  restart-required result for successfully applied repairs.
- [x] Invalidate all affected local snapshots after committed repair and use the
  database generation to recover other replicas.

### Phase 6. Source guardrails and regression coverage — source complete, execution pending

- [x] Add unit/regression coverage for generation commit/rollback, idempotent
  migration replay, exact role no-op, malformed multiple assignments and
  last-super-admin rejection.
- [x] Add architecture guards for control-plane ownership, tenant integrity,
  transaction-only mutation APIs, split repair APIs, atomic CLI repair and
  unified invalidation generation.
- [x] Add worker lifecycle guards and panic/restart regression fixtures for the
  durable watchdog and invalidation listeners.
- [x] Keep the FFA/FBA provider registry and native-only admin boundary guarded
  by existing static verification scripts.
- [x] Guard artifact-role permission REST admission so the owner direct-session
  policy precedes `modules:manage`, tenant equality is mandatory and the audit
  actor comes from the authenticated context.
- [x] Guard native RBAC Admin metadata bootstrap with the same owner principal
  policy before `settings:read`; remove the obsolete tenant-only helper.
- [x] Add bounded invalidation metric registration tests, signed lag regression
  coverage and `verify-rbac-invalidation-observability.mjs`.
- [x] Add `verify-rbac-explicit-principal-kind.mjs` and update the Rust/source
  architecture guards so the token resolver is the single classifier.
- [x] Add the source-ready PostgreSQL concurrency harness from PR #2849.
- [x] Add the source-ready independent-process watchdog recovery harness from
  PR #2853.
- [x] Add the RBAC two-process Redis restart source packet in draft PR #2857,
  including real Redis lifecycle, production mutation/listener paths, bounded
  evidence and a fail-closed source verifier.
- [ ] Execute the Rust tests and architecture/source guards in a toolchain-
  enabled environment and fix every compile, formatting or lint failure.

## Remaining work, in priority order

### P0. Compile and targeted verification

- [ ] Run formatting, compilation, Clippy and targeted API/RBAC/Events/server/CLI
  tests on one reconciled exact revision.
- [ ] Generate and review the #2847 event-contract digest through the repository-
  owned generator; never guess or hand-edit it.
- [ ] Record successful module validate/test evidence for `rbac`.
- [ ] Execute the FFA/FBA, tenant-scope, explicit-principal-kind, invalidation-
  observability, incident, resolver, event and recovery source verifiers against
  the same revision.
- [ ] Resolve every failure before claiming the source-complete phases are
  compiled verified.

### P0. Database concurrency and multi-replica recovery evidence

- [x] Add PostgreSQL source evidence for concurrent role replacement,
  last-active-super-admin serialization and unique monotonic generation
  allocation in merged PR #2849.
- [x] Add independent-process durable-watchdog recovery source evidence in
  merged PR #2853.
- [x] Add independent-process Redis available and restart/resubscribe source
  evidence in draft PR #2857.
- [ ] Execute and retain the #2849 PostgreSQL concurrency packet.
- [ ] Execute and retain the #2853 intentionally missed-publication watchdog
  packet.
- [ ] Execute and retain the #2857 Redis available/restart packet.
- [ ] Exercise CLI system-role repair while live replicas are running and prove
  they recover from the committed generation without a restart.
- [ ] Keep the full multi-replica P0 gate open until all retained evidence is on
  one reconciled revision and within documented bounds.

### P1. Invalidation observability and incident operations

- [x] Export metrics for database generation, locally applied generation,
  generation lag, worker running/restart state and recovery/full-clear counts.
- [x] Define alert thresholds for sustained lag, repeated worker restarts,
  generation regression and failed database reads.
- [x] Add an operator runbook covering Redis outage, missed event, generation
  regression, repair execution and verification of effective permissions.
- [x] Add the bounded source-ready policy incident packet in merged PR #2846.
- [ ] Execute and retain the incident packet against production paths.

### P1. Resolver, events and module-owned operator flows

- [ ] Merge and validate draft #2843 without restoring `RoleAssignmentStore` or
  resolver mutation delegation.
- [ ] Generate the digest, merge and validate draft #2847 without dropping the
  read-only resolver correction if #2843 lands first.
- [ ] Define the approved role/permission mutation contract in the owner package,
  including validation, hierarchy, tenant scope, continuity and integration
  event requirements.
- [ ] Route native admin management actions through the module facade without
  adding host-owned relation writes or a parallel `/roles` implementation.
- [ ] Publish and verify expected integration events for committed role and
  permission changes.
- [ ] Decide whether a remote/headless GraphQL or REST management contract is a
  real product requirement; do not add one speculatively.

### P2. Live FBA evidence and promotion

- [ ] Exercise `RbacPermissionDecisionPort` in a composed host with tenant scope,
  representative claims, deadlines, cache hits/misses and degraded behavior.
- [ ] Prove that the module evaluator remains the only decision engine for both
  allowed and denied requests.
- [ ] Record provider/consumer/fallback evidence and promote FBA only when the
  `transport_verified` gate is satisfied.
- [ ] Complete native operator parity evidence before considering FFA
  `parity_verified`.

### P3. Deferred maintenance

- [ ] No standalone P3 is accepted while the P0/P1 execution and ownership gates
  above remain open; record any newly found bounded maintenance issue here.

## Current source packets

### Explicit principal-kind correction

- Status: `merged_source_ready_unvalidated`.
- Merge: PR #2842, commit
  `3a9304aead372b22a5d9069143922d23934e4d7c`.
- The access-token resolver classifies validated claims once and stores the typed
  result on `CurrentUser`; HTTP/native and GraphQL composition only propagates it.
- `RbacControlPlanePrincipal` contains tenant id plus typed kind and cannot infer
  authority from client, grant or session metadata.
- Direct users alone may enter control-plane permission admission.
- Exact-head compilation, focused tests, source verifiers and live negative
  transport requests remain open.

### Read-only resolver correction

- Status: `draft_pr_source_ready_unvalidated`.
- Draft PR: #2843.
- The public permission resolver becomes lookup-only; `RoleAssignmentStore`, the
  server adapter and direct role-mutation delegation are removed atomically.
- No compatibility wrapper or parallel mutation route is retained.
- Exact-head owner/server compile and the focused architecture guard remain open.

### Transactional artifact-permission event correction

- Status: `draft_pr_source_ready_unvalidated`.
- Draft PR: #2847.
- State mutation, idempotency receipt and sealed event publication share one live
  owner transaction through a host-neutral publisher and canonical server Outbox
  adapter.
- Exact replay and no-op confirmation do not emit duplicate or false events.
- The deterministic repository-generated event digest and exact-head execution
  remain mandatory.

### PostgreSQL concurrency evidence

- Status: `merged_source_ready_unvalidated`.
- Merge: PR #2849, commit
  `d1c571b8f859bfafbabb72aa378138a58931fc13`.
- Three ignored isolated-PostgreSQL scenarios use production mutation and durable
  generation APIs.
- The Rust target and source verifier have not been executed.

### Two-process durable-watchdog recovery evidence

- Status: `merged_source_ready_unvalidated`.
- Merge: PR #2853, commit
  `f6c6920c49a74fb5b18f74a15d93ce223a770e02`.
- Independent observer and mutator processes intentionally miss local/Redis
  delivery and require database-watchdog convergence to the authoritative deny.
- PostgreSQL/subprocess execution and the source verifier have not been retained.

### Two-process Redis available/restart evidence

- Status: `draft_pr_source_ready_unvalidated`.
- Draft PR: #2857.
- One observer process and one independent mutator process share only isolated
  PostgreSQL and a real isolated Redis endpoint.
- Available Redis must publish successfully and converge within three seconds.
- A mutation committed while Redis is stopped must record deferred publication,
  retain the stale observer allow, then recover through the supervised subscriber-
  ready callback after restart within five seconds.
- The harness deliberately omits the watchdog because merged #2853 owns that
  separate fallback mechanism.
- Rust, PostgreSQL, Redis, subprocess and source-verifier execution remain absent.

## Verification commands

```bash
cargo fmt --all -- --check
cargo run -p rustok-events --example event_contract_digests -- --write
cargo check -p rustok-api
cargo check -p rustok-api --features server
cargo check -p rustok-events --all-targets
cargo check -p rustok-telemetry
cargo check -p rustok-rbac
cargo check -p rustok-rbac --all-features
cargo check -p rustok-rbac-admin --features ssr
cargo check -p rustok-rbac-cli
cargo check -p rustok-server --lib
cargo test -p rustok-api authenticated_facts_classify_fail_closed
cargo test -p rustok-events --test rbac_artifact_permission_contracts
cargo test -p rustok-server --lib token_claim_classifier_returns_explicit_principal_kinds
cargo test -p rustok-telemetry rbac_invalidation_metrics
cargo test -p rustok-rbac --all-features
cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite
cargo test -p rustok-rbac-admin --features ssr
cargo test -p rustok-migrations --lib rbac_system_role_repair_tests
cargo test -p rustok-rbac-cli
cargo test -p rustok-server --lib rbac_invalidation_generation
cargo test -p rustok-server --lib artifact_permission
cargo test -p rustok-server \
  --test rbac_permission_resolver_read_only_guard \
  --test rbac_artifact_permission_control_plane_guard \
  --test rbac_cache_invalidation_architecture_guard \
  --test rbac_mutation_api_architecture_guard \
  --test rbac_migration_registration_guard \
  --test rbac_startup_invalidation_architecture_guard
cargo test -p rustok-server --test rbac_postgres_concurrency -- --ignored --nocapture
cargo test -p rustok-server --test rbac_two_process_durable_recovery -- --ignored --nocapture --test-threads=1
cargo test -p rustok-server --test rbac_two_process_redis_restart_recovery -- --ignored --nocapture --test-threads=1
cargo test -p rustok-server --test rbac_policy_incident_trace -- --nocapture
cargo clippy -p rustok-rbac --all-features -- -D warnings
cargo clippy -p rustok-rbac-cli -- -D warnings
cargo clippy -p rustok-server --lib -- -D warnings
cargo xtask module validate rbac
cargo xtask module test rbac
node scripts/verify/verify-rbac-artifact-permission-outbox.mjs
node scripts/verify/verify-rbac-explicit-principal-kind.mjs
node scripts/verify/verify-rbac-invalidation-observability.mjs
node scripts/verify/verify-rbac-policy-incident-trace.mjs
node scripts/verify/verify-rbac-postgres-concurrency-source.mjs
node scripts/verify/verify-rbac-two-process-durable-recovery-source.mjs
node scripts/verify/verify-rbac-two-process-redis-restart-source.mjs
node scripts/verify/verify-rbac-admin-tenant-scope.mjs
npm run verify:rbac:admin-boundary
npm run verify:rbac:fba
```

## Completion gates

- Source-complete phases become **compiled verified** only after targeted commands
  pass on one exact revision.
- Durable invalidation becomes **operationally verified** only after retained
  PostgreSQL concurrency, watchdog fallback, Redis available/restart and live CLI
  repair evidence passes.
- Draft #2843 and #2847 must land additively without restoring obsolete mutation
  paths or dropping the transactional event contract.
- FBA remains `boundary_ready` until composed provider/consumer/fallback evidence
  passes.
- FFA remains `in_progress` until approved module-owned management flows and
  native parity evidence are complete.

## Change rules

1. Keep permission evaluation, relation semantics, repair and durable generation
   storage in `rustok-rbac`.
2. Keep authenticated host orchestration, request/process cache adapters and
   runtime worker supervision in `apps/server`; do not duplicate relation writes
   there.
3. Require a caller-owned transaction for low-level authorization mutations and
   invalidate only after successful commit.
4. Treat Redis/local PubSub as a fast path and the database generation as the
   recovery source of truth.
5. Update this plan with every RBAC contract or phase-status change; keep the
   central implementation-plan registry limited to status and nearest priority.
6. Update `rustok-module.toml`, local runtime docs and
   `docs/modules/registry.md` when ownership or FFA/FBA boundary status changes.
7. Do not mark source, compiled, live-service or transport verification complete
   without the corresponding evidence.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-08-01`
- Scope inspected: `explicit principal classification and propagation; tenant-safe GraphQL, REST and native RBAC admission; committed role and artifact-permission mutation ownership; read-only resolver boundary; sealed owner event/outbox publication; durable generation allocation; PostgreSQL concurrency; independent-process watchdog recovery; Redis available publication and restart/resubscribe recovery; invalidation observability and incident evidence`
- Findings: `P0=1, P1=2, P2=0, P3=0`
- Fixed in this pass: `draft PR #2857 adds a source-ready two-process Redis packet using isolated PostgreSQL and a real redis-server. It proves by construction that the production listener receives canonical publication while Redis is available and that a commit made while Redis is stopped remains successful, leaves a stale observer snapshot, and is recovered by the supervised subscriber-ready durable-generation callback after restart. The harness forbids the watchdog and manual cache/generation shortcuts so it stays complementary to merged #2853.`
- Remaining risks or blockers: `#2857 is unexecuted. The #2849 PostgreSQL and #2853 watchdog harnesses are also source-only. Draft #2843 and #2847 remain unmerged; #2847 lacks its repository-generated digest. Same-SHA formatting, API/Events/telemetry/RBAC/Admin/server compilation, focused Rust/Node/module gates, live negative transports, runtime incident evidence, live CLI repair propagation and FFA/FBA management evidence remain absent. Issue #2740 still blocks the known Rust-host path before the server build.`
- Evidence: `source review confirms #2857 uses two independent OS processes, production CacheService/listener and RbacService::replace_user_role_committed, real Redis stop/restart, PUBSUB subscriber readiness, canonical Redis success/failure counters, authoritative permission confirmation and bounded convergence. Its machine-readable packet remains source_ready_unvalidated and explicitly records that Rust, PostgreSQL, Redis, subprocess, verifier, workflow and CI execution did not occur.`
- Next action: `run the #2857 source verifier and ignored integration target, then execute #2853 and #2849 on the same reconciled revision; generate and review the #2847 event digest; reconcile #2843/#2847 additively; finally retain live CLI repair propagation before continuing module-owned management flows`
- Resume command: `cargo fmt --all -- --check && cargo run -p rustok-events --example event_contract_digests -- --write && cargo check -p rustok-events --all-targets && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-server --lib && node scripts/verify/verify-rbac-two-process-redis-restart-source.mjs && cargo test -p rustok-server --test rbac_two_process_redis_restart_recovery -- --ignored --nocapture --test-threads=1 && node scripts/verify/verify-rbac-two-process-durable-recovery-source.mjs && cargo test -p rustok-server --test rbac_two_process_durable_recovery -- --ignored --nocapture --test-threads=1 && node scripts/verify/verify-rbac-postgres-concurrency-source.mjs && cargo test -p rustok-server --test rbac_postgres_concurrency -- --ignored --nocapture && node scripts/verify/verify-rbac-artifact-permission-outbox.mjs && cargo xtask module validate rbac && cargo xtask module test rbac`