# ADR: MCP runtime scaffold flow binds through a pluggable draft store

## Status

Accepted

## Context

`rustok-mcp` already had Alloy scaffold tools and `apps/server` already had persisted
`mcp_scaffold_drafts`, but the live MCP runtime still used process-local in-memory draft state.

That meant:

- a staged draft created through live MCP runtime was not the same draft seen by server management API;
- restarts could drop runtime-only draft state;
- admin/external review surface and live Alloy tool flow were not aligned.

## Decision

We introduce a transport-agnostic runtime contract in `rustok-mcp`:

- `McpScaffoldDraftRuntimeContext`
- `McpScaffoldDraftStore`

`AlloyScaffoldState` can now attach a `McpScaffoldDraftStore`. When such a store is present,
`alloy_scaffold_module`, `alloy_review_module_scaffold`, and `alloy_apply_module_scaffold`
delegate to it instead of using process-local in-memory storage.

`apps/server::DbBackedMcpRuntimeBridge` now implements that contract and maps live MCP runtime
context into persisted `mcp_scaffold_drafts` operations.

## Consequences

Positive:

- live Alloy scaffold MCP flow can share the same persisted drafts as server management API;
- review/apply no longer has to depend on process-local runtime state;
- `rustok-mcp` stays decoupled from SeaORM and server-specific persistence.

Constraint:

- this does not yet create a server-owned remote MCP transport/bootstrap by itself;
- in-memory storage still remains the fallback when no draft store is attached.

## Next steps

1. Use this binding in the future server-owned remote MCP bootstrap path.
2. Add admin UI over persisted draft review/apply.
3. Expand the draft artifact model toward richer Alloy codegen/publish workflows.
