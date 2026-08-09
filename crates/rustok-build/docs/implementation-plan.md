# Implementation plan for `rustok-build`

## Current state

The capability owns build/release persistence models, immutable execution-plan contracts, queued execution, command construction, manifest-snapshot materialization, and the process runner. The server retains only worker event and release-activation adapters; `rustok-cli core rebuild` invokes this capability directly. Host transports consume the `BuildControl` port through `SharedBuildControl`; the server implementation composes the event-aware rollback service so native and GraphQL paths share the same owner operation. The port returns the canonical framework-neutral build/release snapshots from `rustok-api`, and `rustok-build` is the only persistence-to-snapshot mapper.

`ReleaseActivationHook` is the explicit seam for server-owned post-activation
effects. It prevents OAuth synchronization and platform-state projection from
becoming hidden dependencies of build persistence or CLI execution.

Platform rollback now validates non-nil build, tenant, and actor identities and
emits an explicit predecessor transition through `BuildRolledBack`. The owner
event is the only source for event-bus, WebSocket, and GraphQL rollback facts;
rollback no longer masquerades as `BuildCompleted`.

## Planned Module Release Safety Cutover

The accepted
[module release safety decision](../../../DECISIONS/2026-08-06-module-release-rollback-safety.md)
makes `rustok-modules` the sole operator-level owner of static release
selection, predecessor eligibility, rollback, incident outcome, and
desired-versus-observed rollout. `rustok-build` remains the plan/validation
owner for canonical role builds and shared non-operator build primitives.
`rustok-static-distribution-worker` is the sole static role-bundle
executor/publisher and returns one canonical digest-bound role-bundle receipt
covering every automated server/worker role and embedded Leptos/browser
artifact. `rustok-build` must not retain a second static publisher.

The current public active-release head and `rollback_build` mutation are an
explicit cutover gap. Once the `rustok-modules` replacement, outside-candidate
controller, transports, and convergence evidence are complete, migrate every
GraphQL/native/CLI caller and delete the duplicate release head, mutation,
event/DTO surface, schema, tests, and current documentation in the same
change. Do not dual-write or keep the direct build rollback as a fallback.

## Verification

- `cargo check -p rustok-build`
- `cargo check -p rustok-server --lib --no-default-features`
- `cargo check -p rustok-admin --lib --no-default-features --features ssr`
- API surface contract guard after server call sites move.
