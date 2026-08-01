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
- `rustok-telemetry` owns bounded collectors in the canonical registry; it does
  not own RBAC recovery decisions;
- `rustok-rbac-cli` owns the operational command adapter for system-role repair;
  it applies the owner repair plus durable generation in one transaction;
- `RbacRoleAssignmentDbWriter` is an idempotent bootstrap/test persistence
  primitive, while existing-user mutations use explicit transaction-owned or
  committed entry points;
- Redis/local PubSub is a best-effort fast path; the database-backed monotonic
  generation is the recovery source of truth;
- the admin overview remains an intentional native-only module-owned surface.

PR #2747 merged the shared control-plane policy. PR #2837 added bounded
invalidation observability, PR #2842 made the access-token resolver the only
principal-kind classifier, and PR #2846 added a source-ready incident packet.
Those source results remain unvalidated until exact-head Rust, Node and runtime
gates pass.

Draft PR #2843 removes the obsolete resolver mutation/local-only invalidation
bypass. Draft PR #2847 adds a sealed artifact-permission event and publishes it
through the canonical Outbox in the same owner transaction; its repository-
generated contract digest is still absent. The two drafts remain separate and
must be reconciled additively in landing order.

Merged PR #2849 adds real-PostgreSQL concurrency source coverage for same-target
role replacement, last-active-super-admin continuity and unique durable
generation allocation. Merged PR #2853 adds independent-process watchdog
recovery after intentionally missed delivery. Merged PR #2856 adds real Redis
available/outage/restart coverage and excludes both the watchdog and the
thirty-second periodic reconciliation from satisfying its bounded assertions.
Merged PR #2862 adds the remaining operational CLI path through the full
registered `rustok-cli` command runner: two live observers must recover from
`repair-system-roles --apply` through the committed database generation without
Redis or restart. None of these packets has retained execution evidence yet.
Historical draft #2859 was closed without merge after #2862 superseded its
direct-provider harness.

Draft PR #2863 corrects the live Auth admin and GraphQL role-assignment
orchestration so exact canonical role or status replay is an effective no-op.
The caller-owned RBAC helper still repairs multiple or malformed assignments,
while durable generation reservation, fan-out and disabled-status session
revocation now depend on an actual locked-state change.

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
- The in-process provider resolves tenant and actor from `PortContext`, then
  delegates to the authoritative `PermissionResolver`; request claims are not a
  second decision source.
- Do not promote FBA until composed live-host and degraded-path evidence exists.

## Consolidated implementation phases

### Phase 1. Principal classification and control-plane boundary — source complete

- [x] Distinguish direct sessions, OAuth authorization-code users and OAuth
  client-credentials principals.
- [x] Fail closed for malformed or ambiguous subject/grant combinations.
- [x] Intersect OAuth-granted permissions with token scopes before deriving the
  effective role used by hierarchy checks.
- [x] Prevent service principals and delegated users from entering direct role
  or role-metadata control-plane operations.
- [x] Require a direct principal and matching tenant context across GraphQL,
  REST and native RBAC control-plane access.
- [x] Keep authorization decisions on typed permissions.

### Phase 2. Tenant isolation and persistence integrity — source complete

- [x] Require authoritative resolution to confirm user/tenant membership.
- [x] Filter cached relation loading through the actual user tenant.
- [x] Enforce cross-tenant `user_roles` and `role_permissions` integrity at the
  database boundary.
- [x] Keep authoritative and cached resolvers fail-closed for malformed or
  foreign relations.
- [x] Align regression fixtures with database-level rejection.

### Phase 3. Transactional role and user mutation safety — source complete

- [x] Separate transaction-owned role replacement from the committed public
  mutation entry point.
- [x] Keep the legacy low-level alias crate-private and confined to new-user
  creation inside a caller-owned transaction.
- [x] Lock the target user before committed role replacement.
- [x] Lock/check the built-in super-admin role and reject removal of the last
  active super administrator.
