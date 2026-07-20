# Implementation Plan for `rustok-modules`

## Scope

Own the mandatory Core module control plane: identity, releases, marketplace,
installation, composition, lifecycle, effective policy, build/publication
orchestration, rollback, and static promotion. Optional module implementations
must not become server Cargo dependencies through this crate.

The cross-component sequence and completion rules are defined by the
[canonical module-platform plan](../../../docs/modules/module-control-plane-consolidation-plan.md).

## Current state

## FFA/FBA status

- FFA status: `not_started`
- FBA status: `boundary_ready`
- Structural shape: `no_ui_boundary`

Implemented:

- mandatory `ModulesModule` Core entrypoint;
- immutable artifact descriptors, semantic versions, source lineage, payload
  kinds, entrypoints, runtime ABI, digests, and capability declarations;
- Core/Optional effective-policy calculation and dependency-aware toggle
  validation;
- tenant state/settings persistence, lifecycle hooks, journal transitions,
  recovery plans, and post-hook retry;
- digest-pinned OCI manifest/config/layer resolution through
  `OciDistributionArtifactRegistry`;
- package identity, media-type, and payload-digest verification;
- scoped installation persistence with PostgreSQL RLS;
- installed artifact request construction and execution through
  `rustok-sandbox`;
- artifact-only durable execution audit persistence through
  `SeaOrmArtifactExecutionObserver`; it stores redacted start/terminal records
  with the exact installation ID and PostgreSQL tenant RLS and must be attached by artifact runtime
  composition; additive audit metrics persist queue time and policy-admitted
  capability-call count alongside executor duration, instruction/fuel,
  memory-when-observed, and output size;
- rejection of static promotion as a runtime installation path.

Still outside the owner boundary:

- legacy build persistence remains a host adapter. The owner now reads,
  bootstraps, and revision-CAS replaces the canonical active snapshot, owns its
  active-release projection, and owns the CAS-plus-build transaction through
  `ModuleCompositionBuildEnqueuer`; the server retains typed-manifest decoding,
  bootstrap-file loading, build-record adaptation, and post-commit notification;
- registry governance, publication, release approval/yanking, and related
  persistence in the server. Release yanking, ownership binding, owner
  transfer, publish-request rejection, request-changes, hold, resume, and
  final publication are owner slices: after host authorization, typed commands
  atomically persist state plus audit facts. Publication includes the release
  projection, localized metadata, owner binding or authorized rebind, optional
  approval-override evidence, and publish-request finalization in one
  transaction. The owner also records append-only, subject-digest-bound
  publication evidence with a distinct author-signature, build-service,
  marketplace-approval, or platform-admission authority; recording one fact
  never implies another. A domain-separated evidence digest and database
  uniqueness constraint make duplicate concurrent delivery idempotent. A
  marketplace approval cannot enter through the generic evidence command: the
  owner emits it only in the atomic final-publication transaction for the
  canonical staged artifact SHA-256. A build-service attestation also bypasses
  that generic command: `ModuleBuildServiceAttestationCommand` verifies the
  complete build receipt, its declared `build_service` authority, and all
  digest-pinned OCI identities before it records the signature-manifest fact.
  Platform admission is likewise typed: `ModulePlatformAdmissionCommand`
  accepts only an admitted verification decision for the exact OCI manifest,
  binds its signature/SLSA/SBOM outcomes, signer, policy revisions, and
  immutable evidence-reference fingerprint, then records the platform fact.
  Publication now fails closed unless an author signature is bound to the
  staged artifact SHA-256 and a build-service attestation plus platform
  admission share the exact OCI manifest recorded by the current build stage;
  marketplace approval is then
  created atomically with the final release transition. PostgreSQL locks the
  publish request during finalization. A repeated final-publication command
  must carry the same non-nil external idempotency UUID and immutable command
  fingerprint recorded with the durable release, otherwise it fails closed.
  The exact replay returns without another release, evidence, or audit event.
  A yank changes only the release lifecycle and records its reason;
  immutable release storage identity remains unchanged while new resolution
  excludes the yanked release. Reupload advances the staged-artifact timestamp,
  so every required evidence fact must have been recorded after the current
  immutable staging operation. `stage_platform_build` reloads a completed build
  pair under tenant RLS, verifies its request slug/version and payload digest
  against the submitted artifact, and appends its immutable source/component/
  OCI receipt identities. Its idempotency replay compares tenant, build,
  source, component, and authenticated actor. Final publication now requires
  that current stage.
  Artifact origin is explicit and `unclassified` records fail closed. External
  prebuilts use a separate current stage with an approved provenance policy,
  independent quarantine review, and either a reproducible source identity or
  an explicit source-absence reason; they require author signature and platform
  admission bound to the staged payload digest but cannot use a build-worker
  attestation. The server external
  staging adapter is operator-only and derives actor/quarantine approver from
  authenticated `modules.manage` authority. Its replay compares all immutable
  source/provenance/quarantine facts and both authenticated principals. The
  platform build-stage adapter also requires `modules.manage`, derives its tenant only from the
  authenticated session, and forwards only the completed build ID plus an
  idempotency key to the tenant-RLS owner read;
- manual validation-stage reports and requeues now use the owner transaction
  for request-state gating, stage transition rules, attempt creation, and stage
  plus follow-up audit facts. Remote lease claim, heartbeat, terminal
  completion, expired-lease requeue, validation-job enqueue, job claim,
  stale-job recovery, and worker retry telemetry and result materialization now
  use owner transactions. A later authorized enqueue marks a validation job
  still running after 15 minutes as failed with the stable
  `validation_worker_lease_expired` reason and creates the next durable attempt
  atomically. A successful claim now also returns an immutable delivery work
  item containing the exact storage key, SHA-256, size, and content type; if
  those immutable delivery facts cannot be assembled, the owner atomically
  rejects the request and fails the job with content-free audit evidence rather
  than leaving it queued. The independent worker verifies claimed bytes before
  parsing. Bundle contract
  validation itself now runs through the pure owner
  `validate_module_publish_bundle` function against an immutable metadata
  snapshot carried by that work item; it no longer needs a server request model
  in the production claimed-job path. `rustok-registry-validation-worker` now
  independently polls and conditionally claims that durable owner queue,
  verifies the claimed object bytes, and records the typed result. The server
  endpoint only queues work and has no background-spawn execution path.
  The server worker executes bundle checks only, then submits immutable evidence
  to one owner transaction that finalizes the request and job, creates follow-up
  stages, and persists their audit facts;
