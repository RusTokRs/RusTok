---
id: doc://docs/modules/module-control-plane-consolidation-plan.md
kind: implementation_plan
language: markdown
last_verified_snapshot: snap_jsonl_00000040
source_language: markdown
status: verified
---
# Module Platform, Marketplace, and Sandbox Implementation Plan

## Document Authority

This is the canonical cross-component implementation plan for the RusToK module
platform. It coordinates work owned locally by:

- `crates/rustok-modules`;
- `crates/rustok-sandbox`;
- `crates/alloy`;
- `apps/server`;
- module management transports and admin hosts;
- the future isolated module build worker.

Local component plans describe their own implementation details. If a local
plan conflicts with this document, update both documents in the same change and
resolve the conflict in favor of the accepted architecture decisions.

The ownership decision is fixed by
[`DECISIONS/2026-07-11-neutral-sandbox-foundation.md`](../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md).

## Execution Checkpoint

- Current phase: `runtime_foundation_and_control_plane_extraction`.
- Last updated: 2026-07-13.
- Completed foundation:
  - neutral sandbox request, policy, broker, executor, outcome, error, and audit
    contracts;
  - generic Rhai kernel and broker-backed Alloy HTTP adapter;
  - Wasmtime Component Model executor with fuel, epoch deadline, memory limits,
    default-deny imports, and the typed `rustok:module/host.invoke` WIT import;
  - immutable Rhai release lineage and canonical artifact descriptors;
  - digest-pinned OCI resolution and payload verification;
  - durable scoped artifact installation records with PostgreSQL RLS;
  - installed artifact execution through the shared sandbox using admitted,
    digest-pinned CAS bytes; OCI remains the distribution source;
  - module-owned tenant toggle, journal, settings persistence, recovery plan,
    and post-hook retry operations.
- Current critical path:
  1. freeze shared error, revision, and facade contracts;
  2. replace compile-time identity/lifecycle with the artifact-aware definition
     catalog and dispatcher;
  3. add admitted artifact CAS and exact dependency lock graphs;
  4. route Alloy draft execution through `SandboxRuntime`;
  5. move platform composition and registry governance behind the
     `rustok-modules` facade;
  6. replace server and admin bypass paths;
  7. add isolated build, signing, SBOM, publication, and admission stages.
- Known verification blocker outside this plan: a current `rustok-build`
  dependency error prevents a clean workspace-wide server check. It must not be
  confused with evidence that the module-platform slices are complete.
- Open architecture blockers: none.

## Problem Statement

The original module system is manifest-driven static composition of Rust crates
known to the server at compile time. That model is still useful for trusted
distribution builds, but it cannot be the normal marketplace installation
model because:

- the server Cargo graph knows every optional native module;
- installing a module can require source or manifest changes and recompilation;
- package identity is mixed with workspace source layout;
- admin and server code both perform control-plane work;
- sandbox semantics can differ depending on whether code originated in Alloy,
  a marketplace package, or a native crate;
- governance, installation, activation, tenant enablement, and static build
  composition are not consistently separated.

The target platform is artifact-based:

- the server knows stable contracts but not optional module implementations;
- modules are immutable, versioned, digest-pinned artifacts;
- runtime installation never edits server source or `Cargo.toml`;
- untrusted Rhai, WebAssembly, and sidecars enter through one sandbox contract;
- trusted native compilation is an explicit static-promotion distribution mode;
- Alloy-authored releases can be published, installed, forked, and evolved
  without changing the identity of an existing release.

## Explicit Scope Boundaries

This plan delivers the technical marketplace and module runtime. The following
product concerns are separate follow-up tracks and must not be mixed into the
runtime critical path:

- commercial billing, revenue sharing, subscriptions, tax, and payouts;
- public ratings, recommendation ranking, and publisher reputation UX;
- general-purpose source hosting or Git collaboration;
- a transparent general-purpose Rhai-to-Rust compiler;
- arbitrary native dynamic-library loading;
- arbitrary module-provided SQL/SeaORM migrations for untrusted artifacts;
- unrestricted third-party UI code inside host application processes.

Basic catalog metadata, publisher identity, governance, licensing evidence, and
trust decisions remain in scope because admission depends on them. A rating or
publisher reputation signal may inform review priority but can never replace
artifact verification or platform policy.

## Target Ownership

| Owner | Owns | Must not own |
|---|---|---|
| `rustok-sandbox` | Execution envelope, executor registry, capability broker, limits, cancellation/admission, audit records, Rhai/WASM/sidecar executor contracts | Module identity, marketplace state, installation, Alloy drafts, server transports |
| `rustok-modules` | Module identity, artifact descriptors, release lineage, marketplace governance, installation, activation, lifecycle, effective policy, rollback, build/publication orchestration | Language runtime internals, Alloy revision workspaces, host HTTP/GraphQL concerns |
| Alloy | Source workspaces, drafts, revisions, tests, repair, AI-assisted evolution, release staging and forking | Generic sandbox policy, marketplace installation, OCI trust policy, parallel production executor |
| `apps/server` | Host composition, infrastructure adapters, authentication/tenant context, GraphQL/REST/native mounting, process lifecycle | Module business rules, artifact verification algorithms, direct control-plane writes |
| Admin hosts | Transport calls, view models, route state, UI effects | SQL, manifest parsing, hashing, dependency solving, build planning, lifecycle taxonomy |
| Build worker | Hermetic source validation, dependency inspection, test/build commands, SBOM/provenance production | Marketplace decisions, tenant state, runtime capability access, registry credentials beyond scoped publication |

`rustok-modules` is a mandatory Core module. Its dependency on platform host
infrastructure is supplied through explicit adapters; it cannot be disabled.

## Runtime and Trust Model

Artifact origin is lineage metadata, not a security boundary.

| Payload | Normal execution | Distribution | Trust rule |
|---|---|---|---|
| Alloy Rhai draft | `rustok-sandbox` Rhai executor | Not installed | Draft subject, authoring limits, explicit grants |
| Published Rhai | `rustok-sandbox` Rhai executor | OCI artifact | Same broker and limits as any installed artifact |
| Rust compiled to WASM Component | `rustok-sandbox` Wasmtime executor | OCI artifact | Default-deny imports and versioned WIT ABI |
| Native/container service | Future sidecar executor | OCI image/artifact | Hardened process/container boundary and brokered capabilities |
| Reviewed native Rust | In-process static composition | Explicit distribution build | No sandbox claim; review and CI promotion required |

There is one execution sandbox contract. The isolated Rust build worker is a
supply-chain service, not a second runtime sandbox.

## Non-Negotiable Invariants

1. Runtime installation never modifies the server source tree, workspace
   manifest, `modules.toml`, or Cargo dependency graph.
2. An installed runtime artifact is addressed by immutable manifest digest and
   verified payload digest. Tags are discovery aliases only.
3. A marketplace release is immutable. Any edit creates a new version, digest,
   and lineage edge.
4. Core modules cannot be disabled. Optional dependencies and dependents are
   validated before state mutation.
5. Platform installation, tenant enablement, and channel binding are separate
   states and must remain separately visible in API and UI.
6. Capability access is default-deny. Rhai functions, WIT imports, sidecar RPC,
   UI, and transports cannot expand grants.
7. Every control-plane operation has one owner-owned production write path.
8. GraphQL and native server-function transports expose the same canonical
   result, error, revision, and recovery facts.
9. Server and admin code may adapt owner contracts but may not reproduce their
   validation, hashing, lifecycle, or persistence rules.
10. Native dynamic libraries are not a marketplace runtime. `libloading`, `.so`,
    `.dll`, and `.dylib` installation paths are prohibited.
11. `catch_unwind` and async timeouts are reliability tools, not native-code
    isolation.
12. Static promotion is never an automatic fallback for a failed sandboxed
    install.
13. Runtime identity and dependency decisions use a durable artifact-aware
    definition catalog. The compile-time `rustok_core::ModuleRegistry` is only a
    registry of Core and explicitly static-promoted implementations.
14. Admission copies verified executable bytes into a platform-controlled
    content-addressed store. Normal execution does not download payload bytes
    from an external registry on every invocation.
15. Untrusted artifact lifecycle hooks, events, schedules, commands, and HTTP
    bindings dispatch through the sandbox; they never require a
    `RusToKModule` implementation in the server process.
16. Untrusted artifacts cannot register arbitrary native GraphQL fields,
    Axum routers, database migrations, permissions, or UI code. They contribute
    only versioned declarative bindings admitted by the platform.
17. Persistent module data, settings, secrets, and files are tenant- and
    module-namespaced capabilities. An artifact never receives a raw database,
    filesystem, object-store, or secret-store client.
18. Control-plane state changes and their domain events use a transactional
    outbox or equivalent atomic event boundary.

## Approved Implementation Building Blocks

The platform should reuse maintained tooling for infrastructure primitives and
keep custom code limited to RusToK domain contracts.

