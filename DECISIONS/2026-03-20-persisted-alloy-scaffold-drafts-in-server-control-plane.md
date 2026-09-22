# ADR: Persisted Alloy scaffold drafts in `apps/server`

## Status

Accepted

## Context

`rustok-mcp` already had an in-memory review/apply boundary for Alloy scaffold drafts, but that
state disappeared with the process and could not be managed through server APIs or future admin UI.

## Decision

We add a persisted server-side control plane for Alloy scaffold drafts in `apps/server`:

- table `mcp_scaffold_drafts`;
- management methods in `McpManagementService`;
- REST endpoints under `/api/mcp/scaffold-drafts*`;
- GraphQL fields `mcpModuleScaffoldDraft*`.

The persisted control plane stores staged scaffold request/preview payloads and the apply outcome
metadata, but it does not yet replace the in-memory draft store used by the current `rustok-mcp`
runtime flow.

## Consequences

Positive:

- scaffold drafts survive process restarts;
- admin and external applications get a stable management API;
- the platform now has a real persisted review surface for Alloy-generated module drafts.

Constraint:

- live MCP runtime still uses process-local in-memory draft storage;
- a follow-up slice is required to bind runtime tooling to the persisted server-side store.

## Next steps

1. Bind live MCP runtime to the persisted draft store.
2. Add admin UI for listing, reviewing, and applying drafts.
3. Extend persisted draft metadata toward richer codegen workflows.
