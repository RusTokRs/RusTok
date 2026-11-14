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

## Invariants
- The host composes control-plane services; it never constructs owner types
  directly. Every module/control-plane read and write goes through
  `ModuleControlPlane` (`catalog`, `lifecycle`, `transitions`, `distribution`,
  `governance`, …), obtained from the runtime database handle.
- Replay identity for idempotent control-plane commands is the semantic request
  digest plus `actor_id` (and the tenant/revision fields the owner compares).
  `trace_id` and `correlation_id` are per-attempt observability evidence: they
  are persisted, but a changed trace/correlation value replays the stored
  outcome instead of raising an idempotency conflict.
- Module lifecycle writes are tenant-scoped and revision-guarded. An unresolved
  release transition blocks enable/disable toggles in the owner executor, and
  an unresolved rollout blocks settings writes in the owner lifecycle writer;
  the host must not pre-empt those fences with its own checkpoint queries.
- Authorization failures on the permission-availability path fail closed: a
  resolution error is returned to the caller rather than treated as "no
  module permissions".

## Common AI Mistakes
- Reintroducing framework-global context instead of passing the explicit server runtime context.
- Registering a module without declaring its dependencies in `ModuleRegistry`.
