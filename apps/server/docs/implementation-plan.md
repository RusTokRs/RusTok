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

The source implementation uses a dedicated host-owned credential boundary:

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
[`host-authority.md`](./host-authority.md). Issue #2680 remains open until
same-SHA source/unit/compile evidence and retained live denial, admission,
rotation, revocation and multi-replica parity evidence exist.

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

### Test coverage

- Increase integration test share for module scenarios with real DB/migrations.
- Add contract tests for API response stability for frontends.
- Include negative tests for RBAC/tenant isolation and failure-mode tests for event transport.
- Add live HTTP/native host-authority tests for no header, wrong token, read/manage hierarchy, audit identity, Iggy authenticated/resolved tenant ownership, rotation/revocation and WebSocket denial.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `pending`
- Last verified at (UTC): `2026-07-31`
- Scope inspected: `cross-owner Tenant trust sweep only: host-global Events native, Iggy native, System GraphQL and Settings GraphQL authority; tenant OAuth administration; credential middleware and HTTP GraphQL/WebSocket composition`
- Findings: `P0=2, P1=1, P2=1, P3=0` (the two P0 findings and one P1 raw-credential-lifetime defect belong to the cross-owner Tenant/Events interaction; the existing P2 is the earlier module-control-plane construction finding)
- Fixed in this pass: `the earlier fail-closed host context is retained; a rejected OAuth-client allowlist design was removed before PR after proving tenant settings:manage can rotate OAuth app secrets; the replacement authenticates a dedicated host-owned opaque token by SHA-256 digest, removes the raw header before dispatch, inserts typed native authority, scopes typed authority across HTTP GraphQL, leaves WebSocket denied, moves the separate Iggy native read/write adapter from tenant SETTINGS_* to host Read/Manage, uses the host audit actor, and retains authenticated/resolved tenant equality for Iggy secret ownership`
- Remaining risks or blockers: `the complete apps/server Wave 2 inspection has not started; host-authority formatting, source guard, server/API unit tests, server/events/Iggy-admin compile checks and live denial/admission/rotation/revocation/replica evidence are pending; issue #2680 remains open`
- Evidence: `PR #2726; apps/server/src/host_authority.rs; apps/server/src/middleware/auth_context.rs; apps/server/src/graphql/system.rs; apps/server/src/graphql/settings/mod.rs; crates/rustok-events-module/admin/src/transport/native_server_adapter.rs; crates/rustok-iggy-connector/admin/src/transport/native_server_adapter.rs; apps/server/docs/host-authority.md; scripts/verify/verify-host-global-authority-boundary.mjs; local execution unavailable because github.com DNS resolution fails`
- Next action: `finish the current core/tenant item; inspect PR #2726 same-SHA checks and fix every branch-related failure, then resume the full server composition audit in Wave 2`
- Resume command: `node scripts/verify/verify-host-global-authority-boundary.mjs && cargo test -p rustok-api host_authority -- --nocapture && cargo test -p rustok-server host_authority --lib -- --nocapture && cargo check -p rustok-events-module && cargo check -p rustok-iggy-connector-admin --features ssr && cargo check -p rustok-server --lib`
