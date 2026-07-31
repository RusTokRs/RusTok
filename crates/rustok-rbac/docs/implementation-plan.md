# Implementation plan for `rustok-rbac`

## Source of truth

This file is the canonical live implementation plan for RBAC. It owns the
current implementation state, completed source phases, remaining priorities and
targeted verification.

- `[x]` means the capability is present in `main` and protected by source-level
  tests or architecture guards.
- `[ ]` means implementation or verification is still required.
- A source-complete item is not considered compiled or operationally verified
  until the corresponding Rust and live-service checks have passed.
- `docs/modules/implementation-plans-registry.md` contains only the current
  status and nearest priority; it must not duplicate this backlog.
- `docs/verification/rbac-server-modules-verification-plan.md` remains the
  cross-platform verification checklist, not a second RBAC implementation plan.

Last reconciled with `main`: 2026-07-31.

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
  request/process cache adapters, distributed invalidation delivery and runtime
  supervision;
- `RbacRoleAssignmentDbWriter` is an idempotent bootstrap/test persistence
  primitive, while existing-user mutations use explicit transaction-owned or
  committed entry points;
- Redis/local PubSub is a best-effort fast path; the database-backed monotonic
  generation is the recovery source of truth;
- the admin overview remains an intentional native-only module-owned surface.
  A GraphQL/REST management path requires an approved remote or headless
  operator contract.

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
- [ ] Execute the added Rust tests and architecture guards in a toolchain-enabled
  environment and fix any compile, formatting or lint failures.

## Remaining work, in priority order

### P0. Compile and targeted verification

- [ ] Run formatting, compilation, Clippy and targeted RBAC/server/CLI tests on
  the reconciled `main` revision.
- [ ] Record successful module validate/test evidence for `rbac`.
- [ ] Execute the existing FFA/FBA verification scripts against the same
  revision.
- [ ] Resolve every failure before claiming the source-complete phases are
  compiled verified.

### P0. Database concurrency and multi-replica recovery evidence

- [ ] Add PostgreSQL integration evidence for concurrent role replacement,
  last-active-super-admin serialization and unique monotonic generation
  allocation.
- [ ] Exercise at least two server replicas with Redis available, unavailable,
  restarted and with intentionally missed PubSub events.
- [ ] Prove that the mutating replica invalidates immediately and that another
  replica catches up through the durable database generation without serving a
  stale authorization snapshot beyond the documented reconciliation bound.
- [ ] Exercise CLI system-role repair while live replicas are running and prove
  they recover from the committed generation without a restart.

### P1. Invalidation observability and incident operations

- [ ] Export metrics for database generation, locally applied generation,
  generation lag, worker running/restart state and recovery/full-clear counts.
- [ ] Define alert thresholds for non-zero sustained lag, repeated worker
  restarts, generation regression and failed database reads.
- [ ] Add an operator runbook covering Redis outage, missed event, generation
  regression, repair execution and verification of effective permissions.
- [ ] Make one policy incident traceable to the evaluator decision, relation
  state, cache snapshot, durable generation and recovery action.

### P1. Explicit actor-kind contract

- [ ] Add an explicit actor/principal kind to the shared authorization context
  and relevant ports instead of permanently inferring control-plane eligibility
  from `client_id`, `grant_type` and `session_id` combinations.
- [ ] Preserve fail-closed compatibility during migration for direct users,
  authorization-code users and client-credentials principals.
- [ ] Add boundary tests proving that only the explicit direct-user actor kind
  can execute control-plane mutations.

### P1. Module-owned operator role and permission flows

- [ ] Define the approved role/permission mutation contract in the owner package,
  including validation, hierarchy, tenant scope, continuity and integration
  event requirements.
- [ ] Route native admin management actions through the module facade without
  adding host-owned relation writes or a parallel `/roles` implementation.
- [ ] Publish and verify the expected integration events for committed role and
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

## Tenant trust boundary correction (2026-07-31)

- Status: `source_ready_unvalidated`.
- Severity: `P0` for authenticated/resolved tenant mismatch; `P1` for raw native
  context-error serialization.
- [x] RBAC Admin bootstrap now requires authenticated and middleware-resolved
  tenant equality before `SETTINGS_READ` permission admission or tenant data use.
- [x] Mismatches fail closed with a static public response and structured tenant
  diagnostics.
- [x] Auth and tenant extractor failures return fixed public messages; typed
  causes stay in server diagnostics.
- [x] `scripts/verify/verify-rbac-admin-tenant-scope.mjs` locks ordering, public
  error safety, direct tracing dependency and exactly one routed bootstrap guard.
- [x] Focused unit coverage retains matching and mismatched tenant cases.
- [ ] Same-SHA `rustfmt`, `cargo check -p rustok-rbac-admin --features ssr`,
  focused unit execution and composed native parity remain required.
- This correction was delivered during the `core/tenant` cross-module work unit
  and is carried evidence for the now-active `core/rbac` verification item.

## Artifact permission and role-metadata control-plane correction (2026-07-31)

- Status: `source_ready_unvalidated`.
- Severity: `P0` invalid authorization grant for REST mutation; `P1` cross-surface
  admission inconsistency for native role/permission metadata.