- draft publish-request creation now uses an owner transaction for the request,
  default-locale metadata translation, and audit fact. Host authorization and
  artifact object storage remain adapters; the owner transaction attaches a
  stored artifact, resets validation attempts on reupload, submits the request,
  and persists audit facts;
- parts of effective-policy input assembly;
- server GraphQL/native transport mappings;
- admin-owned manifest scanning, SQL, hashing, and build planning;
- OCI publication, signature/SBOM/provenance verification;
- isolated Rust component build orchestration;
- explicit static-promotion orchestration.

Important intermediate limitations that must not be mistaken for the target:

- the default `ModuleLifecycleDbWriter` host adapter still materializes its
  catalog from the compile-time `rustok_core::ModuleRegistry`; host composition
  must supply durable catalog loading before artifact-only modules reach that
  adapter. Server lifecycle transports now supply only the active distribution
  defaults and actor identity to this writer for toggle, post-hook retry, and
  compensation. For settings, the server supplies only the host-resolved schema
  and owner-normalized JSON; the writer derives active identity, Core status,
  effective enablement, and persistence, then the transport maps its owner
  result to a host ORM view;
- artifact lifecycle dispatch requires a configured
  `ArtifactLifecycleExecutor`; production host wiring for that executor remains
  to be supplied;
- admission stages, verifies, and publishes payload bytes into CAS before the
  database admission commit; `SeaOrmArtifactInstallationStore` commits the
  installation, admission metadata, and shared outbox envelope atomically, and
  the owner reconciler enforces reference-plus-retention deletion. A
  `StorageArtifactBlobStore` supplies the durable object-storage CAS adapter;
  host infrastructure must wire it to the production object-storage driver;
- OCI admission streams the registry layer into temporary private storage while
  rejecting declared or received payloads above the owner bound and verifying
  SHA-256; the post-verification storage boundary still buffers an accepted
  payload, so streaming sink and multipart CAS publication remain the next
  slice;
- the committed admission row now records the complete status vocabulary with
  initial `admitted` state and revision `1`. Every immutable admission begins
  with an owner-supplied actor and idempotency key: its canonical request digest
  is reserved in the same transaction as the installation, admission metadata,
  and outbox fact. A same-command retry returns the original installation ID;
  reuse of that key for a different immutable request fails closed. Guarded
  lifecycle transitions, rollback pointers, and policy evidence remain separate
  owner-service work;
- artifact descriptors carry dependency, permission, settings, runtime binding,
  persistence metadata, and declarative UI contribution contracts; brokered
  namespaced data, localization delivery, and dynamic host composition remain
  to be implemented.

## Local Work Phases

### M1 - Freeze Owner Contracts

- Define serializable catalog, release, installation, composition, lifecycle,
  effective-policy, governance, build, and promotion snapshots.
- Define canonical errors, structured details, revisions, idempotency, actor,
  tenant, trace, and correlation contexts.
- Add serialization and stale-revision tests.

Current implementation: the shared command context, revisioned command envelope,
optimistic revision/CAS primitive, stable error envelope, and generic typed
snapshot envelope are available from `rustok-modules`. Owner services will adopt
these contracts as their write paths are moved. `ModuleControlPlane` is the
owner composition root for currently extracted database-backed services; it is
not a server/admin compatibility facade or a parallel execution path. Server
lifecycle, composition, artifact runtime/HTTP, and registry-governance adapters
obtain their corresponding owner services through this root. Artifact runtime
also receives its exact data/object capability resolvers and redacted execution
audit observer through the root; outbox projection receives the durable artifact
event projector, and routed artifact HTTP receives its binding-idempotency
store. RBAC permission evaluation remains an RBAC-owner adapter.
`EffectivePolicyService` now exposes the same owner-owned catalog/default/tenant
override resolution used by lifecycle commands, so server guards, GraphQL, and
installer adapters do not query `tenant_modules` to reconstruct policy. The
installer verification adapter also obtains its static catalog through the
same facade rather than rebuilding it independently.

M2 has started with a transport-neutral definition catalog. It derives static
definitions from the compile-time registry while keeping registry handles
limited to static runtime concerns, and rejects ambiguous active definitions.
Effective-policy resolution and toggle validation now consume the catalog.

The lifecycle entrypoints now use `ModuleExecutionDispatcher`, which resolves
the active definition before invoking a static implementation. Artifact
lifecycle bindings execute only through the admitted sandbox adapter supplied
by host composition; no artifact path falls back to a compiled callback.

Lifecycle hooks never receive the transaction that commits tenant state or the
operation journal. Validation and durable intent happen first; the pre-hook
runs through a connection-only dispatcher, then the owner commits state and
journal in one short transaction. Post-hooks and retry attempts run only after
that commit, so their failure is retained as retry/compensation evidence rather
than producing an implicit state rollback. Artifact lifecycle bindings use the
same boundary and never receive a control-plane transaction handle.

Post-hook retry and compensation now require non-nil caller UUID idempotency
keys at the GraphQL boundary. The owner stores each key in the tenant-scoped
lifecycle journal, links the derived operation to its source operation through
the durable correlation field, and returns the existing journal operation
without redispatching a hook when the same request is replayed. Reusing a key
for another actor or operation is an explicit `IDEMPOTENCY_CONFLICT`; no server
generated recovery key exists.

Artifact descriptors now carry versioned declarative bindings with stable IDs,
schema digests, permission, idempotency, limit profile, and declared
capabilities. Descriptor v4 bundles bounded Draft 2020-12 schema documents by
canonical SHA-256 digest: every binding input/output selector and optional
settings/data/persistence selector must resolve to that immutable bundle. It
accepts only in-document `#` references and rejects a mismatched digest before
admission. `ArtifactRuntime` validates every admitted binding input before
sandbox execution and its decoded owner output afterward against those exact
schemas. It uses a bounded compiled-validator cache with Draft 2020-12, strict
formats, linear-time regex limits, and no HTTP/filesystem resolver features.
Settings and data-contract validation remain owner-specific paths. Every
artifact binding and UI contribution must reference an exact declared
module-owned RBAC permission; capability grants remain separate guest-to-host
authorization.

The v1 binding taxonomy now reserves explicit descriptor kinds for readiness,
activation smoke checks, and before/after/on-commit host hooks in addition to
lifecycle, command, HTTP, event, schedule, and health. A binding declaration
does not imply runtime support: an unavailable dispatcher path remains
fail-closed until its host contract is implemented.

