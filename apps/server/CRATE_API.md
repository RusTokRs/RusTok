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
- Direct session issuance (registration, login and refresh) takes the active
  tenant row lock before user/session locks or user/session inserts and holds
  it through commit. A
  refresh request that loses tenant admission may perform replay-only session
  revocation after re-reading the locked lineage, but it must never issue a
  replacement token. Logout and explicit revocation remain available while a
  tenant is inactive.
- A platform-settings write must record who made it. `SettingsError::MissingActor`
  is returned before the transaction opens, because `PlatformSettingsChanged`
  carries a non-nil `changed_by`; GraphQL maps it to a permission error rather
  than an internal failure.

- Global artifact release security uses ModuleControlPlane and a platform-scoped owner command context. The server requires a direct user session, matching tenant, and modules:manage in the request snapshot. The owner invokes the authorizer inside the same fenced DatabaseTransaction used for the state, receipt and outbox write; the server provider locks direct-session credentials and authoritative RBAC role/policy rows through commit. The owner also invokes the host's final authority check immediately before both new-state and exact-replay commits, so the earliest credential deadline is rechecked after slug-lock waits and all writes. expectedRevision is a decimal string at the transport boundary to preserve u64 precision; policyRevision is caller-provided audit/idempotency metadata, not authorization proof.

## Common AI Mistakes
- Reintroducing framework-global context instead of passing the explicit server runtime context.
- Registering a module without declaring its dependencies in `ModuleRegistry`.
