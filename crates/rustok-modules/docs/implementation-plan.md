# Implementation Plan for `rustok-modules`

## Scope

Own the mandatory Core module control plane: identity, releases, marketplace,
installation, composition, lifecycle, effective policy, build/publication
orchestration, rollback, and static promotion. Optional module implementations
must not become server Cargo dependencies through this crate.

The cross-component sequence and completion rules are defined by the
[canonical module-platform plan](../../../docs/modules/module-control-plane-consolidation-plan.md).

## Current State

## FFA/FBA status

- FFA status: `not_started`
- FBA status: `in_progress`
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
- rejection of static promotion as a runtime installation path.

Still outside the owner boundary:

- platform composition/CAS and build enqueue in the server;
- registry governance, publication, release approval/yanking, and related
  persistence in the server;
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
  adapter;
- artifact lifecycle dispatch requires a configured
  `ArtifactLifecycleExecutor`; production host wiring for that executor remains
  to be supplied;
- admission now stages, verifies, and publishes payload bytes into CAS before
  the database admission commit; production durable CAS, outbox, and reconciler
  adapters remain to be supplied by host infrastructure;
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
these contracts as their write paths are moved; no server or admin compatibility
facade was added.

M2 has started with a transport-neutral definition catalog. It derives static
definitions from the compile-time registry while keeping registry handles
limited to static runtime concerns, and rejects ambiguous active definitions.
Effective-policy resolution and toggle validation now consume the catalog.

The lifecycle entrypoints now use `ModuleExecutionDispatcher`, which resolves
the active definition before invoking a static implementation. Artifact
lifecycle bindings are explicitly denied until their admitted sandbox adapter
is wired; no artifact path falls back to a compiled callback.

Artifact descriptors now carry versioned declarative bindings with stable IDs,
schema digests, permission, idempotency, limit profile, and declared
capabilities. Admission rejects duplicate bindings, malformed schemas, and
binding capabilities absent from the descriptor.

`ArtifactRuntimeLifecycleExecutor` now provides the dispatcher-facing sandbox
adapter contract: installation resolution is tenant/scope-aware, effective
grants and limits come from a separate policy resolver, and only a binding
present in the immutable installed descriptor can replace the sandbox
entrypoint. The production RLS/CAS adapters remain the next persistence slice.

CAS admission is explicitly `stage -> durable CAS publish -> database
transaction plus outbox -> reconciler`. A publish preceding a failed database
commit is an orphan candidate, never a runtime installation; the reconciler
may remove it only after reference and retention-policy checks.

The database transaction uses the existing `TransactionalEventBus` and
`OutboxTransport`: admission metadata, the selected dependency graph,
installation/composition revision, and the outbox envelope are one commit. No
module-specific second event journal is allowed.

Dependency resolution now uses `pubgrub` behind the transport-neutral
`ModuleResolutionProvider`. The adapter first collects an immutable candidate
snapshot, filters it by trust, active/yanked/revoked status and runtime ABI,
then writes only the selected exact versions and payload/manifest digests into
the lock graph. Every `InstalledModuleArtifact` now persists that graph with
its revision and digest in the same installation transaction, and runtime
execution rejects a missing or tampered declared dependency. Scope/module-kind
policy, persisted solver input snapshots, and stable derivation explanations
remain owner-service work.

The shared transactional outbox remains the required event boundary, but it is
not wired into platform admission yet: its envelope currently requires a tenant
UUID while platform commands explicitly allow no tenant. A dedicated routing
contract must decide how platform admission events are addressed before that
atomic metadata-plus-outbox adapter is added; no synthetic tenant is used.

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

- Move platform composition snapshot/CAS and atomic build request creation.
- Move registry governance, publication stages, releases, ownership, holds,
  approvals, rejection, yanking, and event taxonomy.
- Move remaining effective-policy composition.
- Migrate server callers, then delete replaced services and duplicate errors.
- Add a static guardrail preventing direct writes outside this crate.

### M4 - Complete Artifact Admission

- Extend descriptor compatibility, dependency, schema/migration, and UI surface
  references.
- Persist verification evidence, policy revision, capability grant revision,
  rollback pointers, status, and optimistic revision.
- Enforce signature, signer, SBOM, provenance, compatibility, dependency, and
  capability admission before activation.
- Resolve and persist exact dependency graphs with a maintained solver adapter.
- Copy admitted payloads into platform content-addressed storage and execute
  from CAS rather than the external registry.
- Add brokered tenant/module namespaced data and JSON-Schema validation;
  prohibit arbitrary untrusted SQL/native migrations.
- Implement upgrade, rollback, quarantine, revocation, and uninstall.

### M5 - Build and Publication Orchestration

- Define immutable build request/result contracts before adding another crate.
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

## Local Verification

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
