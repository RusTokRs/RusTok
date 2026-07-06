# Server App — Implementation Plan

## Focus

Strengthen `apps/server` as the central backend runtime with formal API contracts, predictable operational diagnostics, and hardened security gates.

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
