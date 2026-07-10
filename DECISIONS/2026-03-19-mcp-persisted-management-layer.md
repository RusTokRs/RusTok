# Persisted MCP management layer in `apps/server`

- Date: 2026-03-19
- Status: Superseded by [MCP management owner boundary](./2026-07-10-mcp-management-owner-boundary.md)

## Context

`rustok-mcp` already had runtime identity/tool-policy foundation, but RusToK still lacked a persisted
platform layer for:

- MCP clients and credentials;
- per-client tool/permission/scope policies;
- audit records;
- management API for admin UIs and external control planes.

Keeping this state inside `rustok-mcp` itself would blur the boundary between thin MCP adapter logic
and platform management concerns. It would also make the server runtime depend on MCP transport
details more tightly than needed.

## Decision

We place the persisted MCP management layer in `apps/server`, not in `crates/rustok-mcp`.

The server now owns:

- database tables `mcp_clients`, `mcp_tokens`, `mcp_policies`, `mcp_audit_logs`;
- REST management API under `/api/mcp/*`;
- GraphQL management surface under `mcp*`;
- RBAC permission surface `mcp:*` for tenant-scoped management access.

`rustok-mcp` remains the thin runtime/protocol adapter over upstream `rmcp`.

## Consequences

- RusToK now has a real persisted control plane for MCP access management.
- Admin UI and external applications can target stable server APIs instead of ad hoc runtime config.
- MCP documentation must continue to treat upstream MCP/rmcp docs as the source of truth for protocol behavior.
- The next architectural step is runtime binding: load persisted clients/tokens/policies into actual MCP auth flow and runtime audit decisions.
