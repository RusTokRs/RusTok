# rustok-build documentation

The build capability owns queued build persistence, immutable execution-plan
contracts, command construction, and read-only build projections. It owns no
production release, publisher, deployment, activation, or rollback contract.

`BuildRuntimeMode` and `RoleBuildPlan` carry the selected host lifecycle with
the immutable execution plan. The server manifest composer is the adapter that
selects role-specific embedded surfaces; deployment backends forward the mode
as `RUSTOK_RUNTIME_HOST_MODE` rather than inferring it from artifact names.
`BuildRequest::artifact_identity` keeps selected distribution composition in
the same idempotency boundary as the manifest, profile, and runtime mode.

`BuildService` is the read owner for active build state and bounded build
history pages. Host transports supply only a validated
page request. `rustok-build` maps persistence state to the framework-neutral
`PlatformBuildSnapshot`; transports do not import the underlying SeaORM entity
or reconstruct status/profile codes.

The transport boundary is the read-only `BuildControl`, shared as
`SharedBuildControl`. GraphQL and native admin adapters consume the same active
build and history snapshots.

## Release-Safety Boundary

Under the accepted
[module release rollback safety decision](../../../DECISIONS/2026-08-06-module-release-rollback-safety.md).
`rustok-modules` is the sole operator-level owner of
static release selection, predecessor eligibility, recovery, desired/observed
rollout, and incident outcome. `rustok-build` remains the immutable role-build
plan/validation owner and shared non-operator build foundation.
`rustok-static-distribution-worker` is the sole static role-bundle
executor/publisher and returns one canonical digest-bound receipt.
`rustok-build` retains neither a second static publisher nor an independent
public active-release head or rollback command. There is no dual-write or
fallback path. See the [implementation plan](./implementation-plan.md).
