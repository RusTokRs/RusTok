# Implementation Plan for `rustok-modules`

## Scope

Own the mandatory Core module control plane: identity, releases, marketplace,
installation, composition, lifecycle, effective policy, build/publication
orchestration, rollback, and static promotion. Optional module implementations
must not become server Cargo dependencies through this crate.

The cross-component sequence and completion rules are defined by the
[canonical module-platform plan](../../../docs/modules/module-control-plane-consolidation-plan.md).

## Current state

The owner boundary has a standalone dependency profile: `rustok-modules` does
not directly import or depend on AI, product, commerce, MCP, Alloy, Leptos,
Axum, or Async-GraphQL. Its runtime foundation uses only the neutral
`rustok-api/runtime` feature, keeping HTTP and GraphQL frameworks out of the
module control plane. The repository verifier checks both this dependency
boundary and the module-owned admin transport for backend write/build logic.

## Current verification evidence

On 2026-08-22, the scoped owner test command
`cargo test --locked -p rustok-modules --lib` passed 243 tests. This includes
the durable build execution-claim/recovery cases, artifact lifecycle and
rollback invariants, CAS/trust/runtime contracts, command-context tenant
identity guards, durable lifecycle/data-purge/settings-recovery outbox evidence
assertions, and static-distribution release receipt/replay evidence assertions.
The UUID-only CLI command-context test and `cargo check --locked -p rustok-server`
also passed. `rustfmt --edition 2024` for touched owner files, `git diff
--check`, `cargo metadata --locked --no-deps`, and the module-control-plane
ownership verifier also passed. This is scoped evidence; it is not a claim
that the workspace-wide compile or test suite passed.

On 2026-08-24, the focused external-prebuilt receipt/replay owner test and its
SQLite migration-schema test passed. The matching `rustok-server` check also
passed after the GraphQL composition-error mapper was aligned to the canonical
platform-scope error. `rustfmt --edition 2024`, `git diff --check`, the
module-control-plane write-path verifier, and the build-worker isolation
verifier passed. This remains scoped evidence; no workspace-wide compile or
test suite was run.

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
- tenant-scoped durable artifact binding idempotency with exact request replay,
  PostgreSQL RLS, transaction-local tenant scope, and tenant predicates on
  claim, completion, abandonment, and expired-lease recovery.

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
  new evidence fact carries the observed positive request revision and advances
  that aggregate through the same transaction; an exact evidence replay returns
  the currently locked revision without another transition. The platform
  evidence producer carries the source revision into the build-service fact and
  then carries that result revision into platform admission.
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
  Publish requests now carry one durable positive aggregate revision. The
  owner exposes it through status snapshots and advances it in the same
  transaction as every current request-state transition. Reject,
  request-changes, hold, resume, final-publication, artifact-attach, and
  validation-enqueue commands carry the revision observed by the authorized
  host snapshot; their SQL updates compare it before mutating state and return
  a typed current-versus-expected conflict on staleness. A validation-worker
  lease carries that same revision, so its result cannot overwrite a later
  request transition, while an exact terminal redelivery remains idempotent.
  Platform-build, external-prebuilt, and Alloy-authored staging use the same
  request CAS and return the resulting owner revision for the next command.
  Manual validation-stage reports and requeues use the same CAS and one
  platform-scoped `ModuleCommandContext`; an immutable receipt rejects an
  idempotency reuse with changed actor, trace, correlation, stage, status,
  reason, or requeue evidence. Remote claim and expired-lease requeue advance
  the request revision inside their owner
  transactions; the claim returns that revision and a terminal runner result
  must present it before its stage transition can commit. Lease heartbeat only
  extends the already-issued operational lease and does not change stage
  lifecycle state. Every current request-state and validation-stage transition
  is therefore revisioned.
  A yank changes only the release lifecycle and records its reason;
  immutable release storage identity remains unchanged while new resolution
  excludes the yanked release. Reupload advances the staged-artifact timestamp,
  so every required evidence fact must have been recorded after the current
  immutable staging operation. `stage_platform_build` reloads a completed build
  pair under tenant RLS, verifies its request slug/version and payload digest
  against the submitted artifact, and appends its immutable source/component/
  OCI receipt identities. The component/payload digest and OCI manifest digest
  are intentionally distinct: staging validates both SHA-256 identities,
  matches only the component digest to the uploaded bytes, and reserves the
  manifest digest for signature/admission joins. Its tenant-scoped
  `ModuleCommandContext` is derived only from the authenticated actor/session
  plus request trace and idempotency evidence; the owner requires its actor UUID
  to match the canonical user principal. The immutable staging receipt persists
  and replays expected revision, tenant, actor, trace, correlation, privilege,
  build, source, component, and authenticated principal together. Final
  publication now requires that current stage.
  Artifact origin is explicit and `unclassified` records fail closed. External
  prebuilts use a separate current stage with an approved provenance policy,
  independent quarantine review, and either a reproducible source identity or
  an explicit source-absence reason; they require author signature and platform
  admission bound to the staged payload digest but cannot use a build-worker
  attestation. The server external staging adapter derives a platform-scoped
  `ModuleCommandContext` (`tenant_id: None`) because the registry aggregate is
  global; the session tenant remains authorization evidence and is not a
  registry command scope. The owner binds the actor and quarantine-approver
  user UUIDs to the context actor, requires the authenticated `modules.manage`
  fact, and persists/replays expected revision, actor, trace, correlation,
  privilege, source/provenance/quarantine facts, and both principals as one
  immutable receipt. The platform build-stage adapter derives its
  tenant only from the authenticated session and forwards only the completed
  build ID, idempotency key, and authenticated privilege fact. The owner then
  authorizes the durable request manager: `modules.manage`, the current owner
  binding, or the original requester before a binding exists;
- publisher-controlled marketplace names and descriptions now pass through the
  owner-owned bounded plain-text projection. It rejects control, invisible,
  and bidirectional override characters; category and tags are bounded
  canonical identifier tokens. The projection exposes AI context only as tagged
  structured data without an instruction field, and gives the server canonical
  `plain_text` / `untrusted_publisher_content` catalog labels. README, source,
  comments, and artifact text have no catalog or prompt projection. Manual and
  remote validation detail plus delivery retry errors are treated as untrusted
  observations: the owner discards them and persists only stable content-free
  stage and retry diagnostics;
- manual validation-stage reports and requeues now use the owner transaction
  for request-state gating, stage transition rules, attempt creation, stage
  plus follow-up audit facts, and the observed request revision. Remote lease
  claim, terminal completion, and expired-lease requeue advance that same
  aggregate; a claim returns its post-transition revision and the runner must
  return it with the terminal result. Heartbeat only renews the operational
  lease. Validation-job enqueue, job claim,
  stale-job recovery, and worker retry telemetry and result materialization now
  use owner transactions. A later authorized enqueue marks a validation job
  still running after 15 minutes as failed with the stable
  `validation_worker_lease_expired` reason and creates the next durable attempt
  atomically. A successful claim now also returns an immutable delivery work
  item containing the exact storage key, SHA-256, size, and content type; if
  those immutable delivery facts cannot be assembled, the owner atomically
  rejects the request and fails the job with content-free audit evidence rather
  than leaving it queued. The independent worker verifies claimed bytes before
  parsing. Artifact contract validation now runs through the pure owner
  `validate_module_publish_artifact` function against the immutable origin and
  metadata snapshot carried by that work item; it no longer needs a server
  request model in the production claimed-job path.
  `rustok-registry-validation-worker` now
  independently polls and conditionally claims that durable owner queue,
  verifies the claimed object bytes, and records the typed result. The server
  endpoint only queues work and has no background-spawn execution path.
  The worker executes origin-specific artifact checks only, then submits immutable evidence
  to one owner transaction that finalizes the request and job, creates follow-up
  stages, and persists their audit facts. The thin local-workspace remote runner
  now reconstructs the canonical publish bundle and requires its SHA-256 and
  crate identity to match the owner-issued claim before any command runs. Its
  client DTO now matches the server's canonical camelCase claim wire fields,
  and it keeps claim artifact URLs out of durable stage detail. Executable
  follow-up stages still require origin-aware exact source/build binding before
  they can be treated as publication-grade automated evidence. The owner now
  creates only origin-selected stages: platform-built requests get
  `compile_smoke` and `targeted_tests` as non-manual `owner_evidence` gates,
  external-prebuilt requests get an `owner_evidence` security/policy gate, and
  Alloy-authored requests get an `owner_evidence` security/policy gate. A
  platform build stage is accepted only when
  its durable successful result includes passed `check`, `test`, dependency
  policy, and vulnerability profiles; that same owner transaction passes the
  compile/test stages, including idempotent replay reconciliation. Generic
  remote claims are restricted to explicitly `remote` stages, and the CLI no
  longer reports local Cargo commands as platform build evidence. The external
  security stage now passes only from current owner facts that bind exact
  external staging/provenance/quarantine, author signature, and admitted
  signature/SBOM/SLSA/license/vulnerability evidence to the submitted artifact. Its
  reconciliation is idempotent and independent of whether staging, evidence,
  or validation arrives first. Alloy release staging now requires the fixed
  capability-free `tests/publication_smoke.rhai` entrypoint to return `true`
  through the production neutral sandbox without entity mutations after the
  same request compiles the production entrypoint and reachable imports. Its
  immutable staging evidence binds the logical execution ID, canonical
  domain-separated zero-input/zero-grant scenario digest, executor, shared
  runtime ABI, and effective sandbox-policy digest to the reviewed source. The
  scenario digest is the first durable Rhai/WASM parity case, while candidate
  execution and full parity remain pending. The Alloy security gate reconciles
  that evidence with the current author
  signature and exact platform admission regardless of arrival order. Platform
  admission independently requires and fingerprints signature, provenance,
  SBOM, license-policy, and vulnerability-policy outcomes. Fixture coverage now
  includes canonical bundle/manifest substitutions, lock-graph source policy,
  SLSA/CycloneDX field substitution, malformed Cosign envelopes, independent
  license/vulnerability admission outcomes, and the capability-free Alloy smoke
  contract. On 2026-07-20 the permitted structural checks passed; compile and
  test suites were intentionally not run;
- draft publish-request creation now uses an owner transaction for the request,
  default-locale metadata translation, and audit fact. The host supplies only
  the authenticated principal and `modules.manage` fact; the owner authorizes
  the current binding, or a user principal while no binding exists, before any
  write or idempotent replay. Artifact object storage remains an adapter; the
  owner transaction attaches a
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
  adapter. A static toggle enters this writer only as a
  `ModuleLifecycleToggleCommand` carrying a tenant-matched
  `ModuleCommandContext`; it accepts neither caller-controlled display identity
  nor separate actor, trace, correlation, or idempotency fields. The journal
  persists the context actor, trace, correlation, and idempotency evidence and
  admits an exact replay before evaluating a no-op so explicit intent and its
  committed receipt remain durable.
  Post-hook retry and compensation enter only as a
  `ModuleLifecycleRecoveryCommand` carrying tenant, operation, and the same
  tenant-matched context. Normalized settings use that context in the shared
  owner-operation receipt, so a changed trace or correlation fails closed on
  replay.
  Server lifecycle transports otherwise supply only the active distribution
  defaults. Compensation returns the exact owner-issued module identity, so the
  server cannot preflight a recovery plan to reconstruct its response.
  For settings, the server supplies only the host-resolved schema and
  owner-normalized JSON; the writer derives active identity, Core status,
  effective enablement, persisted enablement, and settings facts. Lifecycle
  command responses map those owner-issued facts directly and never reload
  `tenant_modules` or `module_operations` server models after the command.
  The owner now applies one `module_static_tenant_lifecycle` aggregate to
  static toggles, normalized settings, post-hook retry, and compensation:
  every command is tenant-matched-context/revision bound, claims the
  aggregate before work, advances its revision only in the durable state
  transaction, and releases the claim on all terminal paths. Settings complete
  an exact owner-operation receipt in that same transaction. A replay of a
  terminal settings failure preserves its typed snapshot, disabled-module, or
  revision conflict rather than collapsing into a generic host error, while
  hook recovery retains its `module_operations` evidence;
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

