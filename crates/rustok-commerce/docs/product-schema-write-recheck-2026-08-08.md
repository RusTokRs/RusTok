# Product schema write boundary recheck — 2026-08-08

## Scope

This source-only continuation follows the completed Product read/lifecycle cutovers and rechecks the remaining mounted Commerce GraphQL schema-write boundary. The canonical ecommerce source of truth remains `crates/rustok-commerce/docs/implementation-plan.md`; its combined Product schema-write cutover item remains open in this slice.

## Current source result

- Product now publishes `ProductCatalogSchemaWritePort` as a transport-neutral owner capability for every Product schema write currently used by mounted Commerce GraphQL: attribute and option creation, category/schema/group creation, category schema mode, schema/category bindings, Product attribute-value save, and detached-value clear.
- The embedded `ProductCatalogSchemaService` implements the port and derives tenant/actor only from validated `PortContext`.
- Every call requires `PortCallPolicy::write()`, so caller context must carry write policy requirements including idempotency identity and deadline before owner execution is admitted.
- Product schema-write failures are mapped to stable `PortError` codes/messages while internal database/validation/core causes remain in owner logs rather than public messages.
- `ProductCatalogCommandRuntime` now carries an optional `ProductCatalogSchemaWritePort`. `in_process` composes the embedded owner provider; external command profiles remain fail-closed until a host explicitly supplies a schema-write provider.

## Why the canonical item remains open

Mounted Commerce GraphQL still constructs `ProductCatalogSchemaService` directly for schema writes. This PR only publishes and composes the owner capability; it does not yet cut those consumers over.

The embedded schema service also does not currently persist a command receipt keyed by `PortContext.idempotency_key`. `PortCallPolicy::write()` makes caller identity mandatory at the boundary, but durable replay/adoption semantics must not be inferred from that validation alone.

The next source slice therefore remains:

1. define mounted GraphQL caller idempotency input/identity for the schema-write mutations without breaking Product Admin callers;
2. replace every direct `ProductCatalogSchemaService` construction in mounted Commerce schema writes with the host-selected `ProductCatalogSchemaWritePort`;
3. decide and implement the owner-local durable replay/receipt contract needed for non-idempotent create operations before claiming retry-safe write semantics;
4. update canonical completion only when both consumer cutover and the intended idempotency semantics are source-complete.

## Verification state

No tests, checks, formatters, workflows, or runtime verification were executed in this slice per maintainer instruction. Source and GitHub diff inspection only. No FBA/FFA status is promoted.
