---
id: doc://docs/architecture/dataloader.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# DataLoader and Batch Read Paths

This document captures the role of DataLoader in RusToK as a request-scoped mechanism
for batched read paths, primarily in GraphQL.

## Purpose

DataLoader is needed to solve the N+1 problem in UI-facing read paths:

- GraphQL queries
- related batched loaders in the host/runtime layer
- local read helpers, if they live in request scope

DataLoader is not a place for business logic and must not turn into a
hidden service layer.

## Main Principle

DataLoader:

- collects keys within a single request scope
- performs batched loading
- caches results only for the duration of the request
- returns data to a specific resolver/call site

Its job is read optimization, not ownership of domain logic.

## Where DataLoader Is Allowed

Allowed scenarios:

- GraphQL field resolvers
- batched lookups of related entities
- locale-aware/profile-aware read paths, if batching does not break tenant boundaries

Disallowed scenarios:

- write-side operations
- long-lived shared cache across requests
- domain orchestration
- hidden authorization or tenant resolution inside a loader

## Invariants

For any DataLoader in the platform, the following rules must hold:

- batching does not mix tenant boundaries
- batching does not mix locale contract, if locale affects the result
- loader does not bypass RBAC/auth assumptions of the host layer
- loader can be safely invoked multiple times within a single request scope
- result mapping remains deterministic and idempotent

## Ownership

DataLoader belongs to the host/read layer:

- the module/service layer provides canonical read contracts
- the GraphQL/runtime layer decides whether batching is needed
- loader does not become the source of truth for the domain model

If batching logic becomes complex domain logic, it must be moved to a
module-owned service/read contract.

## Performance Contract

DataLoader is used where it:

- reduces the number of queries
- reduces repeated reading of the same relations
- makes the UI-facing query path more predictable

But it must not be applied automatically to every read path without a real
N+1 problem.

## What Not To Do

- do not put business rules and orchestration in a loader
- do not keep loader cache longer than one request
- do not mix different tenant or locale contexts in a single batched loader
- do not use a loader as a replacement for a proper module-owned read service

## When to Update This Document

This document needs to be updated if any of the following changes:

- the role of DataLoader in the GraphQL/runtime layer
- the request-scope caching contract
- tenant/locale batching rules
- the boundary between loader and module-owned read service

## Related Documents

- [API Architecture](./api.md)
- [Routing and Transport Layer Boundaries](./routing.md)
- [Module Architecture](./modules.md)
- [Platform Architecture Overview](./overview.md)