- Root cause: the REST PUT/DELETE artifact-role permission adapter admitted any
  authenticated principal with effective `modules:manage`, while native RBAC
  Admin bootstrap admitted any principal with `settings:read`. GraphQL correctly
  required a direct session-bound user. Tenant OAuth authorization-code and
  client-credentials principals could therefore mutate artifact grants or read
  control-plane role metadata when their narrowed authority retained those
  permissions.
- [x] Add one host-neutral owner policy over
  `RbacControlPlanePrincipal`; `rustok-rbac` remains compilable without the
  `rustok-api/server` feature or Axum context types.
- [x] Reuse that owner policy from GraphQL, REST and native RBAC Admin rather than
  maintaining transport-specific principal classifiers.
- [x] Require direct grant, non-nil session and authenticated/routed tenant
  equality before `modules:manage` or `settings:read` admission.
- [x] Bind the durable REST operation actor only from trusted
  `AuthContext.user_id`; request payloads cannot supply or replace it.
- [x] Remove the obsolete native generic tenant-only helper and retain static
  public denials plus structured, non-secret diagnostics.
- [x] Add owner unit tests, REST helper tests,
  `rbac_artifact_permission_control_plane_guard` and the updated
  `verify-rbac-admin-tenant-scope.mjs` source verifier for OAuth denial, tenant
  mismatch, permission denial, shared-policy composition and admission order.
- [ ] Same-SHA formatting, default/all-feature RBAC compile, RBAC Admin SSR
  compile, server compile, focused unit/architecture/verifier execution and
  transport-level negative requests are still required.

## Verification commands

- Contract tests cover every public use case.

```bash
cargo fmt --all -- --check
cargo check -p rustok-rbac
cargo check -p rustok-rbac --all-features
cargo check -p rustok-rbac-admin --features ssr
cargo check -p rustok-rbac-cli
cargo check -p rustok-server --lib
cargo test -p rustok-rbac --all-features
cargo test -p rustok-rbac-admin --features ssr
cargo test -p rustok-migrations --lib rbac_system_role_repair_tests
cargo test -p rustok-rbac-cli
cargo test -p rustok-server --lib rbac
cargo test -p rustok-server \
  --test rbac_artifact_permission_control_plane_guard \
  --test rbac_cache_invalidation_architecture_guard \
  --test rbac_mutation_api_architecture_guard \
  --test rbac_migration_registration_guard \
  --test rbac_startup_invalidation_architecture_guard
cargo clippy -p rustok-rbac --all-features -- -D warnings
cargo clippy -p rustok-rbac-cli -- -D warnings
cargo clippy -p rustok-server --lib -- -D warnings
cargo xtask module validate rbac
cargo xtask module test rbac
node scripts/verify/verify-rbac-admin-tenant-scope.mjs
npm run verify:rbac:admin-boundary
npm run verify:rbac:fba
```

PostgreSQL concurrency and multi-replica Redis failure/recovery scenarios remain
manual or dedicated integration-environment gates until an approved automated
harness owns them.

## Completion gates

- Source-complete phases become **compiled verified** only after the targeted
  commands pass on the same revision.
- Durable invalidation becomes **operationally verified** only after PostgreSQL
  concurrency and multi-replica failure/recovery evidence passes.
- FBA remains `boundary_ready` until composed provider/consumer/fallback
  evidence passes.
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
- Last verified at (UTC): `2026-07-31`
- Scope inspected: `principal classification and authoritative request-scope construction; tenant-filtered relation resolution; generation-aware cache fill; committed role replacement; canonical repair; installer bootstrap boundary; artifact permission catalog, durable assignment owner, REST/GraphQL adapters and native RBAC Admin bootstrap`
- Findings: `P0=1, P1=1, P2=0, P3=0`
- Fixed in this pass: `one host-neutral rustok-rbac policy now requires a direct non-nil session and authenticated/routed tenant equality before GraphQL, REST and native RBAC control-plane permission admission; REST no longer treats modules:manage as sufficient principal authority, native bootstrap no longer treats settings:read as sufficient, the obsolete native tenant-only helper is removed and the durable actor remains derived from trusted AuthContext`
- Remaining risks or blockers: `the P0/P1 corrections are source-ready but same-SHA format, default/all-feature RBAC compile, RBAC Admin SSR compile, server compile, focused unit/architecture/verifier tests and live negative transport requests have not run; all previously open PostgreSQL concurrency, durable generation allocation, Redis outage/restart/missed-publication recovery, operator repair propagation, explicit actor-kind design, module-owned management flow and FFA/FBA evidence remain open`
- Evidence: `source review confirms middleware builds request scope from authoritative DB permissions and OAuth only narrows authority; relation resolution tenant-filters role ids and generation-aware fills fail closed; role replacement and repair reserve durable generation in their owner transaction. PR #2747 adds owner, GraphQL, REST, native and architecture/source regressions. No successful execution result is claimed yet.`
- Next action: `collect exact-head format/check/Clippy and focused evidence for PR #2747, fix branch-related failures, then continue the RBAC sweep across remaining CLI/event/worker surfaces and live recovery gates`
- Resume command: `cargo fmt --all -- --check && cargo check -p rustok-rbac && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-server --lib && cargo test -p rustok-rbac --all-features && cargo test -p rustok-rbac-admin --features ssr && cargo test -p rustok-server --test rbac_artifact_permission_control_plane_guard && node scripts/verify/verify-rbac-admin-tenant-scope.mjs && cargo xtask module validate rbac && cargo xtask module test rbac`
