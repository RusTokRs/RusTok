# rustok-build documentation

The build capability owns persistence contracts for queued builds and releases,
including the typed `ReleasePublisherPort` hand-off and portable
`DeploymentSettings`/`DeploymentBackend` configuration plus
`DeploymentWorkspace` artifact/runtime paths. Runtime worker and
concrete filesystem, HTTP, or container deployment adapters remain host
responsibilities.

`BuildRuntimeMode` and `RoleBuildPlan` carry the selected host lifecycle with
the immutable execution plan. The server manifest composer is the adapter that
selects role-specific embedded surfaces; deployment backends forward the mode
as `RUSTOK_RUNTIME_HOST_MODE` rather than inferring it from artifact names.
`BuildRequest::artifact_identity` keeps selected distribution composition in
the same idempotency boundary as the manifest, profile, and runtime mode.

`BuildService` is also the read owner for active build/release state and
bounded build/release history pages. Host transports supply only a validated
page request and map the returned owner models; they do not import the
underlying SeaORM entities.

The transport boundary is `BuildControl` (shared as `SharedBuildControl`). The
server host composes this port with the event publisher required by rollback,
while GraphQL and native admin adapters use the shared handle for active state,
history, and rollback commands.

Rollback publishes `BuildEvent::BuildRolledBack` after the predecessor release
transition. The event preserves the requested and restored build IDs, source
and target release IDs, and verified actor. The host maps the same owner event
to the canonical `build.rolled_back` domain event, WebSocket message, and
GraphQL subscription payload; it does not synthesize another completion.
