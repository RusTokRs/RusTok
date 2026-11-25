# ADR: MCP management owner boundary

## Status

Accepted

## Date

2026-07-10

## Context

MCP management contracts had been split between `rustok-mcp` and `apps/server`. That made the server both compose and define a module-owned capability surface, duplicating management types and workflows.

## Decision

`rustok-mcp` owns the MCP management port, GraphQL roots, REST DTOs and management UI adapters. The July 2026 decision established the public contract boundary and placed the persistence provider in the host. The ownership refinement below governs the provider and storage placement.

## Ownership refinement (2026-11-25)

The July decision established the public contract boundary. The canonical FBA
owner layout extends that boundary to the implementation and storage.

- `rustok-mcp` owns persisted client, token, tool policy, audit and scaffold
  draft state, including the transactions and migrations. The published
  migration names and their prerequisites remain stable. The embedded runtime
  continues to use the host-configured database.
- REST, GraphQL and native admin call the same owner management port. Runtime
  token binding and audit use the same owner storage. The MCP owner enforces
  its delegation rules using authoritative principal facts supplied by the host.
- `apps/server` composes storage, authenticated request context, tenant module
  availability and the authorized host workspace publisher. The owner holds
  the draft claim and recovery rules. Workspace authorization and the native
  filesystem lock precede the draft claim, and the blocking publisher retains
  the lock through publication.
- Existing `/api/mcp/*` and GraphQL contracts stay supported. Standard MCP
  Streamable HTTP and independent deployment are later adapter work with their
  own runtime evidence.

This refines the server-provider placement in the July decision and the
bridge/store placement in the March
[runtime binding](./2026-03-19-mcp-runtime-binding-through-server-bridge.md),
[scaffold storage](./2026-03-20-persisted-alloy-scaffold-drafts-in-server-control-plane.md)
and [scaffold runtime binding](./2026-03-20-mcp-runtime-scaffold-store-binding.md)
decisions. Their authentication, audit and durable draft behavior still applies.

## Consequences

- Module clients use the owner-owned public contract instead of server-local management types.
- The server composes the embedded owner and its host capabilities through explicit dependencies.
- The prior server-owned management ADR is superseded but retained for its historical persistence rationale.
