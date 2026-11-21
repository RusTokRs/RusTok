# Server App — Implementation Plan

## Background worker single-owner admission — Notifications, Search and Index (2026-11-21)

Branch: `agent/server-notification-worker-startup-ownership-20261121`,
forked from main `a9431fdefd08693eab7aa0aaab87169b877af239`;
independent of the module dispatcher and OAuth draft branches.

### Confirmed defects

- Candidate, fanout and outbox-intake startup each checked
  `shared_contains::<...WorkerHandle>()` before dependency construction
  and later executed an unconditional `shared_insert(...tokio::spawn(...))`.
  Concurrent callers could both pass the initial check, launch duplicate
  tasks and overwrite the context's sole JoinHandle, leaving one task
  invisible to the normal drain. In Outbox intake, two callers could also
  concurrently register consumer event types and backfill receipts before
  any handle was published.
- Candidate and fanout worker startup had no admission-time check of the
  shared `StopHandle`. A late caller could spawn a worker while the server
  already had its shutdown signal set.
- Forum Search inbox, Forum Search typed contract consumer, Product Index
  refresh, and Social Graph Index consumer showed the same check-then-publish
  race. The three broker-backed consumers also opened persistent Iggy
  consumer groups asynchronously **before** publishing handles; concurrent
  starters could open two groups and lose a JoinHandle. All four also needed
  a stop check between preparation and task admission.
- A second caller observing an in-progress reservation initially returned
  `Ok(())`, even if the first caller had not yet finished and could still
  fail. That was a false startup-success signal for external bootstrap
  callers not coordinated by `ServerBootstrapInitializationLock`.

### Changes

- Added a typed `ServerRuntimeStartReservation<'_, T>` in
  `server_runtime_context.rs`: it takes a process-local atomic
  `shared_insert_if_absent` claim, releases it on startup error or future
  cancellation via `Drop`, and retains the claim after handle publication
  through `commit`.
- Candidate and fanout reserve after their readiness/dependency checks,
  before constructing/spawning their workers. If construction fails or
  shutdown begins during preparation, the claim is removed so a retry can
  succeed. Only the one admitted caller publishes a JoinHandle.
- Outbox intake reserves **before** async registration and full legacy
  receipt backfill; an aborted backfill cannot strand its local startup
  reservation. Successful backfill followed by handle publication retains
  the claim. Existing durable receipt idempotence remains unchanged.
- Forum Search inbox reserves immediately before worker admission. Its
  unsupported-backend and missing-source early exits remain no-ops without a
  claim, while a shutdown before publication aborts startup.
- Forum Search contract, Product Index refresh and Social Graph Index now
  reserve their distinct worker marker **before** the async persistent
  consumer open. This prevents duplicate group open and ensures cancellation
  during broker I/O automatically releases the reservation. Validate
  configuration and check shared `StopHandle` before/after broker open, before
  recording worker-start telemetry and publishing the task.
- Each of the seven callers now distinguishes a completed worker (idempotent
  success) from another starter still working (retryable `Error::Message`).
  The latter cannot be reported as success until the winner actually
  publishes a handle, and never overrides an existing one.
- The reservation constructor is deliberately lazy (`bool::then`, not
  `then_some`), because a losing contender must **not** construct a temporary
  drop guard capable of deleting the winning claim.
- Tests cover failed-start claim release, retained successful ownership,
  concurrent contention by 12 threads and cancellation by dropping an
  in-flight async startup future before it completes.

**Handoff:** No Cargo tests, rustfmt, clippy, compilation, migrations or
multi-process checks were run per maintainer preference. Run
`cargo check -p rustok-server --lib` and targeted
`services::server_runtime_context::tests`,
`services::notification_candidate_worker`,
`services::notification_fanout_worker`,
`services::notification_outbox_intake_worker`,
`services::forum_search_inbox_worker`,
`services::product_index_refresh_worker`, and
`services::social_graph_index_worker` tests. Validate lifecycle
shutdown/backfill interleavings and network consumer-open cancellation with a
real DB, Iggy broker and two concurrent starter calls. This reserves **one worker per runtime context**, not one per
deployment. It does not add durable cross-replica worker leader election.

## Server runtime follow-up (2026-11-21)

This bounded continuation follows merged PR #4759 and stays on
`agent/server-runtime-lifecycle-followup-20261121`.
The pass examined remaining host services, controller request boundaries,
middleware admission, transaction outcomes, and cooperative shutdown of
registered background workers; it is **not** a claim that every server path
has been exhaustively verified or that the full-stack audit gates are closed.

- Generic settings writes now reject both missing and nil actor UUIDs before
  opening a transaction. This preserves the documented non-nil actor identity
  for persisted settings rows and their transactional outbox events.
- CSP reporting batches count only actual CSP violations against the bounded
  processing limit, so unrelated Reporting API entry types cannot suppress
  CSP reports that appear later in the same batch.
- The search-backend readiness TCP target now parses URL authorities instead
  of splitting strings at slashes. Query strings/fragments no longer become
  hostname bytes; invalid ports and schemes fail consistently, with legacy
  bare `host:port` configuration retained.
- Observability authentication rejects **all** duplicate Authorization fields,
  including identical copies; this avoids intermediary-dependent token parsing.
- Focused regression tests were added for each change. **Cargo tests, format,
  integration, and multi-replica concurrency tests were not run** in the
  GitHub-API-only editing environment, per maintainer request.

### Continuation: retention, tenant parsing, and worker bootstrap

- Auth-artifact retention now deletes sessions, invite consumptions/invites,
  OAuth tokens, and OAuth authorization codes inside a **single transaction**.
  An error in any leg rolls back preceding deletes; counts are returned only
  after commit. A SQLite regression deliberately removes the second table
  after inserting an expired session and checks that the first delete rolls
  back. This changes transaction locking behavior, so verify with PostgreSQL
  under concurrent auth traffic before production rollout.
- The optional Notifications Outbox intake backfill checks the process stop
  signal before registration, after registration, before and after each page,
  and before each receipt. An interrupted bootstrap exits instead of draining
  an unbounded historical receipt backlog while host shutdown waits for the
  bootstrap lock. Outbox registration and acknowledgement remain durable and
  idempotent; no in-flight SQL operation is forcibly cancelled. A watch-signal
  regression test checks refusal after shutdown. This does not make initial
  backfill bounded in **normal** startup, and does not replace durable recovery.
- Tenant subdomain matching now checks UTF-8 slice boundaries before comparing
  configured base-domain suffixes. Malformed non-ASCII input cannot panic the
  handler, even when a caller bypasses the ordinary host validator. A test
  exercises a multibyte boundary.
- These additional changes were committed to the same existing draft PR;
  no tests, builds, linters, format checks, or migrations were executed here.

Unclosed owner-level work remains, including durable delivery/retry for
password-reset and verification email tasks, crash reconciliation of HTTP
installer jobs, and stronger lifecycle fences across process and replica
boundaries. This change does not assert those contracts are solved.

## Focus

Strengthen `apps/server` as the central backend runtime with formal API contracts, predictable operational diagnostics, and hardened security gates.

## Bounded authentication and OAuth follow-up (2026-11-17)