| Concern | Approved implementation |
|---|---|
| Rhai language runtime | `rhai` through `rustok-sandbox` |
| WebAssembly runtime | `wasmtime` Component Model, fuel, epochs, store limits |
| Rust component build | `cargo-component`, `wit-bindgen`, `wit-component`, `wasm-tools` |
| Cargo graph inspection | `cargo metadata` / `cargo_metadata` |
| OCI transport | Existing `oci-distribution` adapter |
| Artifact bytes | OCI digest semantics plus an `ArtifactBlobStore` port backed by platform-controlled content-addressed object storage; reuse `rustok-storage` adapters where they satisfy CAS requirements |
| Module dependency solving | `pubgrub` behind a RusToK provider adapter; replacement requires a documented incompatibility/ADR, never a naive recursive resolver |
| Settings/action schemas | JSON Schema Draft 2020-12 validated with the maintained `jsonschema` crate; generate host-owned schemas with `schemars` where useful |
| SBOM | `cargo-cyclonedx`, CycloneDX artifact/attestation |
| Dependency policy | `cargo-deny`, `cargo-vet`, RustSec-compatible advisory gate |
| Signing and verification | `cosign`/Sigstore workflow; avoid custom cryptography |
| Build/sidecar isolation | OCI job with hardened runtime such as gVisor or Kata where deployed |
| AI providers and tool calling | Existing `rig-core` integration |
| MCP | Existing `rmcp` integration |
| Local worker/sidecar RPC | Existing `tonic`/`prost` generated contracts over an approved local transport; do not invent ad-hoc JSON/stdin framing |
| Durable events | Existing `rustok-outbox` contracts/adapters rather than a module-specific event relay |
| Advanced authorization policy | Keep typed grants/constraints while sufficient; if Phase 0 proves a real ABAC policy-language need, adopt `cedar-policy` behind the policy port rather than building a custom DSL |
| Async orchestration | `tokio`, `async-trait`, typed ports |
| Serialization and telemetry | `serde`, `serde_json`, `tracing` |
| Generative testing | Existing `proptest`; add fuzz targets for untrusted parsers |

Do not embed an unstable library merely to avoid a small adapter. For example,
until the Rust Sigstore API is stable for the required verification policy, a
version-pinned `cosign` worker command is preferable to custom cryptography or a
large unstable in-process dependency.

The dependency-solver and JSON-Schema choices must be wrapped by narrow owner
ports and locked with compatibility fixtures. Library output is not itself the
RusToK domain contract: selected versions, conflict explanations, schema draft,
remote-reference policy, and stable errors remain owner-defined.

## Canonical State Model

The control plane keeps these concepts distinct:

1. **Catalog entry**: discoverable module identity and human-facing metadata.
2. **Release**: immutable semantic version, descriptor, lineage, compatibility,
   publication state, and manifest digest.
3. **Artifact**: executable payload layer plus SBOM, provenance, test evidence,
   and signatures/attestations.
4. **Platform installation**: artifact admitted to a platform deployment.
5. **Tenant lifecycle state**: enabled/disabled/settings/recovery for an
   installed Optional module.
6. **Channel binding**: module availability for a channel or surface.
7. **Static promotion**: trusted release selected for a distribution build.
8. **Build operation**: immutable input snapshot, attempt, logs, outputs, and
   terminal result.
9. **Module definition**: artifact-aware identity, kind, dependencies,
   compatibility, permissions, settings schema, runtime bindings, UI
   contributions, and current active implementation mode.
10. **Resolved installation graph**: exact release/digest lock for every direct
    and transitive dependency in one installation scope.
11. **Runtime binding**: an admitted lifecycle, command, HTTP, event, schedule,
    or hook declaration mapped to a stable dispatch ID.
12. **Artifact blob**: verified executable bytes in the platform CAS, with
    reference count/retention, verification evidence, and last-known-good state.
13. **Module data namespace**: tenant/module-scoped data, file, secret-reference,
    and migration/schema revision owned through broker capabilities.

An effective availability query must return all contributing facts rather than
collapsing them into one boolean.

### Installation Scope and Version Precedence

- Core and static-promoted implementations are platform-scoped only.
- A platform installation admits a release into the platform library and CAS.
- A tenant installation, when policy permits it, references an already admitted
  platform artifact and may select a tenant-specific active release.
- At most one release is active for `(scope, module_slug)`.
- A permitted tenant selection is more specific than the platform default; it
  never changes another tenant's selection.
- Tenant enablement remains separate from release selection. Installing or
  selecting a release does not implicitly enable it.
- The resolved graph records exact versions and manifest/payload digests for all
  dependencies; upgrades create a new graph revision atomically.

## Canonical Error Families

Every owner operation returns a stable code plus structured details. Transport
layers map status/protocol representation without inventing new semantics.

| Family | Required examples |
|---|---|
| Identity | `UNKNOWN_MODULE`, `UNKNOWN_RELEASE`, `ARTIFACT_IDENTITY_MISMATCH` |
| Revision | `REVISION_CONFLICT`, `STALE_OPERATION`, `IMMUTABLE_RELEASE` |
| Compatibility | `ABI_INCOMPATIBLE`, `PLATFORM_VERSION_INCOMPATIBLE`, `DEPENDENCY_CONFLICT` |
| Trust | `SIGNATURE_REQUIRED`, `SIGNATURE_INVALID`, `SIGNER_NOT_ALLOWED`, `SBOM_REQUIRED`, `PROVENANCE_INVALID` |
| Policy | `CAPABILITY_DENIED`, `CORE_MODULE_IMMUTABLE`, `MODULE_NOT_INSTALLED`, `MODULE_NOT_ENABLED` |
| Sandbox | Existing stable sandbox compilation, trap, timeout, limit, cancellation, and host-capability codes |
| Lifecycle | `PRE_HOOK_FAILED`, `POST_HOOK_FAILED`, `STATE_MISMATCH`, `RECOVERY_NOT_ALLOWED` |
| Build | `BUILD_REJECTED`, `DEPENDENCY_POLICY_FAILED`, `TEST_FAILED`, `COMPILATION_FAILED`, `BUILD_TIMEOUT` |
| Persistence | `STORE_CONFLICT`, `STORE_UNAVAILABLE`, `TENANT_SCOPE_VIOLATION` |

Error messages may evolve. Error codes and structured fields require contract
tests and an explicit compatibility decision.

## Phase 0 - Baseline, Contracts, and Guardrails

### Objective

Freeze the vocabulary and public seams before moving the remaining write paths.

### Deliverables

- [x] Accept the neutral sandbox ADR and dependency direction.
- [x] Inventory server/admin lifecycle, composition, governance, manifest,
  build, and registry entrypoints.
- [x] Define serializable snapshots for catalog, release, artifact,
  installation, effective policy, composition, governance, lifecycle, recovery,
  and build operations.
- [x] Define one stable owner error envelope and codes from the families above.
- [ ] Define revision/CAS fields for every mutable aggregate:
  - platform composition revision;
  - publish-request revision;
  - installation revision;
  - tenant settings revision;
  - build attempt/revision.
- [ ] Define actor, tenant, trace, idempotency, and correlation context required
  by every command.
- [ ] Document GraphQL/native compatibility policy and versioning rules.
- [x] Freeze the split between the compile-time implementation registry and the
  durable artifact-aware module definition catalog. `ModuleRegistry` retains
  only static implementation handles; `ModuleDefinitionCatalog` resolves the
  durable static or admitted-artifact definition selected for a composition.
- [x] Freeze installation scope and the exact dependency-lock graph contract.
  `ModuleInstallationScope` is platform or tenant scoped, and every installed
  artifact carries a validated, digest-pinned `ModuleDependencyLockGraph`.
- [ ] Freeze the v1 runtime binding set and dispatch envelope for lifecycle,
  command, HTTP, event, schedule, and hook calls.
- [ ] Freeze v1 artifact persistence: brokered namespaced storage only;
  arbitrary artifact SQL/native migrations remain disabled pending a separate
  ADR and threat model.
- [ ] Freeze v1 dynamic UI delivery:
  - host-rendered declarative settings/actions are required;
  - untrusted custom web UI is isolated in a sandboxed iframe when introduced;
  - native Leptos/Next/Flutter packages require static promotion;
  - no marketplace artifact injects code into a host process.
- [ ] Freeze the admitted artifact CAS, retention, garbage collection, and
  external-registry outage behavior.
- [ ] Add static guardrails prohibiting new direct writes outside owner modules.

### Verification Gate

- Contract serialization fixtures round-trip.
- Unknown enum/code handling is explicit.
- CAS conflict tests prove that stale commands do not mutate state.
- A repository verifier identifies every current bypass and prevents new ones.
- A source-level proof shows that artifact modules can be known, enabled, and
  dispatched without appearing in `rustok_core::ModuleRegistry` or server
  Cargo dependencies.

## Phase 1 - Complete the Neutral Sandbox Runtime

### Objective

