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

The host-workspace apply path is a cooperating writer with native scaffolding. Before claiming a
draft it takes the permanent `.rustok/scaffold/lock` OS lock used by `cargo xtask scaffold` and
refuses to proceed while native recovery markers are present. It stages the complete crate under a
unique private directory inside `.rustok/scaffold` on the same filesystem and publishes the finished directory with one rename;
it never exposes a partially written final crate directory. The permanent lock file is never
removed. Interrupted applies remain quarantined in `applying` or `recovery_required` until an
operator verifies the workspace and explicitly reconciles the persisted draft state.

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