- [x] Treat exact single-role replacement as a no-op without generation advance.
- [x] Repair multiple or malformed assignments even when one matches.
- [x] Revoke sessions for disabling, banning, deleting or password revocation.
- [x] Reserve invalidation generations only for effective changes.
- [x] Preserve exact role/status replay as an Auth admin no-op while retaining
  malformed relation repair in draft PR #2863.
- [ ] Merge draft #2843 so the public permission resolver cannot expose or
  compose role-assignment mutations.
- [ ] Merge and validate draft #2847 so committed artifact-permission changes
  publish one sealed event in the owner transaction.
- [ ] Merge and validate draft #2863 so live Auth admin and GraphQL role replay
  cannot create false generation advances or redundant session revocation.

### Phase 4. Durable cache invalidation and replica recovery — source complete

- [x] Add the singleton durable generation migration with idempotent seed.
- [x] Reserve the generation in the authorization mutation transaction.
- [x] Publish the committed database generation through local and Redis paths.
- [x] Treat post-commit delivery failure as recoverable rather than a false
  mutation failure.
- [x] Share generation storage through typed owner APIs.
- [x] Reconcile missed, duplicate, stale and gapped observations through one
  applied-generation checkpoint.
- [x] Clear all permission snapshots after proven missed generation or lag.
- [x] Supervise watchdog, Redis/local and reconciliation workers.
- [x] Permit pre-install startup and activate after migrations appear.

### Phase 5. Canonical role repair and operational tooling — source complete

- [x] Split repair into read-only planning and transaction-typed apply APIs.
- [x] Apply repair plus durable generation in one transaction.
- [x] Make `rbac repair-system-roles --apply` roll back explicitly on failure.
- [x] Report the committed generation and remove restart-required success output.
- [x] Keep other replicas recovering from the database generation.

### Phase 6. Source guardrails and regression coverage — source complete, execution pending

- [x] Add unit/regression coverage for generation commit/rollback, migration
  replay, exact role no-op, malformed assignments and continuity rejection.
- [x] Add architecture guards for control-plane ownership, tenant integrity,
  transaction-only mutation APIs, split repair and unified generation.
- [x] Add worker lifecycle and restart guards.
- [x] Guard REST and native control-plane admission ordering and trusted actor.
- [x] Add bounded invalidation metrics and source verifier.
- [x] Add explicit principal-kind architecture guards.
- [x] Add the PostgreSQL concurrency source packet in merged PR #2849.
- [x] Add independent-process watchdog recovery in merged PR #2853.
- [x] Add real Redis fast-path/outage/restart recovery in merged PR #2856.
- [x] Add the full CLI repair propagation source packet in merged PR #2862.
- [x] Add transaction result and Auth admin effective-no-op guards in draft PR
  #2863.
- [ ] Execute the Rust tests and architecture/source guards on one revision and
  fix every compile, format or lint failure.

## Remaining work, in priority order

### P0. Compile and targeted verification

- [ ] Run formatting, compilation, Clippy and targeted API/RBAC/Events/server/CLI
  tests on one reconciled exact revision.
- [ ] Generate and review the #2847 event-contract digest through the repository-
  owned generator; never guess or hand-edit it.
- [ ] Record successful module validate/test evidence for `rbac`.
- [ ] Execute FFA/FBA, tenant-scope, explicit-principal-kind, invalidation,
  incident, resolver, event and recovery verifiers against the same revision.
- [ ] Resolve every failure before claiming compiled verification.

### P0. Database concurrency and multi-replica recovery evidence

- [x] Add PostgreSQL concurrency source evidence in merged PR #2849.
- [x] Add independent-process watchdog recovery source evidence in merged PR
  #2853.
- [x] Add independent-process Redis available/outage/restart source evidence in
  merged PR #2856.