`ArtifactRuntimeLifecycleExecutor` now provides the dispatcher-facing sandbox
adapter contract: installation resolution is tenant/scope-aware, effective
grants and limits come from a separate policy resolver, and only a binding
present in the immutable installed descriptor can replace the sandbox
entrypoint. Production host wiring selects the durable object-storage driver
for `StorageArtifactBlobStore`. Rhai artifact inputs are wrapped first in the
owner-owned strict `ArtifactBindingDispatchEnvelope` v1 and then in the neutral
strict `RhaiBindingInput` v1 envelope; results must decode as
`RhaiBindingOutput` v1 before the artifact owner receives its payload. Raw
Rhai input/output compatibility is not accepted. The binding's payload, not
either envelope, is then validated against the descriptor's input/output schema
selectors.

Artifact persistence is a strict descriptor contract: it contains only a
positive revision and an admitted schema digest for brokered namespaced values.
Unknown descriptor fields are rejected during decode, so marketplace artifacts
cannot smuggle SQL, native migrations, object-store paths, or host handles into
the control plane.

Dynamic artifact UI is also strict and declarative: only `admin_settings` and
`admin_actions` metadata surfaces are accepted, with immutable localization and
a module-owned permission. A descriptor cannot include executable component
source, a URL, an iframe, or a native frontend package. Action binding and host
presentation details remain the separate Phase 7 contract.

`SeaOrmArtifactInstallationStore` now implements the production
`ArtifactInstallationResolver` port. It resolves only an active, non-uninstalled
installation for the exact descriptor payload digest, honors the per-installation
tenant disable state, and prefers tenant scope over platform scope. Before
returning, it revalidates the persisted descriptor and immutable dependency lock;
runtime dispatch therefore cannot reconstruct an artifact from registry tags or
mutable catalog state. A host still needs to compose this resolver with the
sandbox policy resolver and the durable event/schedule delivery workers.

`ArtifactBindingDispatch` now carries an explicit installation target. Interactive
dispatch selects the current effective release, while a durable worker must use
`ExactInstallation`. The resolver contract fails closed when that immutable
installation no longer matches the tenant's active selection, preventing a
queued event from silently executing a later artifact revision. The durable
queue, retry, and dead-letter workers are composed through the host's shared
sandbox executor and tenant enumerator.

The lifecycle adapter now implements the generic ArtifactBindingExecutor port.
Lifecycle is only a convenience call over that port; an artifact-only host can
dispatch another admitted binding with an explicit sandbox phase and JSON input
through the same installation resolver, CAS read, capability policy, and
sandbox. Static modules have no dynamic fallback. `SeaOrmArtifactEventDeliveryQueue`
and `SeaOrmArtifactScheduleDeliveryQueue` own artifact subscriptions and
schedules with exact installation identity, lease/retry/dead-letter state, and
shared-sandbox execution. The generic event dispatcher accepts only an exact
valid platform event type; wildcard syntax is reserved for admitted
subscriptions and cannot enter a delivered execution envelope.

`ModuleEffectivePolicyQuery` is the sole owner query for composing immutable
Core definitions, distribution defaults, and persisted tenant overrides. It
returns a typed effective set for a supplied catalog, so the server
effective-policy adapter, lifecycle writer, and installer verification provide
only infrastructure inputs instead of reproducing enablement semantics.

The server constructs the compile-time `ModuleRegistry` exactly once during
runtime bootstrap and shares that static implementation registry with the
router, GraphQL, lifecycle, event-dispatch, and installer adapters. Marketplace
definitions and effective policy are resolved through owner services; no
request path rebuilds a registry from durable artifact state.

Phase 4 begins with the transport-neutral `ModuleBuildRequest` /
`ModuleBuildResult` v7 protocol in this owner crate. It carries immutable source,
dependency, toolchain, independently versioned SDK/template, WIT, resource-limit,
network-policy, validation, and
evidence facts, while `ModuleBuildWorker` is a remote-worker port that cannot
authorize in-process Cargo execution by `apps/server` or the sandbox runtime.
Terminal failures include bounded machine-readable diagnostic `(stage, code)`
facts with the owner-canonical stage for their failure code; they never inline runner output,
compiler paths, or human logs. Alloy, CLI, CI, and admin use those facts and
authorized evidence references instead of parsing worker output. Successful
results also carry one ordered `passed` outcome for every requested validation
profile; a `validation_failed` result must identify a requested profile with a
`failed` outcome.
`SeaOrmModuleBuildService` durably queues tenant/project-idempotent requests
under tenant RLS and emits `module.build.queued` through the transactional
outbox without invoking a worker inline. It records a terminal result only
after validating it against the immutable queued request under the same tenant
scope. `load_completed` exposes that same stored request/result pair only
under tenant RLS and revalidates it before a later owner staging operation may
consume it;
RLS, then emits `module.build.completed`; duplicate results must match their
stored digest. `rustok-module-build-transport` now maps the remote-worker port
onto a versioned mTLS gRPC service with authenticated readiness and no
in-process fallback. `load_queued` and `dispatch_queued` provide the owner-side
outbox-consumer delivery path: they release tenant-scoped database state before
the RPC and accept the terminal result only through immutable owner validation.
`rustok-module-build-worker` is now a separately deployable mTLS process. It
can invoke only a fixed image-owned non-symlink runner in a fixed workdir with
a cleared environment, request-derived timeout, and aggregate streamed output
cap. Its v1 source is a `cas://sha256/<hex>` archive from a deployment-mounted
read-only root; the worker rehashes and safely materializes it under a
request-scoped directory, without a CAS client. Digest, archive-safety, and
extraction-limit violations become terminal owner-validated build results;
only worker I/O faults remain retryable transport failures. The delivery host must consume
`module.build.queued` through an external broker
consumer group, call the worker through mTLS, and invoke only the owner delivery
method for queue/result state. `rustok-module-build-dispatcher` owns the
broker-neutral process-and-ack contract and an Iggy adapter for the dedicated
`module-build` topic. The adapter retains one real remote consumer-group cursor
and commits its offset only after owner-side result persistence. Broker topic
provisioning and deployment configuration remain operational prerequisites. The
separate dispatcher binary owns only the database owner adapter, Iggy client,
and mTLS build-worker client; it has no Cargo or CAS access and no server-local
polling or execution fallback. Evidence-generation tools, signing, and
release-governance promotion remain unfinished.
The preflight now binds raw `Cargo.lock`
bytes to the immutable lock digest and rejects source-local Cargo config,
patch/replacement and path-dependency bypasses, non-allowlisted registries,
forbidden Git sources, and denied build-script/native-link declarations before
the fixed runner starts. It parses the resolved lock graph under bounded
package/dependency limits, requires registry checksums and pinned allowed-Git
revisions, and rejects credential-bearing sources. It is a boundary guard, not
a substitute for `cargo metadata --locked` evidence. The worker now executes
that command before the runner using a fixed image-owned Cargo binary and
deployment-owned pre-materialized cache with a cleared environment, forced
offline mode, a request-derived deadline, and aggregate output cap. It rejects
metadata that changes the resolved package/source graph, exposes a custom build
target or native link denied by policy, escapes the materialized workspace, or
does not close over the returned resolve nodes. Scoped dependency egress now
uses only a fixed image-owned materializer adapter that receives the exact
approved endpoints and fills a fresh job-local Cargo home in a separately
isolated OCI network sandbox. It must return a receipt bound to source, lock,
and endpoint list; the worker rejects cache symlinks and Cargo config before it
runs metadata offline. Missing configuration, receipt mismatch, or endpoint
denial remains fail-closed as `network_policy_denied`.