Make `rustok-sandbox` the sole production execution boundary for Alloy drafts
and installed artifacts.

### 1.1 Existing Foundation

- [x] Typed subject, context, payload, policy, limits, outcome, metrics, and
  error contracts.
- [x] Executor registry with duplicate/missing executor protection.
- [x] Default-deny capability broker and typed capability call.
- [x] Execution observer port and start/success/failure records.
- [x] Rhai executor with instruction, call-depth, time, data, and output limits.
- [x] Wasmtime Component executor with fuel, epoch deadline, store limits, and
  no ambient WASI imports.
- [x] Typed WIT host call routed through `SandboxHost`.

### 1.2 Remaining Runtime Work

- [x] Add an explicit Alloy draft request builder using
  `SandboxSubject::AlloyDraft` and a revision number.
- [x] Replace Alloy's direct production execution path atomically with
  `SandboxRuntime`; do not retain a fallback executor.
- [x] Preserve Alloy-specific entity, parameter, validation, and HTTP behavior
  as request-scoped extensions backed by the capability broker.
- [ ] Define a versioned Rhai input/output binding shared by draft and published
  Rhai artifacts.
- [x] Freeze the WIT v1 package, world, entrypoint, JSON/error encoding, and ABI
  compatibility rules.
- [x] Add request-scoped cancellation propagation through runtime, Rhai,
  Wasmtime, and brokered capability dispatch.
- [ ] Add deadline cancellation for every enabled executor.
- [x] Add runtime-scoped global, executor, tenant, and artifact concurrency
  admission with automatic permit release.
- [ ] Add durable execution audit persistence through an observer adapter.
- [x] Exclude inputs, outputs, headers, credentials, and untrusted error text
  from neutral audit records.
- [ ] Add compiled-component/cache policy keyed by engine version, target,
  runtime ABI, and artifact digest.
- [ ] Add deterministic metrics for fuel/instructions, memory, output size,
  capability calls, queue time, and execution time.
- [x] Replace unbounded thread-per-host-call bridging with a strictly bounded
  one-thread-per-execution bridge. A synchronous guest ABI cannot permit thread
  exhaustion.
- [ ] Validate input and output against admitted JSON schemas with network/file
  schema retrieval disabled; all referenced schemas are bundled by digest.

### 1.3 Capability Broker Requirements

- [x] Move capability policy evaluation before all host adapter invocation.
- [x] Enforce tenant/actor/subject consistency on every capability call.
- [x] Define and enforce HTTP host/method/path constraints before broker
  invocation.
- [ ] Define constraints for storage namespace, event topics, secret references,
  and MCP server/tool names.
- [x] Add per-execution payload-size, call-count, and rolling rate limits before
  broker invocation.
- [x] Ensure denied calls emit redacted audit evidence without protected input.
- [x] Ensure host adapters receive scoped handles, never platform-global clients
  or raw credentials.

### 1.4 Execution Deployment Profiles

The crate is the contract owner; executor placement is a deployment decision.
It does not create a second sandbox API.

- [ ] Define `in_process` and `isolated_worker` executor adapters behind the same
  `SandboxExecutor`/runtime contract.
- [ ] Permit in-process Wasmtime where its threat model and resource controls are
  accepted.
- [ ] Run AI-generated or otherwise untrusted Rhai in an isolated sandbox worker
  in production so interpreter/runtime defects and hard memory/process limits do
  not affect the server process.
- [ ] Keep in-process Rhai only as an explicit local-development or reviewed
  profile; it is not a silent production fallback.
- [ ] Use a versioned framed RPC over a local channel; reject raw stdin/stdout
  ambiguity, oversized frames, unsolicited output, and protocol drift. Prefer
  the workspace `tonic`/`prost` generated contract over a custom codec.
- [ ] Route worker capability requests back through the same host broker without
  giving the worker network, database, filesystem, secret, or MCP clients.
- [ ] Apply process/container CPU, memory, process-count, file, disk, and time
  limits through the deployment runtime rather than hand-writing a platform OS
  sandbox in Rust.
- [ ] Supervise crash, cancellation, forced kill, restart/backoff, and complete
  cleanup with execution audit evidence.

### Verification Gate

- Identical Rhai source and input produce equivalent draft/artifact outcomes
  under the same policy.
- Default-deny tests cover Rhai helpers and WIT imports.
- Timeout, fuel, memory, output, cancellation, and concurrency tests exist for
  each enabled executor.
- Audit records cover success, denial, trap, timeout, cancellation, and host
  capability failure.
- Alloy has no parallel production sandbox or direct infrastructure bridge.
- Untrusted Rhai worker crash/OOM/hang tests cannot terminate or exhaust the
  server, and in-process fallback is disabled in the production profile.

## Phase 2 - Consolidate the `rustok-modules` Control Plane

### Objective

Make one module-owned facade the only production entrypoint for module control
plane reads and writes.

### 2.1 Existing Extraction

- [x] Mandatory `ModulesModule` Core entrypoint.
- [x] Core/Optional effective-policy resolution and toggle topology validation.
- [x] Module-owned tenant state, settings persistence, lifecycle hooks,
  operation journal, recovery plan, and post-hook retry.
- [x] Immutable artifact and release lineage contracts.
- [x] Scoped artifact installation persistence.
- [x] Artifact runtime execution through the shared sandbox.

### 2.2 Replace Compile-Time Identity with an Artifact-Aware Definition Catalog

The durable definition catalog now resolves static and admitted artifact
definitions. `rustok_core::ModuleRegistry` remains only the static implementation
adapter and must not be used as artifact identity or durable policy state.

- [x] Introduce a transport-neutral `ModuleDefinition` contract populated from:
  - Core/static-promoted implementations through a static adapter;
  - admitted artifact releases through durable catalog/install state.
- [x] Keep the existing `ModuleRegistry` only for in-process implementation
  handles, migrations, runtime extensions, and listeners of Core/static modules.
- [x] Move kind, dependency, compatibility, permission, settings, binding, UI,
  and capability metadata into the definition contract.
- [x] Change effective policy, dependency validation, lifecycle, settings, and
  recovery to depend on a definition-catalog snapshot, not a Rust trait object.
- [x] Generate a canonical static module definition from
  `RusToKModule`/`rustok-module.toml` so static and artifact definitions obey the
  same identity and dependency rules.
- [x] Add collision rules: a slug cannot ambiguously resolve to multiple active
  implementations; static promotion and artifact activation require explicit
  mode transition.

### 2.3 Runtime Binding Registry and Dispatcher

- [ ] Extend the versioned descriptor with declarative bindings:
  - lifecycle `pre_enable`, `post_enable`, `pre_disable`, `post_disable`;
  - health/readiness and activation smoke checks;
  - named commands/actions;
  - namespaced HTTP handlers;
  - event subscriptions;
  - schedules;
  - before/after/on-commit hooks where the host contract permits them.
- [x] Give every binding a stable ID, input/output schema digest, permission,
  idempotency mode, timeout/limit profile, and declared capabilities.
- [x] Introduce `ModuleExecutionDispatcher` (working name) that resolves the
  active definition and dispatches:
  - Core/static implementations through a typed static adapter;
  - Rhai/WASM/sidecar implementations through `SandboxRuntime`.
- [x] Replace `run_module_lifecycle_hook(ModuleRegistry, ...)` with the dispatcher
  so artifact modules can participate in lifecycle without a server crate.
- [ ] Dispatch events/schedules from durable binding metadata; do not register
  artifact Rust closures in `ModuleEventListenerRegistry`.
- [ ] Define event delivery as at-least-once with binding-scoped idempotency,
  retry/backoff, dead-letter evidence, payload schema/version, and bounded
  wildcard/topic subscriptions.
- [ ] Define schedule timezone, misfire, overlap/concurrency, deduplication,
  cancellation, and tenant enablement semantics.
- [ ] Define HTTP method/path namespace, auth/permission, request/response media
  type/schema, body/output limit, timeout, streaming policy, and idempotency;
  raw sockets and listener ports are forbidden.
- [ ] Namespace artifact HTTP routes under a platform-owned module route and
  reject route/method collisions. Artifacts cannot mount arbitrary Axum routers.
- [ ] Keep dynamic operations behind generic command/HTTP contracts; artifacts
  cannot inject arbitrary GraphQL schema fields at runtime.
- [ ] Never run untrusted code while holding the database transaction that
  commits lifecycle/control-plane state. Use precomputed intent, idempotency,
  outbox, and documented pre/post failure semantics.

### 2.4 Facade Shape

- [ ] Introduce a single facade with explicit subservices rather than one large
  implementation object:
  - `CatalogService`;
  - `ReleaseService`;
  - `PublicationService`;
  - `InstallationService`;
  - `LifecycleService`;
  - `CompositionService`;
  - `EffectivePolicyService`;
  - `BuildService`;
  - `PromotionService`.
