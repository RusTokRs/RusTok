# Server App — Implementation Plan

## Focus

Strengthen `apps/server` as the central backend runtime with formal API contracts, predictable operational diagnostics, and hardened security gates.

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
`crates/rustok-rbac/docs/explicit-principal-kind.md`. Same-SHA API/auth-resolver/
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
`crates/rustok-rbac/docs/README.md`. Source guard
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