- [x] Add full live CLI system-role repair propagation source evidence in merged
  PR #2862.
- [ ] Execute and retain the #2849 PostgreSQL packet.
- [ ] Execute and retain the #2853 watchdog packet.
- [ ] Execute and retain the #2856 Redis packet.
- [ ] Execute and retain the #2862 CLI repair packet.
- [ ] Keep the full multi-replica P0 gate open until all packets pass on one
  reconciled revision within documented bounds.

### P1. Invalidation observability and incident operations

- [x] Export bounded database/applied generation, lag, worker and recovery metrics.
- [x] Define alert thresholds and recovery runbook.
- [x] Add the bounded source-ready incident packet in merged PR #2846.
- [ ] Execute and retain the incident packet against production paths.

### P1. Resolver, events and module-owned operator flows

- [ ] Merge and validate draft #2843 without restoring `RoleAssignmentStore`.
- [ ] Generate the digest, merge and validate draft #2847 without dropping the
  read-only resolver correction if #2843 lands first.
- [ ] Merge and validate draft #2863 without losing exact-role repair for
  multiple or malformed assignments.
- [ ] Define the approved owner role/permission mutation contract, including
  validation, hierarchy, tenant scope, continuity and events.
- [ ] Route native admin management through the module facade without host-owned
  relation writes or a parallel `/roles` implementation.
- [ ] Publish and verify expected integration events for committed role changes.
- [ ] Decide whether remote/headless management is a real product requirement.

### P2. Live FBA evidence and promotion

- [ ] Exercise `RbacPermissionDecisionPort` in a composed host with tenant scope,
  representative claims, deadlines, cache hits/misses and degraded behavior.
- [ ] Prove the module evaluator remains the only decision engine.
- [ ] Record provider/consumer/fallback evidence before promotion.
- [ ] Complete native operator parity evidence before FFA promotion.

### P3. Deferred maintenance

- [ ] Record newly found bounded maintenance here; no standalone P3 can displace
  the open P0/P1 gates.

## Current source packets

### Explicit principal-kind correction

- Status: `merged_source_ready_unvalidated`.
- Merge: PR #2842, commit
  `3a9304aead372b22a5d9069143922d23934e4d7c`.
- Exact-head compilation, focused tests, verifiers and live negative transport
  requests remain open.

### Read-only resolver correction

- Status: `draft_pr_source_ready_unvalidated`.
- Draft PR: #2843.
- Resolver mutation methods, `RoleAssignmentStore` and server delegation are
  removed without a compatibility path.

### Transactional artifact-permission event correction

- Status: `draft_pr_source_ready_unvalidated`.
- Draft PR: #2847.
- Mutation, receipt and sealed event share one owner transaction.
- Repository-generated digest and execution remain mandatory.

### Auth admin effective no-op correction

- Status: `draft_pr_source_ready_unvalidated`.
- Draft PR: #2863.
- Exact canonical role or status replay does not reserve a new durable
  generation, fan out invalidation or redundantly revoke sessions.
- A matching role among multiple or malformed assignments remains a real repair
  and returns an effective change to the transaction owner.
- Rust regression and architecture guard execution remains absent.

### PostgreSQL concurrency evidence

- Status: `merged_source_ready_unvalidated`.
- Merge: PR #2849, commit
  `d1c571b8f859bfafbabb72aa378138a58931fc13`.
- The ignored Rust target and verifier have not been executed.

### Two-process durable-watchdog recovery evidence

- Status: `merged_source_ready_unvalidated`.
- Merge: PR #2853, commit
  `f6c6920c49a74fb5b18f74a15d93ce223a770e02`.
- Independent observer/mutator processes intentionally miss delivery and require
  watchdog convergence. Execution is absent.

### Two-process Redis recovery evidence

- Status: `merged_source_ready_unvalidated`.
- Merge: PR #2856, commit
  `33bbca155cb816da14d4d5fa966936ae396d58ef`.