This ordinary server development pass is not an activation or completion of
the full-stack audit ledger. Work is isolated in
`fix/server-deep-review-2026-11-17` (PR #4742), based on
`9b32aacabc3c032a009eb9c18609547242766a29`.

- Unknown-email timing equalization now propagates both dummy Argon2
  initialization failures and verification failures; no empty-hash fallback.
- Password-reset credential CAS rejects accounts deactivated while the reset
  password was being hashed. Password changes reject expired current sessions.
- In-place first-party refresh/session lifecycle enforces the original 90-day
  absolute deadline across fresh issuance, refresh, active session checks,
  listing and password-change admission, including legacy rows.
  Follow-up verification corrected unchecked chrono arithmetic near its
  representable bounds and binds browser cookie Max-Age to the actual persisted
  refresh expiry (including near-90-day rotations), not a full fresh TTL.
- OAuth refresh `scope` can only retain or reduce the existing grant, never
  silently grant more than requested. Cookie-session refresh/logout reject
  duplicate same-name cookies rather than choosing an attacker-influenced order.
  OAuth refresh-family absolute deadlines also use checked Chrono arithmetic
  for both family admission and token issuance. Invalid persisted timestamps
  produce a typed OAuth error instead of a request panic.
- GraphQL host preflight now applies field, tenant, module, storefront, forum,
  dashboard and legacy-mutation policies to only the operation selected for
  execution. Module and legacy-mutation fragment walkers now terminate on
  cyclic fragment graphs while preserving detection of reachable protected fields.
- Channel REST authorization now reads the authenticated token's effective
  permissions (including OAuth scope restrictions), not persisted grants that
  could widen authority beyond the token. It also rejects tenant mismatches.
- DLQ listing/replay treats a top-level event-envelope tenant as authoritative;
  nested legacy tenant fallback applies only when the top-level key is absent.
  Replay repeats the tenant predicate within its atomic status update.
- The GraphQL selected-operation helper reads the parser's operation-name map
  keys, not a nonexistent field on the operation AST.
- Blank `OTEL_SERVICE_NAME` no longer masks a usable `RUSTOK_SERVICE_NAME`.
- Focused tests cover the added behavioral branches. Cargo, formatting,
  SQLite/PostgreSQL concurrency and real-HTTP tests are **not run** in this
  GitHub-API-only editing environment; the maintainer must run focused checks
  before merge. Broader server inspection is unfinished.

Additional follow-up: OAuth refresh-token reuse detection now propagates
database read/write failures as `server_error` (instead of reporting only
`invalid_grant`) and a fault-injection test covers retry after recovery.
Family-wide revocation is additionally scoped to the original OAuth subject,
so even malformed cross-user family IDs cannot revoke another user's tokens
inside one app/tenant.
First-party logout also accepts the immediately preceding rotated cookie
under the session row lock, revoking its live successor after a lost refresh
response. Refresh-reuse detection now takes that same database-specific row
lock before evaluating the grace window or revoking the session, preventing a
stale ActiveModel snapshot from overwriting a concurrent token rotation.
Focused regression tests cover the in-grace race, post-grace revocation, and
persistence-failure rollback. Session-id logout also acquires the session row
lock (or SQLite write fence) before converting the row into an ActiveModel
update; its regression test exercises the actual logout service and repeated
idempotent logout. The 30-second grace window is not an idempotent retry
protocol: the old token is rejected without revoking the current session, but
a lost successful response still cannot be recovered.

Remaining auth contract gaps requiring owner-level design and verification:
the in-place first-party refresh session stores only the immediately previous
digest, so replay/logout of still older generations cannot identify or revoke
the live lineage. OAuth still has no durable retry/incident reconciliation if
the caller does not retry after a revocation outage. These are not closed by
this pass and must not be reported as remediated.


## Server Engineering Audit Continuation (2026-10-28)

This is **bounded remediation**, not completion of the exhaustive FS-01
six-dimension audit. The reviewed baseline was
`999c9b2804febfe67a3bef7ef937e891d6aa8657` on `main`.

### Confirmed changes under review

- HTTP installer admission fails closed on a durable-job lookup error.
  A new append-only migration enforces at most one `running`
  `install_http_jobs` record across replicas through a database
  partial unique index on PostgreSQL/SQLite. Concurrent losers receive
  a stable HTTP 409. Existing duplicate `running` rows intentionally
  block migration until their side effects and receipts are reconciled.
  Public installer read errors no longer expose SQL/host paths.
- Manifest OAuth synchronization no longer silently ignores orphan-app
  deactivation failures. Each tenant's orphan deactivations and matching
  refresh-token revocations commit atomically; SQL failures roll back
  those deactivations. Newly created/updated desired apps still execute
  before the orphan transaction and **the complete reconciliation is
  not yet all-or-nothing**.
- REST user listing now has a deterministic tie-breaker for equal
  `created_at` values.

### Unclosed architectural and runtime gates

- **Installer:** the HTTP job still uses an in-process `tokio::spawn`.
  A process crash can strand a `running` job. HTTP now assigns a stable
  session ID before spawning and stores it on the job row, and the executor
  creates that session with the same ID. The executor still has no resume entry
  point; the redacted session plan and receipts do not prove whether an
  unreceipted external effect completed. A safe recovery/fencing design must
  reconcile immutable receipts and actual side effects before taking over; do
  not automatically delete, expire, or replay `running` rows. Add a multi-replica
  PostgreSQL contention test and crash/restart recovery evidence. PostgreSQL
  support for the new index must be verified before deployment.
- **OAuth:** implement a single versioned, serialized per-tenant
  reconciliation transaction that covers all upserts and deactivations,
  with explicit revocation fences for previously minted access tokens.
  Owner/transport authorization semantics must be preserved.
- **Build visibility:** platform-wide build history and unscoped
  `BuildEventHub` broadcasts are currently exposed behind tenant
  `modules:read/list/manage` grants; decide and document the
  authoritative host-global-vs-tenant visibility contract in an ADR
  before altering clients, event schemas and WebSocket behavior.
- **Email:** replace detached password-reset/verification send tasks
  with confidential, durable, idempotent delivery and retry evidence
  without enabling user enumeration.
- **GraphQL:** owner-view UUID projection now fails with a sanitized GraphQL
  error rather than panicking. The added conversion and transport tests still
  need execution with the project toolchain.
- **Build execution:** dry-run no longer claims a build. A live executor now
  attempts to record observed post-claim errors as failures and uses guarded
  status writes, but failed recording, process loss or task cancellation can
  still strand it. Implement durable fenced recovery, global admission and
  transactional notification delivery before claiming crash-safe execution or
  progress streams; the added Rust tests still require execution.
  New build receipts now preserve the inserted execution plan and reason;
  historical receipts retain their original response. Successful-build reuse
  across composition revisions and repeated `BuildRequested` notifications
  still require a coherent owner/event reconciliation design.

Required scoped verification before accepting these changes:
`cargo fmt --all -- --check`,
`cargo check -p rustok-server --lib`,
`cargo test -p rustok-server --lib`,
`cargo test -p rustok-migrations --lib`, and targeted PostgreSQL
schema/admission and OAuth failure-injection tests. None is claimed
passed without execution evidence.

## Search metrics empty-index correction (2026-11-02)

The SQLite search metrics query now normalizes empty-set public and stale
counts to zero, matching the existing PostgreSQL aggregate behavior. Previously
both values were `NULL`, causing a successful empty-index query to be reported
as a failed metrics collection. SQLite search lag calculations now round
integer millisecond differences instead of floating-point Julian-day arithmetic,
which avoids off-by-one results at whole and half-second boundaries. SQLite's
date parser has millisecond precision, unlike PostgreSQL's microseconds; the
readiness and Prometheus queries use the same calculation and compare parsed
instants rather than timestamp text. Search and outbox readiness also compare
signed lag against the full `u64` configuration range without wrapping large thresholds.
Focused Rust tests cover the changed branches but require execution with the
workspace's Rust toolchain and dependencies. This correction does not make the
existing lag or bootstrap metrics proof of source freshness or completeness;
that requires owner-backed reconciliation or durable checkpoints.

## WebSocket Authority Continuation (2026-11-02)

The build WebSocket now shares the GraphQL connection's live authority lease:
initial bearer identity is re-resolved against tenant, credential and current
RBAC state periodically and before event delivery, with a bounded lookup and an
expiry deadline. Outbound writes are bounded to avoid a stalled peer holding
an expired connection indefinitely. The process-local build hub uses atomic
singleton admission. The GraphQL init-receipt deadline is rechecked after it
fires so it does not interrupt an already received initialization; the separate
validation phase has a 15-second bound.
The global build-visibility decision and durable multi-process progress remain
open; this change does not resolve those boundaries. SQLite lease and socket
regression tests and a concurrent hub test are included but require execution
in a Rust-capable environment, alongside GraphQL WebSocket and multi-replica
integration verification.

### Module-work scheduler liveness (verification pending)

Readiness and runtime-worker metrics now include the shared module-work scheduler
when module composition registers scheduled work. Until registrations are inspected,
worker hosts treat the task as required; a confirmed empty set is optional. Surface
hosts do not require a local scheduler. These checks cover only task liveness, not
work progress or backlog. Module-work startup is serialized per runtime context;
concurrent initializers cannot register and spawn duplicate tracked schedulers.
An error or cancellation releases the initialization lock so a later attempt may
retry. Shutdown publishes a stop signal even if startup has not created it yet,
prevents startup after registration completes, and waits up to its shared drain
deadline for an ongoing initialization. This is process-local, not fleet-wide
ownership; registration callbacks retried after error or cancellation must handle
any external side effects they performed before failing. A stuck callback can
outlive the drain deadline. This does not serialize the whole bootstrap or other
worker/dispatcher startup against shutdown. Concurrency, error/cancellation retry,
liveness and shutdown tests are added but require execution with the project toolchain.

### Axum startup failure cleanup (verification pending)

The Axum host now invokes its bounded worker shutdown for ordinary initialization,
router composition, endpoint resolution, listener bind, and serving errors as well
as successful serving, preserving the original result. Shutdown also explicitly
aborts the channel/RBAC invalidation listeners, RBAC generation watchdog, and
in-memory event forwarder even if other handle clones remain. Aborts are requested,
not joined. An RAII guard schedules the bounded shutdown when the host lifecycle
future is cancelled or unwinds, but that cleanup is detached: the cancelling caller
cannot await confirmation, and runtime teardown/process abort can still prevent it
from completing. Not every detached or stop-signal-only task is tracked and drained;
an expired drain deadline can leave other work running. Rate-limit cleanup loops
and the Comments TCP listener are now stored in runtime-owned registries and
included in the bounded host drain. Direct users of the lower-level bootstrap APIs
remain responsible for shutdown. The host, listener, rate-limit, and supervisor
regression tests added in the latest source continuation have not been compiled or
executed.

The pre-existing RBAC source-guard mismatch exposed a rejected-read fallback in
the owner resolver. The owner now returns a typed consistency error after
bounded retries and the server maps it to a generic internal error rather than
using a potentially superseded grant or fabricating a denial. A deterministic
interleaving test and adapter tests are added but have not run. This is a local
cache-fill check, not a transaction or cross-replica revocation fence. See the
[owner decision](../../../DECISIONS/2026-11-02-rbac-cache-resolution-consistency.md).

After a matching request-snapshot invalidation, `RbacService` single/any/all,
permission-list and role lookups and the owner-policy adapter now fail unavailable
rather than fall back to unscoped persisted grants. Full-cache invalidation also
expires a snapshot in the calling task. This is an intentional availability
tradeoff, and a preceding mutation may have committed; clients must not blindly
replay it. The added unit and database regression tests have not been compiled or
executed. See the [request-snapshot decision](../../../DECISIONS/2026-11-02-invalidated-rbac-request-scope.md).

The GraphQL security extension now rejects new normal root resolvers when the
matching task-local snapshot is invalid, even when the owner reads the original
`AuthContext`. After a grant commit the generation-aware role writer clears
the whole local cache and expires the calling snapshot, including for an
unrelated role; the retained legacy hook expires it before a fallible membership
lookup. Added GraphQL sequencing and failed-lookup regression tests are not yet
compiled or run. The hook permits nested fields within a root already admitted, cannot stop
already-started fields, and does not reject a missing task-local scope. State does
not automatically follow spawned tasks or invalidate other requests/replicas.
Actual async-graphql 7.2.1 execution and transport behavior require integration
verification before claiming this boundary is protected end to end.

The owner GraphQL `update_role` now reserves the durable RBAC invalidation
generation in the same transaction as a supplied grant replacement. After commit
the server clears the local cache and attempts namespace-wide generation fan-out;
watchdogs can reconcile a missed fast notification. Added owner and server tests
have not been compiled or run. The durable marker is not an outbox or an immediate
revocation fence, and cancellation, unhealthy watchers and active request
snapshots remain limitations. PostgreSQL and multi-replica verification are open.
See the [grant-invalidation decision](../../../DECISIONS/2026-11-02-rbac-graphql-grant-invalidation.md).

### GraphQL operation admission ordering (verification pending)

Previously, the incoming-socket task tracked new operations before the GraphQL
protocol stream consumed cancellations, so rapidly cancelled subscriptions or
reused ids could be rejected as duplicates or overflow. Admission now occurs
when the protocol stream consumes each queued message; the host handles an
engine `complete` before polling the engine again. Slots are not eagerly
released by incoming cancellation, so an old completion cannot release a new
operation with the same id under that protocol ordering. On review, forwarding
the engine's cancellation acknowledgement was found unsafe for immediate ID
reuse on `graphql-transport-ws`: a client could treat it as the completion of
the new operation. That acknowledgement is now suppressed for the modern
protocol, but forwarded for legacy `graphql-ws`, whose documented flow expects
it. This cannot retract messages already in flight before cancellation; legacy
clients in particular must not assume an old completion identifies a reused ID's
new operation.

Regression tests exercise both supported subprotocols, queued cancellations,
natural completion, id reuse, duplicate ids, and the 32-operation limit. They
have not been compiled or run here. The fix depends on the locked async-graphql
protocol engine yielding a server `complete` for an active cancellation before
consuming a subsequent operation: the public upstream source near the 7.2.1
release shows this behavior, but the exact locked crate was not available for
verification. Run the tests against that crate and re-run them when upgrading it;
also test end-to-end over the actual socket with delayed writes and revocation.
Lease revalidation before polling and output is not a transaction fence for an
already running GraphQL operation: a mutation may commit after its authority
changes but before its result is revalidated and suppressed. Security-sensitive
mutations requiring immediate revocation must recheck authority at their own
commit boundary; the WebSocket transport alone cannot provide that guarantee.
`cargo test -p rustok-server --lib graphql_ws_admission --locked --offline`
could not reach compilation because the pinned Athanor git dependency was not
cached. A standalone Rust harness using the actual admission adapter with mocked
stream/channel/tracker types passed two ordering and overflow checks; it is not
an integration test or a type-check of the server against its real dependencies.

## Server P1 Security and Readiness Remediation (2026-11-01)

This is bounded remediation of the five P1 findings from the `apps/server`
logic review, not completion of the exhaustive FS-01 audit.

### Confirmed changes under review

- Invite acceptance now checks `features.invites_enabled` in the canonical
  `AuthLifecycleService::accept_invite_once_runtime` service before token
  decode. REST middleware, REST handlers, and GraphQL/native providers share
  the same gate; disabled invites remain indistinguishable from invalid tokens.
- Service-token permission snapshots now pass through the same tenant module
  availability filter as direct-user snapshots before role inference. Flex
  REST handlers call the owner-side `flex::rest::ensure_module_enabled` gate,
  and Flex GraphQL `require_access` calls the shared GraphQL module gate.
- Password reset, self-service password change, and admin password updates now
  write `users.password_changed_at` and revoke live delegated OAuth refresh
  tokens in the same transaction. Delegated access-token validation, refresh
  validation, and authorization-code exchange reject credentials issued at or
  before the epoch. A nullable additive migration owns the schema change.
- CORS preflight now allows the session-request, idempotency, host-authority,
  installer setup, and observability token headers already consumed by the
  server. A real preflight test replaces the previous Debug-only assertion for
  this contract.
- Readiness only requires local background-worker handles in `full` and
  `worker` host modes. API/SSR surface modes emit a non-critical worker-host
  mode check instead of failing readiness for workers they do not run.

### Worker liveness follow-up (partial)

Readiness and worker metrics now share local task snapshots for auth retention,
module dispatch and lifecycle maintenance, and feature-gated payment, commerce,
product and workflow workers. Dispatcher composition explicitly records whether
it has listeners, so an uninitialized registration fails closed while a confirmed
empty listener set does not require a task. Host modes without local workers do
not require these handles. These are liveness checks, not proof of successful
processing; progress/backlog observability and coverage of other background
workers remain open. Rust compilation and the added tests require a toolchain.

### Verification status

The sandbox has no project-compatible Rust toolchain, PostgreSQL, Redis, or Docker.
A temporary Rust/Cargo 1.88 was obtained for metadata and formatting inspection,
but the workspace requires Rust 1.96. Offline server checking stops before
compilation because the Athanor git dependency is not cached; registry access is
also restricted. `git diff --check` and diff invariants pass. The architecture
snapshot guard currently reports the existing `rustok-server -> flex` dependency
against its allow-list; the relevant manifests and guard rules are unchanged by
this patch. Rust compilation, crate tests,
migration smoke, and infrastructure-backed tests remain required before merge:
`cargo fmt --all -- --check`,
`cargo check -p rustok-server --lib`,
`cargo test -p rustok-server --lib`,
`cargo check -p flex --lib`,
`cargo test -p flex --lib`,
`cargo check -p rustok-migrations --lib`,
`cargo test -p rustok-migrations --lib`, and
`scripts/verify/verify-migration-smoke.sh`.
No Rust or PostgreSQL verification is claimed passed without execution evidence.

## Module Platform Handoff

The server is the host and transport composition layer for the module platform;
it is not the owner of module marketplace/control-plane business logic. The
canonical sequence is maintained in the
[module-platform implementation plan](../../../docs/modules/module-control-plane-consolidation-plan.md).

Server work for that plan is:

- mount the `rustok-modules` facade through authenticated tenant/actor contexts;
- expose static module lifecycle toggles only through the owner typed command:
  GraphQL derives tenant, actor, and `modules:manage`, requires a UUID
  idempotency key plus a non-negative aggregate revision, and leaves journal
  correlation/replay/no-op receipt handling to the owner. Static toggles,
  normalized settings, retry, and compensation share the owner lifecycle
  aggregate and its fail-closed execution claim;
- expose artifact tenant lifecycle only through the owner-issued GraphQL snapshot
  and revision-CAS mutation; transport derives tenant/actor/permission and does
  not read lifecycle, admission, or outbox tables directly;
- supply database, OCI, trust, events, audit, clock, and other infrastructure
  adapters; module-build scheduling and execution run in their separate
  dispatcher and worker deployments. The server queues and streams build
  events and requests builds through `PlatformCompositionBuildService`, but it
  does not own a build executor. `services/build_executor.rs` was an
  installer-era wrapper that no longer had a caller after the release
  activation hook moved into the owner; it also hard-wired
  `NoopBuildEventPublisher`, so keeping it would have advertised a build path
  that silently published nothing. It is removed, and the only remaining
  `BuildExecutionService` composition is the CLI worker
  (`crates/utils/rustok-cli-platform/src/rebuild.rs`);
- keep MCP stdio out of the host. `DbBackedMcpRuntimeBridge` exposed
  `bind_stdio_config`, an unused helper that implied the host could assemble a
  stdio MCP session; the module plan states generic stdio is a library/CLI
  surface and that tenant-owned tools are reachable only through the
  authenticated JSON/SSE remote transport. The helper had no caller and is
  removed, so the host has exactly one MCP path (remote bootstrap + tool
  transport) and no second, unaudited composition to drift from it;
- obtain every control-plane service through `ModuleControlPlane` instead of
  constructing owner types in the host. The RBAC permission-ownership map is the
  one host adapter that previously built `ModuleDefinitionCatalog` directly
  (`services/rbac_module_availability.rs`); it now derives the same catalog from
  `ModuleControlPlane::new(db).catalog(registry)`. The map is memoized in the
  runtime context, so the added control-plane handle costs one construction per
  process;
- migrate platform composition, build enqueue, registry governance, effective
  policy, GraphQL, and native adapters to owner operations;
- keep production release admission, desired/observed rollout, activation, and
  recovery exclusively in `rustok-modules`; the server exposes only narrow
  authenticated transports and deployment-agent observations, never another
  release head or activation side-effect hook;
- adapt typed manifests and bootstrap-file loading at the host boundary while
  `SeaOrmModuleCompositionService` owns canonical active-snapshot reads and
  bootstrap persistence, revision-CAS updates, and the combined CAS/build
  transaction; the server's build adapter receives the owner transaction and
  publishes the build notification only after commit;
- split compile-time Core/static implementation registration from the durable
  artifact-aware definition catalog and runtime dispatcher;
- keep the static registry boot-owned in `ServerRuntimeContext`; request guards
  consume that injected adapter and fail closed instead of constructing a
  registry per request; installer execution receives the same boot-owned
  registry explicitly and does not construct a second topology;
- supply platform content-addressed artifact storage, transactional outbox, and
  multi-node reconciliation adapters;
- compose artifact and Alloy Rhai through one shared mTLS-authenticated isolated
  worker client, require exact readiness at startup, and never register an
  in-process Rhai fallback; keep Wasmtime placement explicit;
- preserve transactional and transport parity guarantees during cutover;
- delete replaced service business logic, error taxonomies, direct writes, and
  runtime Cargo execution references; trusted static distribution builds remain
  installer/CLI operations and never start from server runtime workers;
- keep only Core/bootstrap and explicitly promoted native modules in static
  host composition.

The server must never compile untrusted module source, load marketplace native
libraries, or modify its source/Cargo graph during runtime installation. It
must also never fetch an external OCI payload for every execution, grant an
artifact raw infrastructure clients, or require an artifact-only module to
implement `RusToKModule` in process.

## Host-global authority composition

Process-wide Events, Iggy, System and Settings resources are not tenant-owned.
The server must not infer authority over them from tenant roles, permissions,
OAuth applications, OAuth scopes, app metadata, a default tenant or magic UUID.

The implementation merged in PR #2735 as
`1ce83819b077ef6e0df009fd5675f556315ef63a` uses a dedicated host-owned
credential boundary:

- callers present `X-RusTok-Host-Token` only over HTTP/native requests;
- the deployment stores only SHA-256 digests, non-nil operator audit ids and
  explicit `read`/`manage` levels in `RUSTOK_HOST_AUTHORITY_CREDENTIALS`;
- parsing is bounded, duplicate token hashes are rejected and digest comparison
  is constant-time;
- Axum middleware removes and authenticates the raw header once before
  downstream dispatch;
- native Events and Iggy server functions receive only typed
  `HostAuthorityContext` request extensions;
- HTTP GraphQL consumes the same typed authority from a request-task-local scope
  and never re-reads the raw header or credential configuration;
- GraphQL WebSocket remains fail-closed and does not inherit host authority;
- Iggy native/GraphQL mutation additionally requires ordinary authenticated
  tenant context equal to the routed tenant because encrypted connector secrets
  remain tenant-owned; the mutation audit actor remains the host operator;
- overlap rotation and revocation are deployment configuration operations, not
  tenant OAuth/RBAC writes.

The operator runbook is
[`host-authority.md`](./host-authority.md). Superseded PR #2726 is historical
staging only. Issue #2680 remains open until same-SHA source/unit/compile
evidence and retained live denial, admission, rotation, revocation and
multi-replica parity evidence exist.

## RBAC artifact permission control-plane composition

Artifact role-permission grants and role/permission metadata are tenant RBAC
control-plane state. Server and native adapters may authenticate and map
transport errors, but they must not define a second principal policy or treat an
effective permission as sufficient proof of principal eligibility.

PR #2747 merged the owner-provided `RbacControlPlanePrincipal` policy as
`75b67f877eb405abe4e6761a16d6b7ece98bc103` for GraphQL, REST and native
RBAC Admin. The current cycle replaces owner-side reconstruction from OAuth
metadata with one explicit shared principal kind:

- `rustok-api::AuthPrincipalKind` is host-neutral and distinguishes direct user,
  OAuth delegated user and service principal;
- the access-token resolver validates the token and classifies the claims once,
  storing the typed result on `CurrentUser`;
- HTTP/native middleware and GraphQL HTTP/WebSocket composition only propagate
  `CurrentUser.principal_kind` into `AuthPrincipalContext`;
- unknown or inconsistent grant/client/session shapes fail closed in the auth
  resolver before request data reaches module policy;
- GraphQL role reads/writes, REST artifact-role permission writes and native
  RBAC Admin require typed principal context before effective permission checks;
- only `DirectUser` with authenticated/routed tenant equality may enter RBAC
  control-plane operations;
- delegated users and services remain denied even when their effective scoped
  permissions include `modules:manage`, `users:manage` or `settings:read`;
- the owner principal contains only typed kind plus tenant id and has no fallback
  to `client_id`, `grant_type` or `session_id`;
- the durable REST operation actor remains `AuthContext.user_id`; request
  payloads cannot supply or replace the audit identity;
- the owner crate remains independent of Axum and the `rustok-api/server` feature.

The contract is documented in
`crates/modules/rustok-rbac/docs/explicit-principal-kind.md`. Same-SHA API/auth-resolver/
RBAC/Admin/server compilation, focused tests, source verifiers and live negative
transport evidence remain required before this correction is verified.

The default `rustok-rbac` crate compiled on exact PR #2747 head
`3cf4b3a44980ca257f7f53849e905673141db289` inside Rust-host workflow run
`30650883159`. That workflow then reproduced issue #2740 before building
`rustok-server`; it is historical evidence only for the pre-typed revision.

## RBAC durable invalidation observability composition

The server owns the process-level watchdog that reconciles the authoritative
RBAC database generation with the generation applied to local permission
snapshots. The cycle-001 RBAC source slice instruments that existing worker; it
does not create a second counter, cache authority, listener or recovery path.

The host adapter now reports through the canonical `rustok-telemetry` registry:

- durable and locally applied generation;
- signed durable-minus-applied lag, including a negative regression signal;
- watchdog running state and bounded restart reasons;
- durable-generation database read failures;
- bounded recovery and process-wide permission snapshot clear reasons.

The worker records positive lag before catch-up, zero after a successful applied
checkpoint, and preserves negative lag when the database regresses below the
monotonic process checkpoint. Recovery still clears permission snapshots through
the existing owner/runtime path. No metric contains tenant, user, role,
permission, session, OAuth client or cache-key labels.

Alert thresholds and Redis outage/restart, missed PubSub, generation regression
and canonical role-repair procedures are owned in
`crates/modules/rustok-rbac/docs/README.md`. Source guard
`scripts/verify/verify-rbac-invalidation-observability.mjs` locks registry,
worker, documentation and cursor synchronization. Compilation, test execution,
two-replica Redis recovery and one complete authorization incident trace remain
required evidence.

## Improvements

### Architecture debt

- Reduce coupling between HTTP/GraphQL layers and modular business logic through stricter service boundaries.
- Unify module lifecycle (bootstrap, readiness, graceful shutdown).
- Reduce transport/auth configuration duplication across subsystems.

### API/UI contracts

- Finalize a unified error contract for REST and GraphQL (codes, machine-readable fields, correlation id).
- Stabilize tenant-aware header and auth claim contracts for all frontend clients.
- Expand public API change versioning via changelog/contract notes.
- Bring MCP management surface (`/api/mcp/*`, GraphQL `mcp*`) to platform-grade: persisted clients/tokens/policies/audit, session-start runtime binding, live binding Alloy scaffold tools to persisted draft store and persisted Alloy scaffold drafts already exist; server-owned remote MCP transport bootstrap (`POST /api/mcp/runtime/bootstrap`) added as primary token-to-runtime-binding handshake; remote JSON/SSE transport for core registry tools (`POST /api/mcp/runtime/tools/call`, `POST /api/mcp/runtime/tools/stream`) added with persisted binding, policy enforcement and audit trail; remote JSON/SSE transport also extended to Alloy scaffold draft tools (`alloy_scaffold_module`, `alloy_review_module_scaffold`, `alloy_apply_module_scaffold`) via server-owned persisted draft store; next step — surface these remote MCP operations in admin UI.

### Observability

- Align metric coverage across all critical endpoints and background event processing.
- Add end-to-end tracing: gateway -> handlers -> modules -> outbox/transport.
- Build SLO dashboards for latency/error budget and health per module.
- Execute and retain RBAC durable-generation metric, lag, worker-restart and full-clear evidence across Redis outage/restart and missed-publication recovery.

### Security

- Strengthen RBAC enforcement checks at middleware and service layer levels.
- Introduce regular security review for sensitive endpoints (auth, tenant, admin operations).
- Expand security event audit (login, privilege changes, tenant boundary violations).
- Retain host-global operator credential denial/admission, rotation and replica-parity evidence without moving credential ownership into tenant OAuth or RBAC.
- Keep role/permission control-plane principal admission owner-defined, typed and ahead of effective permission checks on every transport.

### Test coverage

- Increase integration test share for module scenarios with real DB/migrations.
- Add contract tests for API response stability for frontends.
- Include negative tests for RBAC/tenant isolation and failure-mode tests for event transport.
- Add live HTTP/native host-authority tests for no header, wrong token, read/manage hierarchy, audit identity, Iggy authenticated/resolved tenant ownership, rotation/revocation and WebSocket denial.
- Retain REST, GraphQL and native RBAC tests/verifiers for delegated/service denial, missing typed context, tenant mismatch, missing permission and trusted actor propagation.
- Run the targeted API/auth-resolver/RBAC/Admin/server checks and focused principal-kind and invalidation source verifiers before the complete Wave 2 server sweep.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `pending`
- Last verified at (UTC): `2026-08-01`
- Scope inspected: `cross-owner Tenant trust sweep for host-global Events/Iggy/System/Settings authority; active core/rbac review of access-token claim classification, typed propagation across HTTP/native middleware and GraphQL HTTP/WebSocket, REST artifact-role permission writes and native RBAC Admin; canonical durable-generation watchdog and process telemetry registry`
- Findings: `P0=3, P1=4, P2=1, P3=0` (two host-global P0 findings and one raw-credential-lifetime P1 belong to the Tenant/Events interaction; the third P0 is the historical RBAC REST invalid principal grant; the second P1 is historical native role-metadata admission inconsistency; the third P1 is missing invalidation observability; the fourth P1 is repeated owner inference from grant/client/session metadata; the existing P2 is the earlier module-control-plane construction finding)
- Fixed in this pass: `PR #2735 merged the host-owned operator credential boundary as 1ce83819b077ef6e0df009fd5675f556315ef63a. PR #2747 merged one owner control-plane policy as 75b67f877eb405abe4e6761a16d6b7ece98bc103. Current core/rbac source work adds canonical invalidation observability and introduces host-neutral AuthPrincipalKind plus mandatory AuthPrincipalContext; the access-token resolver is the single classifier, HTTP/native and GraphQL are propagation-only, every RBAC transport consumes the typed context and the owner no longer receives client_id, grant_type or session_id.`
- Remaining risks or blockers: `the complete apps/server Wave 2 inspection has not started; the RBAC corrections lack same-SHA format, rustok-api default/server, telemetry, all-feature owner, admin SSR/server compilation, focused Rust/source verifier tests and live transport evidence; PostgreSQL concurrency, two-replica Redis recovery, one complete authorization incident trace and module-owned management-flow evidence remain absent; host-authority source/unit/compile and live denial/admission/rotation/revocation/replica evidence also remain pending; issues #2680 and #2740 remain open`
- Evidence: `source audit confirms the auth resolver maps validated claims into the closed typed enum and stores it on CurrentUser; middleware and GraphQL HTTP/WebSocket only propagate that value; RBAC GraphQL, REST and native adapters require the typed request context; owner policy contains only kind plus tenant and has no grant/client/session fallback. Source guards cover the single-source typed contract and admission order. No command execution is claimed. Historical PR #2747 head 3cf4b3a44980ca257f7f53849e905673141db289 compiled only default rustok-rbac before issue #2740 stopped the workflow.`
- Next action: `continue the core/rbac cursor: run targeted API/auth-resolver/RBAC Admin/server checks and principal-kind, tenant and invalidation source verifiers on one revision, then retain PostgreSQL concurrency and multi-replica Redis recovery evidence; leave the complete server composition audit for its Wave 2 cursor visit`
- Resume command: `cargo fmt --all -- --check && cargo check -p rustok-api && cargo check -p rustok-api --features server && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-server --lib && cargo test -p rustok-api authenticated_facts_classify_fail_closed && cargo test -p rustok-server --lib token_claim_classifier_returns_explicit_principal_kinds && cargo test -p rustok-rbac --all-features && cargo test -p rustok-server --test rbac_artifact_permission_control_plane_guard && node scripts/verify/verify-rbac-explicit-principal-kind.mjs && node scripts/verify/verify-rbac-admin-tenant-scope.mjs && node scripts/verify/verify-rbac-invalidation-observability.mjs`

## P2 remediation batch (2026-11-01)

This batch continues the server review after the P1 fixes. It is implemented
in the working tree but has not been compiled or exercised against PostgreSQL,
Redis, or a running host in this environment.

### OAuth replay and lineage scope

- `oauth_tokens.authorization_code_id` is an additive nullable column owned by
  `rustok-auth` (`m20261101_000002_add_oauth_token_authorization_code`). It links
  delegated refresh-token lineages to the authorization code that issued them.
  Service-client tokens and pre-migration rows keep NULL.
- Authorization-code exchange now detects replay both before the CAS and on a
  CAS miss. A replay revokes the delegated refresh lineage bound to the same
  code, client, tenant, and subject. The CAS-miss revocation is committed before
  `invalid_grant` is returned.
- Refresh rotation CAS misses run the existing reuse detector. Refresh-family
  root lookup and reuse revocation are scoped by client, tenant, and subject;
  a missing family root fails closed instead of resetting the absolute cap.
- Stateless OAuth access tokens are not extended with a family claim in this
  batch. Their exposure after lineage revocation remains bounded by the
  15-minute user access-token TTL; refresh reuse is denied immediately.

### HTTP edge labels and upload deadlines

- Metrics path labels come from the routed `MatchedPath` captured by a route
  layer. Unmatched requests use fixed `:not_found` / `:unmatched` buckets
  regardless of status; raw URL segments are no longer sanitized into an
  unbounded label vocabulary.
- Extended upload timeouts are limited to the Media owner streaming upload
  route and the registry artifact publication route. Bounded JSON artifact
  dispatch and client-controlled Content-Type alone no longer receive the
  upload deadline.

### Environment and log contracts

- `common::settings::resolve_environment_name` is the canonical selector for
  config loading and production gates. `RUSTOK_ENV`, `RUST_ENV`, and `APP_ENV`
  must agree when present; conflicting aliases fail startup, and earlier
  production checks fail closed.
- Auth entry-point debug logs, disabled email-sender logs, superadmin bootstrap
  logs, and comments schedule-audit ambiguity warnings no longer emit raw
  email addresses, recipients, or claim tokens.

### Verification still required

- `cargo check -p rustok-server`
- `cargo test -p rustok-server oauth_token_service`
- `cargo test -p rustok-server http_stack`
- `cargo test -p rustok-server common::settings`
- `cargo test -p rustok-auth migrations`
- `scripts/verify/verify-migration-smoke.sh` against PostgreSQL
- `scripts/verify/verify-remediation-gate.py` with Cargo available

## P3 runtime-boundary remediation batch (2026-11-01)

This batch continues the bounded server review after P1/P2. It is implemented
in the working tree but has not been compiled or exercised against PostgreSQL,
Redis, or a running host in this environment.

### Module event dispatcher lifecycle

- `spawn_module_event_dispatcher` now starts the module-owned event dispatcher
  only in background-worker profiles (`full` and `worker`). API, admin SSR,
  storefront SSR, and registry-only profiles still compose module runtime
  extensions and individually gated helper workers, but do not start the local
  dispatcher receive loop.
- Sequential dispatcher startup checks for a host-owned
  `ModuleEventDispatcherHandle` in `ServerRuntimeContext`; this check is not
  atomic against concurrent bootstrap. `shutdown_runtime_workers` requests a
  cooperative stop and joins the dispatcher with a bounded timeout, falling back
  to abort if the drain exceeds that deadline. This does not establish durable
  delivery or prevent a concurrent startup/shutdown race.

### Platform settings write serialization

- `SettingsService::update` now reads the existing row, preserves secrets,
  validates, and writes inside one transaction. The update is a CAS against
  `schema_version`; a lost race rolls back and retries with a fresh snapshot.
- First-write races are retried when the unique `(tenant_id, category)` index
  rejects a concurrent insert. After a bounded number of attempts the service
  returns `SettingsError::WriteConflict` instead of silently overwriting.
- GraphQL maps the conflict to a stable retryable client signal. The public
  input/payload still does not expose `schema_version`; callers that need
  end-to-end optimistic concurrency must add an expected-version field in a
  separate API contract change.

### GraphQL channel propagation

- HTTP GraphQL requests now insert the middleware-resolved `ChannelContext`
  into async-graphql request data, so owner resolvers that build `PortContext`
  from `ctx.data::<ChannelContext>()` receive the same channel as REST.
- GraphQL WebSocket upgrades carry the handshake-resolved channel into
  `connection_init` data. The channel tenant must match the tenant resolved
  from `connection_init.tenantSlug`; mismatches fail before authentication
  lease creation. `RequestContext` channel fields are populated for both
  transports instead of being hardcoded to `None` on WebSocket.

### Redis rate-limit connection reuse

- The Redis rate limiter now lazily creates one reconnecting
  `ConnectionManager` per limiter and shares it across clones, namespaces,
  limit checks, and health probes. The existing two-second Redis operation
  timeout still bounds connection acquisition and commands.
- This removes per-request connection churn while preserving the distributed
  Lua INCR/EXPIRE/TTL semantics and the memory-backend fallback.

### RBAC availability fail-closed presentation

- When tenant module-availability resolution fails, the host provider no
  longer returns `None` (which the RBAC owner interprets as “all permissions
  are platform-core”). It returns a snapshot with known permission ownership
  and an empty enabled-module set, so module-owned permissions are presented
  as dormant and newly dormant grants are rejected.
- Hosts without a module registry still return `None` through the existing
  handle construction path; this change only affects resolution failures on
  hosts that do have a registry.

### Verification still required

- `cargo check -p rustok-server`
- `cargo test -p rustok-server module_event_dispatcher`
- `cargo test -p rustok-server app_lifecycle`
- `cargo test -p rustok-server settings_service`
- `cargo test -p rustok-server graphql::settings`
- `cargo test -p rustok-server controllers::graphql`
- `cargo test -p rustok-server rate_limit`
- `cargo test -p rustok-server rbac_module_availability`
- PostgreSQL concurrent settings-update and unique-insert contention tests
- Redis outage/reconnect and rate-limit contention tests
- `scripts/verify/verify-remediation-gate.py` with Cargo available


## P4 protocol-boundary remediation batch (2026-11-01)

Scope: settings write-conflict taxonomy, RFC 7009 refresh-family revocation,
and GraphQL WebSocket operation/auth lifetime bounds. This batch does not
introduce a client-visible settings version contract and does not add a
cooperative dispatcher drain; those remain separate follow-ups.

### Implemented

- `rustok-api` GraphQL error taxonomy now includes `ErrorCode::Conflict`
  (`CONFLICT`) and `GraphQLError::conflict`. Platform settings
  `WriteConflict` maps to that extension code instead of `BAD_USER_INPUT`,
  so clients can distinguish retryable optimistic-concurrency conflicts from
  invalid input. Regression tests assert the extension code in both the API
  crate and the server settings mutation mapper.
- Settings unique-violation detection uses SeaORM's portable
  `DbErr::sql_err()` / `SqlErr::UniqueConstraintViolation` classifier instead
  of matching only one runtime-error shape.
- RFC 7009 revocation is transactional and family-scoped. The service reads
  the token by hash inside the transaction, coalesces `family_id` to the row
  id for pre-lineage rows, and revokes the whole lineage with one
  `update_many` scoped by client, tenant, and user (or `user_id IS NULL` for
  service tokens). Unknown hashes still succeed without revealing existence.
  The controller uses the security-only OAuth client lookup so presentation
  translation hydration cannot decide credential validity. SQLite regression
  tests cover unknown-token success, whole-family idempotent revocation, and
  cross-client/cross-tenant isolation.
- GraphQL WebSocket hosts admit operations as the protocol engine consumes
  queued input: at most 32 active ids, ids must be non-empty and at most
  128 bytes, duplicate ids close with 4409, overflow with 4429, and invalid
  ids with 4400. Slots are released when the server emits `complete`.
  `CurrentUser` carries the signed access-token expiry; the WS lease closes
  at that expiry and revalidates authorization every 30 seconds while idle.
  Pure tracker tests cover cap/duplicate/release/id validation.

### Known limitations / pending implementation

- OAuth access JWTs are registered credentials. RFC 7009 and security-family revocation
  revoke access and refresh rows together, fencing already-issued access JWTs before TTL
- A shorter service-token TTL policy and an OAuth code-reuse domain event
  remain separate contract changes.
- PostgreSQL/Redis fault-injection checks remain a deployment gate.

### Verification status

Written without a local Rust toolchain; not compiled or tested in this
sandbox. Run before merge:

```bash
cargo check -p rustok-api
cargo test -p rustok-api --lib
cargo check -p rustok-server
cargo test -p rustok-server --lib graphql::settings
cargo test -p rustok-server --lib services::oauth_app
cargo test -p rustok-server --lib controllers::graphql
cargo test -p rustok-server --lib extractors::auth
```


## Runtime task drain completion (2026-11-12)

Scope: close confirmed shutdown gaps in runtime-owned tasks that subscribe to
the host stop signal or have an explicit runtime handle. This is not a
repository-wide detached-task audit; test-only spawns, supervisors whose
abort-on-drop ownership already bounds lifetime, and unrelated service
lifecycle work are outside this batch.

- Host shutdown now takes and joins the physical Event DLQ duplicate-alert
  observer and its observability projection, Product channel reconciliation,
  Product Index refresh, and Social Graph Index consumer/position/poison
  observer handles, and the Comments schedule-audit handoff worker after
  broadcasting stop. Both Forum Search workers (inbox and typed-contract
  consumer) are drained too. Tenant cache-generation listeners now retain every
  spawned local/Redis/reconciliation task for the same drain. Tenant locale
  listener tasks, previously cancelled only when their abort-on-drop handle was
  dropped, are also explicitly cancelled and joined by host shutdown. The same
  explicit drain also covers Channel/RBAC invalidation listeners, the RBAC
  generation watchdog, Event forwarder, Cache Redis status monitor,
  field-definition cache invalidation/reconciliation workers, and the
  feature-gated SEO redirect-cache reconciler. Their abort-on-drop wrappers
  remain as a fallback.
- Module-worker drains now retain abort-on-drop `JoinHandle`s through timeout;
  if a worker exceeds the deadline, shutdown aborts it and awaits cancellation
  instead of dropping the join future and detaching the task. If the host drain
  itself is cancelled, the local guards still abort owned tasks.
- The Forum Search typed-contract consumer handle is included alongside the
  inbox worker; it was previously absent from the host drain.
- The Outbox relay supervisor uses a `JoinSet` for its inner processing task:
  shutdown aborts and joins it, and dropping the supervisor aborts children
  instead of detaching them.
- Each drain is bounded to five seconds. If a task does not cooperate, shutdown
  aborts it and awaits the join result rather than dropping the JoinHandle and
  leaving the task detached.
- Added a regression test for the timeout abort-and-join fallback. Existing
  signal/handle shutdown tests continue to cover the previously fixed listener
  and rate-limit cleanup tasks.
- The one-shot installer HTTP execution task remains outside this worker-drain
  batch. Its durable `running` row is protected by a cross-replica unique index,
  so stale-job recovery/shutdown handling needs a separate ownership and
  recovery design; it must not be cleared by a blind startup sweep.
- Cargo is unavailable in this environment, so compilation and Rust tests remain
  unverified; `git diff --check` is the available whitespace check.

## P5 dispatcher cooperative-drain batch (2026-11-01)

Scope: close the remaining dispatcher shutdown gap from P3. The core
`EventDispatcher` now owns a cooperative stop signal and a real in-flight
task boundary; host shutdown uses it instead of aborting the receive loop
as the primary path.

### Implemented

- `RunningDispatcher` carries a `watch` stop signal. `request_stop` stops
  admission of new events; the receive loop then drains its `JoinSet` of
  admitted dispatch tasks before exiting. `stop` is now an async
  request-stop-plus-join operation, while `abort`/`abort_handle` remain the
  emergency cancellation path.
- Concurrent handler tasks are no longer detached from the dispatch task.
  `dispatch_to_handlers` owns a handler `JoinSet`, waits for all matching
  handlers, then releases backpressure and records dispatch latency. This
  makes dispatcher drain observable at the handler boundary and prevents
  finished-task retention in the receive-loop `JoinSet`.
- Host shutdown calls `ModuleEventDispatcherHandle::request_stop`, joins
  under the existing five-second module-worker deadline, and aborts through
  an `AbortHandle` only if the drain times out.
- Existing core/blog/commerce dispatcher callers were updated to the async
  `stop` contract. Regression tests cover cooperative drain of an in-flight
  blocking handler, host-handle cooperative join, and the existing abort
  fallback.

### Known limitations / pending implementation

- Drain is bounded by the host deadline; a handler that ignores cancellation
  and blocks past the deadline is still aborted with the dispatcher task.
  Durable recovery remains the responsibility of the outbox/worker contracts.
- Client-visible expected-version settings CAS and stale source-guard
  stabilization remain separate follow-ups.

### Verification status

Written without a local Rust toolchain; not compiled or tested in this
sandbox. Run before merge:

```bash
cargo check -p rustok-core
cargo test -p rustok-core --lib events::handler
cargo check -p rustok-server
cargo test -p rustok-server --lib services::module_event_dispatcher
cargo test -p rustok-blog --test comment_projection_postgres_test
cargo test -p rustok-blog --test comment_projection_dispatcher_duplicate_postgres_test
cargo test -p rustok-commerce --test integration
```

## P6 — Client-visible optimistic concurrency for platform settings

Internal compare-and-swap alone still lets two admins load the same settings page and silently
overwrite each other. The generic settings write path now exposes the persisted revision to every
current admin client:

- `SettingsService` returns `SettingsSnapshot { settings, schema_version }` from `get_snapshot` /
  `get_all_snapshots`. A missing row is revision `0`; the cache category still reports the
  deployment-resolved document but carries the persisted row's revision.
- `update_if_current(..., expected_schema_version)` checks the expected revision inside the same
  transaction that reads, validates, preserves secrets, and writes the row. A mismatch rolls back
  and returns `WriteConflict`; negative expectations are validation errors. The bounded internal
  retry loop remains only for the internal/bootstrap `update` wrapper.
- GraphQL `PlatformSettingsPayload` and `UpdatePlatformSettingsPayload` expose `schemaVersion`.
  `UpdatePlatformSettingsInput` requires `expectedSchemaVersion`; stale writes map to the shared
  `CONFLICT` error so clients can re-read and retry rather than assuming success.
- `rustok_cache::CacheSettingsAdminPort` now reads/updates `CacheSettingsSnapshot` and requires an
  expected revision. The host adapter delegates to `SettingsService`, and the cache admin Leptos
  transport/UI keeps the loaded revision and advances it only after a successful save.
- `apps/admin` email settings and the Next.js email/cache admin packages were updated in lockstep:
  queries/mutations select `schemaVersion`, forms store the loaded revision, and saves send
  `expectedSchemaVersion`. There is no client-side blind-write fallback.

Regression coverage:

- SQLite service test: revision `0` insert, sequential revision increments, stale expected revision
  conflict, stored settings unchanged after conflict, and negative revision rejection.
- Cache host adapter tests assert revision increments and that the invalid-mode update uses the
  loaded revision.
- Cache port recording test asserts a stale update is rejected before recording a write.

Verification limits: no Rust toolchain or database runtime is available in this workspace, so
`cargo check` / `cargo test` and the remediation gate remain pending. Static verification only:
tree-sitter parse of changed Rust files (two pre-existing unrelated ERROR nodes), `git diff --check`,
remediation diff invariants, the runtime-context/auth-admin-tenant-scope guards, and the updated
server platform-settings tenant-scope guard (now asserting the snapshot/CAS GraphQL contract).

## P7 — OAuth deployment gate and authorization-code retention

The `features.oauth_enabled` setting previously deserialized but did not affect runtime composition:
OAuth endpoints and discovery metadata were mounted even when the feature was disabled. The host now
treats the flag as a real fail-closed composition gate:

- `build_application_router` mounts `controllers::oauth` and `controllers::oauth_metadata` only when
  `settings.features.oauth_enabled` is true.
- OpenAPI prunes `/api/oauth/*` when the gate is closed, so the published contract matches the mounted
  surface.
- Router tests assert closed-gate 404s for token/revocation/discovery paths and an open-gate mounted
  token endpoint. An OpenAPI test asserts pruning and restoration.

Auth retention also now purges expired `oauth_authorization_codes`. A used code is the replay anchor
for the delegated refresh lineage it issued, so codes are retained on the same 120-day outer window as
OAuth token families rather than the shorter 30-day session/invite window. The retention worker reports
the new per-table count, and a SQLite regression test proves old codes are removed, recent used codes
survive, and a second purge is idempotent.

Verification limits: no Rust toolchain or database runtime is available in this workspace, so
`cargo check` / `cargo test` and the remediation gate remain pending. Static verification only:
tree-sitter parse of changed Rust files (two pre-existing unrelated ERROR nodes), `git diff --check`,
remediation diff invariants, and the existing runtime-context/auth-admin-tenant-scope/platform-settings
guards.


## P8 — OAuth access-token revocation fence

OAuth access JWTs now carry a `jti` bound to a durable `oauth_tokens` access row
(`token_kind = access`). The extractor requires an active, hash-bound, client/tenant/grant/
subject-scoped row, so RFC 7009, app/consent revocation, credential change, code replay, and
refresh reuse fence already-issued access JWTs before their signed TTL. Delegated rows capture
`consent_granted_at`; consent revoke/re-grant cannot resurrect older credentials. Refresh-only
queries filter `token_kind = refresh`, and admin active-token counts remain refresh-lineage counts.

Regression coverage:

- SQLite RFC 7009 test revokes an access registration and its refresh family by access hash.
- Extractor tests reject unregistered and revoked OAuth access registrations.
- Existing refresh rotation / code replay tests now include access rows and the new columns.

Verification limits: no Rust toolchain or database runtime is available in this workspace, so
`cargo check` / `cargo test` and the remediation gate remain pending. Static verification only.

## PR integration and verification handoff (2026-11-02)

The P1–P8 changes were prepared from base
`0aaf455a45556f4e39a5ed66b8a1184369e5e93f`. Before opening the draft PR, `main` was
refreshed to `a418bbbed64e53cdf841b87dada7ae5799770cd2` and merged into the task branch.
The integration preserves the upstream runtime/readiness changes, ADR registry repairs,
and admin contract tests. No audit-ledger advancement is part of the task delta.

The upstream cache/email admin fixtures now exercise the versioned settings contract:
they submit `expectedSchemaVersion`, retain the returned revision when normalizing invalid
JSON, propagate conflicts, and reject a missing revision instead of inventing a CAS baseline.
The settings source guard checks both transactional event publication and the CAS entrypoint.

Executed local evidence:

- `node --test apps/next-admin/packages/cache/src/cache-admin.test.mjs apps/next-admin/packages/email/src/email-admin.test.mjs` — 19 passed, 0 failed.
- `npm run verify:docs` — passed.
- `node scripts/verify/verify-adrs.mjs` — passed for 164 decisions.
- Runtime-context, auth-admin tenant-scope/boundary, explicit RBAC principal-kind, and
  platform-settings tenant-scope source guards — passed.
- Task delta against refreshed `main`: `git diff --check` and remediation diff invariants — passed.

Still blocked locally: cargo check/test/clippy/format, Rust crate contract tests, full Rust
remediation/architecture gates, admin typechecks/builds, and PostgreSQL/Redis fault injection.
The environment has no Cargo/Rust toolchain, PostgreSQL, Redis, Docker, or installed frontend
dependencies. These are CI/maintainer gates, not claimed passing results. The PR remains a
draft until the affected crate and integration checks have actually run.

Deferred work remains the service-token TTL policy, the cross-crate OAuth code-reuse domain
event, deduplication of `RbacService::record_claim_role_mismatch()`, and database/Redis
failure-path evidence. The historical source-guard/ADR failures recorded in earlier batches
were resolved by the refreshed `main`; the current runs above pass.


## P9 — Empty module listener registry routing (2026-11-18)

A worker host must bind the module listener registry even when the composed
module set registers zero handlers. Previously the host treated an empty set as
a reason not to start the dispatcher. That left the configured transport unable
to distinguish an intentionally empty registry from a dispatcher that had not
initialized: remote-only sealed families were refused before reaching Iggy, and
the single-node outbox target had no local receiver for root events.

### Implemented

- Worker hosts now start the dispatcher for both populated and empty listener
  sets. The empty registry is explicit and remains part of readiness; it is not
  represented as a missing worker.
- Required local sealed families still fail closed when no matching handler
  exists. Remote-only families can be sent to the configured primary when the
  registry is explicitly empty.
- Added regression coverage for empty-registry startup and remote-only delivery.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::module_event_dispatcher
cargo test -p rustok-server --lib services::event_transport_factory
```



## P10 — Unique notification lease-owner identities (2026-11-18)

The server adapters previously derived the distributed lease owner for
notification fanout and candidate work from a process-local atomic counter.
Every replica therefore started with the same owner string (for example,
`notification-fanout-1`). The owner services fence completion by comparing
that string and the lease expiry. If a slow worker outlived its lease and another
replica reclaimed the row with the same owner string, the stale worker could
mistake the new lease for its own and commit after the reclaim.

### Implemented

- Fanout and candidate lease-owner IDs now include a fresh UUID for each worker
  instance. The local atomic instance ID remains only for host-local handle
  identity and diagnostics.
- Added regression tests proving that two worker instances with the same local
  counter value receive different lease-owner IDs and remain within the owner
  contract's length limit.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::notification_fanout_worker
cargo test -p rustok-server --lib services::notification_candidate_worker
```



## P11 — Comments worker readiness visibility (2026-11-18)

The Comments TCP listener and schedule-audit handoff worker already had bounded
shutdown drains, but neither appeared in the runtime worker snapshots. A task
could therefore terminate unexpectedly while the HTTP health endpoint remained
green.

### Implemented

- The optional Comments TCP listener and schedule-audit handoff worker are now
  included in readiness whenever their handles are present.
- Added a lifecycle regression test that verifies both workers are required
  while running and are removed by the normal shutdown drain.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib --features mod-comments
cargo test -p rustok-server --lib services::app_lifecycle::lifecycle_tests::comments_listener_and_audit_handoff_workers_are_required_when_started
```



## P12 — Notification intake versus Outbox retention (resolved by P18)

The original failure boundary was real: Notifications persisted accepted
receipts and permanent rejections in owner tables while Outbox pruned dispatched
`sys_events` rows solely by age. A stalled intake worker could therefore lose
its source envelope before recording a terminal outcome.

P18 resolves the cross-owner boundary with Outbox-owned durable consumer
registrations and per-event acknowledgements. The Outbox owner does not query
notification-owned tables, and Notifications does not mutate relay state.
See the accepted [consumer-retention decision](../../../DECISIONS/2026-11-18-outbox-consumer-retention-acknowledgements.md).

The guarantee applies to rows that still exist when the consumer registration
is committed. Already-pruned historical events cannot be reconstructed; old
terminal notification receipts are backfilled before retention starts.



## P13 — Acknowledge root-event projections before outbox completion (2026-11-18)

The sealed contract path already awaited matching module handlers, but root
`DomainEvent` delivery only broadcast into an in-memory channel. The outbox
relay could mark a row dispatched before root projections completed; handler
errors after the configured retry budget were logged and swallowed. In the
Iggy profile, a local root-projection failure was also swallowed after remote
acceptance, leaving no durable local retry.

### Implemented

- Added an acknowledged root-delivery path through the existing explicit module
  listener registry. It waits for every matching handler and propagates handler,
  capacity, and shutdown failures to the outbox relay.
- The single-node `outbox` profile now uses the acknowledged local target.
  The `outbox_iggy` profile also returns local projection failures so the
  canonical outbox envelope is retried unchanged.
- An unavailable local registry is rejected before irreversible remote
  publication. A projection failure after remote acceptance may still cause
  the same envelope to be published again; remote consumers must be idempotent.
- Added regression coverage for handler completion, projection failure,
  explicit empty registries, primary refusal, and retry with the same event ID.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-core --lib
cargo test -p rustok-core --lib events::contract_delivery
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::event_transport_factory
```



## P14 — Retry and graceful shutdown for the in-process event forwarder (2026-11-18)

The server event forwarder previously consumed an in-memory event and only
logged a transport error. A transient database failure therefore lost the
event before it reached the durable outbox. Backpressure accounting also had
no release path for forwarded events or lagged messages, and the configured
queue depth could exceed the Tokio broadcast ring capacity, allowing the ring
to drop events before backpressure rejected new publishers. Shutdown aborted
the forwarder immediately, discarding queued or in-flight events.

### Implemented

- The forwarder retries the same envelope with bounded exponential delay until
  the transport accepts it; transport panics are caught and retried as well.
- Root OutboxTransport publication is idempotent by envelope ID and verifies
  exact canonical facts before accepting a duplicate, covering ambiguous
  database commit responses.
- Backpressure permits are released after persistence, for lagged messages,
  and when queued messages are discarded during task teardown.
- The effective backpressure depth is clamped to the broadcast capacity, and
  permit acquisition uses compare-and-swap so concurrent publishers cannot
  exceed the configured critical depth.
- Shutdown signals the forwarder to drain accepted messages and allows up to
  30 seconds before forced abort. P17 orders this dedicated drain after producer
  workers have joined.
- Added regression coverage for transient transport failure, shutdown draining,
  permit release, concurrent backpressure acquisition, and idempotent outbox
  retries.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-core --lib
cargo test -p rustok-core --lib events::backpressure
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::event_bus
cargo test -p rustok-server --lib services::app_lifecycle
cargo test -p rustok-outbox --test sqlite_transport_regression_test
```



## P15 — Event forwarder readiness visibility (2026-11-18)

The event forwarder was supervised and drained during shutdown, but it was not
included in the runtime worker readiness snapshot. If its supervisor exited,
the HTTP host could remain green even though the in-memory-to-outbox bridge was
no longer running.

### Implemented

- A started event forwarder is now a required worker in local runtime readiness.
- Readiness reports it as finished if its supervisor task has terminated.
- Added a lifecycle regression assertion for the forwarder's required/live
  snapshot state.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::app_lifecycle::tests::shutdown_aborts_invalidation_and_event_supervisors_even_with_live_clones
```



## P16 — Cross-cutting event-delivery invariants documented (2026-11-18)

Codified the recurring failure patterns found in the server audit in
`docs/standards/logic-hygiene-patterns.md`: replica-unique lease ownership,
atomic backpressure reservation, durable idempotent forwarding, acknowledged
projection delivery, and cooperative shutdown. Each invariant now includes a
specific regression check so future changes can preserve the same guarantees.


## P17 — Close event-forwarder admission and shutdown races (2026-11-18)

The P14 bridge still had two loss windows: with the default
`backpressure.enabled = false`, the Tokio broadcast ring could overwrite
accepted events while the forwarder retried a failed outbox write; and the
shared stop signal let the forwarder drain concurrently with producers that
were still completing in-flight work. A third startup race allowed the first
publisher to run before the spawned forwarder had subscribed.

### Implemented

- Install a hard admission controller for the server-owned EventBus-to-Outbox
  forwarder bus, including when configurable backpressure is disabled. The hard
  depth equals the broadcast capacity; enabling backpressure adds configurable
  early thresholds and a possibly smaller depth.
- Honor configured `channel_capacity` even when EventBus is requested before
  EventRuntime has been composed.
- Subscribe the initial forwarder receiver synchronously before returning the
  EventBus to its first publisher; supervised restarts create a fresh receiver.
- Give the forwarder a dedicated stop signal. The global stop first quiesces
  producers; the forwarder is stopped and drained only after all other
  registered worker drains have joined.
- Add regressions for default hard admission, first-publish subscription, and
  producer-first shutdown draining.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::event_bus
cargo test -p rustok-server --lib services::app_lifecycle
```


## P18 — Retain Outbox events until required consumers acknowledge (2026-11-18)

The Notifications intake worker previously wrote terminal outcomes into
notification-owned tables, while Outbox retention deleted dispatched source
events by age alone. A prolonged intake outage could therefore remove an event
before its notification receipt or permanent rejection existed.

### Implemented

- Added Outbox-owned `sys_event_consumers` registrations and
  `sys_event_consumer_receipts` acknowledgements. Retention now deletes a
  dispatched event only after every registered consumer for its event type has
  acknowledged the canonical event ID.
- Added an append-only platform migration and shared standalone schema helper.
  Receipt rows cascade with the canonical Outbox event.
- Notifications registers its supported event types before the server starts
  runtime workers and retention. A neutral receipt port commits the Outbox
  acknowledgement in the same transaction as the accepted intake receipt or
  permanent rejection; retryable failures create neither terminal receipt nor
  acknowledgement.
- Added bounded startup backfill for existing terminal Notifications receipts
  before retention starts. Already-pruned IDs are harmless because the owner
  terminal receipt is already durable.
- Added regression coverage for multi-consumer retention holds, idempotent
  acknowledgements, and atomic notification receipt/Outbox acknowledgement.
- Registration is durable and additive. Disabling intake does not silently
  release its retention obligation; unacknowledged events remain stored until
  intake recovers or an operator explicitly retires the consumer after drain.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-outbox-storage --lib
cargo test -p rustok-outbox-storage --lib consumers
cargo check -p rustok-outbox --lib
cargo test -p rustok-outbox --lib retention
cargo check -p rustok-notifications --all-targets
cargo test -p rustok-notifications --test outbox_intake_sqlite
cargo check -p rustok-server --lib --features mod-notifications
cargo test -p rustok-server --lib services::notification_outbox_intake_worker
cargo test -p rustok-migrations
node scripts/verify/verify-notifications-outbox-intake.mjs
node scripts/verify/verify-outbox-retention-and-event-metrics.mjs
node scripts/verify/verify-migration-backfill-contracts.mjs
```

PostgreSQL migration and concurrent retention/acknowledgement behavior remain
maintainer/CI gates.


## P19 — Release backpressure reservations on dispatcher lag (2026-11-18)

The module dispatcher handled `broadcast::RecvError::Lagged` by advancing the
receiver and continuing, but did not release the backpressure reservations for
the skipped envelopes. Under a slow handler, the broadcast ring can overwrite
events; those accepted-but-lost messages therefore remained counted forever and
could eventually make the backpressure controller reject healthy publishers.

### Implemented

- Release one EventBus backpressure reservation for every skipped envelope when
  the module dispatcher observes broadcast lag.
- Clamp dispatcher semaphore capacities in the legacy `start()` path so
  externally supplied oversized `DispatcherConfig` values cannot panic inside
  the spawned task.
- Use saturating retry-attempt arithmetic so `retry_count = usize::MAX` cannot
  overflow before handler execution.
- Added a regression test proving lagged reservations return the controller to
  zero after the retained event completes.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-core --lib
cargo test -p rustok-core --lib events::handler
```



## P20 — Surface core runtime worker failures in readiness (2026-11-18)

The readiness review identified the Outbox relay, Outbox retention,
remote-executor reaper, and SEO workers as critical runtime obligations.
The health controller already owned explicit, configuration-aware checks for
these workers. Mirroring them in `additional_runtime_worker_snapshots` was
redundant and emitted duplicate Outbox/remote-executor checks plus a second,
non-canonical SEO worker label. P21 records and corrects that implementation
mistake; the explicit checks in `health.rs` remain authoritative.

### Implemented

- Kept the existing explicit checks for Outbox relay/retention,
  remote-executor reaper, and each enabled SEO worker as the single source of
  readiness truth.
- Kept the event-forwarder snapshot in the additional worker list because it
  has no separate explicit check; it becomes required when the EventBus
  forwarder handle is registered.
- Added a regression test that rejects duplicate readiness check names and
  asserts one canonical check for each core worker.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib controllers::health::tests::runtime_worker_readiness_uses_one_canonical_check_per_worker
cargo test -p rustok-server --lib services::app_lifecycle::tests::additional_workers_respect_host_modes_and_auth_retention_config
cargo check -p rustok-server --lib --features commerce-marketplace-financial
cargo check -p rustok-server --lib --no-default-features --features mod-seo
```


## P21 — Keep runtime worker readiness single-source (2026-11-18)

A source-level re-review found that `controllers/health.rs` already creates
configuration-aware checks for the Outbox relay, Outbox retention,
remote-executor reaper, and SEO worker variants. The additional lifecycle
snapshot list had duplicated those checks. The extra Outbox/remote entries
reused the same check names; the SEO entry used a second name for the same
supervised handle. Duplicate readiness entries can confuse operators and
produce repeated worker metrics.

### Implemented

- Removed the redundant Outbox relay, Outbox retention, remote-executor, and
  SEO entries from `additional_runtime_worker_snapshots`; the existing
  `health.rs` checks remain the only canonical entries for those workers.
- Retained the EventBus forwarder snapshot, which has no separate explicit
  check, and classified it as optional until its handle is registered.
- Removed a duplicate feature-gated import that would fail compilation when
  `commerce-marketplace-financial` is enabled.
- Removed the redundant SEO snapshot (which had been gated by both
  `mod-workflow` and `mod-seo`); the canonical SEO readiness checks remain
  independently gated by `mod-seo`.
- Added a regression test asserting readiness check names are unique and that
  Outbox, remote-executor, and SEO worker checks each have one canonical entry.
- Updated the host-mode snapshot test to treat EventBus forwarding and optional
  Comments workers as optional until their runtime handles are registered.
- Updated the readiness runbook to document Outbox retention, EventBus
  forwarder, and the three independently configured SEO worker checks.

### Verification status

Not run in this task, at the user's request. No Rust compilation, formatting,
or tests are claimed. Run the commands listed under P20 on one revision before
integration; tests remain with the maintainer.

## P22 — Atomic tenant-scoped OAuth manifest reconciliation (2026-11-18)

The manifest sync previously committed app upserts and translation writes
individually, then used a separate transaction for orphan cleanup. A conflict
or storage failure late in the pass could leave a tenant with a partially
updated OAuth application set. Concurrent server replicas could also perform
the same read/upsert/orphan sequence without a tenant-level serialization fence.

### Implemented

- The sync_app_connections flow now acquires an exclusive tenant-row lock before
  reading the current managed-app set. PostgreSQL/MySQL use FOR UPDATE;
  SQLite takes its write fence before the first read. Existing managed-app
  rows are then locked with the same app-mutation fence used by token issuance
  and consent before security-sensitive configuration is updated.
- App creation/reactivation, first-party manifest updates, translation writes,
  orphan deactivation, token revocation, consent revocation, and authorization-code
  invalidation now share one transaction per tenant.
- A failure in any manifest entry or orphan cleanup rolls back all earlier
  writes from that tenant's reconciliation.
- Reconciliation includes already-inactive managed apps, repairs stale credentials
  left by older versions, and clears credentials again before reactivating a
  previously disabled app. Embedded-app reactivation also checks the combined
  active/revoked state instead of the raw is_active column alone.
- Explicit manual app revocation uses the same credential cleanup path and
  revokes active user consents alongside tokens and pending authorization codes.
- Added a regression case where an embedded app is upserted before a later
  manual-app slug conflict; the earlier embedded-app write must not leak.
- The all-tenants bootstrap remains a sequence of per-tenant transactions,
  not a single global transaction across every tenant. A failure can leave
  earlier tenants committed while later tenants have not yet been processed;
  rerunning the idempotent sync reconciles those tenants.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo test -p rustok-server --lib services::oauth_app::tests::manifest_sync_refuses_to_take_over_a_manual_app_with_a_matching_slug
cargo test -p rustok-server --lib services::oauth_app::tests::app_revocation_revokes_active_user_consents
cargo test -p rustok-server --lib services::oauth_app::tests::manifest_sync_reactivates_embedded_app_with_revocation_timestamp
cargo test -p rustok-server --lib services::oauth_app::tests::sync_app_connections_creates_manifest_managed_apps_and_deactivates_orphans
cargo test -p rustok-server --lib services::oauth_app::tests::orphan_deactivation_database_failure_is_not_silently_ignored
```

## P23 — Unique installer lease-owner identities (2026-11-18)

The HTTP adapter used the shared literal owner `http`, while the CLI used the
shared literal `rustok-cli install apply`. Those labels were not unique to an
execution attempt. The persistence layer records the owner at acquisition but
does not yet compare it on every state/receipt write, so unique values alone
are diagnostic/preparatory rather than a complete stale-worker fence.

### Implemented

- Added one shared lock-owner builder in `rustok-installer`. It sanitizes a
  short diagnostic label and appends the optional stable session/job ID plus a
  fresh attempt UUID; the result is bounded below the `VARCHAR(128)` session
  column limit.
- The HTTP adapter now uses its durable job/session ID and a fresh attempt ID.
- The CLI adapter and `InstallApplyOptions::default()` also generate a fresh
  attempt-specific owner rather than a static label.
- Added contract coverage for unique owners across attempts, label
  sanitization, storage length, and unique defaults; added HTTP-adapter coverage.

### Remaining boundary

This is a unique owner token, not a monotonic attempt-generation protocol,
and persistence writes do not yet verify owner equality. The fail-closed
session-admission gate added in P25 prevents automatic overlap after lease
expiry, but the HTTP executor still runs in-process, is not resumable after
process loss, and does not reconcile ambiguous external effects. Stale
`running` jobs must not be reset based only on lease expiry. The durable
recovery ADR remains in progress.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo test -p rustok-installer --lib --features host-runtime install_lock_owner_is_unique_per_attempt_and_bounded
cargo test -p rustok-server --lib controllers::installer::tests::installer_lock_owner_is_unique_per_attempt_and_bounded
cargo check -p rustok-installer-cli
```

## P24 — Bounded installer lease TTL input (2026-11-18)

The HTTP adapter accepted arbitrary `lock_ttl_secs` values while the CLI only
checked positivity. The persistence layer added the TTL to the current
timestamp directly, so an extreme value could overflow the timestamp range.

### Implemented

- Added a shared contract: installer lease TTL must be between 1 and 86,400
  seconds (default 900). The execution state machine validates it even when
  called outside the HTTP/CLI adapters.
- HTTP and CLI adapters reject zero, negative, and over-limit values before
  creating or starting an installer job.
- Persistence now uses checked timestamp addition and returns a database error
  instead of panicking if a direct caller supplies an unrepresentable duration.
- Added shared contract and HTTP-controller regression coverage for both bounds.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo test -p rustok-installer --lib --features host-runtime installer_lock_ttl_is_bounded
cargo test -p rustok-server --lib controllers::installer::tests::installer_http_lock_ttl_rejects_zero_negative_and_overflowing_values
cargo check -p rustok-installer-cli
cargo test -p rustok-installer-persistence --lib
```

## P25 — Fail-closed installer session admission (2026-11-18)

Lock arbitration previously treated `lock_expires_at` as proof that a worker
had stopped. A new apply could therefore start while a stale executor was
still running. The current persistence adapter does not yet compare owner
identity on every state/receipt mutation, so automatic lease-based takeover
was unsafe.

### Implemented

- A session row can be claimed only once (`lock_owner IS NULL` and
  `status = draft`); a second attempt cannot overwrite its existing owner.
- Any other unreconciled session with a recorded owner blocks a new apply
  even when its lease timestamp has expired. Expiry alone is no longer treated
  as proof that the previous process stopped. `recovery_required` also blocks
  admission until manual reconciliation, despite being terminal for execution.
- PostgreSQL and MySQL lock acquisition now uses a serializable transaction;
  SQLite obtains its write fence through the conditional session-claim update.
- `completed`, safely `failed`, and `fresh_install_cleaned` sessions release
  the global admission gate. `recovery_required` and stale unfinished sessions
  require explicit reconciliation before another apply can start.
- Regression coverage verifies same-session re-claim rejection, expired-lease
  blocking, and release after finalization.

### Remaining boundary

State, tenant, and receipt mutations do not yet compare the persisted owner on
every write. There is no monotonic attempt generation, periodic lease renewal,
resumable executor, or automatic external-effect reconciliation. The fail-closed
gate prevents automatic overlap but intentionally leaves stale sessions blocked
until an operator reconciles them. The durable recovery ADR remains in progress.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo test -p rustok-installer-persistence --lib session_persistence_lifecycle_and_state_transitions
cargo test -p rustok-installer-persistence --lib acquire_lock_rejects_out_of_range_ttl_before_claiming_session
cargo test -p rustok-installer --lib --features host-runtime install_lock_owner_is_unique_per_attempt_and_bounded
cargo test -p rustok-server --lib controllers::installer::tests::installer_lock_owner_is_unique_per_attempt_and_bounded
cargo test -p rustok-server --lib controllers::installer::tests::installer_http_lock_ttl_rejects_zero_negative_and_overflowing_values
cargo check -p rustok-installer-cli
```

## P26 — Serialize first-party refresh-token reuse revocation (2026-11-18)

Refresh-token reuse detection previously read the immediately preceding token
hash without locking its session row, then converted the read snapshot into an
ActiveModel update. A concurrent successful refresh could rotate the row
between those operations; the stale update could then overwrite the newer
token hash or expiry while revoking the session.

### Implemented

- Reused the database-specific previous-session lookup used by logout, which
  acquires `FOR UPDATE` on PostgreSQL/MySQL and a conditional SQLite write
  fence before returning the row.
- Refresh reuse detection now holds that row lock while evaluating the grace
  window and applying the revocation update.
- Session-id logout no longer reads an active session and then updates its stale
  snapshot without a lock. It locks the row on PostgreSQL/MySQL and reserves a
  SQLite write before reading, then publishes the logout event in the same
  transaction as the revocation.
- Added regression tests for an in-grace duplicate (reject without revoking),
  post-grace replay (revoke the live lineage), and failed revocation persistence
  (propagate the storage error and preserve the active session).
- Corrected the grace-window contract for both first-party and OAuth
  refresh: it suppresses revocation for a likely concurrent duplicate but is
  not an idempotent retry. The old token still receives an invalid-token error;
  if the successful response is lost, it cannot recover the replacement
  cookie/token. Full rotation history and idempotent response recovery remain
  open owner-level design gaps.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::auth_lifecycle::tests::refresh_reuse_inside_grace_is_rejected_without_revoking_live_session
cargo test -p rustok-server --lib services::auth_lifecycle::tests::refresh_reuse_after_grace_revokes_live_session_lineage
cargo test -p rustok-server --lib services::auth_lifecycle::tests::refresh_reuse_revocation_failure_is_propagated_without_false_success
cargo test -p rustok-server --lib services::auth_lifecycle::tests::logout_can_find_previous_rotated_token_without_crossing_tenant
cargo test -p rustok-server --lib services::auth_lifecycle::tests::logout_by_session_id_revokes_under_lock_and_is_idempotent
```

## P27 — Use write-first SQLite fences in auth and OAuth mutations (2026-11-18)

Several auth and OAuth mutation paths used SELECT followed by a no-op
UPDATE to acquire SQLite's writer lock. In a deferred transaction this can
race another writer during lock promotion; the direct profile path also
converted a row read under unsupported SQLite row-lock syntax into a full
ActiveModel update.

### Implemented

- Profile updates now use the shared database-specific user-row lock helper.
- Login user lookup reserves SQLite's writer by the tenant/email key before
  reading the account; user-by-id and current-session mutation helpers also
  reserve the write lock before their first read.
- Refresh and previous-refresh lookups now reserve the SQLite writer directly
  by the presented digest, then reload the matching row while the transaction
  owns the write fence.
- Session-id logout already uses a write-first fence by tenant/session ID.
- OAuth token issuance now reserves SQLite's writer by known app/subject IDs
  before reloading the current client and subject state; it no longer performs
  a read-first lock upgrade on those rows.
- OAuth admin app mutations use the same write-first fence by tenant/app ID;
  admin user mutations fence by tenant/user ID before loading their snapshots.
- Admin user mutations also reserve SQLite's writer by tenant/user ID before
  reading the row used for status, role, password, or profile changes.
- Unsupported database backends now fail closed in these auth lock helpers
  instead of silently running an unlocked read.
- Existing SQLite tests exercise login, profile update, password change,
  refresh/reuse, and logout paths through the new lock helpers.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::auth_lifecycle::tests::failed_attempts_lock_atomically_and_ignore_obsolete_credentials
cargo test -p rustok-server --lib services::auth_lifecycle::tests::update_profile_trims_name_and_allows_clearing_with_blank_value
cargo test -p rustok-server --lib services::auth_lifecycle::tests::change_password_rejects_expired_current_session
cargo test -p rustok-server --lib services::auth_lifecycle::tests::refresh_reuse_after_grace_revokes_live_session_lineage
cargo test -p rustok-server --lib services::auth_lifecycle::tests::logout_by_session_id_revokes_under_lock_and_is_idempotent
cargo test -p rustok-server --lib services::oauth_token_service::tests::refresh_rotation_consumes_a_token_exactly_once
```

## P28 — Revalidate OAuth consent inside token-issuance transactions (2026-11-18)

Authorization-code and refresh flows validated consent before starting their
issuance transaction. They later acquired the app and subject locks, but did
not re-read consent under those locks. A concurrent consent re-grant or scope
narrowing could therefore commit between preflight validation and token
persistence; the exchange could return credentials carrying a stale consent
epoch or scopes that the user had just narrowed.

### Implemented

- Prepared user-token bundles now retain the consent grant timestamp observed
  during preparation.
- Authorization-code exchange revalidates active consent, requested scopes,
  and the prepared grant epoch after acquiring the app and subject locks.
- Refresh rotation revalidates active consent and scope coverage against the
  presented refresh token's stored consent epoch under the same locks.
- A stale epoch returns invalid_grant and a scope no longer covered by the
  current consent returns invalid_scope before the authorization code or
  refresh token is consumed and before replacement credentials are persisted.
- Added a SQLite regression test that changes the consent epoch after token
  preparation and verifies refresh commit rejects without consuming the old
  refresh token or inserting replacement credentials; it also verifies
  narrowed consent rejects wider token scopes.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::oauth_token_service::tests::locked_consent_revalidation_rejects_stale_grant_epoch_and_scopes
cargo test -p rustok-server --lib services::oauth_token_service::tests::refresh_rotation_consumes_a_token_exactly_once
cargo test -p rustok-server --lib services::oauth_admin_guard::tests::guarded_secret_rotation_checks_tenant_and_delegated_permission_ceiling
```