- [ ] Define infrastructure ports for registry transport, artifact blob access,
  signature verification, SBOM/provenance verification, build scheduling,
  transactional storage, events, audit, clock, and ID generation.
- [ ] Keep transaction boundaries inside owner services while accepting a
  caller-provided database/transaction adapter where required.
- [ ] Add idempotency keys for install, publish, build, retry, rollback, and
  promotion commands.

### 2.5 Server Service Cutover

- [ ] Move platform composition snapshot/CAS logic from
  `PlatformCompositionService` into the module owner.
- [ ] Move build enqueue coordination into `BuildService`, preserving atomic
  composition CAS plus build-request creation.
- [ ] Move registry ownership, publish-request, release, validation-stage,
  yanking, and governance rules from `RegistryGovernanceService`.
- [ ] Move remaining manifest validation that is platform-domain policy into
  `rustok-modules`; keep only host boot/loading adapters in the server.
- [ ] Move effective availability composition behind one typed query.
- [ ] Replace server `build_registry()` usage in guards, lifecycle, event
  dispatch, runtime boot, installer, and APIs with the correct split between
  static implementation registry and durable definition/effective-policy
  services.
- [ ] Replace server error taxonomies with transport mappings of owner errors.
- [ ] Delete superseded server models/helpers after each atomic caller cutover.

### 2.6 Write-Path Guardrail

The static verifier must reject SQL/entity writes to these aggregates outside
the owner implementation and migrations:

- platform composition state;
- module operation journal;
- tenant module state/settings;
- artifact installations and grants;
- catalog/release/publish-request/governance tables;
- build requests tied to module composition;
- static promotion records.

### Verification Gate

- Exactly one production write entrypoint exists per operation.
- Tenant journal plus state and composition CAS plus build enqueue remain
  transactional.
- Core immutability and dependency topology are enforced on all entrypoints.
- Recovery and compensation use canonical owner state and codes.
- Server services contain adapters and transport mapping only.
- An artifact-only pilot can be discovered, installed, enabled, dispatched, and
  disabled while absent from the server Cargo graph and `ModuleRegistry`.

## Phase 3 - Canonical Artifact and Installation Model

### Objective

Complete durable identity, compatibility, installation, activation, and
rollback without relying on workspace source composition.

### 3.1 Artifact Descriptor

- [x] Slug, semantic version, payload kind, runtime ABI, payload digest,
  entrypoint, and declared capabilities.
- [x] Digest-pinned OCI manifest reference and verified payload media type.
- [ ] Add platform compatibility range and required feature/capability schema.
- [ ] Add dependency constraints by module slug and release range.
- [ ] Add module kind, namespaced permission definitions, settings schema,
  runtime bindings, localization catalog, data contract, and UI contribution
  metadata.
- [ ] Require bundled JSON Schema documents and forbid network/file `$ref`
  resolution during validation.
- [ ] Add persistence/schema contribution metadata without executing any data or
  migration operation at descriptor parse/admission time.
- [ ] Add UI metadata/artifact references without embedding executable UI logic
  in the server or host applications.
- [ ] Version the descriptor schema independently from module semantic version.
- [ ] Namespace artifact-defined permissions by module slug, reserve platform
  permission namespaces, and validate collisions before publication.
- [ ] Register admitted permissions through the RBAC owner service with
  localized labels/descriptions; installation never grants them to roles or
  actors automatically.
- [ ] Require every runtime/UI binding to name the exact permission it checks;
  capability grants authorize guest-to-host access and are not substitutes for
  actor RBAC.

### 3.2 Dependency Resolution and Lock Graph

- [x] Resolve semantic-version constraints with a maintained solver such as
  `pubgrub` behind a deterministic provider adapter; do not implement a naive
  recursive/backtracking resolver. The current adapter builds an immutable
  admitted-candidate snapshot before solving and writes selected versions and
  digests to the owner lock-graph contract.
- [ ] Include platform/runtime ABI, module kind, yanked/revoked status, scope,
  trust policy, and active-release constraints in the provider.
- [x] Persist the complete selected graph with exact semantic versions,
  manifest/payload digests, and a graph revision/hash.
- [ ] Produce stable human/machine conflict explanations from solver derivation
  evidence without exposing library-specific types as the public API.
- [ ] Resolve upgrades and rollbacks against a snapshot, then atomically switch
  the full graph revision; never partially upgrade dependencies.
- [ ] Detect cycles, self-dependencies, scope violations, and attempts to replace
  Core/static-only providers.

### 3.3 Platform Content-Addressed Artifact Store

The current `ArtifactRuntime` re-fetches the external OCI package for every
execution. Digest verification prevents identity substitution, but this path is
not the production target because it couples execution latency/availability to
the external registry.

- [ ] Introduce an `ArtifactBlobStore` port addressed only by verified digest.
- [ ] Use `stage -> durable CAS publish -> DB transaction + outbox ->
  reconciler` for admission. PostgreSQL does not claim atomicity with external
  object storage; reconciliation completes/fails interrupted admission and
  removes orphan blobs only after reference and retention-policy checks.
- [x] Commit admission metadata, dependency lock, installation/composition
  revision, and the existing transactional-outbox envelope in one database
  transaction; do not introduce a module-specific second event journal.
- [ ] During admission, stream the selected payload into a platform-controlled
  CAS, verify digest/size while streaming, then atomically publish the blob and
  installation record.
- [x] Execute from the admitted CAS blob; external OCI is a distribution source,
  not the per-request runtime store.
- [ ] Bound descriptor/config/layer size before allocation and support streaming
  reads rather than unbounded `Vec<u8>` downloads. The OCI adapter now rejects
  oversized config and declared layer sizes before `pull_blob`, then streams
  received bytes through temporary storage with size and digest checks;
  replacing the post-verification `Vec<u8>` boundary with a streaming sink
  remains required.
- [x] Store verification evidence and blob metadata separately from executable
  bytes; do not copy large payloads into PostgreSQL. The admission record now
  persists the signer, policy revisions, required-check outcomes, and redacted
  evidence references alongside the CAS identity.
- [ ] Define local/node caches keyed by digest with verified reads, atomic fill,
  corruption detection, and safe eviction.
- [ ] Define reference counting/retention for active, rollback, quarantined,
  audit-retained, and unreferenced blobs.
- [ ] Support execution during an external registry outage when the admitted blob
  is present; fail closed with an availability error when it is not.
- [ ] Re-verification after trust-policy/root changes updates admission state
  without changing the immutable blob.

### 3.4 Installation State

- [x] Platform and tenant scope contract with RLS-backed persistence.
- [x] Add explicit statuses: resolved, verifying, admitted, installed, active,
  failed, inactive, and rolled_back.
- [x] Store verification evidence references and policy decision revision.
- [x] Store a durable nullable previous-installation pointer for rollback. The
  admission transaction selects the latest same-scope installation for the
  module and writes the self-reference together with the new installation,
  admission row, and outbox event. A later rollback command advances it with
  its status transition.
- [ ] Store capability grant revision separately from artifact declaration.
- [ ] Store migration/application checkpoint and irreversible migration flags.
- [ ] Add optimistic revision and idempotency key.

### 3.5 Admission Sequence

#### Approved Trust-Admission Baseline

- Use Sigstore Cosign verification for OCI signatures. Marketplace artifacts
  require an allowed signer identity plus issuer/trust-root validation and a
  digest-bound transparency bundle; first-party private publication may use an
  explicitly configured KMS/key trust root instead.
- Require an in-toto SLSA provenance attestation for compiled WASM, sidecar,
  and reviewed build outputs. Its subject digest, builder identity, source
  repository/ref, and build type must match the owner policy.
- Require a CycloneDX JSON SBOM attestation for compiled artifacts. Validate
  the attestation subject, schema/media type, and module license/vulnerability
  policy before admission.
- Apply policy as typed owner code: every required check passes (`AND`); a set
  of approved authorities for one check is alternative (`OR`). Persist the
  trust-policy and capability-policy revisions with the decision.
- `rustok-modules` owns the typed `TrustVerifier` policy port and the
  fail-closed admission decision. An isolated verification worker/service owns
  Cosign execution, trust-root material access, SLSA provenance parsing, and
  CycloneDX validation; neither `apps/server` nor the module runtime executes
  those tools or receives their credentials.
- The worker returns only a typed decision and redacted evidence references.
  It must run with scoped registry/trust access, resource limits, and no module
  runtime capabilities. The owner commits its decision with admission metadata
  and outbox only after every required check passes.
- Worker implementation lives in `crates/rustok-verification-worker/`. The
  typed tonic gRPC listener/client lives in
  `crates/rustok-verification-transport/` so the owner port remains independent
  of a concrete transport. `ModuleInstaller` requires a `TrustVerifier` and
  policy revisions at construction, calls it before CAS stage/publish, and
  commits the resulting decision as admission evidence. Worker unavailability,
  malformed responses, policy-revision mismatch, or incomplete evidence reject
  installation; no local or legacy verifier exists as a fallback. The remaining
  slice is injected Cosign/SLSA/CycloneDX adapters in the isolated worker.
