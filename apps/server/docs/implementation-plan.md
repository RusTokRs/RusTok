# Server App — Implementation Plan

## Focus

Strengthen `apps/server` as the central backend runtime with formal API contracts, predictable operational diagnostics, and hardened security gates.

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
  A process crash can strand a `running` job. A safe recovery/fencing
  design must reconcile immutable receipts and actual side effects
  before taking over a lease; do not automatically delete or expire
  `running` rows. Add a multi-replica PostgreSQL contention test and
  crash/restart recovery evidence. PostgreSQL support for the new
  index must be verified before deployment.
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
not joined. This is not cancellation- or panic-safe, and not every detached or
stop-signal-only task is tracked and drained (including rate-limit cleanup and the
Comments TCP listener); an expired drain deadline can leave other work running.
Direct users of the lower-level bootstrap APIs remain responsible for shutdown.
The new host and supervisor regression tests have not been compiled or executed.

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
  dispatcher and worker deployments;
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