Current implementation: the UUID-backed shared command context, revisioned
command envelope, optimistic revision/CAS primitive, stable error envelope, and
generic typed snapshot envelope are available from `rustok-modules`. Artifact
activation, deactivation, tenant lifecycle, uninstall, rollback, and migration
checkpoint commands now validate the context against their scope, persist the
same evidence in immutable receipts, and forward it to the outbox envelope.
Dynamic artifact-data purge and the settings-recovery lifecycle use the same
tenant-matched context. Their durable receipts retain actor, trace,
correlation, and idempotency facts; a settings-collection resume reloads the
context that authorized the original `collecting` transition before emitting
its terminal event.
Artifact-data snapshot create, restore, retention, and collection commands do
the same: a staging snapshot retains its create context until finalization,
and a resumed collection emits with the context that committed `collecting`.
Artifact secret binding also uses the tenant-matched context: its durable
operation receipt preserves all five evidence fields, rejects conflicting
idempotency reuse, and emits its outbox event with the same identity. Sandbox
handle acquisition and host-only secret use remain execution-scoped reads and
do not become management-command adapters.
Registry platform-build staging also uses the tenant-matched context. Its
append-only staging receipt binds the completed build and the authenticated
principal to the complete command evidence, so replay with a changed actor,
trace, correlation, expected revision, or privilege fact fails closed.
External-prebuilt staging is platform-scoped because it mutates the global
registry aggregate. Its immutable receipt has no tenant field, binds both
authenticated user principals to the context actor UUID, and rejects changed
expected revision, actor, trace, correlation, privilege, or evidence on replay.
Alloy-authored staging is tenant-scoped: its HTTP and GraphQL adapters derive
the context from authenticated tenant/user identity, request idempotency, and
telemetry trace. Its immutable receipt binds the expected request revision,
Alloy tenant/script, reviewed source and sandbox evidence (including the fixed
publication-smoke scenario digest), and full context;
the staged user principal must equal the context actor UUID, and any changed
context evidence on replay fails closed.
Global artifact security transitions also use the context with no tenant scope.
Their receipt persists the complete platform command evidence and their
quarantine/revocation event preserves the same actor, trace, and correlation
identity.
Static promotion request and approval use the same platform-scoped command
context in their independent receipts and promotion outbox events.
Static distribution bootstrap import, admission, and revocation now use the
same platform-scoped context in their shared release idempotency ledger. That
ledger persists actor, trace, correlation, and idempotency facts, rejects any
replay with changed evidence, and the admission/revocation outbox events retain
the original command identity.
Platform composition now uses the same platform-scoped context. Its receipt is
isolated in the shared ledger's explicit `platform` namespace, keeps the full
context in the request fingerprint, and rejects a tenant-scoped command before
reading the global projection. The post-commit platform build notification
retains the same actor, correlation, and trace facts instead of generating a
new delivery identity. Build and authoring callers use the same typed evidence
without string parsing or compatibility adapters. Other owner services will
adopt these contracts as their write paths are moved. `ModuleControlPlane` is the
owner composition root for currently extracted database-backed services; it is
not a server/admin compatibility facade or a parallel execution path. Server
lifecycle, composition, artifact runtime/HTTP, and registry-governance adapters
obtain their corresponding owner services through this root. Artifact runtime
also receives its exact data/object capability resolvers and redacted execution
audit observer through the root; outbox projection receives the durable artifact
event projector, and routed artifact HTTP receives its binding-idempotency
store. RBAC permission evaluation remains an RBAC-owner adapter.
`EffectivePolicyService` now exposes the same owner-owned catalog/default/tenant
override resolution used by lifecycle commands as a serializable
`ModuleEffectivePolicy`. Its deterministic digest revision covers the exact
catalog, normalized defaults, tenant overrides, and exact artifact runtime
evidence. Selected artifacts now require the existing tenant-RLS installation
resolver, the matching durable sandbox-policy revision, an injected isolated
executor, and an enabled dependency closure. Every module result carries typed
contributing facts and stable denial reasons; no capability grant contents or
resolver error text enter the decision. Enabled-module sets are only
projections of that decision, so server guards, GraphQL, and installer adapters
do not query `tenant_modules` to reconstruct policy. The
installer verification adapter also obtains its static catalog through the
same facade rather than rebuilding it independently.

M2 has started with a transport-neutral definition catalog. It derives static
definitions from the compile-time registry while keeping registry handles
limited to native runtime concerns, and rejects ambiguous active definitions.
The generic static source was replaced atomically by distinct
`platform_native` and `promoted_native` identities. A verified distribution
catalog maps each compiled promotion to its exact promotion revision, registry
release, distribution release/revision, native artifact digest, and persisted
`static_native` executor mode. Effective-policy resolution and toggle
validation consume the same catalog, and the static-distribution lifecycle
constructor retains compiled handles without flattening promoted identity.

The first runtime-activation slice now adds a durable native rollout aggregate.
`ModuleControlPlane::static_distribution_rollout` pins a topology reference and
digest, the active verified release, policy revision, and executor mode in a
desired rollout. Node observations are exact-identity, agent-bound, and
revisioned. An authenticated agent claims exactly one node/role assignment
under a five-minute owner-clock lease; a lost claim response replays the same
unexpired claim to that agent, while another agent can reclaim it only after
expiry. Heartbeat and report verify the same lease, and an agent receives only
the minimum immutable rollout identity for its node/role. The owner enforces
`prepared -> healthy -> active`, emits an activation transition only after
every target is healthy, converges only after every target is active, and turns
post-convergence drift into a recoverable `degraded` state. Request/report
idempotency and all state transitions are transactional with outbox events;
authenticated outside-candidate deployment transport, process supervision, and
traffic wiring remain the uncomposed boundary.

Platform rollout and recovery commands carry one platform-scoped
`ModuleCommandContext`. Their idempotency receipt persists actor, trace,
correlation, and idempotency evidence, rejects a changed replay, and creates
the requested/recovery outbox envelope from that exact context. Node-agent
reports remain separately authenticated deployment observations rather than
operator commands.

The preceding static-distribution build-intent command uses the same
platform-scoped context. Its durable receipt persists actor, trace,
correlation, and idempotency evidence before the immutable build snapshot and
its transactional outbox event are committed.

`ModuleDesiredObservedState`, `ModuleReconciliationPhase`,
`ModuleReconciliationEvidence`, and `ModuleReconciliationFailure` now form the
single reusable desired/observed vocabulary. The static rollout persists and
uses that shared contract without flattening its native release, topology, or
role identities. `ModuleControlPlane::artifact_node_reconciliation` now owns
the durable dynamic artifact/sandbox assignment aggregate. It resolves a
trusted sorted topology to current admitted installation identities inside the
owner transaction; stores release/payload digests, payload kind, admitted
payload media type, admission, dependency graph and capability revisions, ABI,
policy, reports, claims, desired/observed heads,
idempotency receipts, and transactional events; rejects stale lifecycle
identity; and alone activates a fully healthy set. Node agents can claim and
report only one exact assignment. `SeaOrmArtifactNodeReadiness` now gates the
server's non-lifecycle artifact executor against the observed head, comparing
the selected installation, current admission, current canonical policy
revision, and both desired/observed identities exactly, including payload kind
and media type. The neutral sandbox
receives no owner database or policy
access. `rustok-worker-transport` now exposes only the canonical SHA-256
fingerprint of a verified mTLS leaf certificate. The current
`rustok-artifact-node-transport` consumes it for mTLS claim/heartbeat/report
framing and requires an injected fingerprint-to-node/agent map; it never
accepts a principal from an RPC body. The independent
`rustok-artifact-node-controller` composes only that narrow
`ModuleControlPlane::artifact_node_agent()` port rather than reconciliation
authoring or topology access. The separate
`rustok-artifact-node-reconciler` now composes the authenticated topology
service: a verified operator certificate supplies the canonical actor and
limits every target node, while the request carries a platform-scoped
`ModuleCommandContext` (trace, correlation, and idempotency evidence), bounded
topology, expected durable-state revision, and canonical policy revision. The
owner validates the topology and reloads all admitted installation
identity in its transaction, so operator input cannot inject release, payload,
capability, readiness, or policy facts. Its canonical topology digest is also
part of the owner command's idempotency identity, preventing an otherwise
matching replay from substituting a target set. The durable reconciliation
operation ledger retains trace and correlation only for operator requests;
agent reports retain null values because their mTLS principal is their sole
command evidence. The serving runtime now places a bounded
digest-keyed `VerifiedArtifactNodeCache` in front of durable CAS and rehashes
every cache hit. The separately deployed `rustok-artifact-node-agent` now
claims only those mTLS-authenticated assignments, retrieves the exact admitted
digest directly from CAS, atomically materializes and rehashes its canonical
node-local payload cache, and records a runtime-fingerprint-bound preparation
marker. It reports `prepared` only after non-executing local Rhai/Wasm
preparation, and `healthy` only after authenticated remote Rhai-worker
readiness or local Wasm Component compilation; it never executes guest code
for readiness. Static-promotion and sidecar assignments fail closed, while
CAS/filesystem/sandbox outages stay retryable. The process has no database,
topology, release-selection, policy, capability, tenant, AI, Alloy, or
application-server dependency. Authenticated target-topology input and the
effective-policy replacement of the older ephemeral node-readiness input are
complete. Deployment-supervisor evidence and traffic wiring remain open.

The lifecycle entrypoints now use `ModuleExecutionDispatcher`, which resolves
the active definition before invoking a static implementation. Artifact
lifecycle bindings execute only through the admitted sandbox adapter supplied
by host composition; no artifact path falls back to a compiled callback.

Tenant lifecycle toggles calculate the canonical before/after effective-policy
revision and use `ModuleEffectivePolicyTransitionCoordinator` to advance the
durable lifecycle cursor and publish the predecessor-bound transition event in
the same state transaction; a stale lifecycle cursor aborts the state mutation
rather than advancing a divergent projection. A previously unseen tenant first
initializes that cursor to the empty predecessor, so its first effective-policy
transition is durable rather than failing as if a cursor row had been lost.

`ArtifactRuntimeLifecycleExecutor` now requires a host-owned
`ArtifactEffectivePolicyResolver` and re-resolves the canonical policy before
every non-lifecycle binding. Its resolved exact installation is part of that
resolver contract, allowing server composition to require the durable observed
node assignment before CAS/sandbox execution. HTTP, command, event, and
scheduled dispatches therefore fail closed after a policy or node-reconciliation
change even if their transport-level check observed an older revision;
lifecycle hooks remain governed by the owner toggle transaction. Durable event
and schedule adapters use the same shared executor rather than a parallel
runtime path.

The owner facade also exposes bounded `TenantModuleOverrideSnapshot` reads for
operator transports. This is intentionally distinct from effective
availability: it shows persisted tenant intent and settings, while
`ModuleEffectivePolicy` remains the only enabled/denied decision. GraphQL no
longer reads `tenant_modules` directly for this surface or after a lifecycle
mutation; inherited compensation availability is resolved by the same owner
policy service.

Recovery-plan reads now follow the same boundary: the lifecycle owner accepts
the authenticated tenant and returns no global operation view for a foreign
tenant. GraphQL invokes that owner facade for both a single plan and the
bounded failed-plan collection, rather than reading then filtering owner state.

The admin GraphQL adapter now fails closed when module-control-plane reads fail;
it no longer converts its generated navigation registry into synthetic module
registry, installation, tenant-intent, or marketplace success responses. The
native module catalog and registry lifecycle reads now consume the owner-backed
`SharedModuleMarketplaceCatalog` and governance lifecycle snapshot. The admin
workspace/Cargo scanner, local catalog synthesis, canonical hashing,
dependency/build planning, and direct registry SQL have been deleted. The
governance owner also projects durable release metadata onto the shared
marketplace DTO; GraphQL and the public registry adapter now map that DTO and
do not read release or translation tables. Broader transport parity evidence
remains Phase 7 work.

