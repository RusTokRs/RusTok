# rustok-modules

See [CRATE_API.md](CRATE_API.md) for the formal public API surface, capability bindings, and minimum contract set.

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
- Restrict marketplace UI contributions to a bundled-schema-validated,
  host-rendered declarative contract with exact-locale plain-text catalogs;
  native UI packages remain static-promotion-only.
- Define the owner ports for marketplace publication, installation, activation,
  rollback and policy.
- Own durable transition checkpoints and rollout-window retention holds.
  Activation creates both atomically; direct-predecessor rollback updates the
  checkpoint and serving selection in one transaction; convergence requires a
  revision-guarded, idempotent owner command and emits transactional outbox
  evidence. Tenant lifecycle and settings guards read this state through the
  owner service and fail closed when it is unavailable.
- Expose a transport-neutral marketplace catalog port for host-composed local
  and remote providers, plus an owner lifecycle snapshot for registry owners,
  requests, releases, validation gates, events, moderation policy, and
  available governance actions.
- Record reviewed static-promotion requests and approvals only for exact
  platform-built release, CAS source, dependency, Cargo package, and native
  entry-type evidence; runtime code cannot compile or activate native artifacts
  through this service.
- Queue immutable build-lineage-linked static distribution build intents only
  from complete approved-promotion selections with pinned platform source,
  toolchain and target identities; every item explicitly persists
  `static_native` executor mode and selection never mutates active runtime
  state.
- Lease distribution build intents to separately authorized workers with
  immutable attempts, heartbeat ownership, expired-lease reclaim, and
  digest-pinned terminal publication evidence. The static-distribution worker
  alone publishes one complete OCI role bundle and canonical receipt.
- Admit verified role bundles without changing the serving head. The
  production rollout operation freezes the then-serving direct predecessor,
  advances desired state, and activates only after every exact `(node, role)`
  assignment converges healthy.
- Bind every compiled promoted module definition to its exact promotion,
  registry release, distribution release, and native artifact identity before
  lifecycle dispatch; platform-native definitions remain distinct.
- Own topology-bound native rollout state with exact role/failure-domain candidate and
  predecessor assignments, per-role observation revisions, owner-issued agent
  leases with same-agent exact replay and expiry-only reassignment, convergence,
  degradation/recovery, and transactional outbox facts.
  Deployment agents report evidence; this crate never starts a native process
  or mutates the running server.
- Retain, pre-stage, revalidate, and redeploy the exact
  then-serving direct-predecessor role bundle through the normal rollout;
  incident recovery never builds or republishes bytes. The obsolete
  rebuild-on-rollback command and duplicate `rustok-build` rollback authority
  are removed.
- Own artifact-release quarantine and terminal emergency revocation separately
  from registry yanking; security state blocks new execution while preserving
  tenant enablement intent and records exact revisioned outbox evidence. The
  owner invokes the host authorization port inside the transition transaction,
  before replay and the platform slug lock, so credential and RBAC fences stay
  held through commit.
- Supply owner clock and identity ports through one
  `ControlPlaneInfrastructure` context rather than process-global calls.
- Map installed artifacts to neutral sandbox requests and capability grants.
- Accept command and HTTP artifact dispatch through typed
  `ArtifactCommandBindingRequest` and `ArtifactHttpBindingRequest` envelopes so
  release, binding, installation, tenant, payload, and execution context cannot
  drift across host call sites.
- Validate artifact settings, structured data, and every runtime binding payload
  against exact descriptor-bundled schemas through one bounded validator
  implementation.
- Own dynamic artifact-settings recovery separately from structured-data
  deletion: a host-composed KMS cipher and policy port create an exact
  encrypted recovery point before an independently authorized purge writes its
  tombstone; recovery retention is revision-guarded and monotonic (expiry and
  holds may only increase), rewrap is
  KMS-owned, and a crash-resumable collection erases ciphertext but preserves
  terminal evidence. A direct restore pins the target's admission revision;
  restore otherwise creates a fresh non-serving settings instance;
  an intentionally unbound instance can later be one-time bound only to a
  continuity-authorized compatible inactive installation under the same data
  owner and exact registry/repository lineage.
- Resolve secret values only inside a host-composed exact-revision consumer;
  sandbox capabilities receive logical handles and redacted receipts only.
- Create bounded durable artifact-data snapshots with private verified object
  copies, and restore them only into an empty active namespace through owner
  authorization, revision CAS, durable idempotency, and outbox evidence.
