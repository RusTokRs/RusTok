# apps/server / CRATE_API

## Public Modules
- Loco API server entry point (`src/main.rs`, `src/app.rs`, HTTP/GraphQL handlers).
- Integration of `ModuleRegistry` and domain `rustok-*` modules.

## Key Structures/Contracts
- `AppContext` from `rustok-core` as the main runtime context.
- Public HTTP/GraphQL server contract (endpoints, schema, auth middleware).
- Event runtime and outbox relay initialization.

## Events
- Publishes: integration domain events from connected modules.
- Consumes: broker/outbox stream for background processing and indexing.

## Dependencies on Other Crates
- `rustok-core`, `rustok-events`, `rustok-outbox`, domain `rustok-*` modules.

## Common AI Mistakes
- Confusing the server `AppContext` with local module contexts.
- Registering a module without declaring its dependencies in `ModuleRegistry`.