- A long-lived observer plus two mutators use real Redis, bounded fast-path and
  resubscribe-ready recovery while excluding watchdog/periodic reconciliation.
- PostgreSQL, Redis, subprocess and verifier execution are absent.

### Live CLI repair propagation evidence

- Status: `merged_source_ready_unvalidated`.
- Merge: PR #2862, commit
  `f23ab50984c2e1196cbe899705a5b32c75798144`.
- Two independent server observer processes warm stale Manager authorization and
  run the canonical cache listener plus durable-generation watchdog.
- A third process executes the full registered `rustok-cli rbac
  repair-system-roles --apply` path without Redis.
- The source packet requires generation one, two affected users, preserved
  observer process identity, `generation_advanced` recovery/full-clear evidence,
  and cached plus authoritative deny without restart.
- Rust, PostgreSQL, subprocess and verifier execution remain absent.

## Verification commands

```bash
cargo fmt --all -- --check
cargo run -p rustok-events --example event_contract_digests -- --write
cargo check -p rustok-api
cargo check -p rustok-api --features server
cargo check -p rustok-events --all-targets
cargo check -p rustok-telemetry
cargo check -p rustok-rbac --all-features
cargo check -p rustok-rbac-admin --features ssr
cargo check -p rustok-rbac-cli
cargo check -p rustok-server --lib
cargo test -p rustok-api authenticated_facts_classify_fail_closed
cargo test -p rustok-events --test rbac_artifact_permission_contracts
cargo test -p rustok-rbac --all-features
cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite
cargo test -p rustok-rbac-admin --features ssr
cargo test -p rustok-rbac-cli
cargo test -p rustok-server transaction_role_replacement_reports_exact_noop
cargo test -p rustok-server transaction_role_replacement_repairs_multiple_assignments
cargo test -p rustok-server status_effective_change_ignores_exact_replay
cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard
cargo test -p rustok-server --test rbac_mutation_api_architecture_guard
cargo test -p rustok-server --test rbac_postgres_concurrency -- --ignored --nocapture
cargo test -p rustok-server --test rbac_two_process_durable_recovery -- --ignored --nocapture --test-threads=1
cargo test -p rustok-server --test rbac_two_process_redis_restart separate_process_redis_fast_path_survives_restart_and_recovers_missed_publication -- --ignored --nocapture
cargo test -p rustok-cli --test rbac_live_repair_propagation live_cli_system_role_repair_reaches_two_running_replicas_without_restart -- --ignored --nocapture
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
node scripts/verify/verify-rbac-cli-live-repair-propagation-source.mjs
node scripts/verify/verify-rbac-admin-tenant-scope.mjs
npm run verify:rbac:admin-boundary
npm run verify:rbac:fba
```

## Completion gates

- Source-complete phases become compiled verified only after targeted commands
  pass on one exact revision.
- Durable invalidation becomes operationally verified only after retained
  PostgreSQL concurrency, watchdog, Redis and CLI repair packets pass.
- Draft #2843, #2847 and #2863 must land additively without restoring obsolete
  paths, dropping the transactional event contract or reintroducing false
  generation advances for exact admin replay.
- FBA remains `boundary_ready` until composed provider/consumer/fallback evidence
  passes.
- FFA remains `in_progress` until approved module-owned management flows and
  native parity evidence are complete.

## Change rules

1. Keep permission evaluation, relation semantics, repair and durable generation
   storage in `rustok-rbac`.
2. Keep authenticated host orchestration, cache adapters and worker supervision in
   `apps/server`; do not duplicate relation writes there.
3. Require caller-owned transactions for low-level authorization mutations and
   invalidate only after commit.
4. Treat Redis/local PubSub as fast paths and the database generation as recovery
   authority.