The public publish-status path now uses the owner-scoped
`ModuleGovernancePublishRequestStatusSnapshot` for the exact publish request
and approval previews. The status snapshot supplies request identity,
warnings/errors, accepted state,
approval-override guidance, and the semantic next action in addition to
persisted validation stages, follow-up gates, and actor-visible actions. The
server supplies only authenticated principal/permission context and maps the
semantic action to its HTTP route/text; it cannot substitute lifecycle facts
from another request with the same slug or recreate lifecycle policy from a
SeaORM model. Once all required follow-up stages pass, an approved request now
resolves to final publication rather than repeatedly suggesting the completed
stage operation.
Staging adapters invoke the external-prebuilt and platform-build owner commands
directly and use the same status snapshot for their response
identity/status in dry-run and committed paths. They no longer issue a
server-local publish-request existence preflight or post-command model read and
preserve the owner `not_found` contract at the HTTP edge.
Creation and upload now use that exact status projection for their committed
responses as well. Creation carries only authenticated principal/privilege
facts; the owner checks the current binding before writing or replaying it.
Before an upload, the owner authenticates the actor against
the durable request/binding facts and returns only a SHA-256-derived immutable
slot. The host uses conditional object creation, rehashes a collision before it
can become a replay, and attaches the same metadata through the owner. It never
constructs an artifact key from a server model or deletes a prior object inline;
retention-aware owner policy is the only cleanup authority. The platform
authoring producer uses the same slot, preventing a second artifact-storage
write path.
Release yanking now likewise dispatches directly to the owner. The command
carries platform-scoped context plus authenticated principal/privilege facts;
the owner locks the exact release, derives permission from `modules.manage`,
the durable owner binding, or the release publisher, and returns a minimal
owner-issued result instead of a server SeaORM release model or post-mutation
reread. Its immutable receipt binds actor, trace, correlation, principal,
privilege, reason, and reason code, so only the exact retry succeeds after the
release has been yanked.
Owner transfer follows the same rule: its command carries the authenticated
principal/privilege fact, and the owner locks the current binding, authorizes
`modules.manage` or the bound owner, and records the transition atomically.
Validation enqueue, manual validation-stage reporting, and all live decisions
also map their committed response from the exact owner status projection; the
server no longer rebuilds acceptance, errors, or next-step guidance from a
post-command request model.
Remote-runner heartbeat and terminal-completion adapters now receive the
owner-issued `ModuleRemoteValidationStageTransition` instead of loading a
server validation-stage model after the lease transition. The duplicate
registry-governance remote-runner mutation adapter has been removed, so the
owner-routed transition path is the only implementation. Both remote HTTP
adapters preserve the owner-issued governance error category/code rather than
maintaining a lease-specific error taxonomy; not-found detail stays
content-free.
Runtime guardrails likewise obtain their active and expired remote lease counts
from the owner-issued `ModuleRemoteValidationRunnerSnapshot`, not a server
`registry_validation_stages` model query. Only `running` remote stages count as
active or expired leases; a failed owner snapshot is reported as critical
instead of being hidden as zero work.
Manual validation-stage reports now carry stage/status/reason data directly to
the owner, which canonicalizes and validates the command before its state
transition. The obsolete server stage model and parser were deleted, and the
public request no longer accepts its formerly ignored `detail` field.
The owner status projection now also returns durable actor-specific
`can_manage` and `can_review` facts. Live validation, validation-stage report,
and moderation adapters authorize from those facts instead of server-local
publish-request or owner-binding reads; unauthenticated status projections
carry no governance actions. It also supplies rejected-request retry
eligibility, effective publisher identity, and latest validation-stage facts,
removing server publish-request model reads from every live operation on an
existing request.
The artifact-download adapter likewise receives only a host-only owner snapshot
of attached storage key and content type. It treats a missing or unattached
artifact as unavailable and does not expose storage topology through the public
publish-status contract.
Validation-queue and validation-stage dry-run previews likewise load only the
exact owner status snapshot after authenticated authority is established; they
do not preflight a server request model. Their live commands remain delegated
to owner-authorized mutation services.
Approve, reject, request-changes, hold, and resume previews use the same
snapshot; approval override warning text and pending-stage facts stay
owner-derived rather than reconstructed by the HTTP adapter.
The owner-transfer adapter now sends only authenticated host facts to its
owner command. The owner derives authorization from the locked durable binding,
and the adapter performs no preflight or post-command binding read.
The registry access middleware also consumes only the owner-issued request
authorization snapshot; it no longer reads publish-request or owner-binding
SeaORM models before forwarding a request to the controller.
The remaining server-local publish-request, translation, validation-job, and
governance-event SeaORM models, plus their unused status-label mapper, have
also been deleted: lifecycle status is now carried exclusively by the
`rustok-modules` owner projection.
`ModuleGovernanceError` likewise owns the stable error category and code used
by registry transports. The HTTP adapter maps only that owner contract to an
HTTP envelope, keeps not-found detail content-free, and no longer maintains a
parallel lifecycle-error taxonomy.

GraphQL platform-native install, uninstall, and upgrade mutations require a
direct SuperAdmin whose authenticated tenant matches the routed tenant and
whose effective permissions include `modules:manage`. The routed tenant is an
authorization anchor, not composition scope: the adapter constructs a
platform-scoped command context with no tenant identity, a non-nil idempotency
UUID, and a positive expected revision. The owner rejects any tenant-scoped
context before receipt admission, admits the canonical command in the platform
receipt namespace before the host reads the durable snapshot, then atomically
commits the composition CAS, build enqueue, and terminal owner-operation
receipt. An exact retry replays the original immutable build after later
composition changes; at-least-once platform build notification is re-emitted
with the original actor, correlation, and trace evidence without another build
record. The admin obtains only the owner-issued composition revision and
forwards it with a UUID; it neither calculates a manifest hash nor parses the
build execution identity. No GraphQL resolver loads, mutates, validates,
serializes, or hashes a composition manifest directly.

The artifact tenant-lifecycle owner now exposes a bounded snapshot for one
admitted Optional installation and tenant. It returns inherited enabled intent
as revision `0` with next expected revision `1`, while an explicit state returns
its current revision as the next CAS precondition. GraphQL maps this owner
snapshot and a single `setArtifactTenantEnabled` mutation: authenticated tenant,
actor, and `modules:manage` permission are derived at the transport boundary;
the caller supplies only installation identity, enablement, positive expected
revision, reason, and UUID idempotency key. The mutation delegates to the
existing revision-CAS/exact-replay/audit/outbox owner transaction and returns no
admission, storage, or raw conflict internals. The same authenticated tenant
boundary now owns `activateTenantArtifact`, `deactivateTenantArtifact`,
`uninstallTenantArtifact`, and `rollbackTenantArtifact`. Each command derives
`ModuleInstallationScope::Tenant` from `TenantContext`; GraphQL has no client
scope or rollback-target selector. The rollback request carries only the
owner-required capability-grant revision and migration mode, so the owner can
choose only its retained direct predecessor. Platform-scoped lifecycle commands
are intentionally not exposed through GraphQL: `modules:manage` from a tenant
context is not platform authority, and the server fails closed pending an
explicit platform-operator authorization contract.

The server's shared GraphQL document authorization classifies the tenant
lifecycle snapshot as `modules:read` and all tenant lifecycle mutations,
including enablement, as `modules:manage` before resolver execution. The owner
facade remains the second authorization boundary. The same guard classifies the
composition snapshot as read and marketplace registry freshness as manage,
matching those resolver/owner boundaries. `enabledModules`, the tenant
availability projection consumed by admin navigation, is read-gated at both
layers.

Focused server verification for this transport slice passed
`cargo check --locked -p rustok-server`, the six `module_security` library
tests (covering the lifecycle snapshot, all five lifecycle mutations,
composition snapshot, registry freshness, and enabled-module availability),
and the lifecycle conflict redaction unit test. These are package-scoped checks; no workspace-wide
compilation or test run is claimed.

Platform active-build/history reads are host-composed through the read-only
`rustok_build::SharedBuildControl`. The duplicate build-owned release table,
active head, rollback command/event, GraphQL/native surface, and admin controls
are removed. Static release admission, activation, desired/observed rollout,
recovery, and their events now have one owner in `rustok-modules`.

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
Artifact settings reuse that cache through a separate owner write entrypoint:
the definition catalog retains the selector digest and admitted schema bundle,
and `persist_artifact_settings` resolves and validates that exact object before
the tenant state write. Static host-manifest normalization has a distinct
entrypoint which rejects artifact definitions, and the lower-level tenant
settings store is no longer exported. Data-contract validation remains its
separate installation-scoped owner path but now shares the same bounded
compiled-validator implementation instead of maintaining a second JSON Schema
configuration. Every
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

Dynamic artifact UI is strict and declarative. The bundled
`contracts/ui-contribution.schema.json` plus typed descriptor contract admit
only host-rendered `admin_settings`, `admin_actions`, `admin_status`,
`admin_help`, `admin_navigation`, `admin_table`, `admin_form`, and
`storefront_slot` surfaces. Contributions have no executable component source,
HTML/markup, CSS, URL, iframe, query, authentication behavior, locale fallback,
or native frontend package. They use a digest-verified, bounded plain-text
localization catalog; every declared locale carries the same key set and lookup
is exact, leaving locale selection entirely to the host. Actions/forms must
reference the exact admitted `Command` binding, with the same module-owned RBAC
permission, bundled input/output schemas, required idempotency, explicit
confirmation/destructive parity, and a required audit policy.

`SeaOrmArtifactInstallationStore` resolves typed navigation-route (including
child-page) and storefront-slot collisions before it changes admission state.
It locks global resource identities in deterministic order, then compares a
tenant candidate against the platform baseline and its own overlay, or a
platform candidate against every active tenant overlay. PostgreSQL grants that
cross-overlay read only to the transaction-local module-control-plane platform
owner context; tenant transactions retain their normal RLS view. The same
guard is applied before a rollback reactivates its predecessor, and rejected
candidates remain admitted at their prior revision. The platform-owned
`POST /api/artifacts/{installation_id}/ui/contributions/{contribution_id}/execute`
transport accepts only an admitted Action or Form contribution, resolves its
exact required-idempotency Command binding, and delegates to the same RBAC,
schema-validation, durable idempotency, and audited sandbox path as generic
artifact commands. The runtime supplies the exact admitted binding ID as a
host-selected redacted neutral sandbox audit label. The companion
`GET /api/artifacts/{installation_id}/ui/contributions/{contribution_id}/audit`
transport resolves and authorizes the same contribution, then reads evidence
only for its exact tenant, installation, and binding. It returns no payload,
output, actor, trace, credential, capability, or grant data. Host renderers
receive effective-locale delivery through the server-owned
`GET /api/artifacts/{installation_id}/ui/contributions` projection. It takes
the effective locale only from server middleware, filters each contribution by
its admitted dynamic RBAC permission, and returns host-safe localized text plus
admitted schemas where rendering requires them. The client cannot select a
locale; an unavailable exact locale hides the contribution rather than falling
back. `rustok-api` owns the framework-neutral
`ArtifactUiContributionView` DTO consumed by host transports; the module owner
maps admitted descriptor data into it once. REST and the headless GraphQL
`artifactUiContributions(installationId)` read share one server adapter for
per-contribution dynamic RBAC and exact request locale; GraphQL has no locale
argument or fallback. The headless action mutation
`executeArtifactUiAction(installationId, contributionId, input,
idempotencyKey)` resolves only an admitted action/form contribution to its
exact Command binding, then shares REST's effective-policy, dynamic-RBAC,
durable-idempotency, sandbox-dispatch, and audit path. It exposes no raw
binding selector. The REST and GraphQL audit reads resolve that same
contribution and return the framework-neutral
`ArtifactBindingExecutionAuditEntry` DTO; they expose neither a raw binding
selector nor payload, output, actor, trace, credential, capability, or grant
data. Raw catalogs/keys, permissions, binding IDs, and executable UI material
are absent from the response.

Earlier focused verification on 2026-08-22 passed all 227 `rustok-modules`
library tests after aligning the Alloy-fork SQLite fixture with the
owner-required publish-request `updated_at` field. After adding the audit
reader, its binding-evidence and canonical SQLite-migration tests both passed,
as did the package-scoped `rustok-server` check. Exact-locale projection and
binding-identity redaction tests also passed. The canonical `rustok-api`
artifact-UI DTO tests and a dependent `rustok-server` check then passed after
the projection moved out of the module owner. The shared GraphQL contribution,
action, and audit adapters then passed the same `rustok-server` check plus
focused `rustok-api artifact_ui` (3 passed), server `artifact_ui` (1 passed),
and `module_security` (4 passed) library tests. `rustfmt --edition 2024`,
`git diff --check`, and `cargo metadata --locked --no-deps` are rerun before
handoff. No workspace-wide compile or test run is claimed.

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
returns a typed, revisioned decision set with per-module facts and denial
reasons for a supplied catalog. The server effective-policy adapter, lifecycle
writer, and installer verification provide only infrastructure inputs instead
of reproducing enablement semantics. Artifact lifecycle policy now resolves the
exact active installation and matching grant revision through the same owner
ports used by runtime and denies missing executors or dependencies. The
channel boundary is now typed in the policy owner. A host or `rustok-channel`
adapter maps its resolved `ChannelDetailResponse` to
`ModuleEffectivePolicyChannelInput` (tenant, channel, surface, immutable
channel revision, and module bindings) and calls
`EffectivePolicyService::resolve_for_channel`. The modules crate does not
resolve channel tables or depend on `rustok-channel`; missing optional bindings,
disabled bindings, and inactive channels are explicit denial reasons and the
channel snapshot participates in the policy revision. The policy owner now
also accepts a revisioned maintenance snapshot with explicit global or
module-scoped impact and a bounded reason code. Active maintenance produces a
typed denial without rewriting tenant intent. Dynamic node readiness is not a
host-supplied effective-policy input: the stale snapshot family and its
policy-revision contribution were removed. Instead, the server resolves the
canonical base policy and then `SeaOrmArtifactNodeReadiness` requires the exact
admitted installation identity from the converged durable observed head before
any non-lifecycle artifact dispatch can read CAS or cross the sandbox boundary.
No host-provided readiness value can alter artifact availability, routing, or
the policy revision. Core process health remains a distinct host readiness
concern and cannot substitute for an artifact assignment.