- Alloy/Rhai drafts are not marketplace-installable and do not require this
  publication trust policy. Static promotion uses its separate reviewed
  distribution-build policy.

1. Resolve catalog release to immutable manifest digest.
2. Fetch descriptor/config without executing payload.
3. Verify manifest digest and descriptor schema.
4. Verify signature, signer identity, trust root, and transparency evidence.
5. Fetch and verify SBOM/provenance/test attestations.
6. Evaluate platform/runtime ABI compatibility.
7. Resolve module dependencies against platform installation state.
8. Validate declared capabilities against platform policy.
9. Fetch exactly the descriptor-selected payload layer with explicit size limits.
10. Stream it into the platform CAS while verifying digest and media type.
11. Resolve and persist the exact dependency lock graph.
12. Persist admitted installation, blob reference, graph, and evidence atomically.
13. Activate bindings only through the lifecycle/dispatcher service.

### 3.6 Artifact Module Data and Migrations

For v1, untrusted artifact modules do not supply executable SeaORM/Rust
migrations or arbitrary SQL.

- [ ] Provide brokered namespaced storage capabilities for structured values,
  objects/files, indexes/query patterns supported by the platform, and
  secret-reference handles.
- [ ] Scope every operation by tenant, module slug, data-contract revision, and
  policy; the guest cannot choose a physical schema/table/bucket path.
- [ ] Validate data/settings/action payloads with bundled JSON Schema using the
  maintained `jsonschema` validator and bounded regular-expression settings.
- [ ] Define quotas, pagination, transactions/batches, optimistic revisions,
  idempotency, backup/export, retention, and deletion semantics.
- [ ] Keep secret values outside settings and module data; store only brokered
  secret references.
- [ ] Define data-contract upgrade hooks that transform through bounded sandbox
  commands without holding control-plane transactions.
- [ ] Before allowing declarative DDL migrations, create a separate ADR and
  threat model covering allowed operations, schema isolation, locks, rollback,
  backup, cross-module references, tenant rollout, and failure recovery.
- [ ] Static-promoted modules continue to use reviewed module-owned
  `MigrationSource` migrations in distribution builds.

### 3.7 Rollback, Uninstall, and Purge

- [ ] Rollback selects a previously admitted immutable release; it never edits
  the failed release.
- [ ] Capability grants are re-evaluated for the target release.
- [ ] Data migrations declare whether rollback is reversible, compensating, or
  prohibited.
- [ ] Runtime activation and tenant enablement rollback remain distinct.
- [ ] Every rollback is a new audited operation with actor and reason.
- [ ] Define disable, deactivate, uninstall, and purge as distinct operations:
  - disable preserves installation and data;
  - deactivate removes runtime bindings but preserves admitted release/rollback;
  - uninstall removes the scope's selection after dependent checks;
  - purge deletes retained module data only through an explicit destructive,
    authorized, audited operation.
- [ ] Uninstall never silently deletes tenant data, logs, evidence, or rollback
  artifacts.
- [ ] Garbage collection runs only after reference, retention, legal-hold,
  rollback, and audit checks.

### Verification Gate

- Tag mutation cannot change an installed artifact.
- Descriptor, payload, manifest, signature, SBOM, and provenance mismatch tests
  fail before persistence/activation.
- Concurrent installs and stale rollback requests are deterministic.
- Tenant RLS tests cover read, install, update, activate, and rollback.
- External registry outage does not break execution of an admitted cached blob.
- Artifact lifecycle, event, command, and HTTP dispatch work without a compiled
  `RusToKModule` implementation.
- Namespaced storage tests prove tenant/module isolation, quotas, revisions,
  backup/export, and explicit purge behavior.

## Phase 4 - Isolated Rust Module Build Worker

### Objective

Build Rust source into reproducible WASM Component artifacts without compiling
untrusted source inside `apps/server` or the runtime sandbox process.

### 4.1 Ownership and Deployment

- [ ] Keep build request/result/domain orchestration in `rustok-modules`.
- [ ] Define the worker protocol before creating another crate or service.
- [ ] Initially implement the worker as a separately deployable binary/process;
  split a package only when the protocol and operational lifecycle justify it.
- [ ] Run builds as isolated OCI jobs. Production untrusted builds use a
  hardened runtime such as gVisor or Kata where available.
- [ ] The worker has no tenant database access and no general platform secrets.

### 4.2 Build Request Contract

The immutable request contains:

- request, tenant/project, actor, and correlation IDs;
- source artifact reference and source digest;
- expected module slug and version;
- target runtime ABI and WIT world/version;
- pinned Rust toolchain and component target;
- locked dependency policy and allowed registries/sources;
- CPU, memory, disk, process, output, and wall-clock limits;
- network policy, defaulting to denied after dependency materialization;
- requested validation/test profiles;
- idempotency key and build attempt.

The request must not contain registry signing keys or reusable platform
credentials.

### 4.3 Worker Pipeline

1. Materialize immutable source into an empty workspace.
2. Verify source digest and safe archive paths.
3. Inspect the graph using `cargo metadata`/`cargo_metadata`.
4. Reject disallowed sources, Git revisions, build scripts, native links,
   unsafe policy violations, or dependency limits according to policy.
5. Run `cargo deny`, advisory checks, and `cargo vet` policy where configured.
6. Format/check/lint/test using pinned commands and locked dependencies.
7. Build the component with `cargo component build --locked`.
8. Validate and inspect exports/imports using `wasm-tools`.
9. Require the configured WIT world and reject undeclared imports.
10. Generate CycloneDX SBOM.
11. Produce provenance containing source, toolchain, command, dependency, WIT,
    and output digests.
12. Emit payload, SBOM, provenance, logs, metrics, and structured result to the
    publication service.

### 4.4 Worker Isolation Requirements

- read-only base image and toolchain;
- ephemeral source, target, and cache volumes;
- no host filesystem mount or container runtime socket;
- no privileged mode, device access, host PID/network, or reusable credentials;
- explicit process, CPU, memory, disk, output, and time quotas;
- dependency cache addressed by verified checksums;
- network disabled during compilation and tests unless a reviewed profile
  explicitly permits a scoped endpoint;
- logs and artifacts capped and streamed without blocking the worker;
- forced termination and cleanup after cancellation or deadline.

### 4.5 Build Result Contract

The terminal result contains:

- success or canonical build error code;
- source, dependency lock, toolchain, WIT, component, and SBOM digests;
- component imports/exports summary;
- validation, test, lint, policy, and vulnerability results;
- provenance and log references;
- duration/resource metrics;
- retryability and next allowed action.

### 4.6 Author SDK and CLI

- [ ] Generate Rust guest bindings from the frozen WIT contract with maintained
  Bytecode Alliance tooling; do not hand-maintain duplicate ABI structs.
- [ ] Add `rustok module` CLI flows for init, validate, test, build, package,
  inspect, and publish through existing CLI provider contracts.
- [ ] Provide module templates containing descriptor, WIT bindings, tests,
  locked toolchain, dependency policy, settings/action schemas, localization,
  and example brokered capabilities.
- [ ] Provide a local sandbox harness with the same request/policy/error contract
  and fixture capability broker as production, but no production credentials.
- [ ] Emit machine-readable diagnostics and build evidence usable by Alloy,
  CLI, CI, and admin without parsing human logs.
- [ ] Version SDK/templates independently and record their versions in build
  provenance.

### Verification Gate

- Identical request inputs reproduce the same logical output digest or emit a
  documented nondeterminism failure.
- Malicious archives, dependency graphs, build scripts, infinite builds,
  oversized output, network access, and undeclared WIT imports are rejected.
- Worker termination cannot affect server or sandbox runtime availability.
- The server never invokes Cargo directly for runtime marketplace installation.
- Generated guest bindings and local harness compatibility are tested against
  the exact host WIT/runtime ABI version.

## Phase 5 - OCI Publication, Signatures, SBOM, and Provenance

### Objective

Publish build/release outputs as verifiable supply-chain artifacts and enforce
trust policy before admission.

### 5.1 OCI Layout

- [ ] Freeze media types for descriptor/config, Rhai source, WASM Component,
  sidecar metadata, static-promotion source reference, SBOM, provenance, test
  evidence, and release lineage.
- [ ] Publish by content digest; tags point to immutable releases but are never
  accepted as installation identity.
- [ ] Attach SBOM/provenance/signature evidence using OCI referrers or a
  documented compatible layout.
- [ ] Ensure exactly one executable layer matches descriptor payload kind and
  digest.
- [ ] Use short-lived, least-privilege registry credentials through the host
  secret/provider boundary; credentials never enter descriptors, build inputs,
  logs, Alloy tools, or sandbox requests.
- [ ] Define registry redirect, cross-host auth, TLS, proxy, timeout, retry,
  maximum-size, and decompression policies explicitly.

### 5.2 Signing

