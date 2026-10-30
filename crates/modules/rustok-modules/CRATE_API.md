# `rustok-modules` crate API

`rustok-modules` is the platform-owned control plane for module artifact identity,
marketplace publication, admission, installation, lifecycle, static distribution,
and effective tenant policy. This file is the crate-level contract included in
its root Rust documentation. The implementation modules remain private unless
explicitly listed below or re-exported from `src/lib.rs`.

## Public Modules

The explicit public modules are:

- `module` and `migrations`;
- `data_backfill`, `data_copier`, `data_object_migration`,
  `data_post_purge_recovery`, `data_snapshot_intents`,
  `data_snapshot_readiness`, and `data_upgrade`;
- `executor_readiness`, `external_prebuilt_ingress`, `oci_admission`,
  `operations_tool`, `operator`, `queue_drain`, `source_object`, and
  `static_settings_translation_read`.

The `gc` and `wave_rollout` modules, their re-exports, and
`InMemoryArtifactBlobStore` are available only under the `test-support`
feature: they are test doubles/design models, not production services, and
must never be composed as durable owners.

`ModulesModule` is also re-exported at the crate root.

The crate root currently re-exports roughly one thousand names so that
in-workspace consumers (hosts, workers, sibling module crates) can name owner
types without deep module paths. That breadth is a workspace convenience, not
the supported contract. The supported composition surface is
`ModuleControlPlane`, the owner subservices derived from it, the public
modules listed above, and the primary types and operations below. Any other
root re-export is workspace-internal in practice and may be renamed, moved,
or withdrawn when the surface is curated down; consumers must not treat
unlisted re-exports as stable API.

Hosts compose one `ModuleControlPlane` per process/database (typically
behind an `Arc`) and derive owner subservices from it per operation; the
facade itself is a cheap handle over the shared connection pool and
`ControlPlaneInfrastructure`, so per-request construction is unnecessary
and makes injected clock/ID/event-writer infrastructure request-dependent.

## Primary Public Types and Operations

- `ModulesModule` implements the platform module and migration-source contracts
  and registers the artifact-event and schedule-delivery workers.
- `ModuleArtifactDescriptor`, `ArtifactReleaseDraft`, `ArtifactRelease`, and
  `OciArtifactReference` carry immutable artifact identity and release lineage.
- `ModuleInstaller`, `ArtifactRuntime`, `ArtifactCommandBindingRequest`, and
  `ArtifactHttpBindingRequest` admit and dispatch exact installed artifacts.
- `ModuleEffectivePolicy` and `SharedModuleEffectivePolicyReader` provide the
  owner-issued, redacted availability decision.
- `ModuleLifecycleDbWriter::with_static_settings_schema` supplies the active
  host-resolved schema for one static module. Enables with this schema validate
  retained settings under the lifecycle claim and persist the normalized value
  with the lifecycle state. Enable-direction static compensation requires it;
  all other static enable consumers must also compose it to obtain preflight.
- Post-hook recovery requires the original hook settings digest and rejects
  changed settings or historical operations without evidence. Recovery replay
  identity binds the exact source operation. Successful retry and original
  failure resolution are committed together.
- `ModuleMarketplaceCatalog` and `ModuleGovernanceLifecycleSnapshot` expose
  transport-neutral marketplace and governance reads.
- `ModuleBuildResultReader`, `ModuleAuthoringBuildCommand`, and
  `ModuleAuthoringPublishCommand` bind authoring and publication to immutable
  build evidence.
- `SeaOrmModuleStaticDistributionRolloutService`,
  `SeaOrmModuleArtifactSecurityService`, `SeaOrmArtifactSecretUseService`,
  `SeaOrmArtifactDataSnapshotService`, `SeaOrmModulePromotionService`, and
  `SeaOrmModuleStaticDistributionReleaseService` own their respective durable
  state transitions.
- `SeaOrmModuleTransitionService` (via `ModuleControlPlane::transitions()`)
  owns durable transition checkpoints and the governed artifact transition:
  `start_governed_automatic_artifact_transition` accepts a
  `GovernedArtifactTransitionCommand`, derives preflight from owner state,
  requires passing smoke readiness on every required executor pool, and
  activates only Automatic-mode candidates. Maintenance-classified
  data/settings changes fail closed until a dedicated guarded migration
  owner exists.
- `ExecutorReadinessService` (via `ModuleControlPlane::executor_readiness`)
  prefetches, rehashes, and smoke-verifies candidate and predecessor payloads
  per executor pool and generation. `SeaOrmExecutorPoolRegistry` (via
  `ModuleControlPlane::executor_pool_registry`) durably registers each
  host-composed pool identity (`ExecutorPoolIdentity`,
  `ExecutorPoolRegistration`); the monotonic pool generation advances
  whenever the runtime fingerprint, placement, or placement attestation
  changes, and smoke evidence never carries across generations.
- `ArtifactExecutionAdmission` is the opaque execution token minted only by
  `ModuleEffectivePolicy::admit_artifact_execution`. Binding dispatches carry
  it; executors must refuse a binding whose admission does not authorize the
  resolved installation, and non-lifecycle execution still re-checks fresh
  effective policy before running.
- `RetentionHoldStore` owns the retention holds that pin artifact payloads
  and OCI manifests against garbage collection across observation windows
  and predecessor-recovery periods.

## Invariants

- Artifact `(slug, version)` identity is immutable; content and release evidence
  are digest-pinned and verified before admission or execution.