5. Update this plan with every RBAC contract or phase change.
6. Update manifest/runtime docs when ownership or FFA/FBA status changes.
7. Never mark source, compiled or live verification complete without evidence.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-08-01`
- Scope inspected: `typed principal propagation; tenant-safe control-plane admission; committed mutation and read-only resolver ownership; Auth admin and GraphQL effective-change semantics; transactional artifact-permission events; durable generation allocation; PostgreSQL concurrency; independent-process watchdog recovery; Redis fast-path/outage/restart recovery; full CLI system-role repair propagation; invalidation observability and incident evidence`
- Findings: `P0=1, P1=3, P2=0, P3=0`
- Fixed in this pass: `draft PR #2863 corrects a live operator-path P1: Auth admin previously treated the mere presence of role or status input as an authorization change. The new transaction helper reports exact canonical role replay as changed=false while preserving repair for multiple or malformed assignments. Status is compared against the locked user row. Generation reservation, post-commit fan-out and disabled-status session revocation now occur only for an effective change. Focused Rust regressions, a fail-closed source guard and ownership documentation are included. Merged PR #2862 is now the canonical full CLI repair propagation source packet; historical draft #2859 was closed without merge. Drafts #2843 and #2847 were reconstructed as clean mergeable branches before later main advances.`
- Remaining risks or blockers: `#2863 is unexecuted. The #2849, #2853, #2856 and #2862 packets are source-only. Draft #2843 and #2847 remain unmerged; #2847 lacks its repository-generated digest. Same-SHA formatting, API/Events/telemetry/RBAC/Admin/server/CLI compilation, focused Rust/Node/module gates, live negative transports, runtime incident evidence and FFA/FBA management evidence remain absent. Issue #2740 still blocks the known Rust-host path before the server build.`
- Evidence: `source review confirms the GraphQL role-assignment owner already enforces direct principal and users:manage admission and delegates through Auth orchestration with hierarchy, continuity, transaction ownership and durable generation. #2863 removes the remaining presence-based false-change marker, compares status after row lock, and retains malformed relation repair. #2862 reaches the registered CLI command path and two independent observers without Redis or restart. No execution evidence is claimed.`
- Next action: `run the #2863 regression and architecture guards, then execute #2862/#2856/#2853/#2849 on one reconciled revision; generate and review the #2847 event digest; reconcile #2843/#2847/#2863 additively; then define semantic role-change events and native operator parity`
- Resume command: `cargo fmt --all -- --check && cargo run -p rustok-events --example event_contract_digests -- --write && cargo check -p rustok-events --all-targets && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-rbac-cli && cargo check -p rustok-server --lib && cargo test -p rustok-server transaction_role_replacement_reports_exact_noop && cargo test -p rustok-server transaction_role_replacement_repairs_multiple_assignments && cargo test -p rustok-server status_effective_change_ignores_exact_replay && cargo test -p rustok-server --test rbac_auth_admin_effective_noop_guard && cargo test -p rustok-server --test rbac_mutation_api_architecture_guard && node scripts/verify/verify-rbac-cli-live-repair-propagation-source.mjs && cargo test -p rustok-cli --test rbac_live_repair_propagation live_cli_system_role_repair_reaches_two_running_replicas_without_restart -- --ignored --nocapture && node scripts/verify/verify-rbac-two-process-redis-restart-source.mjs && cargo test -p rustok-server --test rbac_two_process_redis_restart separate_process_redis_fast_path_survives_restart_and_recovers_missed_publication -- --ignored --nocapture && node scripts/verify/verify-rbac-two-process-durable-recovery-source.mjs && cargo test -p rustok-server --test rbac_two_process_durable_recovery -- --ignored --nocapture --test-threads=1 && node scripts/verify/verify-rbac-postgres-concurrency-source.mjs && cargo test -p rustok-server --test rbac_postgres_concurrency -- --ignored --nocapture && node scripts/verify/verify-rbac-artifact-permission-outbox.mjs && cargo xtask module validate rbac && cargo xtask module test rbac`