- [ ] Use `cosign`/Sigstore-compatible signing rather than custom cryptography.
- [ ] Define accepted trust roots, signer identities, certificate constraints,
  transparency-log policy, offline verification behavior, and key rotation.
- [ ] Separate author signature, build-service attestation, marketplace approval,
  and platform admission decisions.
- [ ] Do not equate a valid signature with a trusted module; policy must verify
  who signed what under which build/provenance conditions.

### 5.3 Publication Governance

- [ ] Stage release from an immutable source/build result.
- [ ] Run automated descriptor, compatibility, dependency, signature, SBOM,
  provenance, license, vulnerability, and sandbox smoke checks.
- [ ] Record review decisions, required changes, holds, approvals, rejections,
  yanks, and reasons as owner events.
- [ ] Publish creates a release once; retry resumes idempotent stages instead of
  duplicating a release.
- [ ] Yanking prevents new resolution but does not mutate existing installed
  artifact identity.
- [ ] Distinguish platform-built and externally-built artifacts:
  - platform-built releases require the RusToK build attestation;
  - external prebuilt artifacts require an approved external provenance policy
    and stricter review/quarantine;
  - absence of source or reproducible evidence is an explicit trust fact, never
    silently treated as equivalent.
- [ ] Treat marketplace README, metadata, source comments, test output, and
  artifact text as untrusted content for both UI rendering and AI prompts.

### Verification Gate

- Tampered payload, signature, certificate identity, SBOM, provenance, or
  referrer relationships fail admission.
- Trust-root rotation and offline verification have fixtures.
- Publish retry, hold/resume, approve/reject, and yanking preserve one event
  taxonomy and immutable release identity.
- Registry credential leakage, redirect/auth confusion, replayed attestations,
  malicious metadata rendering, and untrusted prompt-content fixtures fail
  safely.

## Phase 6 - Alloy Authoring and Release Evolution

### Objective

Use Alloy as the authoring environment for Rhai and AI-assisted Rust/WASM
evolution while sharing the production sandbox and module release contracts.

### 6.1 Draft Runtime

- [ ] Represent every execution with draft ID and monotonic revision.
- [ ] Reject execution/publish commands for stale revisions.
- [ ] Execute validation, tests, manual runs, hooks, schedules, and preview
  scenarios through `SandboxRuntime`.
- [ ] Convert Alloy entity/parameter behavior into explicit request-scoped
  bindings without adding generic Alloy concepts to `rustok-sandbox`.
- [ ] Persist execution evidence linked to revision and policy revision.
- [ ] Evolve the current single `code: String` model into a revisioned workspace
  contract for sources, imports/modules, tests, fixtures, schemas, policy, and
  generated artifacts. DB/object storage remains the source of truth; guests do
  not receive direct filesystem access.
- [ ] Resolve Rhai imports through an Alloy-owned bounded module resolver keyed
  by workspace/revision, with cycle, depth, size, and path validation.

### 6.2 Release Creation

- [x] Stage and package immutable Rhai descriptors with source digest/lineage.
- [ ] Validate declared capabilities from observed/declared tool use.
- [ ] Submit release source and descriptor to `rustok-modules` publication;
  Alloy does not write marketplace tables.
- [ ] Preserve author, prompt/tool provenance, tests, and review evidence under
  explicit retention/redaction rules.

### 6.3 Marketplace Fork and Continued Development

- [ ] Import an eligible published Rhai source and lineage into a new Alloy
  workspace.
- [ ] Fork records parent release, never mutates or overwrites it.
- [ ] Require a newer semantic version and new source/artifact digest.
- [ ] Allow tests and preview against the same WIT/capability policy as the
  installed parent.
- [ ] Publish the fork through the same governance pipeline as any release.

### 6.4 Rhai-to-Rust Evolution

This is an AI-assisted rewrite and validation workflow, not a transparent AST
transpiler.

- [ ] Generate typed Rust against the versioned WIT guest contract.
- [ ] Preserve the Rhai parent release and source lineage.
- [ ] Run generated Rust only through the isolated build worker.
- [ ] Compare scenario/contract evidence between Rhai and WASM versions.
- [ ] Publish the WASM implementation as a new release after review.
- [ ] Never emit or runtime-load a native dynamic library.

### 6.5 Agent Tools

Expose typed owner-backed tools such as:

- execute/validate/test draft;
- save revision;
- request build and inspect result;
- stage/review/publish release;
- import/fork release;
- inspect capability and policy failures;
- invoke approved MCP tools through the broker.

Tools must not expose unrestricted shell, filesystem, network, database, signing
keys, or registry credentials.

Marketplace descriptions, source code, README files, build logs, test output,
MCP results, and module responses are untrusted model input. They cannot alter
system/tool policy or grant capabilities.

- [ ] Separate trusted system/tool instructions from untrusted artifact context.
- [ ] Label and delimit untrusted context and cap its size.
- [ ] Validate every tool call against typed schema, actor/tenant, revision,
  capability, and operation policy outside the model.
- [ ] Bound agent iterations, parallelism, tokens/cost, execution/build attempts,
  and tool output.
- [ ] Require explicit operator approval for publish, destructive data change,
  trust-policy change, static promotion, and other externally consequential
  operations.
- [ ] Audit model/provider, prompt/template revision, tool requests/results,
  policy decisions, and resulting source/build/release lineage with redaction.

### Verification Gate

- Alloy and installed Rhai parity evidence passes.
- Stale revisions cannot execute or publish as current.
- Forking creates new lineage/version/digests and leaves the parent reproducible.
- Rhai-to-WASM scenario parity and review evidence are attached to publication.
- Prompt-injection and malicious tool-output fixtures cannot bypass tool policy,
  revisions, capability grants, approval, or audit.

## Phase 7 - Transport and Admin Cutover

### Objective

Make all operator surfaces thin consumers of owner-owned contracts.

Transport adapter preparation may proceed in parallel, but this phase cannot
complete until Phase 8 provides the canonical effective-policy, activation, and
multi-node reconciliation path consumed by those transports.

### 7.1 GraphQL

- [ ] Migrate catalog, release, publication, installation, lifecycle,
  composition, build, effective-policy, recovery, rollback, and promotion
  resolvers to the facade.
- [ ] Map canonical codes/details without reconstructing issue/retry taxonomy.
- [ ] Require typed actor, tenant, permission, idempotency, and revision inputs.
- [ ] Keep subscriptions/build events as transport adapters over owner events.

### 7.2 Native Leptos Server Functions

- [ ] Add owner-backed native operations for required Leptos admin surfaces.
- [ ] Reuse canonical DTOs through the approved framework-neutral contract
  layer; do not duplicate GraphQL types in the UI package.
- [ ] Preserve GraphQL as the public/headless surface.
- [ ] Add GraphQL/native parity fixtures for success, validation, conflict,
  policy denial, recovery, and build failure.

### 7.3 Dynamic Marketplace UI Boundary

Compile-time module-owned Leptos/Next/Flutter packages cannot be the normal UI
delivery mechanism for a runtime-installed artifact. The marketplace therefore
uses an explicit UI trust boundary.

- [ ] V1 requires host-rendered declarative contributions for settings,
  commands/actions, status, help, navigation metadata, tables/forms supported
  by the shared UI schema, and result/error presentation.
- [ ] Define one framework-neutral UI contribution schema and validate it with
  bundled JSON Schema. Leptos, Next, and Flutter hosts adapt the same contract;
  modules do not publish host-specific query/i18n/auth behavior.
- [ ] Bind every action to an admitted runtime binding, permission, input/output
  schema, confirmation/destructive flag, idempotency, and audit policy.
- [ ] Resolve route, navigation, child-page, and storefront slot collisions in
  the owner control plane before activation.
- [ ] Use the host-provided effective locale and signed/admitted localization
  catalogs; reject module-owned locale fallback chains and unsafe markup.
- [ ] If custom untrusted web UI is introduced, run it from an isolated origin in
  a sandboxed iframe with strict CSP and a versioned, origin-checked,
  schema-validated message SDK. Do not provide platform cookies, bearer tokens,
  DOM access, arbitrary navigation, or direct APIs.
- [ ] Native Leptos, Next, and Flutter code packages are allowed only through
  reviewed static promotion/distribution composition.
- [ ] Add a dedicated ADR before enabling iframe/custom UI artifacts; the
  declarative V1 path must not grow ad-hoc executable expressions.

### 7.4 Admin Simplification

- [ ] Remove direct SQL to `platform_state`, build, registry, release, publish,
  installation, and lifecycle tables.
- [ ] Remove admin-owned module/Cargo manifest scanning and filesystem loading.
- [ ] Remove admin-owned canonical hashing, dependency solving, build planning,
  and marketplace synthesis.
- [ ] Remove local lifecycle/governance/status/retry mappings.
- [ ] Keep transport facade, route/query state, view models, optimistic UI keyed
  by revision/idempotency, and presentation effects.