- Keep marketplace release, platform installation and tenant enablement separate.
- Resolve effective availability as one serializable owner decision set with a
  deterministic policy revision, typed contributing facts, and stable denial
  reasons. Artifact decisions require an exact active installation, matching
  durable capability-policy revision, a supplied executor-availability input,
  and an available dependency closure. The current lifecycle adapter derives
  that input from executor kind support, not worker health. A channel owner may supply a tenant-safe
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

Artifact installation selection resolves tenant/platform precedence per slug
before comparing release identity and rejects an ambiguous winning scope. The
batched read does not provide a repeatable-read snapshot across chunks.
Artifact binding admission and runtime revalidation require an owner policy with
matching artifact release and installation facts. The current server resolver
constructs a static-only policy, so it fails closed for these bindings until
artifact-aware policy composition is wired. A same-slug static module is not
a substitute. This does not apply to the separate lifecycle-hook admission path.

## Entry points

- `ModulesModule`
- `ModuleArtifactDescriptor`
- `ArtifactReleaseDraft`
- `ArtifactRelease`
- `ModuleInstaller`
- `ControlPlaneInfrastructure`
- `ArtifactRuntime`
- `ArtifactCommandBindingRequest`
- `ArtifactHttpBindingRequest`
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
- For continued development, it materializes only an exact active published Rhai
  workspace from its owner projection and verified CAS bytes; it does not own
  Alloy drafts, catalog DTOs, or mutable OCI references.
- `apps/server` bootstraps this Core module without owning marketplace policy.

## Known Limitations / Pending Implementation

Migration preflight now shares `rustok-core::MigrationSafetyAssessment` with
schema tooling. Automatic admission checks that fact, settings protection and
owner denials through one receipt predicate; transition and data-upgrade
consumers cannot override it with a mode projection. The coordinator binds the
receipt to the exact operation/module. Native digest-bound phase authorization,
observed schema evidence and complete production migration/recovery wiring
remain pending; a supplied-operation summary is not that authority.

Owner-level and bounded SQLite/local-storage evidence does not establish
production readiness. The documented implementation and operational gaps include:

- Server SQL capability effects now recheck exact active installation and
  durable grants under the existing platform/tenant activation locks in the
  effect transaction. Exact release quarantine/revocation and security revision
  are also fenced, including implicit clear and quarantine/clear ABA.
  Independent monotonic registry release authority fences publication, yank,
  unyank and availability ABA for the root and entire serving artifact closure.
  Migration 000098 moves PostgreSQL registry fence admission to a BEFORE guard
  with the canonical advisory gate and nonblocking late acquisition; the
  AFTER trigger only advances the witness. Coordinated writer cutover and
  PostgreSQL contention verification remain required.
  Artifact scope selection generations fence lifecycle ABA on the exact
  serving closure. Migration 000095 requires coordinated writer/reader
  cutover. Mixed native/artifact serving closures also fence tenant lifecycle
  revision and optional override intent. The verified active composition
  revision/hash is captured with the serving policy and checked under the
  owner row lock in the SQL effect transaction. Serving closures bind and
  transactionally revalidate converged deployment ledger evidence. Full effective-policy,
  HTTP channel context is now evaluated and fenced by the existing Channel generation.
  Operational artifact serving maintenance now captures and fences the durable
  Modules-owned revision (migration 000097); operator transport/authorization
  composition and native serving are still open.
  Deferred/background channel context, continuous node liveness,
  Cross-owner ordering now aligns the inspected credential/composition/role and lifecycle cursor/slug paths; the full lock audit and external-effect fencing remain open; see the
  [SQL capability authority contract](./docs/README.md#sql-capability-effect-authority).
  Source regressions and compilation/concurrent PostgreSQL evidence remain unrun.

- Sealed artifact subscription intake is now source-wired through the existing durable queue
  with original-envelope dedupe and unchanged root representation. Two new definitions are
  uncompiled/unrun; actual backend exact/wildcard delivery, worker/restart and transport recovery
  remain open. See the [intake API](CRATE_API.md#sealed-artifact-subscription-intake-2026-11-01-source-only).

- The new delivery and settings claim fences require a coordinated stop of old
  workers and static lifecycle/settings writers before migrations `000092` and
  `000093`. Legacy active static claims must be reconciled first. Mixed-version
  operation and automatic downgrade are unsafe. Aggregate claim ownership is
  now explicitly separated between journal and settings; this change and its
  tests remain uncompiled and unverified. Lifecycle aggregate heartbeat and
  advance now lock before evaluating PostgreSQL lease expiry; real lock-wait
  verification is still required. Rust tests and PostgreSQL/RLS, crash and
  concurrent-reclaim verification are still required.
- Scoped OCI admission and executor-readiness identity changes require
  PostgreSQL migration, RLS, and concurrent-admission verification before
  production use.
- Static settings numeric bounds are represented by `f64`; comparisons against
  integer documents do not round the integer. Integer schemas reject bounds with
  absolute value at least 2^53 because those inputs may already have been rounded.
  This conservatively rejects even exactly representable large boundaries; an exact
  schema-boundary representation remains a contract change. Rust tests for the
  current comparison and schema guard need execution.
- Static settings writes conservatively reject detected unresolved transitions
  with a predecessor, but exact release schemas and an atomic fence covering
  static rollouts are missing. Installer seed enables fail closed because they
  cannot yet compose release-bound settings schemas, and recovery and retry
  paths need production evidence.
- Artifact-data object migration and the physical namespace cutover still need
  production fleet/job/write/recovery/retention fences, actual migration-object
  copy publication, PostgreSQL crash/race evidence, a separate full-manifest
  serving cutover, and safe orphan collection.
- Structured namespace purge requires a direct operator's transaction-bound
  credential, live RBAC/module policy and admitted grant ceiling, acquired before
  installation locks and receipt reads. Every commit checks the credential
  deadline, including exact replay; retention and operational drain remain open.
- Post-purge recovery and structured copy still need production policy and host
  transport composition, lifecycle outbox facts, whole-operation source
  retention, full-snapshot verification, and production hold/fleet fences.
- Static settings rows do not yet persist the exact schema digest and the
  `not_applicable`/`ready`/`migration_required` state.
- Production secret management/revocation and value-use authorization remain
  open; stateless MCP runtime verification is pending.
- Release-safety closure still needs the release-admission installer and update
  coordinator, admission/recovery reconcilers and executor prefetch/readiness,
  authenticated outside-candidate deployment/process supervision, singular
  static publisher/rollout identity, operations-tool fleet coordination, and
  complete retention collection.

The [owner documentation](./docs/README.md) and
[implementation plan](./docs/implementation-plan.md) track the current boundaries
and evidence. These items must not be presented as production-ready until their
owner contracts, runtime composition, and production evidence are complete.

See the [local documentation](./docs/README.md).

Deployment reconciliation and migration checkpoints now share the platform slug
ordering: reconciliation fences all target slugs before readiness state, uses
read-only identity locks compatible with PostgreSQL FK KEY SHARE, and rechecks
all target identities before convergence. This is source-only remediation;
registry platform trigger entry is fenced by migration 000098. Migration 000099
also rejects contended late release/request and publication/yank/unyank receipt
FK parent acquisition (PostgreSQL 55P03). Retry from a new transaction; broader
FK/receipt ordering and PostgreSQL runtime verification remain open.

Registry validation request-first locking and migration 000100 extend the
nonblocking FK admission boundary to review/validation/evidence receipts,
request staging, admissions, release artifacts and owner transfer receipts.
PostgreSQL contention and migration verification remain pending; the broader
Auth/RBAC and cross-owner FK order is not certified.

Security-state operations now acquire platform slug and (for quarantine/revoke)
the exclusive security epoch before reserving a new operation receipt. Exact
replay still requires current host authorization, but does not advance or
lock the epoch. The PostgreSQL lock-order regression is authored, unrun;
global deadlock freedom is not established.

Registry continuity witness migration 000101 protects permanent identities and
strict integer successors from direct SQL resets or deletion (including SQLite
replacement). SQLite also rejects release replacement by primary key,
slug/version or explicit rowid, and rejects rowid changes on releases and
witnesses, so implicit deletes cannot bypass the continuity trigger or destroy
receipts. PostgreSQL release triggers fail rather than wait for contended
activation/witness relation locks or an existing witness row. This is a DML
integrity fence, not protection against privileged DDL or database restoration.
Source regressions are unrun.

Security epoch migration 000102 protects the existing singleton from ordinary
SQL deletion, replacement, reset, skipped successors and metadata-only changes.
Exact no-op updates remain legal for SQLite writer admission. The baseline is
validated, not recreated; privileged DDL and restore are outside the fence.
SQLite and opt-in PostgreSQL regressions are authored, unrun.
