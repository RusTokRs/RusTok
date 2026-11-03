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

New build creation returns the inserted model, including the saved reason and
execution metadata, so immediate projections and newly created composition
receipts use the same data as persistence. Existing immutable receipts are not
rewritten; a historical replay can still contain an incomplete older result,
and operators should consult the build read surface for current state.

The transport boundary is the read-only `BuildControl`, shared as
`SharedBuildControl`. GraphQL and native admin adapters consume the same active
build and history snapshots.

`BuildExecutionService` dry-runs render the selected queued plan without
claiming the build, changing persistence, publishing lifecycle events, writing a
manifest, or starting a process. The preview is a read snapshot, not a lease:
another executor can claim or change the build immediately afterward. Running
and terminal builds remain ineligible for preview.

For errors observed by a live executor after a claim, including plan preparation
and manifest materialization, the executor attempts to record a failed build.
If start-event publication fails immediately after a claim, the claim path also
attempts to record failure before returning the delivery error. Failure-recording
or failure-event errors are reported alongside the original error. Persisted and
broadcast executor-failure messages are generic; the caller receives the original
error chain. A committed terminal status is not overwritten by these failure attempts, and a status
update that observes a final or concurrently changed status fails rather than
silently continuing. Publication is still post-commit: an error can describe a
notification failure after a success or failure was already recorded.

These measures are not crash recovery or process fencing. Process loss, task
cancellation, a hung or failed database/publisher, or a failed failure-recording
attempt can still leave a running build. A cancellation racing process launch
cannot reliably stop that process, and the same-status compare-and-swap is not a
lease, a global build lock or an ordered progress version. Notifications are not
an atomic durable outbox and may be lost, repeated or reordered. Operators must
reconcile state and actual side effects before recovery. The targeted PostgreSQL
terminal-transition contention test and its execution prerequisites are listed
in the [implementation plan](./implementation-plan.md); it has not been run in
the current sandbox.

## Release-Safety Boundary

Under the accepted
[module release rollback safety decision](../../../../DECISIONS/2026-08-06-module-release-rollback-safety.md).
`rustok-modules` is the sole operator-level owner of
static release selection, predecessor eligibility, recovery, desired/observed
rollout, and incident outcome. `rustok-build` remains the immutable role-build
plan/validation owner and shared non-operator build foundation.
`rustok-static-distribution-worker` is the sole static role-bundle
executor/publisher and returns one canonical digest-bound receipt.
`rustok-build` retains neither a second static publisher nor an independent
public active-release head or rollback command. There is no dual-write or
fallback path. See the [implementation plan](./implementation-plan.md).