- [ ] Add a static verifier preventing backend logic from returning to the admin
  host.

### Verification Gate

- Admin module transport contains no SQL or workspace filesystem/Cargo scanning.
- GraphQL/native operations return equivalent canonical facts and codes.
- UI displays platform-installed, tenant-enabled, channel-bound, trust, build,
  and update states separately.
- No transport path bypasses owner authorization, policy, audit, or revision
  checks.
- Declarative UI parity fixtures render equivalent actions/status/errors across
  applicable hosts, and custom UI cannot access host credentials/DOM/API.

## Phase 8 - Effective Policy and Runtime Activation

### Objective

Produce one explainable availability decision used consistently by server,
workers, transports, and UI.

### Inputs

- module/release existence and compatibility;
- admitted platform installation and active release;
- Core/Optional kind;
- tenant override/settings and dependency state;
- channel binding;
- capability grants and policy revision;
- release yanked/revoked/security state;
- runtime executor availability;
- maintenance/quarantine state.

### Deliverables

- [ ] Return decision, contributing facts, policy revision, and denial reasons.
- [ ] Use the same decision in lifecycle writes, runtime dispatch, routing,
  events, scheduler, APIs, and admin UI.
- [ ] Invalidate/cache decisions using explicit revision dependencies.
- [ ] Quarantine blocks new execution without silently changing tenant intent.
- [ ] Revocation policy distinguishes emergency stop from ordinary yanking.
- [ ] Implement a durable desired-state/observed-state reconciler for every
  server/sandbox node; in-memory registries and caches are never control-plane
  sources of truth.
- [ ] Publish composition, installation, activation, grant, quarantine,
  revocation, and binding changes through the existing transactional outbox.
- [ ] Make consumers idempotent and revision-aware because delivery is
  at-least-once; stale/out-of-order events cannot reactivate old state.
- [ ] Define node readiness: required Core/static definitions, active artifact
  graph revision, CAS availability, executor ABI, and policy revision must be
  reconciled before serving affected traffic.
- [ ] Define prepare -> health/smoke -> activate transitions and optional
  tenant/cohort canary rollout for upgrades.
- [ ] Drain or cancel old-revision executions according to binding policy before
  releasing old blob/cache references.
- [ ] Use distributed leases/locks only where necessary and always pair them with
  database revisions/idempotency; a lease alone is not correctness evidence.

### Verification Gate

- Tenant/channel isolation and dependency tests cover every branch.
- Stale cached decisions cannot execute after policy/revocation change.
- Core, Optional, installed, enabled, bound, and executable states are not
  conflated.
- Multi-node restart, partition, duplicate/out-of-order event, rolling upgrade,
  stale cache, canary failure, and emergency revocation tests converge to the
  same durable desired state.

## Phase 9 - Sidecar Executor

### Entry Condition

Start only after sandbox audit/cancellation/admission, WIT/WASM, OCI trust, and
artifact installation are stable and verified.

### Deliverables

- [ ] Freeze a versioned sidecar control/data protocol and health lifecycle.
- [ ] Use generated `tonic`/`prost` contracts for the v1 control/data plane
  unless an ADR demonstrates that a WIT-native RPC implementation is mature and
  materially better; do not write a custom socket/JSON protocol.
- [ ] Run each untrusted sidecar in a hardened process/container boundary.
- [ ] Use a scoped local RPC channel; never load sidecar code into the server.
- [ ] Route all platform access through the same capability broker semantics.
- [ ] Enforce startup, request, idle, memory, CPU, concurrency, output, and
  shutdown limits.
- [ ] Implement crash isolation, health checks, backoff, circuit breaking,
  cancellation, and forced cleanup.
- [ ] Verify sidecar image digest, signature, SBOM, provenance, and declared
  protocol/capabilities before start.
- [ ] Emit the same sandbox outcome and audit taxonomy where semantics match;
  add sidecar-specific structured details without new transport taxonomy.

### Verification Gate

- Crash, hang, fork/process bomb, network attempt, disk growth, oversized RPC,
  and capability denial tests cannot affect the host.
- Tenant/artifact process and credential isolation is demonstrated.
- Sidecar removal leaves no process, volume, socket, or credential residue.

## Phase 10 - Trusted Static Promotion

### Objective

Retain native performance and deep integration as an explicit reviewed
distribution mode, not the default marketplace installation path.

### Deliverables

- [ ] Define promotion request, review, approval, build, release, rollback, and
  revocation records.
- [ ] Require source availability, trusted ownership, dependency audit, tests,
  static review, and platform-team approval.
- [ ] Pin the promoted release and source/dependency digests.
- [ ] Generate distribution composition through build tooling; runtime install
  never edits the server Cargo graph.
- [ ] Compile promoted crates in CI/distribution builds, not the running server.
- [ ] Map the native module to the same module/release identity and lifecycle
  facts while marking executor mode as static/native.
- [ ] Require a new distribution build for promotion, upgrade, removal, or
  rollback.
- [ ] Do not claim sandbox isolation for native execution.

### Verification Gate

- Only approved promotion records affect distribution composition.
- Runtime marketplace operations cannot trigger native compilation.
- Static and sandboxed variants cannot be ambiguously active for the same
  installation scope/release.
- Distribution rollback and database migration compatibility are tested.

## Phase 11 - Operations, Security, and Performance

### Observability

- [ ] Correlate publish, build, install, activate, tenant lifecycle, sandbox,
  capability, rollback, and promotion operations.
- [ ] Define metrics for queue depth, build duration/failure, verification
  failure, install/activation latency, sandbox saturation, execution outcome,
  capability denials, cache hit rate, sidecar health, and rollback frequency.
- [ ] Add structured logs with mandatory tenant/actor/artifact/revision fields
  and redaction.
- [ ] Add dashboards and alerts for trust failures, sandbox saturation,
  repeated traps/timeouts, build worker exhaustion, and revocation.
- [ ] Bound metric label cardinality: raw tenant, artifact digest, URL, actor,
  and error text belong in traces/logs with policy, not unbounded metric labels.

### Security

- [ ] Threat-model untrusted source archives, manifests, OCI registries,
  signatures, attestations, Rhai, WASM, sidecars, agent tools, and admin APIs.
- [ ] Fuzz descriptor, OCI config, WIT/component, SBOM, provenance, and sidecar
  protocol parsers.
- [ ] Add dependency/license/advisory gates to platform and worker builds.
- [ ] Test SSRF, path traversal, archive bombs, decompression bombs, signature
  confusion, digest confusion, confused-deputy capabilities, and tenant leaks.
- [ ] Define incident response for quarantine, revocation, emergency disable,
  trust-root compromise, and malicious publisher.
- [ ] Add backup/restore and disaster-recovery procedures for control-plane DB,
  artifact CAS, trust roots/policies, module data namespaces, audit evidence,
  and outbox/reconciliation checkpoints.
- [ ] Verify restored installations against digest/trust evidence before
  execution; restore never implicitly clears quarantine or revocation.
- [ ] Define tenant export/deletion and legal-hold behavior for module data,
  logs, source workspaces, build evidence, and artifacts.

### Performance

- [ ] Establish cold/warm Rhai and WASM execution baselines.
- [ ] Benchmark fuel/epoch, audit, broker, cache, and admission overhead.
- [ ] Benchmark OCI resolution and verification with bounded caches.
- [ ] Benchmark build concurrency and define worker autoscaling/backpressure.
- [ ] Define SLOs before enabling broad marketplace publication.
- [ ] Load-test definition lookup, resolved-graph cache, CAS node cache,
  reconciler convergence, declarative UI schema, and namespaced storage.

### Verification Gate

- Security review has no unresolved critical/high issues for the enabled mode.
- Operational runbooks exist for every terminal and recovery state.
- Performance budgets and saturation behavior are tested under tenant load.
- Backup/restore, regional/node recovery, CAS rebuild, and outbox replay drills
  converge without identity, tenant, quarantine, or revocation loss.

## Phase 12 - Atomic Cutover and Removal

### Objective

Remove the old hardcoded optional-module control plane after all consumers use
the target architecture.

### Cutover Sequence

1. Freeze canonical contracts and guardrails.
2. Migrate owner services and transactional writes.
3. Migrate GraphQL/native transports.
4. Migrate admin and internal callers.
5. Enable artifact publication/admission/runtime for selected pilot modules.
6. Verify policy, tenant, rollback, and operational evidence.
7. Remove server/admin bypass implementations and duplicate DTOs.
8. Remove optional runtime Cargo features/dependencies from the normal server
   distribution.
9. Retain only Core/bootstrap crates and explicitly promoted native modules in
   static composition.
10. Run the complete verification and documentation audit.

### Required Removals

- server-owned composition/governance business logic replaced by owner facade;
- admin SQL and manifest/Cargo scanning;
- duplicate hashing, dependency, build-planning, lifecycle, recovery, trust,
  and status mapping;
