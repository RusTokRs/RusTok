# apps/server / CRATE_API

## Public Modules
- Pure Axum server entry point (`src/main.rs`, `src/host.rs`, HTTP/GraphQL handlers).
- Integration of `ModuleRegistry` and domain `rustok-*` modules.

## Key Structures/Contracts
- `ServerRuntimeContext` and `ServerAuthRuntime` as the explicit host runtime contexts.
- Public HTTP/GraphQL server contract (endpoints, schema, auth middleware).
- Event runtime and outbox relay initialization.

## Events
- Publishes: integration domain events from connected modules.
- Consumes: broker/outbox stream for background processing and indexing.

## Dependencies on Other Crates
- `rustok-core`, `rustok-events`, `rustok-outbox`, domain `rustok-*` modules.

## Common AI Mistakes
- Reintroducing framework-global context instead of passing the explicit server runtime context.
- Registering a module without declaring its dependencies in `ModuleRegistry`.