The reusable `ModulePolicyRevisionTransition` and `ModulePolicyRevisionGate`
contract is the common consumer primitive for existing transactional module
events. It does not infer ordering from digest values: only a matching durable
predecessor applies, exact replays are idempotent, and divergent transitions
remain stale until an owner reconciliation supplies the correct predecessor.
`SeaOrmModulePolicyRevisionConsumer` is the durable adapter: it creates and
row-locks one `(tenant_id, consumer_key)` cursor under tenant RLS, applies the
gate, and commits only an `Applied` successor. Duplicate and stale deliveries
commit without advancing the cursor. Its `apply_in_transaction` entry point
also lets an owner append its state mutation, outbox event, and cursor advance
to one existing transaction; no consumer may acknowledge a transition before
the corresponding owner mutation is durable.
`ModuleEffectivePolicyTransitionPublisher` is the matching producer boundary:
it validates a real `sha256:` predecessor/successor pair and appends an
explicit `module.effective_policy_revision_changed` event to the owner
transaction. Existing security and distribution revisions remain separate
contracts and must not be routed through this publisher.

Resolved policy snapshots now also expose a tenant-scoped
`EffectivePolicyCacheIdentity`. It binds any future cache entry to both the
tenant and the exact content-addressed policy revision; a TTL or process-local
generation cannot make a stale decision current. The server carries this
identity with its effective-policy snapshot. Shared cache storage and durable
invalidation wiring remain open.

The current Phase 8 security slice adds `SeaOrmModuleArtifactSecurityService`.
It persists global `clear/quarantined/revoked` state keyed by immutable artifact
release identity, separates ordinary registry yanking from emergency
enforcement, and uses exact idempotency receipts plus revision CAS. Quarantine
can be cleared only through an authorized command; revocation is terminal. A
tenant's enablement row is never rewritten by these transitions. The read-only
security resolver contributes registry status and redacted security evidence to
`ModuleEffectivePolicy`. Focused policy evidence now asserts the complete
execution distinction: quarantine and emergency revocation deny a still
tenant-enabled module, while ordinary registry yanking does not stop an
already-installed clear release.

The server constructs the compile-time `ModuleRegistry` exactly once during
runtime bootstrap and shares that static implementation registry with the
router, GraphQL, lifecycle, event-dispatch, and installer adapters. Marketplace
definitions and effective policy are resolved through owner services; no
request path rebuilds a registry from durable artifact state.

Phase 4 begins with the transport-neutral `ModuleBuildRequest` /
`ModuleBuildResult` protocol 8 in this owner crate. It carries immutable source,
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
under tenant RLS at revision `1` and emits `module.build.queued` through the
transactional outbox without invoking a worker inline. `claim_queued` performs
the one durable `queued -> running` revision-CAS transition, returns an opaque
owner claim, and replaces only an expired claim. The lease exceeds the maximum
admitted worker deadline, so a healthy worker cannot be replaced mid-build.
`record_result` accepts a terminal result only from that exact still-live claim,
clears it while advancing the revision, and replays only an identical terminal
result. `load_completed` exposes that same stored request/result pair only
under tenant RLS and revalidates it before a later owner staging operation may
consume it;
RLS, then emits `module.build.completed`; duplicate results must match their
stored digest. `rustok-module-build-transport` now maps the remote-worker port
onto the single current mTLS gRPC service with authenticated readiness, no
generation suffix, and no
in-process fallback. `claim_queued` and `dispatch_queued` provide the owner-side
outbox-consumer delivery path: they release tenant-scoped database state before
the RPC and accept the terminal result only through immutable owner validation
and its durable execution claim.
`rustok-module-build-worker` is now a separately deployable mTLS process. It
can invoke only a fixed image-owned non-symlink runner whose SHA-256 digest is
rehashed at construction, readiness, and immediately before spawning in a fixed
workdir with a cleared environment, request-derived timeout, and aggregate
streamed output cap. Production construction requires the bounded deployment-owned isolation
attestation; its schema rejects unknown fields, there is no public
attestation-free constructor, and execution reloads that file through the
readiness gate before accepting each request. Its current source is a
`cas://sha256:<hex>` archive from a
deployment-mounted read-only root. The worker uses the shared
`rustok-build-source` strict USTAR materializer; the former private parser was
removed rather than retained as a second path. It materializes under a
request-scoped directory without a CAS client. Digest, archive-safety, and
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
polling or execution fallback. Worker evidence generation, scoped OCI
publication, and build-service signing are implemented; deployment supervisor
evidence and the independently authorized final governance decision remain
separate gates.
`rustok-modules-cli` now supplies the owner-local authoring entrypoints for
`module init`, `module validate`, `module test`, `module build`, `module
package`, `module publish`, and `module inspect`. Init
renders the independently versioned
canonical template, writes only create-new paths, uses pinned Cargo to generate
the lockfile under a bounded timeout, and removes only its newly created root
when initialization fails. Validate checks the source declaration, recorded
SDK/template/toolchain identities, native Component target, fail-closed policy,
and checksummed lock graph without accessing the server, database, worker,
sandbox, AI, Alloy, CAS, or publication credentials. Package uses the shared
deterministic bounded USTAR writer and returns the immutable source digest/CAS
identity; inspect uses the same project preflight or strict worker archive
parser without materialization. Test uses sanitized, bounded, offline Cargo to
produce the native WASI P2 Component, rehashes the regular output, then executes
it through the real neutral Wasmtime executor with a bounded scenario that binds
typed grants/limits, fixtures, input, and the expected output/error code. It is
local author feedback, not trusted build evidence. Validation and test
projections expose the scenario's domain-separated canonical digest; a completed
test additionally emits only a redacted `success` or `expected_error`
comparison result, without returning fixture payload. Build requires explicit
tenant, actor, project, trace, correlation, and idempotency identity and calls
only the shared owner control with a non-serializable
`PreparedModuleSourceArchive`, not a transport-supplied filesystem path.
The shared `ModuleAuthoringSourceArchiveBuilder` is the only host-materializer
path for a queued build: it writes the private deterministic archive with the
same fixed profile the owner uses for its later CAS scan, so the CLI and any
future Alloy materializer cannot diverge on archive limits.
Template initialization also delegates its data-only rendered files to the
shared `SourceTreeMaterializer`, eliminating CLI-local recursive filesystem
writes and giving reviewed host materializers the same path and resource policy
before source archive creation.
`SeaOrmModuleAuthoringBuildService` constructs
the immutable request with owner-selected build policy, while
`CasArchivePublisher` rehashes and strictly scans the private archive before an
atomic no-replace source-CAS commit. Queue persistence and its outbox fact remain
owned by `SeaOrmModuleBuildService`; remote dispatcher delivery remains outside
the CLI process. Publish constructs the current source-manifest metadata bundle
and calls only `ModuleAuthoringPublishControl`. The owner reloads the completed
tenant build, content-addresses the bundle, creates the deterministic governance
request, binds the build stage, and queues validation; approval, admission, and
final release creation remain outside the author CLI.
The shared source-archive crate passes its five focused deterministic
writer/strict reader/publisher unit tests. A filtered provider test for the expanded CLI
did not finish dependency compilation within either bounded 60-second attempt,
so the package/inspect adapter does not yet have Rust compile evidence. Its
static boundary guard, Rust 2024 formatting, metadata, and diff checks pass; no
full compile or test suite was run. The neutral local-scenario harness now
passes all three focused tests with the real `wasm-component` feature enabled.
The focused template scenario test and earlier five-command CLI provider test
did not finish compilation inside their bounded windows, so their direct Rust
compile evidence remains open. The first focused owner authoring-request attempt
also exceeded that window, but after the dependency cache completed both owner
request/policy tests passed. The expanded seven-command CLI provider test again
exceeded 60 seconds while compiling dependencies and was terminated without a
result.
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

The source archive must contain a strict `module-artifact.json` declaration.
The worker validates it before any author code runs, rejects an author-supplied
component digest, and binds its module identity and executable contract to the
immutable request. The runner's successful result is now bound to the fixed
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
creates the final descriptor exactly once after Component/WIT inspection by
inserting the independently verified component digest into that source
declaration; runner-provided descriptor output is rejected. It collects only
fixed inspected output files, uses its
deployment-owned scoped registry destination, and attaches the receipt to the
terminal result. Owner persistence rejects a successful result without that
receipt. Signing and admission are enforced by the separate build-signature and
verification-worker policies described below.

The former server background `rustok-build` polling executor has been removed.
`rustok-build` remains only for reviewed static role-plan construction and
trusted build primitives. It cannot consume `module.build.queued`, implement
the module build-worker port, publish a role bundle, or own release state.

The current build result derives its toolchain and WIT digests from domain-separated
immutable request fields. The owner rejects a result that substitutes either
contract, in addition to checking its source, dependency lock, attempt, tenant,
resource bounds, and terminal outcome. `retryable` is true exactly when the
terminal result permits `retry_build`; no worker may label a retry as either
forbidden or required while reporting the opposite next action.

An optional reviewed Rhai predecessor is an immutable field of the canonical
platform build request. The owner copies that exact release reference into the
durable platform-build staging receipt, compares it on idempotent replay, and
projects it into final artifact lineage. Before staging and final publication,
the owner requires the predecessor to be the same module slug, semantically
older, active, published through the Alloy Rhai path, and admitted as a Rhai
runtime. The worker receives the immutable provenance fact but does not decide
marketplace ancestry.

OCI artifact media types are frozen in the owner crate for immutable descriptor
config, Rhai, WASM Component, sidecar, static-promotion payloads, and
SBOM/provenance/test-evidence/release-lineage referrers. The distribution
adapter rejects mismatched config media types, declared sizes, and raw config digests, then
accepts exactly one descriptor-selected executable layer. The scoped publication
adapter uploads verified descriptor-configured payloads and OCI 1.1
SBOM/provenance referrers. The isolated build worker then signs the returned
digest-pinned artifact through fixed Cosign/KMS configuration and records the
resolved compatible signature-manifest digest. Owner governance keeps the
component/payload digest distinct from that OCI manifest identity and requires
the matching author, build-service, platform-admission, and marketplace facts
before final publication.

The public OCI reader and publisher constructors now create the platform-owned
strict registry transport. It enforces HTTPS, verified TLS, no redirects, no
process/system proxy, connection/request deadlines, bounded retries, bounded
transfer and decompressed response size, identity-only response encoding, and
one request at a time. It holds that request permit through response streaming,
rejects cross-origin upload locations, and never forwards Basic credentials to
a different host. The transport owns only the digest/tag manifest and streaming
blob reads plus monolithic blob and manifest writes used by the control plane;
unsupported OCI workflows fail closed. `oci-distribution` remains only for its
OCI data model and registry-auth DTO. OCI identities are constrained to
registry host, repository, and digest rather than URLs; the build worker obtains
repository-bound credentials only after its credential-broker lease. The
registry adapter separately bounds complete descriptor/layer admission to five
minutes, streams the config only after its declared descriptor-size check, and
cancellation-safely deletes a partial staging file. Config and payload streams
reject received bytes beyond their OCI-declared size before extending memory or
disk staging, and reject a final size mismatch before descriptor parsing or
payload digest acceptance. The worker separately bounds its complete
publication window to 15 minutes, while the OCI adapter cancels a complete
artifact-and-referrer publication after ten minutes, leaving bounded time for
Cosign within that worker deadline.

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
physical storage identity. The server registers `platform.secrets` through
`ModuleControlPlane::artifact_secret_handle_policy`; it repeats exact
installation, lifecycle, capability-revision, explicit-grant, and
derived-scope validation immediately before the logical binding read. MCP and
every other unregistered capability remain default-deny until their owner
deployment adapters are available.

`resolve_granted_artifact_capability` is the shared gate for every dynamic
owner route. It resolves the exact immutable installation, applies active
admission, tenant lifecycle, uninstall state, durable policy revision, and the
named explicit grant before a broker is constructed. The concrete
`SeaOrmArtifactDataCapabilityBrokerResolver`,
`SeaOrmArtifactDataObjectCapabilityBrokerResolver`,
`SeaOrmArtifactSecretCapabilityBrokerResolver`, and the facade-constructed
`ArtifactMcpCapabilityBrokerResolver` derive their scopes only from that exact
result. The sandbox host already enforces data operation/prefix, logical-secret,
object-data prefix/operation, logical-secret, and MCP server/tool grant
constraints before a route runs. The composed server executor registers
`platform.data`, `platform.data.objects`, `platform.secrets`, and
`platform.mcp`. The secret route returns only an owner-issued logical handle
and revision; it never resolves a value or discloses resolver identity. The MCP
route recognizes only the deployment-owned stable `rustok` alias, derives a
service identity from the exact admitted artifact subject, applies the MCP
owner access policy, and requires redacted durable audit before invoking the
owner-defined read-only registry tool surface. There is no endpoint,
credential, discovery, arbitrary network, or fallback broker. Artifact event
delivery is durable ingress into admitted bindings and is not modeled as an
outbound guest capability.

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
an ambient timestamp or caller-owned installation identity. Its complete
scope-matched `ModuleCommandContext` is persisted with the actor, trace,
correlation, and idempotency evidence, while its canonical request digest covers
that context plus the immutable OCI reference, scope, dependency lock, and
sandbox policy. The store reserves that identity before inserting installation
state and binds it before committing the outbox fact. Successful retries return
the same installation identity; a permission-registration retry refetches and
verifies the immutable descriptor so it can replay the owner request. A reused
key with changed context or request evidence fails closed.

