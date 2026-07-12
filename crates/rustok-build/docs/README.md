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