- Registry publication, platform installation, and tenant enablement are
  separate owner decisions.
- Lifecycle, policy, security, promotion, and retention mutations use typed
  authorization, revision/CAS, and idempotency contracts where required; durable
  state and outbox evidence are committed atomically.
- `ModuleDefinitionCatalog` construction rejects locally invalid definitions,
  self-dependencies, and dependency cycles among known definitions; absent
  dependencies remain effective-policy denials rather than catalog errors.
- Dependency resolution is bounded and skips malformed provider-owned release
  candidates with stable diagnostics; malformed requests and provider failures
  remain typed errors.
- Runtime execution uses admitted, verified content and an isolated sandbox; it
  does not fetch mutable registry references at execution time.
- Secret values and private authorization material do not cross artifact
  capability or browser-facing contracts.
- Artifact HTTP binding paths are literal segments only in v1 (no path
  parameters such as `{id}`); parameterized routes require a descriptor
  schema revision, not a validation relaxation.
- Artifact schedules fire at most once per minute in v1: six-field crons
  must name one fixed second; sub-minute cadences are rejected at
  admission.

## Authorization Posture

Owner commands carry the host-authenticated actor identity in
`ModuleCommandContext`; the owner validates that context (an invalid context
is a typed `InvalidCommandContext` failure, never a generic store error) and
retains it in durable receipts and outbox evidence, but never sees transport
credentials. Two enforcement shapes exist:

- Commands that declare a capability fact are enforced in-service and fail
  closed when the fact is absent: the governed artifact transition
  (`GovernedArtifactTransitionCommand.actor_can_manage_modules`) and the
  registry governance ownership, evidence, and staging commands
  (`governance_actor_can_*`). Registry publication additionally enforces
  the validation-stage/approval-override gate inside the owner; host
  projections only mirror that decision.
- All other owner commands are host-authorized: the composing host must
  authenticate the caller and authorize the operation before invoking the
  owner. This currently includes the registry governance review actions
  (reject, request changes, hold, resume publish request): the owner
  validates command shape, actor context, revisions, and state transitions
  for them, but performs no in-service reviewer-capability check. The owner
  never derives authorization from artifact payloads, descriptors, or
  caller-supplied identity strings.

Computing capability facts from an authenticated principal is always the
host's responsibility; the owner only enforces the facts it is given.

## Platform Capability Bindings

This crate consumes the following platform-owned capabilities through their
contracts:

- `rustok-api` — host/runtime context, typed ports, tenant and actor context,
  and transport-neutral API errors;
- `rustok-core` — module lifecycle, migration-source, and core domain contracts;
- `rustok-events` — canonical event envelopes and event-contract validation;
- `rustok-outbox` — transactional event writing for owner commands;
- `rustok-runtime` — host runtime composition and worker registrations;
- `rustok-sandbox` — isolated artifact execution and capability-broker contracts;
- `rustok-secrets` — host-composed secret resolution and handle contracts;
- `rustok-storage` — owner-scoped durable object storage.

The deterministic source-archive implementation is supplied by
`rustok-build-source`; it is a supporting library rather than a second module
control plane.

## Domain Collaborations

- `alloy` uses the owner build-result and publication-evidence contracts to
  materialize and publish source-backed releases; it does not own registry
  identity, catalog projections, or mutable OCI references.
- `apps/server` is the composition host. It supplies persistence, storage,
  authorization, and transport adapters and mounts owner contracts; it does not
  own marketplace or tenant-policy decisions.
- Artifact execution delegates to `rustok-sandbox`; worker processes use the
  owner-issued build, verification, distribution, and delivery contracts rather
  than querying private module-control-plane tables.

## Minimum Contract Set

### Input DTOs/Commands

Public command and request families include `OciReleaseAdmissionCommand`,
`ModuleStaticPromotionRequestCommand`, `ModuleTransitionFinalizeCommand`,
`ArtifactCommandBindingRequest`, `ArtifactHttpBindingRequest`,
`ModuleAuthoringBuildCommand`, and `ModuleAuthoringPublishCommand`. Read surfaces
include `ModuleEffectivePolicyReader` and `ModuleMarketplaceCatalog`.

### Domain Invariants

Admission and execution are bound to exact artifact, release, installation,
tenant, and capability evidence. Tenant policy is an owner decision, not a
projection reconstructed from tenant rows. Retention holds, transition
checkpoints, and recovery receipts preserve exact identity and revision across
retries and recovery.

### Events / Outbox Side Effects

Commands that require durable event effects use the transactional event writer
provided through `ControlPlaneInfrastructure` in the same transaction as their
state and evidence. `ModulesModule` registers the canonical artifact-event and
schedule-delivery workers. There is no module-local event bus or synthetic
success event path.

### Errors / Failure Codes

Failures remain typed at their owning boundary. Public examples include
`ModuleControlPlaneError`, `ModuleArtifactError`, `ModuleInstallationError`,
`ModuleEffectivePolicyError`, `ModuleGovernanceError`,
`ModuleBuildProtocolError`, `OciReleaseAdmissionError`, `ArtifactDataError`,
`ModuleStaticDistributionError`, `ModuleTransitionServiceError`,
`GovernedArtifactTransitionError`, `ExecutorReadinessError`, and
`ExecutorPoolRegistryError`. Adapters preserve source errors rather than
collapsing operational, persisted-evidence, policy, or validation failures into
one generic result; in particular, an invalid `ModuleCommandContext` surfaces
as its own typed validation failure with the offending field named, never as
an idempotency or storage error.