The runner's successful result is now bound to the fixed
`output/component.wasm` artifact. The worker rehashes a regular non-symlink
file under a memory/disk-derived 64 MiB ceiling, validates that it is a
WebAssembly Component with the maintained parser, and compares its root
imports/exports with the result evidence before accepting the result. The
deployment-owned `wasm-tools` executable extracts WIT from that same payload;
the worker parses it and requires the request's package, world, version, and
complete import/export surface to match exactly, rejecting undeclared
capability imports. The worker now also rehashes and parses fixed CycloneDX SBOM and SLSA in-toto
provenance output files before accepting a successful result. Provenance must
bind the immutable source, lock, toolchain, WIT, and component digests plus
independently versioned SDK/template inputs through the RusToK
external-parameters envelope. `OciDistributionArtifactPublisher`
now accepts only a publication bundle bound to that successful immutable result,
publishes the descriptor-configured executable layer, and uploads OCI 1.1 SBOM
and provenance referrers with an exact subject descriptor. It verifies every
registry-returned manifest digest and returns only digest-pinned identities;
its deterministic write tags are never installation identity. The worker now
collects only fixed inspected output files (including the descriptor), uses its
deployment-owned scoped registry destination, and attaches the receipt to the
terminal result. Owner persistence rejects a successful result without that
receipt. Signing and admission trust policy remain unfinished.

The former server background `rustok-build` polling executor has been removed.
`rustok-build` remains only for reviewed static platform-release composition in
installer/CLI operations and cannot consume `module.build.queued` or implement
the module build-worker port.

The v1 build result derives its toolchain and WIT digests from domain-separated
immutable request fields. The owner rejects a result that substitutes either
contract, in addition to checking its source, dependency lock, attempt, tenant,
resource bounds, and terminal outcome. `retryable` is true exactly when the
terminal result permits `retry_build`; no worker may label a retry as either
forbidden or required while reporting the opposite next action.

OCI artifact media types are frozen in the owner crate for immutable descriptor
config, Rhai, WASM Component, sidecar, static-promotion payloads, and
SBOM/provenance/test-evidence/release-lineage referrers. The distribution
adapter rejects mismatched config media types, declared sizes, and raw config digests, then
accepts exactly one descriptor-selected executable layer. The scoped publication
adapter uploads verified descriptor-configured payloads and OCI 1.1
SBOM/provenance referrers. The isolated build worker then signs the returned
digest-pinned artifact through fixed Cosign/KMS configuration and records the
resolved compatible signature-manifest digest; admission and release governance
remain unfinished.

The public OCI reader and publisher constructors now always create the strict
distribution client: HTTPS only, invalid certificates rejected, no platform
resolver, and one concurrent upload/download. OCI identities are constrained to
registry host, repository, and digest rather than URLs; the build worker obtains
repository-bound credentials only after its credential-broker lease. The
registry adapter additionally bounds complete descriptor/layer admission to five
minutes, streams the config only after its declared descriptor-size check, and
cancellation-safely deletes a partial staging file. Config and payload streams
reject received bytes beyond their OCI-declared size before extending memory or
disk staging, and reject a final size mismatch before descriptor parsing or
payload digest acceptance. The current distribution library still buffers
manifests and does not expose redirect, proxy, retry, per-request timeout, or
decompression controls. Those controls therefore remain an explicit deployment
egress responsibility, together with manifest and transfer ceilings; the worker
separately bounds its complete publication window to 15 minutes. The OCI adapter
also cancels a complete artifact-and-referrer publication after ten minutes,
leaving bounded time for Cosign within that worker deadline. This is a
deliberately partial OCI transport policy, not evidence that the remaining
controls are implemented.

Artifact Event bindings now declare up to 32 exact or terminal-wildcard topics
inside the admitted descriptor. The generic dispatcher matches only those
topics and requires the Event sandbox phase; a binding kind cannot be invoked
under another phase. `SeaOrmArtifactEventDeliveryQueue` now materializes one
tenant-scoped `(source event, installation, binding)` delivery state machine
without creating a second event journal: `sys_events` remains the source of
truth. It hashes the complete versioned source envelope, rejects conflicting
idempotency retries, leases one work item at a time, applies queue-owned
bounded exponential backoff, and retains terminal dead-letter evidence. Its
worker adapter reads the admitted descriptor and executes only the exact
immutable installation target through the shared sandbox port. A host still
decorates its durable `sys_events` outbox relay with the owner projector before
downstream publication, so an outbox record is not acknowledged until every
binding delivery has been materialized. Platform-global events have no tenant
artifact composition and are intentionally not projected. The same owner queue
now implements a `ModuleWorkScheduler` source/handler pair: it enumerates only
host-supplied tenants, claims one tenant-RLS delivery, and dispatches the
persisted binding against its exact immutable installation. Event and Schedule
adapters share explicit host handles for the sandbox-backed executor and tenant
enumerator; neither may construct a fallback runtime or issue an unscoped
tenant query. The neutral artifact subject now carries the exact owner-selected
installation ID, so a future dynamic capability router can resolve the correct
scope without treating release slug/version/digest as tenant identity. The
production server now supplies the active-tenant enumerator through the tenant
owner service. `ResolvingArtifactCapabilityBroker` now defines that dynamic
router contract: a host-owned resolver receives the exact subject/tenant
identity and must return only the eligible owner broker for the requested
capability. It has no default route. The host-owned admission command carries
the initial durable `SandboxPolicy`; the normal empty policy issues no grants.
It is tenant-bound for tenant installations and otherwise a platform default.
Admission rejects duplicate or undeclared grants. The owner resolver rechecks
the exact active installation, tenant lifecycle, policy revision, and descriptor
declarations before returning it; a missing row or revision mismatch denies
execution. The server composes the shared CAS-backed executor before worker
registration, with the Rhai `capability_call` bridge, Wasm component executor,
durable execution audit, and exact policy resolver. It registers structured
`platform.data` plus `platform.data.objects`: the latter accepts only logical
object names and explicit prefix/operation grants. Small reads and writes use
at most 44 KiB decoded base64; large writes use durable owner-owned upload
sessions with ordered 44 KiB chunks, final size/SHA-256 verification, expiry
reaping, and retention-GC hand-off before private-object publication. It never exposes
physical storage identity. Secret, MCP, and every other capability remain default-deny
until their deployment adapters are available.

