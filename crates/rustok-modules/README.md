# rustok-modules

## Purpose

`rustok-modules` is the mandatory Core owner of module artifact, marketplace,
installation and tenant-policy contracts. Its persistent control-plane adapters
are being moved here from the server incrementally.

The owner is independently usable without AI, product, commerce, MCP, Alloy,
Leptos, Axum, or Async-GraphQL dependencies. Its standalone Cargo profile uses
the neutral `rustok-api/runtime` feature only; host transports provide adapters
around the owner contracts.

## Responsibilities

- Define immutable module artifact identity, payload kind and source lineage.
- Verify digest-pinned OCI packages before installation and admit only isolated
  payload kinds to the sandbox.
- Retain and collect admitted CAS bytes through explicit owner retention
  snapshots; execution reads verified CAS only and never retries an OCI
  registry at runtime.
- Enforce descriptor-only brokered persistence; marketplace artifacts cannot
  declare SQL, native migrations, storage paths, or host handles.
- Restrict marketplace UI contributions to host-rendered declarative metadata;
  native UI packages remain static-promotion-only.
- Define the owner ports for marketplace publication, installation, activation,
  rollback and policy.
- Expose a transport-neutral marketplace catalog port for host-composed local
  and remote providers, plus an owner lifecycle snapshot for registry owners,
  requests, releases, validation gates, events, moderation policy, and
  available governance actions.
- Record reviewed static-promotion requests and approvals only for exact
  platform-built release, CAS source, dependency, Cargo package, and native
  entry-type evidence; runtime code cannot compile or activate native artifacts
  through this service.
- Queue immutable predecessor-linked static distribution build intents only
  from complete approved-promotion selections with pinned platform source,
  toolchain and target identities; every item explicitly persists
  `static_native` executor mode and selection never mutates active runtime
  state.
- Lease distribution build intents to separately authorized workers with
  immutable attempts, heartbeat ownership, expired-lease reclaim, and
  digest-pinned terminal artifact/SBOM/provenance/signature/test evidence.
- Activate only the current successfully completed static distribution build
  through a separately authorized, externally verified, predecessor-linked
  release ledger; release activation does not deploy native code.
- Bind every compiled promoted module definition to its exact promotion,
  registry release, distribution release, and native artifact identity before
  lifecycle dispatch; platform-native definitions remain distinct.
- Own topology-bound native rollouts through durable desired/observed state:
  per-node observation revisions, prepare/health/activate transitions,
  convergence, degradation recovery, exact replay and transactional outbox
  events. Deployment agents report evidence; this crate never starts a native
  process or mutates the running server.
- Revoke static releases through release-head CAS and queue direct-predecessor
  rollback only as a new fully verified distribution build; old native bytes
  are never reactivated.
- Own artifact-release quarantine and terminal emergency revocation separately
  from registry yanking; security state blocks new execution while preserving
  tenant enablement intent and records exact revisioned outbox evidence.
- Supply owner clock and identity ports through one
  `ControlPlaneInfrastructure` context rather than process-global calls.
- Map installed artifacts to neutral sandbox requests and capability grants.
- Validate artifact settings, structured data, and every runtime binding payload
  against exact descriptor-bundled schemas through one bounded validator
  implementation.
- Resolve secret values only inside a host-composed exact-revision consumer;
  sandbox capabilities receive logical handles and redacted receipts only.
- Create bounded durable artifact-data snapshots with private verified object
  copies, and restore them only into an empty active namespace through owner
  authorization, revision CAS, durable idempotency, and outbox evidence.
- Keep marketplace release, platform installation and tenant enablement separate.
- Resolve effective availability as one serializable owner decision set with a
  deterministic policy revision, typed contributing facts, and stable denial
  reasons. Artifact decisions require an exact active installation, matching
  durable capability-policy revision, an available isolated executor, and an
  available dependency closure. A channel owner may supply a tenant-safe
  channel snapshot; channel bindings and the channel revision are evaluated in
  the same decision. Operational maintenance can likewise block globally or
  by module through a revisioned snapshot without rewriting tenant intent;
  node-owned readiness must observe the base policy revision before serving and
  is included in the final revision; enabled-module lists remain projections
  of the resulting decision. Outbox consumers use the owner-provided
  `ModulePolicyRevisionGate` to apply predecessor-bound transitions, replay
  duplicates, and reject stale delivery. `SeaOrmModulePolicyRevisionConsumer`
  persists that cursor per tenant and consumer key inside a row-locked
  transaction.

## Entry points

- `ModulesModule`
- `ModuleArtifactDescriptor`
- `ArtifactReleaseDraft`
- `ArtifactRelease`
- `ModuleInstaller`
- `ControlPlaneInfrastructure`
- `ArtifactRuntime`
- `ModuleLifecycleDbWriter`
- `ModuleEffectivePolicy`
- `ModuleMarketplaceCatalog`
- `SharedModuleMarketplaceCatalog`
- `ModuleGovernanceLifecycleSnapshot`
- `ModuleDefinitionCatalog::from_static_distribution`
- `SeaOrmModuleStaticDistributionRolloutService`
- `SeaOrmModuleArtifactSecurityService`
- `SeaOrmArtifactSecretUseService`
- `SeaOrmArtifactDataSnapshotService`
- `SeaOrmArtifactDataSnapshotRetentionService`
- `SeaOrmArtifactDataSnapshotCollectionService`
- `SeaOrmModulePromotionService`
- `SeaOrmModuleStaticDistributionReleaseService`
- `OciArtifactReference`
- `OciRegistryTransportPolicy`
- `OciDistributionArtifactRegistry`
- `SeaOrmArtifactInstallationStore`

## Interactions

- Resolves and verifies installed artifacts before executing them through
  `rustok-sandbox` for Rhai, WebAssembly and future sidecar execution.
- Alloy creates and evolves source-backed release drafts through these contracts.
- `apps/server` bootstraps this Core module without owning marketplace policy.

See the [local documentation](./docs/README.md).
