# ADR: MCP management owner boundary

## Status

Accepted

## Date

2026-07-10

## Context

MCP management contracts had been split between `rustok-mcp` and `apps/server`. That made the server both compose and define a module-owned capability surface, duplicating management types and workflows.

## Decision

`rustok-mcp` owns `McpManagementPort`, GraphQL roots and types, REST/control-plane DTOs, and module-owned management UI adapters. `apps/server` is the composition root: it supplies persistence-backed providers, schema and route composition, authentication/RBAC extraction, and concrete runtime handles without defining parallel management DTOs or workflows.

## Consequences

- Module clients use the owner-owned public contract instead of server-local management types.
- Server code remains an adapter and composition layer for persisted state.
- The prior server-owned management ADR is superseded but retained for its historical persistence rationale.