Admitted artifact permissions are represented by immutable localized
label/description entries. The current installation path sends them through the
shared `ArtifactPermissionRegistrationPort` after its durable installation
commit and uses installation ID as idempotency identity. The release-safety
cutover replaces that ambiguous admission/install coupling: release admission
atomically stores inert definitions keyed by exact
release/module/definition digest and creates no scope or grant; scoped install
projects the exact definitions idempotently under
`(scope, installation, release)`, and enablement resolves separate scope-owned
role/actor grants against the active serving generation. Rollback, disable,
remove, uninstall, retention, and collection preserve definition/grant/audit
references under their exact holds. This path can only register RBAC
vocabulary; role and actor grants are absent by contract.
The target permission preview compares predecessor/candidate stable identities,
exact canonical authorization fingerprints, and affected roles. A grant may
carry only when identity and every authorization-relevant
scope/key/resource/action/binding constraint has the same fingerprint and an
RBAC-owner continuity receipt authorizes it; localized display text is excluded
or governed separately. The receipt and every carry/rollback commit bind the
current monotonic scope grant/role-membership epoch under the RBAC owner fence.
Any fingerprint change requires explicit approval, removed grants become
dormant, and rollback selects predecessor definitions then evaluates current
grants without restoring a revoked grant or membership. Admission and install
never assign access implicitly.
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
  composition, effective-policy, build, promotion, and static-distribution
  subservices.
- Define narrow infrastructure ports for database transactions, OCI, trust
  verification, build scheduling, events, audit, clock, and IDs.
- Keep atomic boundaries inside owner operations.
- Introduce the durable artifact-aware module definition catalog and generate
  static definitions from the compiled implementation registry.
- Move dependency/effective-policy/lifecycle decisions off Rust trait objects.
- Introduce the runtime binding registry/dispatcher for static and sandboxed
  implementations.

Current infrastructure slice: the owner now exposes
`ControlPlaneInfrastructure` with narrow clock and UUID ports.
`ModuleControlPlane` carries that context into its installation, build, release,
publication, binding-idempotency, event-projection, and schedule-delivery
services, while `ModuleInstaller` accepts the same context for deterministic
installation and verification identities. Admission persistence uses one
injected time for its command reservation, sandbox policy, and admission
evidence; installation lifecycle operations use injected operation/outbox
identities; build submission/completion uses injected outbox identities; and
governance uses the context for aggregate, evidence, stage, claim, event, and
validation-lease identities. Durable binding claims now derive operation and
lease identities from that context. Event and schedule queues derive delivery
and scheduler work-lease identities from it, and schedule materialization uses
the injected owner time. Object-data upload sessions, private object/chunk keys,
GC candidates, export aggregates, and export/purge outbox events now use the
same context; the facade exposes the object capability resolver, export, purge,
and retention-GC owner services. Transactional database-expression timestamps
intentionally remain storage-owned. Secret-binding mutation receipts and outbox
events, generated lifecycle correlations, durable/in-memory CAS stages, and OCI
temporary staging paths also use the context. The server obtains its runtime CAS from the same
operation-scoped facade as capability, installation, and policy services. A
crate-wide production-source audit now leaves direct system clock and random
UUID access only in the default infrastructure adapters; tests may create their
own fixtures. Registry, CAS, trust, OCI publication, composition-build enqueue,
and isolated-build worker ports already exist. The caller-supplied SeaORM
connection plus owner-opened transaction is the storage adapter;
`rustok-outbox::TransactionalEventWriter` is injected through the infrastructure
context; and redacted runtime `ExecutionObserver` plus transactional owner audit
rows/outbox facts are the audit boundaries. Domain operations no longer
construct `OutboxTransport`, publish outside their transaction, or write a
second audit journal. The M2 infrastructure-port slice is complete.
On 2026-07-20 the permitted `rustfmt --edition 2024`, `git diff --check`, and
`cargo metadata --no-deps` structural checks passed; compile and test suites
were intentionally not run.

### M3 - Complete Server Ownership Cutover

- The server no longer exposes its unused database-truncation helper. That
  helper directly deleted owner-owned tenant-module rows, so the target runtime
  boundary removes it instead of preserving a privileged reset backdoor.
  The static write-path guard now detects direct SeaORM `Entity` mutation
  methods in addition to raw SQL and protected `ActiveModel` construction.
- Transactional module events now use the facade infrastructure for their
  identity, correlation identity, timestamp, tenant scope, and available actor
  identity. The former direct constructor calls confused the event identity
  with the tenant field and the tenant identity with the actor field. A static
  guard now reserves root-envelope construction to the infrastructure adapter.
- `ModuleControlPlane::promotion` is the sole production composition root for
  the current static-promotion request and approval owner. It admits only an
  active platform-built release, revalidates the completed build request/result
  and publication receipt under tenant RLS, and pins the exact source and
  dependency-lock digests. Approval requires ownership, dependency-audit, test,
  and static-review evidence with revision CAS, durable exact-replay
  idempotency, and a distinct host authorization decision. Request and approval
  authorization are separate fail-closed port methods, and the owner rejects an
  approval from the persisted requester. `ModuleControlPlane::static_distribution`
  is now the sole production composition root for complete approved-promotion
  selections. It revalidates every pinned release/build pair, pins platform
  source, toolchain and target identities, creates an immutable
  build-lineage-linked build intent under a separate CAS head, and records exact-replay idempotency
  plus outbox evidence. `ModuleControlPlane::static_distribution_worker` is the
  separate worker composition root for atomic claim/reclaim, bounded leases,
  heartbeats, immutable attempts, and terminal completion evidence. Expired
  claims are closed before reuse and completion replay requires the identical
  command digest. None of these services can compile, mutate active composition,
  activate a release, or load native code.
- The owner now runs platform composition snapshot/bootstrap/revision-CAS,
  receipt admission/completion, and atomic build-request creation. The server
  performs full typed-manifest, deployment-selection, and registry validation,
  supplies the build-record adapter, and publishes the build notification only
  after the owner transaction commits. The platform-state digest is the
  canonical composition snapshot identity; the build record stores the distinct
  immutable execution-request identity that also binds that composition digest,
  deployment profile, and execution plan.
- Move registry governance, publication stages, releases, ownership, holds,
  approvals, rejection, yanking, and event taxonomy.
  `registry_publication_evidence` is the authority-separated immutable ledger
  for release evidence, and the final owner publication transaction enforces
  the required authority facts. Both platform-build and external-prebuilt
  staging are owner-owned, durable, and exposed through authenticated server
  adapters. Alloy-authored staging is also owner-owned and binds the exact
  uploaded workspace checksum to the reviewed Alloy source digest, fixed
  production-sandbox smoke execution and canonical scenario digest, shared runtime ABI, and effective
  capability-free policy digest. Its owner-evidence security stage also
  requires the current author signature and exact platform admission. The
  independent registry validation worker treats artifact contents as untrusted:
  it verifies the claimed storage facts, selects validation by immutable origin,
  bounds normal publish bundles at 2 MiB and Alloy workspaces at 1 MiB before
  parsing, and emits content-free diagnostics, so raw artifact/request strings
  do not enter governance events through this validation path. Rendering and AI
  prompt boundaries remain separate unfinished
  work alongside OCI policy enforcement.
- Move remaining effective-policy composition.
- Own static module-settings schema validation and normalization behind the
  neutral `ModuleSettingSpec` contract. The server resolves its typed manifest
  schema only, then passes that schema and the requested JSON object to the
  owner before lifecycle persistence. Static normalized persistence and dynamic
  artifact settings persistence are now explicit separate entrypoints; the
  latter locks and resolves one active admitted installation, uses its exact
  descriptor selector, and cannot accept a host-supplied schema. Dynamic
  values persist only under `(tenant_id, data_owner_id, settings_instance_id)`;
  compatible activation inherits those opaque identities from its direct
  predecessor only with matching registry/repository continuity and immutable
  settings schema. Static manifest settings remain in `tenant_modules`.
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
  artifact declaration and capability policy. Admission leaves the pointer
  unset. The owner activation operation serializes `(scope, slug)`, freezes the
  sole active non-uninstalled predecessor at its then-serving revision, makes
  it inactive, writes the candidate pointer, and makes the candidate active in
  one transaction. Its durable operation receipt makes exact retries replayable
  and its outbox fact is `module.artifact.activated`; ambiguous serving state is
  rejected. The later rollback command advances the predecessor atomically with
  its lifecycle transition. A separate rollback-operations record supplies
  durable actor/reason audit and a unique idempotency key; it does not duplicate
  mutable lifecycle state. Its immutable command fingerprint also records the
  selected capability-grant revision and migration rollback mode, together with
  the committed source/target revisions, so an exact retry replays after the
  source admission changes. Historical rows without that complete fingerprint
  fail closed rather than guessing a result.
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
the `get`/`put`/bounded-`put_batch`/`delete`/bounded-`list` input shapes.
`SeaOrmArtifactDataCapabilityBroker` routes those operations to this owner
service after tenant/subject checks; batch entries must have distinct keys and
idempotency keys under declared prefixes, while list queries use an escaped
logical-prefix filter and continuation validation.
Structured `delete` requires an exact positive revision and UUID idempotency
key, obtains a distinct host authorization decision, removes all materialized
indexes for the logical key in the same transaction, and persists its replay
receipt under tenant/data-contract/policy scope.
Structured and direct-object put receipts now use that same policy revision in
their durable idempotency primary keys, so a UUID result from an older
capability policy cannot replay under a newer policy. Owner-only export evidence
and namespace-purge receipts also persist the exact policy revision used for
authorization.
An authorized namespace purge removes structured records and private-object
metadata in its transaction and queues every unreachable private key. The
tenant-scoped `SeaOrmArtifactDataObjectGcService` deletes a queued key only
after a supplied retention snapshot explicitly approves it; missing rules and
legal/audit/rollback holds fail closed rather than issuing a guest-driven
physical delete.
That purge does not delete artifact settings. The implemented separate dynamic
artifact-settings owner service creates a protected encrypted recovery point
bound to exact scope, stable data owner, installation-to-settings-instance
binding, settings instance/revision, admitted schema/descriptor, canonical
validated value, and unresolved secret handles. Host policy supplies the
retention snapshot; its purge/restore authorization receives immutable
recovery policy, retention revision, holds, KMS key version, and lineage.
Purge revalidates authenticated ciphertext and commits a monotonic settings
tombstone. Restore creates a fresh non-serving settings instance and may bind
it only to an explicit compatible inactive installation under the same owner;
after uninstall/retirement it stays unbound and never clears retirement.
Settings deletion is denied when matching restore-tested evidence is missing;
role/actor grants and external secret bytes are never implicit snapshot or
purge targets. Recovery retention is revision-guarded and monotonic (it can
only extend expiry or add holds), KMS rewrap is host-owned, and collection
records a durable `collecting` intent before it terminally clears ciphertext
while preserving recovery evidence and the original typed command context. A
resumed collector reloads that context, so its terminal outbox event cannot
acquire a different actor, trace, or correlation identity. An
intentionally unbound restored instance has a separate one-time,
continuity-authorized bind command that requires exact data owner,
registry/repository lineage, slug, schema, and inactive successor checks.
The target settings owner installs a compatibility guard before dynamic or
native/static rollout, binding both N/N+1 schema digests and rollback-window
identity. Every concurrent write CAS-revalidates the intersection through
rollback closure. A one-sided value requires a separate confirmed maintenance
command that fences writers and atomically closes rollback eligibility.
Settings recovery points persist independent encrypted retention/hold/
collection state and exact KMS-key/schema/descriptor roots; purge/restore
  revalidate decryptability, target schema/admission revision, secret handles, and holds. Retention
