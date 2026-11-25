# ADR: Persisted Alloy scaffold drafts in `apps/server`

## Status

Accepted

The [MCP management owner boundary](./2026-07-10-mcp-management-owner-boundary.md#ownership-refinement-2026-11-25) refines the placement of the persistence implementation. This record preserves the original decision and workflow.

## Context

`rustok-mcp` already had an in-memory review/apply boundary for Alloy scaffold drafts, but that
state disappeared with the process and could not be managed through server APIs or future admin UI.

## Decision

We add a persisted server-side control plane for Alloy scaffold drafts in `apps/server`:

- table `mcp_scaffold_drafts`;
- management methods in `McpManagementService`;
- REST endpoints under `/api/mcp/scaffold-drafts*`;
- GraphQL fields `mcpModuleScaffoldDraft*`.

The server-hosted MCP runtime uses `DbBackedMcpRuntimeBridge` as its
`McpScaffoldDraftStore` implementation. Stage, review and apply operations are routed through
`mcp_scaffold_drafts` and `McpManagementService`, so remote server sessions use durable,
tenant-bound draft state rather than the generic in-memory store. Standalone/in-process
`rustok-mcp` runtimes may still use the owner crate's in-memory draft store when no server
persistence bridge is supplied.

The host-workspace apply path is a cooperating writer with native scaffolding. Before claiming a
draft it takes the permanent `.rustok/scaffold/lock` OS lock used by `cargo xtask scaffold` and
refuses to proceed while native recovery markers are present. It stages the complete crate under a
unique private directory inside `.rustok/scaffold` and publishes the finished directory with one same-filesystem rename;
if `.rustok/scaffold` and `crates/modules` are on different filesystems, publication fails closed without exposing a partial final crate directory. The permanent lock file is never
removed. Interrupted applies remain quarantined in `applying` or `recovery_required` until an
operator verifies the workspace and explicitly reconciles the persisted draft state.

## Consequences

Positive:

- scaffold drafts survive process restarts;
- admin and external applications get a stable management API;
- the platform now has a real persisted review surface for Alloy-generated module drafts.

Constraint:

- standalone/in-process MCP runtimes without the server bridge may still use process-local draft storage;
- server-hosted remote MCP stage/review/apply operations use the persisted control plane.

## Next steps

1. Extend persisted draft metadata toward richer codegen workflows.
2. Continue verifying admin UI and operator workflows against the persisted control plane.
