# rustok-build documentation

The build capability owns persistence contracts for queued builds and releases,
including the typed `ReleasePublisherPort` hand-off and portable
`DeploymentSettings`/`DeploymentBackend` configuration plus
`DeploymentWorkspace` artifact/runtime paths. Runtime worker and
concrete filesystem, HTTP, or container deployment adapters remain host
responsibilities. The local filesystem adapter is physically contained under
`<instance-root>/releases/platform/sha256` through the canonical
`rustok-runtime::InstanceLayout`; `DeploymentSettings` cannot select a
second local root.

`BuildRuntimeMode` and `RoleBuildPlan` carry the selected host lifecycle with
the immutable execution plan. The server manifest composer is the adapter that
selects role-specific embedded surfaces; deployment backends forward the mode
as `RUSTOK_RUNTIME_HOST_MODE` rather than inferring it from artifact names.
`BuildRequest::artifact_identity` keeps selected distribution composition in
the same idempotency boundary as the manifest, profile, and runtime mode.

`BuildService` is also the read owner for active build/release state and
bounded build/release history pages. Host transports supply only a validated
page request. `rustok-build` maps persistence state to the framework-neutral
`PlatformBuildSnapshot` and `PlatformReleaseSnapshot` contracts from
`rustok-api`; transports do not import the underlying SeaORM entities or
reconstruct status/profile codes.

The transport boundary is `BuildControl` (shared as `SharedBuildControl`). The
server host composes this port with the event publisher required by rollback,
while GraphQL and native admin adapters use the shared handle for active state,
history, and rollback commands. Both adapters consume the same snapshots.

Rollback publishes `BuildEvent::BuildRolledBack` after the predecessor release
transition. The event preserves the requested and restored build IDs, source
and target release IDs, and verified actor. The host maps the same owner event
to the canonical `build.rolled_back` domain event, WebSocket message, and
GraphQL subscription payload; it does not synthesize another completion.

## Accepted Release-Safety Cutover

The current active-release and rollback surface is an implementation gap under
the accepted
[module release rollback safety decision](../../../DECISIONS/2026-08-06-module-release-rollback-safety.md).
The atomic target makes `rustok-modules` the sole operator-level owner of
static release selection, predecessor eligibility, recovery, desired/observed
rollout, and incident outcome. `rustok-build` remains the immutable role-build
plan/validation owner and shared non-operator build foundation.
`rustok-static-distribution-worker` is the sole static role-bundle
executor/publisher and returns one canonical digest-bound receipt.
`rustok-build` retains neither a second static publisher nor an independent
public active-release head or rollback command.

The cutover migrates all GraphQL, native, and CLI callers and removes the
duplicate head, mutation, event/DTO surface, schema, tests, and current
documentation in one change. There is no dual-write or fallback path. See the
[implementation plan](./implementation-plan.md) for the tracked work.
