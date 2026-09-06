# MCP runtime binding through a server-owned bridge

- Date: 2026-03-19
- Status: Accepted

## Context

RusToK already had:

- MCP identity/tool-policy foundation in `crates/modules/rustok-mcp`;
- persisted MCP clients/tokens/policies/audit plus management API in `apps/server`.

What was still missing was the actual bridge between these layers: a way to resolve a persisted MCP
token into an effective runtime `McpAccessContext` and to persist runtime audit decisions without
moving server-specific ORM/runtime code into the thin MCP adapter crate.

At the same time, upstream MCP/rmcp documentation remains the source of truth for protocol,
authorization semantics, and security guidance:

- [MCP docs](https://modelcontextprotocol.io/docs)
- [MCP spec](https://modelcontextprotocol.io/specification/2025-03-26)
- [`rmcp` docs](https://docs.rs/rmcp/latest/rmcp/)
- [Authorization guide](https://modelcontextprotocol.io/docs/tutorials/security/authorization)
- [Security best practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)

## Decision

We keep the boundary split in two layers:

1. `crates/modules/rustok-mcp` owns generic runtime hooks only:
   - `McpSessionContext`
   - `McpAccessResolver`
   - `McpRuntimeBinding`
   - `McpAuditSink`
   - `McpToolCallAuditEvent`

2. `apps/server` owns the persisted/runtime bridge through `DbBackedMcpRuntimeBridge`, which:
   - resolves plaintext MCP token -> persisted token/client/policy;
   - builds the effective `McpAccessContext` at session start;
   - updates `last_used_at` for both client and token;
   - writes runtime `allowed`/`denied` tool-call audit events into `mcp_audit_logs`.

The bridge is server-owned platform infrastructure and is registered during server runtime bootstrap.

## Consequences

- `rustok-mcp` remains a thin adapter over upstream `rmcp`, not a persistence layer.
- `apps/server` becomes the single place that knows how persisted MCP state maps into live runtime access.
- Runtime MCP audit now has a concrete persisted path instead of stopping at management-plane records.
- The current binding model is session-start based and matches the present stdio adapter path.
- The next architectural step is a server-owned remote MCP transport/session bootstrap plus admin UI.