`resolve_granted_artifact_capability` is the shared gate for every dynamic
owner route. It resolves the exact immutable installation, applies active
admission, tenant lifecycle, uninstall state, durable policy revision, and the
named explicit grant before a broker is constructed. The concrete
`SeaOrmArtifactDataCapabilityBrokerResolver`,
`SeaOrmArtifactDataObjectCapabilityBrokerResolver`,
`SeaOrmArtifactSecretCapabilityBrokerResolver`, and
`ArtifactMcpCapabilityBrokerResolver` derive their scopes only from that exact
result. The sandbox host already enforces data operation/prefix, logical-secret,
object-data prefix/operation, logical-secret, and MCP server/tool grant
constraints before a route runs. The composed server executor registers
`platform.data` and `platform.data.objects`; secret and MCP routes await their
explicit deployment adapters, and there is no default or network fallback broker.

`ArtifactBindingExecutionContext` carries only bounded host-supplied actor and
trace identities through generic artifact dispatch, sandbox capability calls,
and durable execution audit. The descriptor and artifact payload cannot set
those values.

Schedule bindings now carry an immutable cron expression, timezone, misfire
policy, overlap policy, and deduplication policy. Admission accepts only a
bounded cron/timezone form and rejects schedule metadata on any other binding
kind. It now validates semantic six-field cron syntax and real IANA timezone
identities; a five-field minute expression is canonically evaluated with a
zero-second prefix. `module_artifact_schedule_deliveries` provides the
tenant-RLS durable slot projection with immutable schedule digest, per-slot
deduplication, lease, cancellation, retry, and dead-letter state, while
`module_artifact_schedule_cursors` preserves the materialized watermark across
restarts. `ArtifactScheduleMaterializer` is invoked by the shared
`ModuleWorkScheduler` adapter before it claims tenant work, so no artifact
timer loop or unscoped RLS query exists.

On first observation, or after an immutable schedule digest changes, the
materializer initializes its cursor at the host clock and does not replay an
old contract. `skip` ignores slots older than the configured grace interval;
`run_once` materializes one due slot and advances through the poll; `catch_up`
materializes at most the configured bounded batch and leaves its cursor at the
last selected slot for later polls. `forbid` advances the clock but drops new
slots while a pending/running slot exists for the same immutable binding;
`queue` and `allow` retain their distinct slots, with actual parallelism still
owned by scheduler deployment capacity. The durable uniqueness key always
prevents duplicate delivery of a physical slot; `none` means the descriptor
adds no guest/application idempotency condition beyond that transport safety.
The queue derives the digest from the admitted binding, cancels a slot whose
lifecycle or descriptor is no longer eligible, and executes only the exact
installation. The production server supplies the active-tenant source and the
shared CAS-backed sandbox executor before the registration starts.

HTTP bindings now carry a platform-owned literal relative path, method,
JSON-only request/response media types, bounded body/output sizes, a bounded
timeout, and an explicit no-streaming policy. Admission rejects HTTP metadata
on other binding kinds and duplicate `(method, path)` pairs. The generic
dispatcher matches only an admitted route and enforces JSON envelope sizes
before and after sandbox execution. `ArtifactRuntime` validates the declared
binding schemas and clamps the effective sandbox wall-clock limit to the
admitted HTTP timeout; an HTTP host must still own the external route prefix,
authenticate and authorize the binding permission, map transport responses, and
apply the binding's idempotency policy. `SeaOrmArtifactBindingIdempotencyStore`
owns durable request identity, replay output, and an expiring execution lease
for every externally routed binding. The server HTTP route is
`/api/artifacts/{installation_id}/{*path}`: it resolves only an exact active
installation, matches a literal admitted method/path pair, authorizes the
binding's declared dynamic RBAC key, accepts exactly JSON, and dispatches only
through the shared CAS sandbox executor. The platform command route is
`POST /api/artifacts/{installation_id}/commands/{binding_id}`: it selects only
an admitted Command binding by exact ID, applies the same installation, RBAC,
JSON, idempotency, and sandbox constraints, and does not create a dynamic
GraphQL field or artifact-owned router.

CAS admission is explicitly `stage -> durable CAS publish -> database
transaction plus outbox -> reconciler`. A publish preceding a failed database
commit is an orphan candidate, never a runtime installation; the reconciler
may remove it only after reference and retention-policy checks. The durable
snapshot policy fails closed when a digest has no rule: deletion requires an
explicit expired rule with no legal hold, rollback protection, or audit
retention. Runtime has no registry fallback; it reads and rehashes admitted CAS
bytes and returns `BlobNotFound` before sandbox execution when they are absent.

`SeaOrmArtifactInstallationStore` uses the existing `OutboxTransport` in the
same transaction as admission metadata, the selected dependency graph, and the
installation record. `EventEnvelope` carries an optional tenant identifier, so
platform-scoped admission emits without a synthetic tenant. No module-specific
second event journal is allowed.

Artifact admission accepts only an explicit `ArtifactAdmissionCommand`, never
an ambient timestamp or caller-owned installation identity. Its actor and
idempotency key are scoped by platform or tenant, while its canonical request
digest covers the immutable OCI reference, scope, and dependency lock. The
store reserves that identity before inserting installation state and binds it
before committing the outbox fact. Successful retries return the same
installation identity; a permission-registration retry refetches and verifies
the immutable descriptor so it can replay the owner request. A reused key with
a different digest fails closed.