mutation, KMS rewrap, terminal crash-resumable collection, and one-time
continuity binding are explicit owner-service lifecycle operations with their
own durable receipts and outbox facts.
The current artifact-settings owner already binds values to a stable opaque
data owner and exact settings instance rather than a tenant/slug row. The
structured-data `ArtifactDataScope` still derives its persistence identity from
tenant, module slug, data-contract revision, and policy revision; its separate
release-safety cutover must replace that attach authority with the same stable
owner model and verified publisher/module lineage. Reinstall and update require
an exact continuity receipt; a different publisher using the same slug/revision
is denied, while a legitimate owner change uses a separate privileged,
conflict-fenced governance-transfer receipt without copying or deleting data.
The target canonical mutable-state key is
`(scope_id, data_owner_id, namespace_or_settings_instance_id, revision)` for
platform- and tenant-scoped dynamic installations. First install creates only
declared mutable boundaries; stateless/no-settings releases persist
`not_applicable`. Active update inherits the exact owner and instances;
`start_empty` is limited to first install or reinstall, while changing an
active binding is a separate fenced maintenance migration/cutover.
Artifact-data snapshots bind exact scope, stable data owner, namespace instance
and revision, and data-contract digest. Slug/version/installation are metadata,
not restore authority. Post-purge restore assembles a new isolated namespace
under the same owner and CAS-cuts over the active reference; the old tombstone
is never cleared.
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
true streaming WIT object I/O remains future work. Object metadata and all
durable digest columns require canonical lowercase
`sha256:` values, and upload idempotency is isolated by immutable policy scope.
The owner enforces the 32 MiB object quota across the full durable chunk set,
not merely at completion.
Completion claims a durable `completing` state before publication; expiry reaping
atomically transitions only expired open/completing sessions before queuing
chunks, so completion and collection cannot race the same session.
The object broker also exposes an explicitly granted logical `delete`
operation. It requires an exact positive object revision and UUID idempotency
key, obtains a distinct `ObjectDelete` host authorization decision, removes
only matching metadata, persists the replay receipt under tenant RLS, and
queues the private storage key for retention-aware GC in the same transaction.
The guest receives only the logical name and deleted revision; it cannot
request or observe inline physical deletion.

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
and their idempotency facts together. `ArtifactDataQuota` is the host-selected
quota snapshot for the exact policy-scoped broker. Artifacts cannot set or
increase it, and custom deployment limits cannot exceed the platform ceilings:
10,000 structured records/64 MiB of canonical JSON, 1,024 live objects/256 MiB,
sixteen active upload sessions, and 64 MiB of staged chunks per namespace.
Projected structured/object count and byte usage is checked under the same
namespace lock and transaction as the revision write. Replacements subtract
the prior value, batches see earlier writes in their transaction and roll back
fully on rejection, logical deletion releases live capacity, and staging
limits aggregate every active session in the tenant/module/data-contract
namespace across policy revisions. Guarded restore receives its target quota
from `ArtifactDataSnapshotAuthorizer` and rejects an oversized canonical
manifest inside the restore transaction before publishing rows.
`ArtifactDataQuotaPolicy` is the standalone owner port for resolving stricter
deployment limits after exact installation/capability admission;
`ModuleControlPlane` composes the same policy into both structured and object
capability resolvers.
The durable secret-reference slice now stores a
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
the logical reference and revision. `ModuleControlPlane` is the production
composition root for the binding service, dynamic secret-capability resolver,
and host-only value-use service; the control-plane verifier rejects direct
SeaORM construction outside the owner crate. Value consumption is now a
separate host-only service rather than a sandbox `get_value` operation.
`SeaOrmArtifactSecretUseService` requires an exact handle revision and stronger
`ArtifactSecretUseAuthorizer`, reloads the reference under tenant RLS, closes
that transaction, resolves a redacted `SecretString`, and lends it to one
host-composed fixed-purpose `ArtifactSecretValueConsumer`. The consumer returns
no payload; the service exposes only logical reference, revision, and purpose
in its receipt, while resolver/consumer failures remain content-free.
`ModuleControlPlane::artifact_secret_use` is the composition entrypoint.
Concrete consumers retain responsibility for their operation-specific
idempotency and redacted audit evidence. The structured-value namespace now has a separate
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
sandbox capability and is deliberately not described as a full backup snapshot.

The current durable backup/restore implementation is a separate owner boundary
exposed only through
`ModuleControlPlane::artifact_data_snapshot`. Snapshot creation locks an exact
active namespace revision under tenant RLS and captures at most 1,000 structured
records, 64 private objects, 8,192 materialized index rows, and 256 MiB of object
bytes. Object metadata is staged transactionally, then each immutable source
key is copied to a snapshot-owned private key and re-hashed. Retries resume the
same idempotent `staging` snapshot; only complete copies publish a `ready`
canonical logical manifest digest and outbox event. Object GC takes the same
namespace lock and retains a source key while a staging snapshot references it.

Restore requires separate host authorization, the same tenant/module/data
contract identity, a ready manifest with verified digest, and an empty active
target at the expected namespace revision. It copies and re-hashes snapshot
objects before atomically restoring structured values, object metadata,
materialized indexes, the index contract, namespace revision CAS, durable
idempotency/audit data, and the restore outbox event. A purge tombstone is never
cleared and live data is never replaced.

The accepted release-safety cutover replaces that current restore identity with
exact `(scope_id, stable data_owner_id, namespace_instance_id,
namespace_revision, data_contract_digest)` and adds durable per-copy intents.
Module slug and installation are metadata, never attach authority. Post-purge
restore builds a new isolated non-serving namespace under the same data owner
and performs a separately authorized active-reference CAS; it never clears the
old tombstone or reuses the purged namespace.

Snapshot retention has its own optimistic revision and owner authorization.
The idempotent retention command can only extend the deadline, while legal hold
can be explicitly applied or released; the state, redacted operation, and
outbox event commit together. `ModuleControlPlane::artifact_data_snapshot_collection`
composes a bounded collector over private storage. A pass scans at most 1,000
ready/collecting snapshots and completes at most 100. New collection requires
a distinct host authorizer, deadline expiry, no legal hold, and an explicit policy-snapshot rule with no
audit or rollback hold; missing rules fail closed. The owner records actor,
reason, and policy identity before switching to `collecting`. Blob deletion is
idempotent, so a crash resumes the durable decision, and final manifest deletion
preserves independent retention, restore, and collection audit rows while
publishing `module.artifact.data_snapshot_collected`. Full control-plane
disaster recovery remains Phase 11.

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
through the existing installation revision CAS/outbox path. Its owner command
supplies authenticated actor/reason/idempotency facts, and an immutable receipt
digest replays an uncertain successful checkpoint without a second revision or
outbox event. It holds no control-plane transaction across the page. A
checkpoint failure can retry the same plan safely; distributed rollout,
rollback, and quarantine policies remain pending.
- Implement upgrade, rollback, quarantine, revocation, and uninstall.

Artifact migration checkpoints are authenticated revision-CAS commands. Their
durable receipt binds installation/scope/revision, checkpoint digest,
irreversibility, actor, reason, and idempotency key before the installation and
outbox transition commit. An exact replay returns the stored revision without
another event; divergent reuse conflicts. The
`module.artifact.migration_checkpointed` event contains only installation
identity, revision, and the irreversibility fact; checkpoint contents remain
owner metadata, bounded to 16 KiB before a control-plane transaction begins.
Focused SQLite lifecycle receipt/replay, data-upgrade forwarding, and lifecycle
identity/revision validation tests passed after this command contract was
added; no workspace-wide test run is claimed.

Artifact uninstall replaces a scoped, inactive marketplace selection only after
checking active direct dependents and records actor, reason, revision,
idempotency, and outbox evidence in one transaction. An idempotency replay must
match the complete immutable command (installation, revision, actor, and
reason), not just its key; a new command against an already uninstalled
selection is rejected before it can reach persistence. It retains CAS bytes,
tenant data, evidence, and rollback history for the retention/reconciler path.
Artifact deactivation is a separate scoped lifecycle operation: it moves only
an active admission to `inactive`, checks active direct dependents, and writes
the audit/outbox fact while preserving the admitted release, data, CAS, and
rollback evidence. Deactivate, tenant disable/enable, and uninstall reject nil
installation, actor, idempotency, and tenant-scope identities before opening a
transaction. Artifact tenant intent is owned by the installation aggregate;
the generic compiled-module toggle is never reused for an artifact-only
module. The dispatcher retains an explicit artifact-only constructor for a
host-composed admitted lifecycle-binding workflow, with no static registry
fallback, but it is not a second tenant-intent persistence path. The paired
`disable_artifact_for_tenant` and `enable_artifact_for_tenant` commands share
one revision-CAS tenant-intent path with actor/reason/idempotency metadata and
the corresponding `module.artifact.tenant_disabled` or
`module.artifact.tenant_enabled` outbox fact. They do not change immutable
admission, CAS, data, or runtime-binding state and accept only an admitted
non-uninstalled Optional artifact visible in the requesting tenant scope. An
uninstall operation therefore rejects a later tenant lifecycle command before
it can write a new intent record. Destructive purge remains a separate
authorized data-owner operation.

The owned tenant lifecycle schema now separates `enabled` intent and its
revision from the immutable installation/admission record. Its immutable
tenant-scoped receipt ledger records installation, requested state, expected
and committed revisions, actor, reason, and idempotency key. Exact retries
therefore replay their original committed revision after later commands or
uninstall without another event; divergent key reuse fails closed. The mutable
intent row retains only its current state and most recent audit metadata. The
commands use expected-revision CAS and outbox. The
structured-value namespace now has an explicit destructive data-owner command.
Its host authorization adapter remains responsible for retention, legal hold,
and installation lifecycle preconditions before that command may delete data.

Focused SQLite lifecycle coverage verifies that an exact tenant-disable replay
after a later enable and after uninstall returns its original revision without
mutating current intent or emitting another outbox fact.

### M5 - Build and Publication Orchestration

- Define immutable build request/result contracts before adding another crate.
- Keep the owner-owned OCI config, executable-layer, and evidence-referrer
  media types frozen and enforce them when resolving distribution artifacts.
- Publish verified Component bundles only through the owner publication port;
  the distribution adapter uploads the descriptor-configured layer and OCI 1.1
  SBOM/provenance referrers, then fixed Cosign/KMS signing contributes a
  digest-pinned signature-manifest receipt.
- Schedule an isolated worker that uses `cargo_metadata`, pinned native Cargo
  targeting `wasm32-wasip2`, `cargo-deny`, `cargo-vet`, `wasm-tools`, and
  `cargo-cyclonedx`. Do not retain the superseded `cargo-component` path.
- Accept only verified build outputs and provenance.
- Publish OCI artifacts and attestations by digest; sign through a
  Sigstore/cosign workflow rather than custom cryptography.
- [x] Stage completed author builds through the owner-local publication
  control. The current CLI builds a bounded metadata bundle from
  `module-artifact.json` and `Cargo.toml`; the owner reloads the exact
  tenant-scoped completed build, fixes platform-built/third-party/sandboxed
  policy, stores the bundle by SHA-256, creates an immutable idempotent request,
  binds the source/Component/OCI receipt stage, and queues registry validation.
  Metadata-bundle, Component payload, and OCI manifest digests are separate;
  the author path cannot approve, admit, sign, or finalize a release.
- [x] Persist platform admission as a complete immutable publication contract:
  logical registry identity, OCI repository and manifest, payload and canonical
  descriptor digests, descriptor, runtime/media type, and one typed
  digest-bound signature, provenance, and SBOM evidence identity. Conflicting
  admission replay fails closed.
- [x] Materialize the canonical federated artifact projection in the final
  owner publication transaction. It joins the admitted contract to exact
  origin-specific source lineage, author signature, build-service attestation
  when applicable, platform admission, and marketplace approval, validates the
  shared marketplace DTO, and persists it create-once beside the immutable
  release. Active catalog releases are exposed only through this owner query;
  a missing or corrupt contract is not downgraded to metadata-only output.
- [x] Compose the production caller for the reserved build-service-attestation
  and platform-admission owner operations. The independent registry-validation
  worker reloads exact stage/build facts through the owner, obtains a short-lived
  registry credential lease, fetches and revalidates the digest-pinned OCI
  package, calls the isolated verifier through readiness-gated mTLS, and records
  both immutable facts through owner operations. The server, Alloy, MCP, AI, and
  module runtime receive neither registry credentials nor trust roots.
- [x] Project configured-registry freshness through the owner catalog port.
  The neutral `rustok-api::MarketplaceRegistryFreshness` contract carries only
  logical registry identity, typed status, last-success Unix milliseconds, and
  consecutive failures. It omits endpoint and remote error content. GraphQL
  and native operator transports require `modules.manage`, and the Leptos and
  Next module operator screens render and refresh the same projection. The
  local manifest provider is not misreported as a federated registry.
- The 2026-07-27 projection slice was checked only with touched-file
  `rustfmt --edition 2024`, `git diff --check`, and
  `cargo metadata --no-deps`; no compile or test suite was run in the shared
  worktree.

### M6 - Transports, Alloy, and Promotion

- Provide the owner operations used by GraphQL and native adapters.
- Accept Alloy stage/fork/publish commands without owning Alloy draft
  workspaces. For continued development it materializes a published Rhai
  workspace only from the exact active owner projection and verified CAS bytes,
  after media-type, lineage, and canonical-digest validation; Alloy, catalog
  DTOs, and mutable OCI references are not source authorities.
