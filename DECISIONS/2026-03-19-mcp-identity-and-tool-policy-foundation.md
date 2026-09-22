# MCP identity and tool policy foundation in `rustok-mcp`

- Date: 2026-03-19
- Status: Accepted

## Context

`rustok-mcp` was already used as a thin adapter on top of the official Rust SDK `rmcp`, but until now
access control was effectively reduced to a coarse-grained allow-list via `enabled_tools`.

For RusToK this is insufficient:

- it needs to distinguish between human, service, and model actors;
- an explicit layer of identity/scopes/permissions for the MCP boundary is needed;
- `enabled_tools` must not become a false replacement for a full authz model;
- at the same time, existing stdio/runtime surface should not be broken and a new dependency cycle should not be created.

## Decision

The decision is to lay the foundation access-layer directly inside `rustok-mcp`, without turning it into a
runtime module and without extracting it into a separate persisted management subsystem for now.

Within the foundation:

- `rustok-mcp` remains a capability/adapter crate on top of `rmcp`;
- public types `McpIdentity`, `McpAccessContext`, `McpAccessPolicy`,
  `McpToolRequirement`, `McpWhoAmIResponse` are introduced;
- authorization of tool calls is structured as:
  1. legacy `enabled_tools`;
  2. then identity/policy/permissions/scopes via `McpAccessContext`;
- an introspection tool `mcp_whoami` is added;
- `enabled_tools` is retained as a compatibility shim, not as a long-term authz model.

Persisted clients/tokens/policies/audit trail and management API/UI remain as the next layer and are
not considered part of this decision.

## Consequences

- RusToK gains a real MCP identity/policy foundation without breaking current clients.
- A clear integration point for future management API and admin UI is established.
- Documentation should reference the official MCP/rmcp upstream as the source of truth for
  protocol, security, and authorization semantics.
- The next step is to design persisted models for MCP clients/tokens/policies/audit and their API.
