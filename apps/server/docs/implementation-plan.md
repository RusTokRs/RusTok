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
libraries, or modify its source/Cargo graph during runtime installation.
It must also never fetch an external OCI payload for every execution, grant an
artifact raw infrastructure clients, or require an artifact-only module to
implement `RusToKModule` in process.

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

### Test coverage

- Increase integration test share for module scenarios with real DB/migrations.
- Add contract tests for API response stability for frontends.
- Include negative tests for RBAC/tenant isolation and failure-mode tests for event transport.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `pending`
- Last verified at (UTC): `2026-07-20`
- Scope inspected: `partial preflight only: registry governance owner-service construction used by the Alloy release handle`
- Findings: `P0=0, P1=0, P2=1, P3=0`
- Fixed in this pass: `the Alloy governance handle now obtains the publication service through ModuleControlPlane instead of constructing SeaOrmModuleGovernanceService directly`
- Remaining risks or blockers: `the complete apps/server Wave 2 inspection has not started`
- Evidence: `node scripts/verify/verify-module-control-plane-write-path.mjs`
- Next action: `resume the normal queue; perform the full server composition audit after all Core modules`
- Resume command: `cargo test -p rustok-server --lib`