Admitted artifact permissions are represented by immutable localized
label/description entries and sent through the shared
`ArtifactPermissionRegistrationPort` after a durable admission commits. The
installation ID is the idempotency identity, so a retried command repeats a
failed registration without creating another release selection. This path can
only register RBAC vocabulary; role and actor grants are absent by contract.
The durable RBAC catalog adapter now has an explicit tenant-role assignment
service and exact installation-scoped authorizer. The server admin transport
requires `modules:manage` and derives tenant/actor identity from trusted
request context. Artifact HTTP route composition remains pending; installation
never creates an automatic role or actor grant.

Dependency resolution now uses `pubgrub` behind the transport-neutral
`ModuleResolutionProvider`. The adapter first collects an immutable candidate
snapshot, requires the exact deployment platform version and descriptor
compatibility range, then filters by trust, active/yanked/revoked status,
scope, module/provider kind, and runtime ABI before PubGrub runs. It rejects
malformed platform facts fail-closed and writes only the selected exact
versions and payload/manifest digests into the lock graph. Every
`InstalledModuleArtifact` now persists that graph with its revision and digest
in the same installation transaction, and runtime execution rejects a missing
or tampered declared dependency. Persisted solver input snapshots and stable
derivation explanations remain owner-service work.

The shared transactional outbox is the required event boundary for committed
admission. It records `module.artifact.admitted` in the same transaction as the
installation and admission metadata; platform-scoped events use the canonical
absence of a tenant identifier.

### M2 - Introduce the Facade

- Expose explicit catalog, release, publication, installation, lifecycle,
  composition, effective-policy, build, and promotion subservices.
- Define narrow infrastructure ports for database transactions, OCI, trust
  verification, build scheduling, events, audit, clock, and IDs.
- Keep atomic boundaries inside owner operations.
- Introduce the durable artifact-aware module definition catalog and generate
  static definitions from the compiled implementation registry.
- Move dependency/effective-policy/lifecycle decisions off Rust trait objects.
- Introduce the runtime binding registry/dispatcher for static and sandboxed
  implementations.

### M3 - Complete Server Ownership Cutover

- The owner now runs platform composition snapshot/bootstrap/revision-CAS and
  atomic build-request creation. The server validates its typed host manifest,
  supplies the build-record adapter, and publishes the build notification only
  after the owner transaction commits.
- Move registry governance, publication stages, releases, ownership, holds,
  approvals, rejection, yanking, and event taxonomy.
  `registry_publication_evidence` is the authority-separated immutable ledger
  for release evidence, and the final owner publication transaction enforces
  the required authority facts. Both platform-build and external-prebuilt
  staging are owner-owned, durable, and exposed through authenticated server
  adapters. The independent registry validation worker treats artifact metadata
  and embedded manifest text as untrusted: it verifies the claimed storage
  facts, bounds every parsed manifest, caps the complete publish bundle at 2
  MiB before JSON parsing, and emits content-free validation diagnostics, so
  raw artifact/request strings do not enter governance events through this
  validation path. Rendering and AI prompt boundaries remain separate unfinished
  work alongside OCI policy enforcement.
- Move remaining effective-policy composition.
- Own static module-settings schema validation and normalization behind the
  neutral `ModuleSettingSpec` contract. The server resolves its typed manifest
  schema only, then passes that schema and the requested JSON object to the
  owner before lifecycle persistence.
- Own static `rustok-module.toml` metadata validation through the neutral
  `StaticModulePackageContract`; the host parses files and maps stable errors,
  while the owner validates package identity, SemVer dependencies/conflicts,
  admin surfaces, settings schemas, and crate-local runtime binding
  normalization.
- Own static catalog metadata through `StaticModuleCatalogContract`, including
  ownership/trust, admin-surface conflicts, description length, and allowed
  HTTP(S) marketplace asset URLs. Resolve the canonical static UI
  classification from host-parsed surface flags and evaluate platform-version
  compatibility in the same owner boundary. Validate and normalize static UI
  i18n metadata and HTTP provider exclusivity there before host filesystem
  adapters inspect bundle paths or qualify crate-local symbols.
- Own resolved static catalog topology through `StaticModuleTopologyContract`.
  The host applies TOML/package overlays and supplies only neutral defaults,
  dependency/conflict/version facts, and its parsed platform version; the owner
  validates default enablement, direct dependencies, conflicts, dependency
  ranges, and platform compatibility. The owner validates host-decoded
  deployment build-surface semantics (standalone requirements, URL syntax, and
  storefront identity uniqueness); filesystem checks stay in the server host.
- The owner also invokes the canonical shared manifest-versus-registry
  comparison contract. The server supplies neutral facts extracted from its
  compile-time `ModuleRegistry`; it does not reimplement comparison semantics.
- Migrate server callers, then delete replaced services and duplicate errors.
- The marketplace registry adapter maps the complete stable
  `ModuleGovernanceError` contract at the HTTP boundary instead of translating
  owner failures into its server-local governance taxonomy. Its host-only
  authorization and storage-adapter failures remain transport concerns.
- `ModulePublishRequestCreateCommand` owns publish-request slug, semantic
  version, locale, metadata, and UI-package validation, then derives the
  durable warning set itself. The HTTP adapter supplies transport decoding and
  authenticated authority only; it cannot persist caller-selected warnings.
- Add a static guardrail preventing direct writes outside this crate. The
  repository verifier `verify-module-control-plane-write-path.mjs` rejects
  direct composition, lifecycle, artifact installation, build-request, and
  registry governance aggregate writes from server, installer persistence,
  worker, and transport production sources. It also covers artifact-data tables,
  requires matching owner write implementations, and rejects direct construction
  of extracted owner SeaORM services outside `rustok-modules`; production roots
  must use `ModuleControlPlane` with no worker or transport carve-outs.

### M4 - Complete Artifact Admission

- Extend descriptor compatibility, dependency, schema/migration, and UI surface
  references.
- Persist verification evidence, policy revision, capability grant revision,
  rollback pointers, status, and optimistic revision. The installation schema
  records both a nullable self-referencing predecessor pointer and an explicit
  capability-grant revision selected by the owner, independently of the
  artifact declaration and capability policy. The later rollback command will
  advance the predecessor atomically with its lifecycle transition. A separate
  rollback-operations record supplies durable actor/reason audit and a unique
  idempotency key; it does not duplicate mutable lifecycle state. Its immutable
  command fingerprint also records the selected capability-grant revision and
  migration rollback mode, together with the committed source/target revisions,
  so an exact retry replays after the source admission changes. Historical rows
  without that complete fingerprint fail closed rather than guessing a result.
- Enforce signature, signer, SBOM, provenance, compatibility, dependency, and
  capability admission before activation.