- Static-promotion request, review, approval, and idempotency records are now
  owner-owned. Approved-record distribution selection is now owner-owned as a
  full immutable snapshot and queued build intent. Worker claim, lease,
  heartbeat, attempt audit, and terminal completion are also owner-owned.
  Promotion and distribution items pin the registry-owned Cargo package and
  normalized native entry type in addition to exact CAS source and dependency
  digests. These fields participate in composition hashing and activation/
  rollback revalidation; promotion callers cannot supply them.
  Verified release activation now accepts only the current successful build,
  revalidates its immutable selection, persists inert admission and emits
  outbox evidence without deploying code. The rollout operation freezes the
  then-serving direct predecessor and exact `(node, role)` assignments;
  convergence activates the candidate. Recovery revalidates and redeploys
  retained predecessor bytes and never queues a build. Revocation serializes
  through owner release state and preserves immutable evidence.
  The current-only `ModuleStaticDistributionExecutor` port and owner
  `dispatch_next` orchestration now claim before invoking the external
  executor, heartbeat the durable lease while it runs, and persist its outcome
  only through the existing completion validation. Transport failure remains
  reclaimable and is not rewritten as a terminal build failure. The shared
  build-worker transport now maps the port to the separate current-only
  `rustok.static_distribution` mTLS service with authenticated readiness and
  no plaintext constructor. `rustok-distribution` now generates deterministic
  Cargo dependency, promoted registry source, and canonical manifest outputs
  only from a fully validated running claim; its output digest binds every
  immutable manifest field, output destination, and generated Cargo/Rust byte
  sequence. `rustok-static-distribution-worker` now independently hosts the
  current mTLS service, re-hashes its fixed launcher/config at
  startup/readiness/execution, stages idempotent bounded claim inputs, clears
  the launcher environment, bounds its lifetime, suppresses concurrent duplicate
  execution, and accepts only a terminal receipt bound to every immutable job
  identity. Its strict job config now pins and revalidates CAS, Cargo, Rustc,
  publisher, toolchain, target, and resource identities. The launcher library
  regenerates all inputs, materializes a new platform workspace through the
  shared strict archive parser, verifies promoted Cargo package/version/lock
  identity, rejects alias collisions, and applies generated files only there.
  The fixed launcher binary now resolves the final workspace lock offline,
  binds its digest to evidence, runs only locked tests and release compilation,
  invokes the digest-pinned publisher, validates its fully bound receipt, and
  writes the owner receipt. Infrastructure retries rebuild only the derived
  workspace and reuse a valid publisher receipt; immutable attempt inputs are
  never overwritten. The concrete publisher now uploads the fixed native
  executable, CycloneDX SBOM, SLSA provenance, and raw test evidence through
  generic current OCI publication primitives, signs the exact artifact digest
  through the shared digest-pinned KMS Cosign boundary, resolves the signature
  manifest, and writes a create-only fully bound receipt. Deployment-owned
  registry/KMS configuration and end-to-end integration evidence remain, along
  with independent worker deployment.
  Every immutable distribution item now persists `static_native` executor mode,
  which participates in composition and generated-output digests. Verified
  release reads reload and validate the complete build before exposing its
  items. Runtime catalog composition maps those items to exact promoted-native
  lifecycle definitions instead of treating them as anonymous platform-native
  modules. Runtime rollout and desired/observed convergence remain separate.
  Promotion reads the current release
  `checksum_sha256` identity directly and does not retain a legacy checksum
  alias or fallback query.
- The build-worker transport exposes only the current `rustok.module_build` and
  `rustok.static_distribution` services. The generation-suffixed module-build
  package was deleted instead of retaining a compatibility service, plaintext
  constructor, or fallback route.
- The 2026-07-22 Phase 10 slices were checked only with
  touched-file `rustfmt --edition 2024 --check`, `git diff --check`, and
  `cargo metadata --no-deps`; no compile or test suite was run in the shared
  worktree.
- The 2026-07-22 Phase 7 marketplace/lifecycle cutover and current-only catalog
  route update were checked with touched-file `rustfmt --edition 2024`,
  `git diff --check`, and `cargo metadata --no-deps`; no compile or test suite
  was run in the shared worktree.
- Keep static/native composition distinct from runtime installation.
- Publish declarative UI contributions and bind actions to admitted runtime
  bindings; custom untrusted UI and native UI follow the central isolation and
  static-promotion rules.

## Planned Cross-Module Release Safety Integration

`rustok-modules` is the canonical operator-level owner of module update intent,
the executable transition decision, direct-predecessor selection, the
durable operation and atomically acquired cross-scope conflict fence set,
rollback eligibility, incident outcome, and
desired-versus-observed rollout state. `rustok-build`, `rustok-migrations`,
sandbox, and deployment components remain narrow execution/evidence ports and
must not retain a second operator rollback lifecycle. The cross-module
adoption plan is documented in
[Module Release and Rollback Plan](../../../docs/modules/module-release-rollback-plan.md).

Automatic mode is computed for one exact candidate/predecessor pair and live
scope from owner-validated evidence; caller migration modes, module prose, and
the central readiness board cannot authorize it. Every updateable module,
including a stateless module, records its readiness constraints locally.
Dynamic recovery selects one installation predecessor. Static recovery selects
the complete predecessor role composition, including embedded Leptos assets,
and deploys already retained and revalidated immutable bytes rather than
compiling during the incident.

`rustok-build` owns canonical role-plan/validation primitives, while
`rustok-static-distribution-worker` is the sole static role-bundle
executor/publisher and returns one canonical receipt. Release identity binds
the immutable bundle; live topology, controller authority, observations, and
deployment receipts bind the rollout operation. Outside-candidate recovery
uses atomically reserved single-operation authority whose exact replay resumes
idempotently and whose divergent replay is denied.

The controller and node agent come from one separately signed operations-tool
release outside the application bundle. Fresh bootstrap verifies and installs
that exact prerequisite before the owner ledger exists. After minimal owner
schema import, every upgrade uses `operations_tool_maintenance` as an operation
class in this same canonical operation/receipt ledger with the fleet/module-
transition conflict fence, exact host/component desired/observed assignments,
old/new protocol matrix, and one predecessor recovery. The host supervisor is a
narrow executor that retains predecessor tools and reports idempotent
observations; it owns no version selection or second authoritative
ledger/lifecycle.
It may keep the documented non-authoritative local restart journal for exact
assignment replay; PostgreSQL owner state remains the sole authoritative ledger
and convergence source.

The target supply boundary is explicit: this owner's single unversioned
`SourceObjectStore` authenticates preparation scope and owns globally
deduplicated `source_digest` blobs, distinct RLS-scoped `source_receipt_id`
records over owner/preparation/media-type/length/manifest, same-request
idempotency, and all-reference retention in the generic source CAS
(deterministic archives for
platform/native/WASM and canonical bounded-workspace objects for reviewed Rhai
releases), trusted build attempts use isolated job directories, complete static role
bundles and evidence publish to OCI, dynamic WASM/Rhai payloads publish into
the platform object-store CAS, PostgreSQL stores only owner control records,
and deployment nodes materialize digest-addressed static bytes into disposable
cache plus predecessor-preserving slots. A node path, symlink, tag, PID, or
build row is never production identity. Candidate and direct
predecessor role bytes must be pre-staged and rehashed on every node that can
lose predecessor capacity before automatic mode is admitted.

The first durable native-rollout slice now records each exact
`(node_id, role)` assignment, its candidate role digest, its operation-bound
predecessor role digest when one exists, and role-scoped observation/idempotency
receipts. Multiple roles may therefore converge on one portable instance
without sharing a mutable node-level observation. The remaining recovery work
must consume these retained predecessor digests through the same desired/
observed reconciler; it must not queue a compiler build during an incident.

For the default local/monolith installation, every physical plane is derived
from one trusted operator-selected `<instance-root>` using the canonical
relative layout in the cross-module plan. The path may be anywhere supported
by the operating system and is host placement/restart evidence only.
Distributed adapters may map subtrees to external providers; no fixed Linux
path, drive, container mount, or directory spelling becomes release, module,
migration, object, or operation identity.

Candidate preparation and a production transition are separate durable
operations. Preparation failure rejects the candidate without changing the
serving release. The transition vocabulary covers initial platform install,
platform update, operations-tool maintenance, native module add/update/remove,
dynamic admit/install/enable/
update/disable/remove/uninstall/reinstall, explicit predecessor recovery, and
separate guarded dynamic artifact-data/artifact-settings purges. Dynamic remove
retains the exact inactive installation as
the direct predecessor; terminal uninstall is allowed only after that
code-rollback eligibility closes. Uninstall from disabled-selected state first
commits absent selected/desired state and tenant intent plus a new binding/work
generation, then retires the identity; delayed enable/outbox delivery cannot
reactivate it. Uninstall after remove-to-absent only performs retirement.
First platform install has no invented
predecessor; a first dynamic
installation may recover only to an explicit absent/disabled baseline. Dynamic
selection, binding generation, durable work generation, outbox delivery, and
retention collection must converge together so stale work cannot reactivate a
removed release. Next deployments are external and manual and do not gate this
lifecycle; only generic public-client compatibility evidence can affect
automatic backend eligibility.

Dynamic release admission and scoped installation are also separate target
states. Admission publishes one immutable verified release/CAS identity and
creates no tenant selection or executable binding. A later install/update
operation creates the inactive scoped installation, predecessor/lock graph,
and non-routable binding/work intent before readiness and enablement. The
current combined installation/admission persistence path is replaced
atomically; it does not survive as an alternate command.

Dynamic cache identity uses a stable fingerprint over the exact
executor/engine binary, engine-config revision, isolated-worker image/target
where applicable, runtime ABI, and placement-relevant target. Pool generation
is a separate mandatory readiness identity: compatible prepared bytes may be
rehashed and reused, but smoke readiness repeats for every new generation.
Automatic mode is denied unless both candidate and predecessor have current
receipts on every fingerprint/generation that may serve or recover the
operation.

The target adds one crash-recoverable operation, canonical conflict-set
acquisition, one automatic attempt, trusted candidate-attributed health
evaluation, pre-traffic recovery after predecessor displacement, an
outside-candidate static control path, mixed N/N+1 data and bounded drain
evidence, and a point-of-no-return gate before irreversible effects. It never
automatically restores production data. The direct `rustok-build` operator
rollback and rebuild-on-rollback path are removed. Caller-selected artifact
migration mode remains a required atomic cutover item; no compatibility path is
retained.

Each preparation owns a `preparation_id` and an explicit platform-public or
tenant-private authorization/RLS domain. Immutable CAS bytes may deduplicate
without exposing private preparation/release metadata, evidence, or logs;
cross-tenant reference is allowed only for a platform-authorized public catalog
release. Each platform- or tenant-scoped transition creates a separate
RLS-isolated `operation_id`, correlation, idempotency, and diagnostic domain and
receives only authorized facts plus sanitized evidence references. Concurrent
tenant installs never share authority, raw logs, or replay identity.

Current production gaps include the uncomposed release-admission installer and
update coordinator, empty unfinished-admission recovery scan, absent executor
prefetch/readiness and admission/recovery reconcilers, the authenticated
outside-candidate deployment transport/process supervisor that composes the
owner-issued assignment lease with node-local materialization, singular static
publisher and rollout identity, missing `operations_tool_maintenance`
coordinator/fleet projection, and incomplete retention collection. The artifact
runtime lifecycle executor itself is already composed by the server and is not
this gap.
The cutover removes these authorities and gaps atomically; documentation or UI
projection cannot report the release-safety target as available before the
corresponding runtime verification gates pass.

## Verification

### 2026-08-30 registry publication command-context slice

- Final registry publication now enters the owner only through
  `ModulePublishRequestPublicationCommand` with a validated platform-scoped
  `ModuleCommandContext`. The owner binds the structured user principal to the
  context actor UUID and stores actor, trace, correlation, idempotency, and
  approval facts in the immutable publication receipt. Exact replay succeeds;
  a changed trace, correlation, actor, or approval fact fails closed.
- The session-backed REST approval transport derives the context from verified
  authentication, the required idempotency header, and server telemetry; it
  does not accept actor or correlation evidence from the request payload.

### 2026-08-30 registry review command-context slice

- Reject, request-changes, hold, and resume now enter the owner with the same
  validated platform-scoped `ModuleCommandContext`. Each command binds the
  structured user principal to its context actor UUID.
- A single immutable review-receipt ledger records the operation kind, expected
  revision, actor, trace, correlation, idempotency, reason, reason code, and
  committed result. Exact replay returns successfully after the request has
  transitioned; reusing the key with a changed context or review fact fails
  closed.

### 2026-08-30 registry release-yank command-context slice