- direct optional-module crate references in normal runtime composition;
- fallback-to-legacy executor, install, read, or write paths;
- dynamic native library loading or source-copy installation paths;
- artifact identity/policy/lifecycle decisions based solely on the compile-time
  `ModuleRegistry`;
- per-execution payload downloads from an external OCI registry after admission;
- arbitrary artifact SQL migrations, routers, GraphQL fields, host-process UI,
  or raw infrastructure clients;
- temporary bridges without an explicitly approved owner and deadline.

### Verification Gate

- Repository guardrails find no forbidden paths.
- Fresh runtime installation succeeds without source/Cargo changes.
- Server starts without compile-time knowledge of pilot optional modules.
- Install, activate, execute, upgrade, rollback, disable, uninstall, revoke,
  fork, rebuild, and republish scenarios pass end to end.
- Artifact-only lifecycle hooks, events, schedules, commands, HTTP bindings,
  namespaced data, declarative UI, and multi-node reconciliation pass without a
  compiled module implementation.
- GraphQL/native/admin, tenant/channel, audit, metrics, and runbooks agree.

## Critical Path and Parallel Work

The minimum critical path is:

```text
Phase 0 contracts
  +-> Phase 1 shared draft/artifact runtime
  +-> Phase 2 definition catalog/dispatcher and owner facade
        -> Phase 3 dependency lock, CAS, artifact/install/data state
  [Phase 1 and Phase 2/3 foundations merge]
  -> Phase 2 write-path/composition/governance cutover
  -> Phase 4 build worker
  -> Phase 5 trust/publication
  -> Phase 6 Alloy evolution
  -> Phase 8 effective runtime policy and reconciliation
  -> Phase 7 transport/admin cutover
  -> Phase 12 removal
```

Permitted parallel tracks:

- build-worker protocol may be designed while facade extraction proceeds, but
  publication cannot ship before owner artifact/trust contracts are frozen;
- admin view models may prepare for canonical DTOs, but backend SQL is removed
  only with the working owner transport replacement;
- operations/threat modeling starts immediately and is completed continuously;
- sidecar starts only after the stated entry condition;
- static promotion design may proceed in parallel but cannot become a fallback
  marketplace path.

## Suggested Atomic Work Packages

Do not implement the whole plan in one branch. Each work package moves all of
its internal callers and removes its superseded path before merge.

| Order | Suggested branch scope | Required result | Deliberately excluded |
|---|---|---|---|
| 1 | `module-platform-contracts` | Owner command context, stable errors, revisions/CAS, idempotency, serialized snapshots, bypass inventory/verifier | Service moves |
| 2 | `module-definition-catalog` | Artifact/static `ModuleDefinition`, exact dependency solver/lock contract, policy no longer tied to trait objects | Runtime dispatch |
| 3 | `module-artifact-cas` | Admission streaming into CAS, verified node cache, runtime reads CAS, retention/GC basics; remove per-call OCI fetch | Signatures/SBOM policy |
| 4 | `module-runtime-dispatcher` | Static/sandbox binding dispatcher over admitted CAS bytes; artifact lifecycle/command/event pilot; remove lifecycle dependence on native hooks | Publication governance |
| 5 | `alloy-sandbox-cutover` | Revisioned `AlloyDraft` requests and all production Alloy execution through shared runtime; remove parallel path | Full workspace/review UX |
| 6 | `module-composition-facade` | Platform composition CAS plus build enqueue owner operation; migrate callers and remove server implementation | Registry governance |
| 7 | `module-governance-facade` | Catalog/publication/release/approval/yank owner state machine and transports; remove server business logic | Build worker |
| 8 | `module-build-worker-contract` | Build request/result, worker deployment, generated WIT SDK, deterministic component build/tests/SBOM/provenance | Signing/admission approval |
| 9 | `module-trust-admission` | OCI publication, cosign verification, SBOM/provenance/trust policy, external artifact rules, rollback/quarantine | Sidecar |
| 10 | `module-namespaced-data` | Namespaced data/file/secret-reference capability, schema validation, quota/export/retention/purge contracts | Arbitrary SQL/migrations |
| 11 | `module-declarative-ui` | Declarative UI/actions, localization, host parity, route/slot collision and isolation guardrails | Custom host-process UI |
| 12 | `module-admin-cutover` | GraphQL/native parity, admin SQL/filesystem/build logic removal, effective-state UI | Static promotion |
| 13 | `module-static-promotion` | Reviewed distribution composition and native identity/mode transitions | Runtime fallback |
| 14 | `module-sidecar-executor` | Hardened sidecar protocol/runtime after entry gates pass | In-process native plugins |
| 15 | `module-platform-final-cutover` | Multi-node evidence, operations/DR, complete bypass removal, optional Cargo path deletion | New features |

The first branch must not introduce empty facades without migrated callers. If
contract extraction reveals a missing decision, update Phase 0 and the local
owner plan before implementing downstream services.

## Pilot Strategy

Use three pilots to prove different properties:

1. A pure Rhai module authored and forked through Alloy proves draft/artifact
   parity, immutable lineage, artifact-only lifecycle/command/event dispatch,
   namespaced storage, and one declarative admin action.
2. A Rust-to-WASM module with one brokered capability proves build, WIT, OCI,
   SBOM/signature, CAS admission, dependency lock, installation, multi-node
   reconciliation, and sandbox execution.
3. A reviewed existing native module promoted statically proves distribution
   composition without redefining runtime installation.

Do not choose a pilot whose business complexity hides platform failures. Each
pilot must have deterministic fixtures and an explicit rollback path.

## Repository Verification Matrix

| Scope | Required evidence |
|---|---|
| `rustok-sandbox` | Unit/contract tests with `rhai` and `wasm-component`; default-deny, limits, cancellation, audit, concurrency |
| `rustok-modules` | Artifact, OCI, trust, install, lifecycle, recovery, CAS, RLS, rollback, facade integration tests |
| Alloy | Runtime static verifier plus executable draft/artifact parity, revision, fork, and publication tests |
| Server | Thin-adapter tests, GraphQL/native parity, no direct write guardrail, host composition check |
| Admin | No SQL/filesystem/build logic guardrail, transport/view-model tests, browser scenarios |
| Build worker | Malicious-input, isolation, deterministic build, WIT, SBOM, provenance, cancellation tests |
| End to end | Publish -> install -> activate -> execute -> upgrade -> rollback -> revoke; Alloy fork and republish; static promotion |

Minimum commands evolve with implementation, but the final gate includes:

- `cargo test -p rustok-sandbox --features "rhai wasm-component" --lib`;
- `cargo test -p rustok-modules --lib`;
- targeted Alloy execution/publication tests;
- `npm run verify:alloy:runtime-contract`;
- module-control-plane static guardrails;
- `cargo check -p rustok-server --lib`;
- targeted admin tests and browser smoke scenarios;
- workspace manifest/module validation.

## Phase Completion Rules

A checkbox is complete only when:

1. production callers use the target path;
2. the superseded path is removed, unless explicitly required as an external
   compatibility surface;
3. tests cover success, conflict, denial, and failure/recovery behavior;
4. local and central documentation matches the code;
5. observability and operator recovery are defined for persistent operations;
6. evidence is stronger than a type compiling or an isolated helper test.

Partial scaffolding, an unused facade, a request builder without production
callers, or a green narrow test does not complete a phase.

## Definition of Done

This plan is complete only when all of the following are proven:

- `rustok-modules` is the sole owner of module marketplace/control-plane
  orchestration and durable writes;
- `rustok-sandbox` is the sole sandbox execution contract for Alloy drafts and
  installed Rhai/WASM/sidecar artifacts;
- Alloy publishes and forks immutable releases through owner services;
- Rust marketplace source is built outside the server in an isolated worker;
- published artifacts are digest-pinned and verified with required signature,
  SBOM, provenance, compatibility, dependency, and capability evidence;
- runtime install/upgrade/remove never changes server source or Cargo metadata;
- artifact modules are identified and resolved from the durable definition
  catalog rather than requiring a compile-time `ModuleRegistry` entry;
- admitted payloads execute from platform CAS rather than an external-registry
  fetch on each call;
- artifact lifecycle, command, HTTP, event, and schedule bindings dispatch
  through the shared runtime without native routers/closures;
- artifact module data is brokered and tenant/module scoped, and untrusted
  artifacts cannot execute arbitrary migrations;
- dynamic marketplace UI follows the declarative/isolated boundary and cannot
  inject code or credentials into host processes;
- server and admin have no backend bypass logic;
- GraphQL and native transports have verified semantic parity;
- tenant, channel, capability, revision, RLS, audit, rollback, revocation, and
  operational invariants pass end-to-end tests;
- trusted native code enters only through explicit static promotion;
- multi-node reconciliation, outbox replay, rolling upgrade, backup/restore,
  quarantine, and revocation preserve the same durable revisions and identity;
- the old hardcoded optional-module runtime path and all internal compatibility
  fallbacks are deleted.