- Use Cosign/Sigstore for digest-bound OCI signature and transparency-bundle
  verification; require SLSA in-toto provenance and CycloneDX JSON SBOM for
  compiled artifact classes. The owner policy records exact trusted authority,
  issuer/root, builder/source, SBOM, trust-policy, and capability-policy
  decisions rather than exposing verifier-library types.
- Keep tool execution outside the server and module runtime: `rustok-modules`
  owns a typed fail-closed `TrustVerifier` port, while an isolated verification
  worker owns Cosign, trust-root access, SLSA parsing, and CycloneDX validation.
  `ModuleInstaller` requires that port and selected policy revisions at
  construction, verifies before CAS publication, and persists the redacted
  decision/evidence references in the atomic admission transaction. The
  `rustok-verification-transport` crate provides the tonic gRPC client/server
  adapter; worker or transport failures reject admission without a fallback.
- Resolve and persist exact dependency graphs with a maintained solver adapter.
- Copy admitted payloads into platform content-addressed storage and execute
  from CAS rather than the external registry.
- Add brokered tenant/module namespaced data and JSON-Schema validation;
  prohibit arbitrary untrusted SQL/native migrations.

The Phase 3.6 entry contracts are `ArtifactDataBroker` and
`ArtifactDataObjectBroker`: every operation carries host-owned
tenant/module/data-contract/policy scope and logical names only. They expose no
physical storage or secret clients. `SeaOrmArtifactDataBroker` supports bounded
structured JSON values (256-byte logical keys and 64 KiB payloads), while
`SeaOrmArtifactDataObjectBroker` accepts bounded private objects (32 MiB),
derives their digest from accepted bytes, stores an owner-generated private key,
and re-hashes bytes on every read. Both use a tenant-RLS namespace, optimistic
revisions, and immutable idempotency operation results. Both brokers require a
host-provided `ArtifactDataAuthorizer`; the structured broker also requires an
`ArtifactDataSchemaValidator`: the latter resolves the
admitted data-contract schema and must use the maintained `jsonschema`
validator with bounded regular expressions before a value becomes durable.
`SeaOrmArtifactDataSchemaValidator` is constructed with the exact immutable
installation ID selected by the host. It resolves only that RLS-scoped admitted
descriptor and persistence revision, never the latest release by module slug.
The exact installation ID now travels only as host-controlled sandbox subject
metadata so the dynamic capability router can select that scope; it is never
artifact input or an artifact-readable capability value.
The neutral `platform.data` grant limits the sandbox adapter to
injected tenant/module/data-contract scope, declared logical-key prefixes, and
the `get`/`put`/bounded-`put_batch`/bounded-`list` input shapes.
`SeaOrmArtifactDataCapabilityBroker` routes those operations to this owner
service after tenant/subject checks; batch entries must have distinct keys and
idempotency keys under declared prefixes, while list queries use an escaped
logical-prefix filter and continuation validation.
An authorized namespace purge removes structured records and private-object
metadata in its transaction and queues every unreachable private key. The
tenant-scoped `SeaOrmArtifactDataObjectGcService` deletes a queued key only
after a supplied retention snapshot explicitly approves it; missing rules and
legal/audit/rollback holds fail closed rather than issuing a guest-driven
physical delete.
`CapabilityBrokerRouter` composes this data adapter with the durable secret
handle adapter and future owner-owned capability adapters using exact capability
names, rejecting duplicate or unregistered routes instead of adding a global
fallback. `ArtifactMcpCapabilityBroker` now verifies the same tenant/subject
scope, accepts only a logical server alias, tool name, and optional arguments,
and forwards scoped execution identity to `ArtifactMcpInvoker`. It has no MCP
endpoint, token, credential, or discovery input; deployment composition must
still bind the owner port to the existing MCP access-policy, audit, and
configured server-alias implementation.
The sandbox object capability limits each base64 call to 44 KiB. Larger objects
use durable owner-owned upload sessions with ordered bounded chunks, final
owner-side size/digest verification, expiry reaping, and retention-GC hand-off;
true streaming object I/O, indexed-query, and export remain separate unfinished
work. Object metadata and all durable digest columns require canonical lowercase
`sha256:` values, and upload idempotency is isolated by immutable policy scope.
The owner enforces the 32 MiB object quota across the full durable chunk set,
not merely at completion.
Completion claims a durable `completing` state before publication; expiry reaping
atomically transitions only expired open/completing sessions before queuing
chunks, so completion and collection cannot race the same session.

The immutable persistence contract now reserves bounded logical scalar indexes:
each declaration has a host-validated name, a narrow logical JSON pointer, and
a scalar value type. It exposes no physical index identity or query expression.
The owner computes the canonical scalar projection in Rust and stores it in a
separate tenant-RLS table in the same write/purge transaction. The
first indexed write binds that namespace to the exact immutable index
declaration digest, while indexed reads only validate it. A changed declaration
requires a new data-contract revision and owner-mediated upgrade; a legacy
namespace with data but no binding fails closed rather than returning incomplete
index results. The
`platform.data.query_index` capability requires its own typed grant operation
and an exact granted logical-key prefix. It permits only equality against one
declared index plus keyset pagination; ranges, sorting, joins, offsets, and
query plans are unavailable.
`put_batch` accepts at most 32 distinct logical keys and
idempotency keys. It validates every schema and host authorization decision
before opening one tenant-RLS transaction, then commits all structured writes
and their idempotency facts together. The durable secret-reference slice now stores a
tenant/module/data-contract-scoped logical name and a host-authorized
`SecretRef` in a separate revisioned/idempotent table with a redacted outbox
fact. The returned artifact handle contains only logical name and revision.
`RegistryArtifactSecretAuthorizer` validates a binding through the deployment
`SecretResolverRegistry` without resolving its value, then requires a host
`ArtifactSecretPolicy` for lifecycle, admitted-policy, and RBAC decisions.
`platform.secrets` admits only declared logical reference and operation names
at the sandbox boundary; resolver aliases, resolver keys, and secret values
remain host-only. Its owner-provided `acquire_handle` broker additionally
checks the injected artifact scope and host authorization before returning only
the logical reference and revision. `ModuleControlPlane` is the only
production composition root for both the binding service and the dynamic
secret-capability resolver; the control-plane verifier rejects their direct
SeaORM construction outside the owner crate. A value-consuming secret-use
broker remains unfinished. The structured-value namespace now has a separate
SeaOrmArtifactDataPurgeService:
it serializes writes and purge through namespace state, permanently tombstones a
purged revision, stores actor/reason/idempotency audit data, and emits an
outbox fact. The service requires a host-provided ArtifactDataPurgeAuthorizer
for lifecycle, legal-hold, retention, and policy decisions; no guest capability
can mark itself authorized.

