# Implementation plan for `rustok-build`

## Current state

The capability owns build/release persistence models, immutable execution-plan contracts, queued execution, command construction, manifest-snapshot materialization, and the process runner. The server retains only worker event and release-activation adapters; `rustok-cli core rebuild` invokes this capability directly. Host transports consume the `BuildControl` port through `SharedBuildControl`; the server implementation composes the event-aware rollback service so native and GraphQL paths share the same owner operation.

`ReleaseActivationHook` is the explicit seam for server-owned post-activation
effects. It prevents OAuth synchronization and platform-state projection from
becoming hidden dependencies of build persistence or CLI execution.

Platform rollback now validates non-nil build, tenant, and actor identities and
emits an explicit predecessor transition through `BuildRolledBack`. The owner
event is the only source for event-bus, WebSocket, and GraphQL rollback facts;
rollback no longer masquerades as `BuildCompleted`.

## Verification

- `cargo check -p rustok-build`
- `cargo check -p rustok-server --lib --no-default-features`
- `cargo check -p rustok-admin --lib --no-default-features --features ssr`
- API surface contract guard after server call sites move.
