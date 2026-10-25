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
- `executor_readiness`, `external_prebuilt_ingress`, `gc`, `oci_admission`,
  `operations_tool`, `operator`, `queue_drain`, `source_object`,
  `static_settings_translation_read`, and `wave_rollout`.

`ModulesModule` is also re-exported at the crate root. Only explicitly listed
modules and root re-exports form the supported public surface.

## Primary Public Types and Operations

- `ModulesModule` implements the platform module and migration-source contracts
  and registers the artifact-event and schedule-delivery workers.
- `ModuleArtifactDescriptor`, `ArtifactReleaseDraft`, `ArtifactRelease`, and
  `OciArtifactReference` carry immutable artifact identity and release lineage.
- `ModuleInstaller`, `ArtifactRuntime`, `ArtifactCommandBindingRequest`, and
  `ArtifactHttpBindingRequest` admit and dispatch exact installed artifacts.
- `ModuleEffectivePolicy` and `SharedModuleEffectivePolicyReader` provide the
  owner-issued, redacted availability decision.
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

## Invariants

- Artifact `(slug, version)` identity is immutable; content and release evidence
  are digest-pinned and verified before admission or execution.
- Registry publication, platform installation, and tenant enablement are
  separate owner decisions.
- Lifecycle, policy, security, promotion, and retention mutations use typed
  authorization, revision/CAS, and idempotency contracts where required; durable
  state and outbox evidence are committed atomically.
- Runtime execution uses admitted, verified content and an isolated sandbox; it
  does not fetch mutable registry references at execution time.
- Secret values and private authorization material do not cross artifact
  capability or browser-facing contracts.

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
`ModuleBuildProtocolError`, `OciReleaseAdmissionError`, `ArtifactDataError`, and
`ModuleStaticDistributionError`. Adapters preserve source errors rather than
collapsing operational, persisted-evidence, policy, or validation failures into
one generic result.