`SeaOrmArtifactDataExportService` provides the first owner-only export slice.
Each bounded keyset page requires a host `ArtifactDataExportAuthorizer`, an
expected active namespace revision, and actor/reason metadata. It holds the
namespace lifecycle lock while it reads the page and records a redacted durable
audit row plus `module.artifact.data_exported` outbox fact. Export is not a
sandbox capability and is deliberately not described as a full backup snapshot;
snapshot/restore remains a separate pending data-plane operation.

Structured values also expose an authorized keyset list operation. It accepts
only a validated logical-key continuation and a bounded page size of 100, never
a database offset, SQL fragment, or query plan.

`ArtifactDataUpgradePlanner` now produces a read/transform-only plan for one
bounded keyset page when advancing to a higher data-contract revision. It first
finishes the broker read, then invokes only a pre-bound admitted `data_upgrade`
sandbox binding per record and validates each transformed value against the
target contract. The owner bridge rejects another binding kind or ID and uses
the existing admitted artifact executor, so CAS, descriptor schema, and
sandbox-policy checks are retained without exposing this hook as a generic
command.
The plan contains source revisions for later optimistic writes but has no write
authority, checkpoint, lifecycle transition, or open database transaction. A
separate `ArtifactDataUpgradeApplier` rechecks those source revisions, writes
only create-if-absent target records with deterministic per-record idempotency
keys derived from the owner `plan_id`, and then records a redacted checkpoint
through the existing installation revision CAS/outbox path. It holds no
control-plane transaction across the page. A checkpoint failure can retry the
same plan safely; outcome recovery after an uncertain successful checkpoint,
distributed rollout, rollback, and quarantine policies remain pending.
- Implement upgrade, rollback, quarantine, revocation, and uninstall.

Artifact migration checkpoints are committed through the scoped installation
revision CAS and publish `module.artifact.migration_checkpointed` in the same
transaction. The event contains only installation identity, revision, and the
irreversibility fact; checkpoint contents remain owner metadata, bounded to
16 KiB before a control-plane transaction begins.

Artifact uninstall replaces a scoped, inactive marketplace selection only after
checking active direct dependents and records actor, reason, revision,
idempotency, and outbox evidence in one transaction. An idempotency replay must
match the complete immutable command (installation, revision, actor, and
reason), not just its key. It retains CAS bytes,
tenant data, evidence, and rollback history for the retention/reconciler path.
Artifact deactivation is a separate scoped lifecycle operation: it moves only
an active admission to `inactive`, checks active direct dependents, and writes
the audit/outbox fact while preserving the admitted release, data, CAS, and
rollback evidence. Deactivate, tenant disable/enable, and uninstall reject nil
installation, actor, idempotency, and tenant-scope identities before opening a
transaction. Artifact disable remains a tenant-lifecycle concern and is
intentionally deferred to the owner-service/dispatcher cutover: the current
tenant toggle is still compiled-registry based and cannot be reused for an
artifact-only module. The dispatcher now has an explicit artifact-only
constructor, and `ModuleLifecycleDbWriter::artifact_only` persists tenant
state through that catalog-driven path while requiring an admitted runtime
executor and having no static registry fallback. The paired
`disable_artifact_for_tenant` and `enable_artifact_for_tenant` commands share
one revision-CAS tenant-intent path with actor/reason/idempotency metadata and
the corresponding `module.artifact.tenant_disabled` or
`module.artifact.tenant_enabled` outbox fact. They do not change immutable
admission, CAS, data, or runtime-binding state and accept only an admitted
Optional artifact visible in the requesting tenant scope. Destructive purge
remains a separate authorized data-owner operation.

The owned tenant lifecycle schema now separates `enabled` intent and its
revision from the immutable installation/admission record. It persists the
command's expected revision alongside actor/reason/idempotency data, so a replay
must match the full immutable disable command. The disable command uses
expected-revision CAS and outbox. The
structured-value namespace now has an explicit destructive data-owner command.
Its host authorization adapter remains responsible for retention, legal hold,
and installation lifecycle preconditions before that command may delete data.

### M5 - Build and Publication Orchestration

- Define immutable build request/result contracts before adding another crate.
- Keep the owner-owned OCI config, executable-layer, and evidence-referrer
  media types frozen and enforce them when resolving distribution artifacts.
- Publish verified Component bundles only through the owner publication port;
  the distribution adapter uploads the descriptor-configured layer and OCI 1.1
  SBOM/provenance referrers, then fixed Cosign/KMS signing contributes a
  digest-pinned signature-manifest receipt.
- Schedule an isolated worker that uses `cargo_metadata`, `cargo-component`,
  `cargo-deny`, `cargo-vet`, `wasm-tools`, and `cargo-cyclonedx`.
- Accept only verified build outputs and provenance.
- Publish OCI artifacts and attestations by digest; sign through a
  Sigstore/cosign workflow rather than custom cryptography.

### M6 - Transports, Alloy, and Promotion

- Provide the owner operations used by GraphQL and native adapters.
- Accept Alloy stage/fork/publish commands without owning Alloy workspaces.
- Add static-promotion records and distribution-build selection.
- Keep static/native composition distinct from runtime installation.
- Publish declarative UI contributions and bind actions to admitted runtime
  bindings; custom untrusted UI and native UI follow the central isolation and
  static-promotion rules.

## Verification

- Artifact descriptor, executor selection, lineage, and immutable-release tests.
- OCI identity, media type, digest, signature, SBOM, and provenance tests.
- Tenant RLS, lifecycle, Core/Optional, dependency, revision, idempotency,
  recovery, and rollback tests.
- Composition CAS plus build enqueue atomicity tests.
- Governance state-machine/property tests.
- GraphQL/native parity integration evidence through host adapters.
- Repository guardrail proving that this crate owns production writes.
- Artifact-only definition/lifecycle/binding tests with no compile-time registry
  entry, CAS outage/cache tests, dependency-lock tests, namespaced data tests,
  and multi-node reconciliation/outbox replay tests.

## Completion Condition

This local plan is complete when every module control-plane operation is owned
here, all server/admin callers use the owner facade, artifact build/publication
and admission are verifiable, and no replaced server/admin backend path remains.

## Update Rules

Update this plan, the central plan, module registry, and affected consumer plans
in the same change whenever identity, lifecycle, marketplace, build, trust,
installation, sandbox admission, or promotion semantics change.