- A live release yank enters the owner with a validated platform-scoped
  `ModuleCommandContext`; its structured user principal must bind to the
  context actor UUID.
- The owner locks the release before checking the immutable receipt ledger and
  records the lifecycle transition, audit event, and receipt together. The
  receipt binds actor, trace, correlation, principal, privilege, reason, and
  reason code; exact replay returns the committed result, while changed input
  or a reused key fails closed.

### 2026-08-30 registry owner-transfer command-context slice

- Owner transfer uses the same validated platform-scoped context and immutable
  exact-replay contract. The owner locks the slug binding and records previous
  and new owners, actor, trace, correlation, privilege, reason, and reason
  code in the transition transaction.

### 2026-08-30 registry publish-request-create command-context slice

- Publish-request creation preserves its deterministic business-command request
  ID while storing command context separately in an immutable receipt. The
  receipt binds idempotency, actor, trace, correlation, principal, and
  privilege facts, preventing context-free success on a later retry.

### 2026-08-30 registry publish-artifact-attach command-context slice

- Artifact attach requires a validated context whose actor UUID matches the
  structured uploader principal. The owner locks the request before checking
  its immutable receipt ledger, then commits the status transition, audit
  event, and receipt atomically.
- The receipt records request revision, artifact metadata and storage result,
  prior storage key, reupload state, actor, trace, correlation, principal, and
  privilege facts. An exact retry returns that stored result; every changed
  immutable fact and any pre-receipt historical attachment fail closed.

### 2026-08-20 typed static lifecycle command slice

- Static tenant lifecycle toggle now requires the owner-only
  `ModuleLifecycleToggleCommand`; the authenticated transport supplies tenant
  and actor UUIDs plus an idempotency UUID, and no-op intent receives its own
  committed journal receipt. An exact key replay returns that receipt, while a
  divergent key reuse is non-retryable.
- Post-hook retry and compensation now require
  `ModuleLifecycleRecoveryCommand`. The owner rejects nil tenant, operation,
  actor, or idempotency identities, derives audit actor text from the actor UUID,
  and does not accept transport-controlled actor labels. Admin transport and UI
  send a fresh idempotency UUID for both recovery mutations.
- Focused verification passed: the owner no-op replay unit test; the complete
  19-test `rustok-server` lifecycle integration target; the 34-test
  `rustok-admin` module-composition GraphQL guard; and the 9-test server
  lifecycle-bypass guard. The checks emitted existing Windows linker messages,
  Cargo's admin output-name collision warning, and the existing
  `proc-macro-error2` future-incompatibility warning. No workspace-wide compile
  or test suite is claimed.

### 2026-08-09 lifecycle owner-result slice

- Lifecycle toggle, retry, compensation, and settings transports now map
  owner-issued operation/state facts and no longer reread `tenant_modules` or
  `module_operations` persistence models after a command. The owner settings
  result returns the module slug, persisted enablement, and normalized JSON;
  the lifecycle toggle result carries the exact settings fact used by its
  command and idempotent replay. The server lifecycle state snapshot preserves
  the owner-issued operation identity for lifecycle transitions rather than
  suggesting a persistence reread.
- Retry and compensation carry the authenticated tenant into the lifecycle
  writer, which treats a recovery operation from another tenant as not found
  before dispatch or state mutation. Retry returns the completed
  owner-issued recovery plan directly, so GraphQL no longer performs an
  authorization pre-read or a post-command recovery-plan reread.
- GraphQL `installedModules` now maps the installed projection returned by the
  platform-composition adapter instead of reading the manifest through
  `ManifestManager` at the transport boundary.
- Passed touched-file `rustfmt --edition 2024`, `git diff --check`,
  `cargo metadata --locked --no-deps`, the module control-plane write-path and
  lifecycle-bypass guardrails, the focused owner snapshot test, the 198-test
  `rustok-modules` library suite, and both default and
  `--no-default-features` `cargo check --locked -p rustok-server`. The server
  checks emit unrelated existing warnings; no workspace-wide compile or test
  suite is claimed.

### 2026-08-09 remote validation owner-observability slice

- The focused owner test proves that active and expired remote lease counts
  exclude terminal and non-remote stages. Touched-file `rustfmt --edition
  2024`, `git diff --check`, `cargo metadata --locked --no-deps`, and the
  module control-plane write-path and lifecycle-bypass guardrails passed. The
  current `cargo test --locked -p rustok-modules --lib` run passed all 198
  tests. `cargo check --locked -p rustok-server --no-default-features` also
  completed successfully, with pre-existing warnings outside this slice.
- The focused `remote_executor_guardrail_tests` server unit test passed with
  `--no-default-features`, as did the owner validation-stage normalization and
  server schema regression tests. The `module_lifecycle` integration target
  was attempted with both feature selections, but the host exhausted Windows
  virtual memory while compiling the default UI graph; the subsequent
  no-default attempt inherited an inconsistent target cache. Neither attempt
  reached the test runner, so no lifecycle integration success is claimed.
- The owner-category/code regression test passed. The follow-up server owner
  cleanup also passed `cargo check --locked -p rustok-server --no-default-features`,
  touched-file `rustfmt --edition 2024`, `git diff --check`, `cargo metadata
  --locked --no-deps`, and the module-control-plane write-path plus lifecycle
  bypass guards. Server warnings remain outside this slice; no workspace-wide
  compile or test suite is claimed.
- The focused remote-transition regression also passed after the owner
  classified a runner-mismatched remote lease as permission denial rather than
  a lifecycle conflict. Both remote HTTP adapters preserve that canonical
  category/code contract.

### 2026-08-02 federated-registry freshness slice

- Passed touched-file `rustfmt --edition 2024`, `git diff --check`, the module
  control-plane owner guard, and
  `verify-marketplace-registry-freshness.mjs`.
- The focused Rust test and final metadata result are recorded after the
  bounded verification step; no full compile or test suite was run.

### 2026-08-02 module-build isolation bypass closure

- Passed touched-file `rustfmt --edition 2024`, `git diff --check`,
  `cargo metadata --no-deps`, and
  `verify-module-build-worker-isolation.mjs`.
- The runtime portion of `verify-worker-runtime-policy.mjs` now passes: shared
  global admission, permit-free readiness, graceful shutdown, and
  cancellation-safe worker subprocesses are composed. Default gate mode remains
  red only because its CI workflow registration requires separate explicit
  authorization. Two focused `rustok-worker-transport` admission tests pass.
  The targeted `rustok-verification-transport` crate test exceeded its bounded
  60-second dependency-compilation window and was terminated without a result;
  no full compile or test suite was run.

### 2026-08-02 production publication-evidence slice

- Passed touched-file `rustfmt --edition 2024`, `git diff --check`,
  `cargo metadata --no-deps`, and the module control-plane, authoring, and source
  archive Node guardrails.
- Three focused Rust test invocations exceeded their bounded 60-second
  dependency-compilation windows and were terminated without a result. They do
  not establish compile or runtime test evidence. No full compile or test suite
  was run.

### 2026-08-02 source-manifest and authoring slice

- Passed touched-file `rustfmt --edition 2024` and `git diff --check`.
- Passed `cargo metadata --no-deps` and generated CLI registry freshness.
- Passed the module build-worker isolation, SDK WIT, canonical template, and
  module authoring CLI Node guardrails.
- Narrow Rust tests for source-manifest finalization and CLI provider/dry-run
  behavior each exceeded the 60-second dependency-compilation limit and were
  terminated without a result. They do not establish compile or runtime test
  evidence. No full compile or test suite was run.
- The owner-backed publish extension passed four focused authoring tests and
  five focused current-bundle validation tests. The CLI provider test again
  exceeded its bounded 60-second dependency-compilation window and was
  terminated without a result. `rustfmt --edition 2024`,
  `cargo metadata --no-deps`, and the authoring boundary guard passed; no full
  compile or test suite was run.

### 2026-07-26 artifact-data quota closure

The owner data broker now applies a host-selected `ArtifactDataQuotaPolicy`
after exact installation/capability resolution. Structured and object writes
enforce projected namespace-wide count/byte limits under the lifecycle lock;
batch rollback, replacement accounting, logical-delete capacity release,
active upload-session/staging aggregation, and guarded restore limits have
focused regression coverage. The targeted `data::tests` set passes 20 tests,
and the quota filter passes four tests including the hard-ceiling policy and restore-manifest unit
case. Touched Rust files pass edition-2024 `rustfmt --check`; no workspace-wide
compile or test claim is made.
The final Phase 3 composition slice registers the production logical-secret
route and adds a durable-state policy test covering successful authorization,
stale policy scope, foreign installation identity, inactive lifecycle, and
grant removal. Phase 3 is complete; MCP server-alias invocation remains a
separate owner-integration item.

### 2026-07-22 quality and isolation audit

The focused owner profile was rechecked after the marketplace and lifecycle
cutovers. `rustok-api/runtime`, `rustok-runtime`, and both standalone/default
`rustok-modules` library profiles compile; the API `server` feature also
compiles. Unit evidence is 25 `rustok-api` tests, 3 `rustok-runtime` tests, and
152 `rustok-modules` tests, all passing with incremental compilation disabled.
The owner dependency tree contains no AI, product, commerce, MCP, Alloy,
Leptos, Axum, or Async-GraphQL packages. The repository guard additionally
checks owner imports and concrete admin transport SQL/filesystem/hash/build
planning bypasses. Direct admin build reads and rollback are now routed through
the host-composed `rustok_build::SharedBuildControl`,
with the server retaining event-aware rollback composition. That port now
returns typed framework-neutral build/release snapshots from `rustok-api`;
`rustok-build` alone maps persistence models, while GraphQL and native admin
consume the same facts. Remaining Phase 7 work covers canonical transport
errors, parity fixtures, and the other resolver families.

### 2026-07-22 cross-boundary error audit

The edition-2024 workspace was rechecked after the owner-neutral product and
inventory projections, the SHA-256 helper migration required by `sha2` 0.11,
and the admin governance lifecycle mapper changes. Targeted `cargo check`
passed for `rustok-forum`, `rustok-pricing`, `rustok-commerce`,
`rustok-groups`, `rustok-server --no-default-features`, and
`rustok-admin --no-default-features --features ssr` (warnings only). The three
focused domain tests in `rustok-groups`, including the SHA-256 encoding
regression test, passed individually. The module
control-plane write-path guard, `git diff --check`, and `cargo metadata
--no-deps` also passed. No workspace-wide compile or test claim is made.

- Restore a crate-wide `cargo fmt -p rustok-modules -- --check` baseline; the
  current formatter reports pre-existing drift across owner source and migration
  files, so cycle fixes format only their touched Rust files until that mechanical
  cleanup is isolated.
- Artifact descriptor, executor selection, lineage, and immutable-release tests.
- OCI identity, media type, digest, signature, SBOM, and provenance tests.
- Tenant RLS, lifecycle, Core/Optional, dependency, revision, idempotency,
  recovery, and rollback tests.
- Composition CAS plus build enqueue atomicity tests.
- Governance state-machine/property tests.
- Add forward database constraints for registry translation ownership and
  default-locale integrity during the foundation migration wave; the owner
  publication boundary already fails closed on invalid or missing default rows.
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

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `completed`
- Last verified at (UTC): `2026-07-20`
- Scope inspected: `rustok-modules` Core ownership, manifests, control-plane contracts, migrations, RBAC, cache/index/event/outbox boundaries, and server composition consumers.
- Findings: `P0=0, P1=2, P2=5, P3=4` (all P1/P2 product defects found in this visit were fixed; formatter and test-fixture debt was recorded or repaired)
- Fixed in this pass: `added PostgreSQL RLS and transaction-local tenant scope for binding idempotency; rejected list continuations outside admitted data/object prefixes; made failed build results reject successful artifact identities; enforced canonical/default registry translations at final publication; made publication prerequisite ordering consistent; routed the Alloy governance handle through ModuleControlPlane; restored marketplace registry rows; repaired stale guards and deterministic fixtures`
- Remaining risks or blockers: `no open P0/P1; PostgreSQL execution of the forward RLS migration and registry translation FK/default-locale constraints remain closing/foundation migration evidence; crate-wide rustfmt still reports pre-existing drift in untouched source`
- Evidence: `target/debug/xtask.exe validate-manifest; target/debug/xtask.exe module test modules; rustok-modules test binary (121 passed); node scripts/verify/verify-runtime-context-invariants.mjs; node scripts/verify/verify-module-control-plane-write-path.mjs; node scripts/verify/verify-oci-registry-transport-policy.mjs; node scripts/verify/verify-module-build-worker-isolation.mjs; scripts/verify/verify-architecture.ps1`
- Next action: `resume the master queue at core/auth; revisit PostgreSQL migration execution and registry translation constraints in the foundation/closing waves`
- Resume command: `target\debug\xtask.exe module test auth`
