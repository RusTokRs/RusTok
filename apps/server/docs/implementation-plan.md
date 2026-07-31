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
- supply database, OCI, trust, events, audit, clock, and other infrastructure
  adapters; module-build scheduling and execution run in their separate
  dispatcher and worker deployments;
- migrate platform composition, build enqueue, registry governance, effective
  policy, GraphQL, and native adapters to owner operations;
- keep release activation as a host side-effect adapter: it synchronizes OAuth
  applications, then delegates the active-release projection to
  `SeaOrmModuleCompositionService` and never writes `platform_state` directly;
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
RBAC Admin:

- only a direct grant with a non-nil session may enter the control plane;
- the authenticated tenant must equal the middleware-routed tenant before
  permission admission;
- OAuth authorization-code and client-credentials principals remain denied even
  when their effective scoped permissions include `modules:manage` or
  `settings:read`;
- permission admission occurs only after principal and tenant admission;
- the durable REST operation actor is always `AuthContext.user_id`; request
  payloads cannot supply or replace the audit identity;
- native RBAC Admin uses the same owner policy and no longer retains a separate
  generic tenant-only helper;
- adapters compose neutral authenticated facts into the owner policy, while the
  owner crate remains independent of Axum and the `rustok-api/server` feature.

The default `rustok-rbac` crate compiled on exact PR head
`3cf4b3a44980ca257f7f53849e905673141db289` inside Rust-host workflow run
`30650883159`. That workflow then reproduced issue #2740 before building
`rustok-server`. Same-SHA formatting, all-feature owner compilation, RBAC Admin
SSR compilation, server compilation, focused tests, verifier execution and live
negative transport evidence remain required before this correction is verified.

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

### Security

- Strengthen RBAC enforcement checks at middleware and service layer levels.
- Introduce regular security review for sensitive endpoints (auth, tenant, admin operations).
- Expand security event audit (login, privilege changes, tenant boundary violations).
- Retain host-global operator credential denial/admission, rotation and replica-parity evidence without moving credential ownership into tenant OAuth or RBAC.
- Keep role/permission control-plane principal admission owner-defined and ahead of effective permission checks on every transport.

### Test coverage

- Increase integration test share for module scenarios with real DB/migrations.
- Add contract tests for API response stability for frontends.
- Include negative tests for RBAC/tenant isolation and failure-mode tests for event transport.
- Add live HTTP/native host-authority tests for no header, wrong token, read/manage hierarchy, audit identity, Iggy authenticated/resolved tenant ownership, rotation/revocation and WebSocket denial.
- Retain REST artifact-role permission and native RBAC Admin tests/verifiers for OAuth delegated/service denial, tenant mismatch, missing permission and trusted actor propagation.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `pending`
- Last verified at (UTC): `2026-07-31`
- Scope inspected: `cross-owner Tenant trust sweep for host-global Events/Iggy/System/Settings authority, followed by the active core/rbac review of authoritative request scope, artifact role-permission REST adapter and native RBAC Admin bootstrap`
- Findings: `P0=3, P1=2, P2=1, P3=0` (two host-global P0 findings and one raw-credential-lifetime P1 belong to the Tenant/Events interaction; the third P0 is the RBAC REST invalid principal grant; the second P1 is native role-metadata admission inconsistency; the existing P2 is the earlier module-control-plane construction finding)
- Fixed in this pass: `PR #2735 merged the host-owned operator credential boundary as 1ce83819b077ef6e0df009fd5675f556315ef63a. PR #2747 merged one owner host-neutral direct-session policy for REST, GraphQL and native RBAC Admin as 75b67f877eb405abe4e6761a16d6b7ece98bc103, with authenticated/routed tenant equality before modules:manage or settings:read, trusted AuthContext actor propagation and removal of the obsolete native tenant-only helper.`
- Remaining risks or blockers: `the complete apps/server Wave 2 inspection has not started; the RBAC P0/P1 corrections still lack same-SHA format, all-feature owner compile, admin SSR/server compile, focused unit/architecture/verifier tests and live transport evidence; host-authority source/unit/compile and live denial/admission/rotation/revocation/replica evidence also remain pending; issues #2680 and #2740 remain open`
- Evidence: `source audit confirms middleware builds AuthContext and request scope from authoritative DB permissions, with OAuth scopes only narrowing authority. Exact PR #2747 head 3cf4b3a44980ca257f7f53849e905673141db289 compiled default rustok-rbac in Rust-host workflow 30650883159; issue #2740 then stopped the job before rustok-server. Standard CI and Hardening remained pending without jobs. Browser E2E retained the unrelated four Next Admin sessionStorage failures while Next Frontend passed. No other queued or pending result is claimed. Host-authority source remains merged at 1ce83819b077ef6e0df009fd5675f556315ef63a.`
- Next action: `continue the core/rbac P0/P1 sweep and obtain exact-SHA format/all-feature/admin/server/focused/module/live evidence; leave the complete server composition audit for its Wave 2 cursor visit`
- Resume command: `cargo fmt --all -- --check && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-server --lib && cargo test -p rustok-rbac --all-features && cargo test -p rustok-rbac-admin --features ssr && cargo test -p rustok-server --test rbac_artifact_permission_control_plane_guard && node scripts/verify/verify-rbac-admin-tenant-scope.mjs`
