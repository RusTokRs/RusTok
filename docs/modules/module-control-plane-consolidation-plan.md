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
- Last updated: 2026-07-16.
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
- [x] Freeze the v1 runtime binding set and dispatch envelope for lifecycle,
  command, HTTP, event, schedule, and hook calls. Every artifact execution
  passes the strict `ArtifactBindingDispatchEnvelope` v1 through the shared
  sandbox path. It carries only the owner-selected binding ID/kind, execution
  phase, and payload; the runtime rejects another version or a binding/phase
  mismatch before it reads CAS bytes. Descriptor input schemas validate the
  enclosed payload rather than the owner metadata, while artifact code never
  selects a binding, phase, or installation.
- [x] Freeze v1 artifact persistence: brokered namespaced storage only.
  `ArtifactPersistenceContract` has only a revision and descriptor-bundled
  schema digest, and the complete v1 descriptor tree rejects unknown fields at
  decode time. Marketplace artifacts therefore cannot declare SQL, DDL, native
  migrations, object-store paths, or host handles; arbitrary artifact
  migrations remain disabled pending a separate ADR and threat model.
- [x] Freeze v1 dynamic UI delivery:
  - Marketplace descriptors admit only the host-rendered declarative
    `admin_settings` and `admin_actions` surface vocabulary, immutable
    localization metadata, and a declared module-owned permission. Phase 7
    owns the later action-to-binding presentation contract.
  - No untrusted custom web UI is admitted in v1. If it is introduced, it must
    use a sandboxed iframe under a separate reviewed contract.
  - Native Leptos, Next, and Flutter packages have no artifact descriptor
    field and require static promotion.
  - Strict descriptor decoding rejects component source, URLs, iframe fields,
    and every other executable or host-specific UI field; marketplace artifacts
    therefore cannot inject code into a host process.
- [x] Freeze the admitted artifact CAS, retention, garbage collection, and
  external-registry outage behavior. Admission is `stage -> durable CAS
  publish -> database transaction plus outbox -> reconciliation`; the
  reconciler deletes only an unreferenced published digest that an explicit
  durable retention snapshot marks expired and unprotected. Missing snapshot
  data fails closed. Runtime reads and rehashes only admitted CAS bytes, so an
  external registry outage is immaterial while the blob exists and produces
  `BlobNotFound` before sandbox execution when it does not.
- [x] Add static guardrails prohibiting new direct writes outside owner modules.
  `verify-module-control-plane-write-path.mjs` rejects direct composition,
  lifecycle, artifact installation/data, build, and registry governance writes
  from the server and module build/verification worker or transport crates. It
  also requires a matching write implementation in the corresponding
  `rustok-modules` owner source.

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
- [x] Define a versioned Rhai input/output binding shared by draft and published
  Rhai artifacts. `RhaiBindingInput`/`RhaiBindingOutput` v1 are strict neutral
  envelopes with no raw-value fallback. Alloy keeps its data-only draft payload
  inside the envelope, while artifact runtime wraps admitted inputs in its
  strict binding-dispatch envelope and unwraps only a valid versioned result
  for its owner.
- [x] Freeze the WIT v1 package, world, entrypoint, JSON/error encoding, and ABI
  compatibility rules.
- [x] Add request-scoped cancellation propagation through runtime, Rhai,
  Wasmtime, and brokered capability dispatch.
- [x] Add deadline cancellation for every enabled executor. Rhai checks the
  request deadline in its progress callback and returns the common timeout
  error; Wasmtime uses a request-private epoch watchdog that interrupts the
  component without affecting another execution. Sidecar is not enabled and
  remains subject to its separate deployment-profile contract.
- [x] Add runtime-scoped global, executor, tenant, and artifact concurrency
  admission with automatic permit release.
- [x] Add durable execution audit persistence through a fallible observer
  adapter. `SeaOrmArtifactExecutionObserver` accepts only
  `SandboxSubject::ModuleArtifact`, persists its exact installation ID with
  redacted start/terminal records under PostgreSQL tenant RLS, and fails the caller when audit persistence is
  unavailable.
  Artifact runtime composition must attach the adapter; the neutral sandbox
  remains storage-neutral and does not persist payloads or policy grants.
- [x] Exclude inputs, outputs, headers, credentials, and untrusted error text
  from neutral audit records.
- [x] Add bounded node-local compiled-component cache policy keyed by Wasmtime
  engine version, host target, admitted runtime ABI, and artifact digest. The
  cache retains only serialized Components and rehydrates them into a
  request-private engine/store; it has entry/byte LRU bounds, never retains
  tenant or host state, and evicts a corrupt value before recompiling.
- [ ] Add deterministic metrics for fuel/instructions, memory, output size,
  capability calls, queue time, and execution time. The neutral runtime now
  records queue time, executor duration, output size, Rhai instructions or
  Wasmtime fuel consumption, and policy-admitted capability-call count for
  success and terminal failure evidence. Artifact audit persists queue time and
  capability calls alongside the existing metrics. Wasmtime now reports actual
  aggregate non-shared guest linear-memory peak through its resource limiter,
  excluding failed growth rather than reporting a configured limit; Rhai peak
  memory still requires isolated-worker observation, so this item remains open.
- [x] Replace unbounded thread-per-host-call bridging with a strictly bounded
  one-thread-per-execution bridge. A synchronous guest ABI cannot permit thread
  exhaustion.
- [x] Validate input and output against admitted binding JSON schemas with
  network/file schema retrieval disabled. `ArtifactRuntime` compiles only
  descriptor-bundled Draft 2020-12 documents into a bounded node-local cache,
  applies strict formats and linear-time regex bounds before sandbox input and
  after decoded output, and rejects non-local `$ref`, `$dynamicRef`, and
  `$recursiveRef` values during admission.

### 1.3 Capability Broker Requirements

- [x] Move capability policy evaluation before all host adapter invocation.
- [x] Enforce tenant/actor/subject consistency on every capability call.
- [x] Define and enforce HTTP host/method/path constraints before broker
  invocation.
- [x] Define constraints for storage namespace, event topics, secret references,
  and MCP server/tool names. The `platform.secrets` grant now accepts only a
  typed, exact logical reference allowlist plus exact operations; guest input
  cannot name a resolver, resolver key, or secret value. The data owner now
  persists a revisioned/idempotent tenant/module/data-contract binding from
  that logical name to a host-authorized `SecretRef` and emits redacted outbox
  evidence. `RegistryArtifactSecretAuthorizer` validates that reference through
  the deployment `SecretResolverRegistry` without resolving it, while a host
  policy port owns lifecycle/RBAC checks. Its `acquire_handle` broker is injected
  with the admitted artifact scope and returns only the logical name and revision
  after host authorization; a value-consuming secret-use broker remains unfinished.
  `platform.events` now requires exact or terminal-wildcard topic grants plus
  exact operations, and accepts only a topic with an optional payload.
  `platform.data` now requires declared logical-key prefixes and `get`/`put`/
  bounded-`list` operations; its input cannot name a table, bucket, path, or
  namespace, and its owner adapter uses escaped prefix queries plus a checked
  continuation. `platform.data.objects` separately requires declared logical
  object prefixes and `get_metadata`/`read`/`put`/`list` operations. For
  larger writes it also has owner-owned `begin_upload`/`append_chunk`/
  `complete_upload` operations: every base64 chunk is capped at 44 KiB, while
  durable private session metadata, ordered chunk verification, final size and
  SHA-256 verification, expiry reaping, and retention-GC hand-off keep the
  artifact away from physical storage identity. `platform.mcp` now requires an exact server/tool pair and its
  `call` operation; endpoint, transport, credential, and tool-discovery fields
  are rejected before broker invocation. `CapabilityBrokerRouter` composes
  owner adapters by exact capability name, rejects duplicate ownership, and
  keeps unregistered capabilities default-deny; it allows data and secret
  adapters to share one runtime without a platform-global fallback. The owner
  `ArtifactMcpCapabilityBroker` now checks its injected tenant/module scope and
  forwards only logical target, arguments, and scoped execution identity to an
  `ArtifactMcpInvoker` port; it has no endpoint, token, credential, or tool
  discovery input. Server composition must still bind that port to the existing
  MCP access-policy, audit, and configured server-alias implementation.
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
- [x] Resolve an active artifact runtime installation from durable owner state.
  `SeaOrmArtifactInstallationStore` resolves the exact descriptor payload digest
  under tenant RLS, prefers an active tenant installation over the active
  platform installation, and excludes uninstalled or tenant-disabled candidates.
  The resolver revalidates descriptor and dependency-lock identity before the
  sandbox receives an execution request; it never rebuilds state from a registry
  tag or catalog mutation.

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

- [x] Extend the versioned descriptor with declarative bindings:
  - lifecycle `pre_enable`, `post_enable`, `pre_disable`, `post_disable`;
  - health/readiness and activation smoke checks;
  - named commands/actions;
  - namespaced HTTP handlers;
  - event subscriptions;
  - schedules;
  - before/after/on-commit hooks where the host contract permits them.
  The immutable descriptor now has distinct kinds for readiness, activation
  smoke, and before/after/on-commit declarations. Event and Schedule bindings
  have durable owner delivery hosts. HTTP bindings now
  declare a host-owned relative literal route, method, JSON media types,
  request/output limits, timeout, and forbidden streaming; the generic
  dispatcher matches only an admitted route and enforces the JSON size limits.
  The server owns authenticated HTTP and command transports, so declaration
  alone never authorizes an external request.
- [x] Give every binding a stable ID, input/output schema digest, permission,
  idempotency mode, timeout/limit profile, and declared capabilities.
- [x] Introduce `ModuleExecutionDispatcher` (working name) that resolves the
  active definition and dispatches:
  - Core/static implementations through a typed static adapter;
  - Rhai/WASM/sidecar implementations through `SandboxRuntime`.
- [x] Use one admitted-artifact binding execution port for lifecycle and
  non-lifecycle dispatch. Lifecycle is a convenience envelope over the generic
  port; artifact-only hosts can dispatch an admitted binding with explicit
  sandbox phase and input, while static modules remain fail-closed for dynamic
  binding IDs.
- [x] Replace `run_module_lifecycle_hook(ModuleRegistry, ...)` with the dispatcher
  so artifact modules can participate in lifecycle without a server crate.
- [x] Dispatch events/schedules from durable binding metadata; do not register
  artifact Rust closures in `ModuleEventListenerRegistry`. Artifact Event
  bindings now carry bounded exact or terminal-wildcard topics in the immutable
  descriptor; the generic dispatcher matches those topics only and rejects a
  binding/ExecutionPhase mismatch. The durable event and schedule hosts execute
  only persisted exact installations. The generic dispatcher also rejects malformed or
  wildcard delivered event types before subscription matching, so only exact
  platform event identities can reach admitted artifact bindings.
- [x] Define event delivery as at-least-once with binding-scoped idempotency,
  retry/backoff, dead-letter evidence, payload schema/version, and bounded
  wildcard/topic subscriptions. `ArtifactBindingDispatch` now distinguishes
  current-release dispatch from an explicit immutable installation target. A
  durable worker must use the exact target, and the resolver fails closed rather
  than executing a changed effective tenant selection. The owner now has a
  tenant-RLS `module_artifact_event_deliveries` projection keyed by source
  event, installation, and binding; it preserves the full versioned source
  digest, atomically claims leased work, applies bounded queue-owned exponential
  retry, and records terminal dead-letter evidence. Its worker adapter executes
  the persisted admitted binding only through `ExactInstallation`; no catalog
  or registry fallback exists. The outbox relay now decorates its downstream
  target with this owner projector before acknowledgement; a transient
  projection failure retries the source `sys_events` record, while global
  events without a tenant composition are deliberately not projected. The
  queue is now also a `ModuleWorkScheduler` source/handler pair: it enumerates
  host-supplied tenants, claims one tenant-RLS delivery, and dispatches only
  that persisted exact installation. Event and Schedule adapters share
  explicit host handles for the sandbox-backed executor and tenant enumerator.
  The neutral artifact subject now also carries the exact owner-selected
  installation ID, which is the mandatory key for a future dynamic capability
  scope router; release slug/version/digest alone cannot select a tenant scope.
  `ResolvingArtifactCapabilityBroker` now provides the fail-closed neutral
  router contract: only a host-owned resolver can return an owner broker after
  it validates the exact installation, tenant, lifecycle, and policy state.
  The host-owned admission command supplies the initial durable sandbox policy
  for that installation; the normal empty policy grants nothing. Admission and
  the owner policy resolver recheck exact active identity, tenant lifecycle,
  revision, and descriptor declarations. A missing policy or revision mismatch
  denies execution, and a declared capability never becomes an implicit grant.
  `resolve_granted_artifact_capability` is the shared exact-installation gate
  for dynamic owner routes: it resolves the immutable admitted installation,
  applies tenant lifecycle and uninstall state, reloads the current durable
  policy revision, and requires the named capability's explicit grant.
  `SeaOrmArtifactDataCapabilityBrokerResolver`,
  `SeaOrmArtifactDataObjectCapabilityBrokerResolver`,
  `SeaOrmArtifactSecretCapabilityBrokerResolver`, and
  `ArtifactMcpCapabilityBrokerResolver` then derive their data-adjacent scopes
  only from that result. The sandbox host checks data and object-data
  prefix/operation, logical-secret, and MCP server/tool constraints before a route runs. The
  server composes a real CAS-backed Rhai/WASM executor with the neutral
  `capability_call` bridge, exact policy resolver, and durable execution audit;
  it registers the event/schedule work entries before the native scheduler
  starts. `platform.data` and owner-owned resumable `platform.data.objects` are composed
  sandbox capability routes; secret, MCP, and every other unregistered
  capability remain default-deny until their deployment adapters exist. Artifact HTTP is separately composed as a
  platform-owned authenticated transport and does not register a sandbox
  capability route or network fallback.
  The production server now provides the active-tenant enumerator through the
  tenant owner service. The production server composes and supplies the shared
  CAS-backed executor before registrations run; the durable scheduler is the
  sole event/schedule loop for admitted artifact bindings.
- [x] Define schedule timezone, misfire, overlap/concurrency, deduplication,
  cancellation, and tenant enablement semantics. The admitted Schedule binding
  now declares timezone, misfire, overlap, and deduplication policy alongside a
  bounded cron form. The owner now has a tenant-RLS durable schedule-slot
  projection keyed by tenant, immutable installation, binding, and scheduled
  instant; it retains schedule digest, deduplication, lease, cancellation,
  retry, and dead-letter state. Semantic cron/IANA timezone validation now
  occurs at descriptor admission; five-field cron expressions normalize to a
  zero-second six-field form. `module_artifact_schedule_cursors` persists the
  materialization watermark, and the `ModuleWorkScheduler` source materializes
  a tenant before claiming its slot. A new or changed immutable schedule starts
  at the current host clock rather than replaying an old contract. `skip`
  ignores slots outside its bounded grace window, `run_once` emits one due slot,
  and `catch_up` advances in bounded batches. `forbid` drops new slots while a
  slot is pending/running for that exact binding; `queue` and `allow` retain
  slots and leave concurrency capacity to the deployment scheduler. The durable
  slot uniqueness key always prevents transport duplicates; `none` only omits
  an additional guest/application deduplication condition. The queue derives
  the digest from the admitted binding, cancels unavailable slots before
  dispatch, and executes only the exact immutable installation. Tenant
  enumeration is an injected host contract, so the worker never queries
  tenant-RLS state without a tenant scope. The production server composes the
  active-tenant source and the shared CAS-backed sandbox executor before
  registering the durable workers; only explicitly composed capability routes
  are available to the executor.
- [x] Define HTTP method/path namespace, auth/permission, request/response media
  type/schema, body/output limit, timeout, streaming policy, and idempotency;
  raw sockets and listener ports are forbidden. The admitted v1 contract now
  fixes literal relative paths, a method, JSON-only media types, bounded body
  and output sizes, a bounded timeout, and no streaming; the generic dispatcher
  rejects unadmitted routes and envelopes over the declared size, while the
  artifact runtime clamps the effective sandbox wall-clock limit to the
  declared timeout. `SeaOrmArtifactBindingIdempotencyStore` supplies one durable
  request-digest/replay/lease coordinator for every externally routed binding,
  so a crashed pending request can be reclaimed after its lease instead of
  becoming permanently stuck. The platform route now resolves an exact active
  installation, matches only its literal admitted binding, authorizes its
  declared RBAC key, and dispatches through the shared CAS sandbox executor.
  It accepts exactly `application/json`, maps declared request limits, and
  returns only the decoded JSON output. The generic dispatch envelope now accepts only bounded,
  host-supplied actor and trace identities and propagates them to sandbox
  capability calls and durable execution audit; descriptors and payloads cannot
  set those identities.
- [x] Namespace artifact HTTP routes under a platform-owned module route and
  reject route/method collisions. Descriptor admission rejects duplicate
  `(method, relative path)` pairs; artifacts cannot mount arbitrary Axum routers.
  The server owns `/api/artifacts/{installation_id}/{*path}` and never accepts
  an artifact-provided router, listener, host, or port. The route resolves the
  immutable installation before RBAC and sandbox execution, so a tenant override
  or lifecycle change fails closed instead of selecting a mutable “latest” release.
- [x] Keep dynamic operations behind generic command/HTTP contracts; artifacts
  cannot inject arbitrary GraphQL schema fields at runtime. The server exposes
  only platform-owned JSON routes: literal admitted HTTP bindings at
  `/api/artifacts/{installation_id}/{*path}` and exact admitted command bindings
  at `POST /api/artifacts/{installation_id}/commands/{binding_id}`. Both resolve
  one exact active installation, use the declared dynamic RBAC permission, run
  through the shared CAS-backed sandbox executor, and apply the same binding
  idempotency/replay lease. They add no artifact-defined GraphQL fields, routers,
  listeners, hosts, or ports.
- [x] Never run untrusted code while holding the database transaction that
  commits lifecycle/control-plane state. Lifecycle validation and durable
  intent/journal transitions happen before the pre-hook. The pre-hook receives
  a connection, never the state-commit transaction; the owner then commits the
  tenant state and operation journal in one short transaction. Post-hooks and
  post-hook retries run only after that transaction commits, so their failure
  becomes durable retry/compensation evidence rather than an implicit rollback.
  Artifact hooks use the same dispatcher boundary and have no transaction
  handle. Admission, rollback, deactivate, uninstall, tenant lifecycle, data
  purge, and migration checkpoints likewise complete their owner transaction
  before any downstream outbox consumer can execute an artifact.
  Deactivation, tenant disable/enable, and uninstall reject nil installation,
  actor, idempotency, and tenant-scope identities before opening that
  transaction, keeping lifecycle audit and idempotency records attributable.

### 2.4 Facade Shape

- [x] Introduce a single facade with explicit subservices rather than one large
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
  `ModuleControlPlane` now provides the owner composition root for the extracted
  catalog, lifecycle, composition, build, installation, release, and
  publication services. Server lifecycle, composition, artifact runtime/HTTP,
  registry release/publication/validation adapters, the independent validation
  worker, module-build dispatcher, and installer persistence adapter now consume
  those services through the facade. The facade also supplies the exact artifact
  data/object capability resolvers, redacted execution-audit observer, durable
  event-subscription projector, and binding idempotency store; server runtime,
  outbox projection, and routed artifact HTTP no longer construct those owner
  adapters directly. It also owns construction of the logical secret-binding
  service and dynamic `platform.secrets` capability resolver, so callers cannot
  bypass their host authorization ports or create a sandbox-visible secret
  broker directly. RBAC permission evaluation remains a separate RBAC-owner
  authorization adapter. `EffectivePolicyService` likewise owns the
  tenant override read and Core/default composition shared by server guards,
  GraphQL, and installer adapters. The static write-path verifier rejects direct
  construction of these extracted SeaORM services outside the owner crate.
  Promotion remains a separate unfinished subservice because no promotion
  workflow has been admitted yet.
- [x] Register the mandatory `ModulesModule` migration source in the shared
  server/installer migrator. Control-plane tables are no longer fixture-only:
  `rustok-migrations::Migrator` now includes the owner migration source before
  schema application, so fresh installations receive artifact admission,
  lifecycle, rollback, and subsequent owner migrations.
- [ ] Define infrastructure ports for registry transport, artifact blob access,
  signature verification, SBOM/provenance verification, build scheduling,
  transactional storage, events, audit, clock, and ID generation.
- [ ] Keep transaction boundaries inside owner services while accepting a
  caller-provided database/transaction adapter where required.
- [ ] Add idempotency keys for install, publish, build, retry, rollback, and
  promotion commands. Post-hook retry is now the first lifecycle slice: its
  GraphQL mutation requires a non-nil UUID key, and the owner persists a
  tenant-scoped unique key in `module_operations`, binds it to the recovered
  operation through durable correlation, replays the original retry journal
  record without another hook dispatch, and rejects mismatched reuse with
  `IDEMPOTENCY_CONFLICT`. Compensation uses the same contract for its reverse
  lifecycle journal record. Artifact rollback now persists its complete
  immutable fingerprint (source installation/revision, actor, reason, selected
  capability-grant revision, and migration rollback mode) and committed
  source/target revisions in the owner operation record; matching retries replay
  after the admission state changes, while legacy incomplete records fail
  closed. Final registry publication requires a non-nil `Idempotency-Key` UUID
  at the live approval endpoint and stores its complete owner command
  fingerprint with the resulting release; only an exact retry replays a
  published request, while a missing legacy record or conflicting key reuse
  fails closed. Install and promotion remain separate unfinished command
  contracts.

### 2.5 Server Service Cutover

- [x] Move platform composition snapshot/CAS logic from
  `PlatformCompositionService` into the module owner. The active-release
  projection has moved first: `SeaOrmModuleCompositionService` owns the
  `platform_state` mutation and fails closed when the durable active
  composition is absent. The same owner service now reads and atomically
  bootstraps the canonical active snapshot from a host-supplied manifest,
  canonicalizes its JSON, computes its digest, and exposes revision-CAS
  replacement. The server release hook now performs only its host-owned OAuth
  synchronization before calling that owner operation. The owner now also opens
  the combined CAS/build transaction; a host enqueuer receives only that open
  transaction and cannot commit a build separately.
- [x] Move build enqueue coordination into `BuildService`, preserving atomic
  composition CAS plus build-request creation. `ModuleCompositionBuildEnqueuer`
  is the owner port; the server adapter creates the existing build record only
  through the owner-owned transaction, and it publishes its non-transactional
  build notification after commit. A failed enqueue rolls the CAS update back.
- [x] Move registry ownership, publish-request, release, validation-stage,
  yanking, and governance rules from `RegistryGovernanceService`. Release
  yanking, ownership binding, owner transfer, publish-request rejection,
  request-changes, hold, resume, and final publication have moved: the server
  performs authenticated authority checks, then calls
  `SeaOrmModuleGovernanceService`, which updates the relevant state and writes
  its governance audit facts in one transaction. Publication atomically writes
  the release projection and translations, owner binding or authorized rebind,
  optional approval-override evidence, and request finalization. Validation
  stages are owner-owned: manual report/requeue transitions, remote lease claim,
  heartbeat, terminal completion, expired-lease requeue, validation-job enqueue,
  job claim, stale-job recovery, worker retry telemetry, and automated result
  materialization. A later authorized enqueue marks a validation job still
  running after 15 minutes as failed with the stable
  `validation_worker_lease_expired` reason, then creates the next durable
  attempt and audit facts atomically. The worker supplies only immutable
  bundle-check evidence; the owner atomically transitions the request and job,
  creates follow-up stages, and writes all related audit facts.
  A successful job claim now carries an immutable delivery work item with the
  exact artifact storage key, SHA-256, size, content type, and publish-metadata
  snapshot. The independent `rustok-registry-validation-worker` polls and
  conditionally claims the durable owner queue, verifies those facts before
  parsing, and invokes the pure owner validator without a server request model.
  If immutable delivery facts cannot be assembled, the owner atomically rejects
  the request and fails the job with content-free audit facts instead of leaving
  it queued. The server endpoint only queues work; it has no server-local spawn
  path.
  Draft publish-request creation is also owner-owned: the owner persists the
  request, default-locale metadata, and creation audit fact together after host
  authorization. Artifact object storage remains a host adapter; its immutable
  result is attached by an owner transaction that resets reupload validation
  attempts, transitions the request to `submitted`, and writes audit facts.
- [x] Move remaining manifest validation that is platform-domain policy into
  `rustok-modules`; keep only host boot/loading adapters in the server. Publish
  request slug/version/locale/metadata constraints, UI-package shape, and
  owner-derived publication warnings now live in
  `ModulePublishRequestCreateCommand`; the controller retains only transport
  schema and authenticated-authority handling. Static module-settings schema
  resolution remains a typed host-manifest adapter, while schema validation and
  normalized settings construction now live in `rustok-modules`; server
  lifecycle code supplies only the resolved neutral schema and persists the
  owner-normalized value. Static `rustok-module.toml` parsing also remains a
  host adapter, but its module metadata, SemVer dependency/conflict, admin
  surface, and settings-schema rules now use the owner static-package contract.
  Static catalog entries use a second owner contract for required ownership and
  trust metadata, surface conflicts, and bounded marketplace descriptions and
  asset URLs. The owner also resolves the canonical static UI classification
  from host-parsed surface flags and rejects an explicit classification that
  contradicts them, and evaluates optional static platform-version ranges
  against a host-supplied RusToK version. Static UI i18n contract semantics
  (locale normalization, default membership, declared bundle paths, and surface
  prerequisites) and static HTTP provider exclusivity are also owner-owned;
  static crate-local runtime binding declarations are qualified by the same
  owner boundary before the server attaches them to its runtime spec;
  filesystem path and locale-file checks remain host adapters. The owner now
  also validates the resolved static catalog topology (default-enabled entries,
  direct dependencies, conflicts, dependency-version requirements, and
  platform-version compatibility) after the host applies TOML/package overlays.
  It also invokes the canonical shared static manifest-versus-registry contract;
  the server supplies only facts extracted from its compile-time registry.
  The owner also validates deployment build-surface semantics from host-decoded
  facts: standalone admin/storefront requirements, URL syntax, and storefront
  identity uniqueness. The remaining server code only reads host TOML/filesystem
  paths and verifies declared crate/locale files exist; it invokes the owner
  package, catalog, topology, i18n, and build-surface contracts for every
  platform-domain decision.
- [x] Move effective availability composition behind one typed query.
  `ModuleEffectivePolicyQuery` now owns core/default/tenant-override semantics
  for any supplied definition catalog. The server effective-policy adapter,
  lifecycle DB writer, and installer verification use it; host code supplies
  only persisted overrides and distribution defaults.
- [x] Replace server `build_registry()` usage in guards, lifecycle, event
  dispatch, runtime boot, installer, and APIs with the correct split between
  static implementation registry and durable definition/effective-policy
  services. The HTTP module guard now consumes the boot-owned static registry
  from `ServerRuntimeContext` and fails closed when bootstrap has not supplied
  it; it no longer constructs a registry per request. The HTTP installer now
  receives that same boot-owned static registry explicitly rather than creating
  a second topology. `bootstrap_app_runtime` is the sole production constructor
  of that compile-time registry; it stores one copy in `ServerRuntimeContext`
  before router, GraphQL, lifecycle, event-dispatch, and installer consumers
  receive it. Durable artifact definitions and effective policy remain owner
  services, so the static registry is never rebuilt from marketplace state.
  The server lifecycle transport now obtains distribution defaults from the
  active composition and calls `ModuleLifecycleDbWriter`; it no longer builds
  an effective-policy set, catalog, or dispatcher for a toggle, post-hook retry,
  compensation, or settings persistence. The server supplies a host-resolved
  settings schema only; the writer derives active identity, Core status, and
  effective enablement before it persists owner-normalized settings.
- [x] Replace server error taxonomies with transport mappings of owner errors.
  The marketplace registry HTTP adapter now maps the complete
  `ModuleGovernanceError` contract at its transport boundary: malformed owner
  commands are `400`, authority-reserved operations are `403`, missing owner
  aggregates are `404`, and state/idempotency/precondition failures are `409`.
  Owner storage faults remain a content-free `500`; server-local governance
  errors remain only for host authorization and storage-adapter concerns.
- [x] Delete superseded server models/helpers after each atomic caller cutover.
  The registry catalog adapter and router now expose only `/v1/catalog` and
  `/v1/catalog/{slug}`. The former `/catalog` compatibility routes, client
  fallback probes, and helper exports were removed rather than preserving a
  dual transport path. Catalog generation now fails closed on an invalid active
  composition instead of silently substituting the builtin manifest. The
  superseded server-local publish-request translation upsert was also removed:
  publication-request translations are now written only by the owner
  create/publication transactions.

### 2.6 Write-Path Guardrail

`scripts/verify/verify-module-control-plane-write-path.mjs` rejects direct
composition, lifecycle, artifact installation, build-request, and registry
governance aggregate writes from every server, installer persistence, worker,
and transport production source. It also rejects direct construction of the
extracted owner SeaORM services in those sources; all production composition
must pass through `ModuleControlPlane` in `rustok-modules`.

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
- [x] Add platform compatibility range and required feature/capability schema.
  Descriptor v4 carries a validated semver compatibility range, bounded feature
  names, and typed declared capabilities.
- [x] Add dependency constraints by module slug and release range. Descriptor
  validation rejects invalid, duplicate, and self dependencies before the
  immutable dependency solver consumes them.
- [x] Add module kind, namespaced permission definitions, settings schema,
  runtime bindings, localization catalog, data contract, and UI contribution
  metadata. Descriptor v4 carries those declarative fields; validation rejects
  unowned permissions, undeclared binding/UI permissions, duplicate UI IDs,
  invalid localization digests, unsafe persistence metadata, and unknown
  descriptor fields.
- [x] Require bundled JSON Schema documents and forbid network/file `$ref`
  resolution during validation. Descriptor v4 bundles bounded Draft 2020-12
  documents under canonical SHA-256 digests; settings, data, persistence, and
  every binding input/output selector must resolve to that immutable bundle.
  Only in-document `#` references are accepted, and a document's declared
  digest must match its canonical JSON.
- [x] Add persistence/schema contribution metadata without executing any data or
  migration operation at descriptor parse/admission time. The descriptor stores
  only a revision and schema digest for host-brokered data.
- [x] Add UI metadata/artifact references without embedding executable UI logic
  in the server or host applications. Contributions are host-rendered metadata
  with a localization digest and declared module-owned permission.
- [x] Version the descriptor schema independently from module semantic version.
  Descriptor v4 rejects an unsupported schema version before admission.
- [x] Namespace artifact-defined permissions by module slug, reserve platform
  permission namespaces, and validate collisions before publication. Validation
  accepts only the descriptor slug prefix and rejects duplicate permission keys.
- [x] Register admitted permissions through the RBAC owner service with
  localized labels/descriptions; installation never grants them to roles or
  actors automatically. The shared registration contract and immutable
  localized descriptor metadata are in place: committed admission invokes an
  installation-idempotent port, and retries repeat registration without
  creating another installation. `RbacArtifactPermissionCatalog` persists the
  vocabulary separately from fixed built-in RBAC permissions and never writes
  `roles` or `role_permissions`; its owner migration is aggregated by the
  production `rustok-migrations::Migrator`. The RBAC-owned assignment service
  now records explicit, idempotent tenant-role grants/revocations in a separate
  relation, validates the exact installation plus platform-or-tenant catalog
  scope, and exposes exact tenant/user/installation/key authorization. The
  server admin transport requires `modules:manage` and derives tenant/actor
  identity from trusted request context. Artifact HTTP route composition remains
  pending; registration never grants access automatically.
- [x] Require every runtime/UI binding to name the exact permission it checks;
  capability grants authorize guest-to-host access and are not substitutes for
  actor RBAC.

### 3.2 Dependency Resolution and Lock Graph

- [x] Resolve semantic-version constraints with a maintained solver such as
  `pubgrub` behind a deterministic provider adapter; do not implement a naive
  recursive/backtracking resolver. The current adapter builds an immutable
  admitted-candidate snapshot before solving and writes selected versions and
  digests to the owner lock-graph contract.
- [x] Include platform/runtime ABI, module kind, yanked/revoked status, scope,
  trust policy, and active-release constraints in the provider. The immutable
  snapshot requires the exact deployment platform version and admitted
  descriptor compatibility range, rejects malformed platform facts, and
  filters trust, active/yanked/revoked status, scope, Optional artifact
  provider kind, runtime ABI, and platform compatibility before PubGrub.
- [x] Persist the complete selected graph with exact semantic versions,
  manifest/payload digests, and a graph revision/hash.
- [x] Produce stable human/machine conflict explanations without exposing
  library-specific types as the public API. The owner error returns a canonical
  `DEPENDENCY_CONFLICT` code, stable message, and sorted involved root slugs;
  PubGrub derivation diagnostics remain internal.
- [x] Resolve upgrades and rollbacks against a snapshot, then atomically switch
  the full graph revision; never partially upgrade dependencies. Every
  immutable installation stores its complete lock graph and revision; rollback
  selects the predecessor installation as one durable transaction rather than
  editing individual dependency selections.
- [x] Detect cycles and self-dependencies in the durable lock graph. The
  current graph validator also rejects duplicate and missing nodes.
- [x] Detect scope violations and attempts to replace Core/static-only
  providers in the resolution/selection service. Candidate scope must match
  the immutable request snapshot; artifact lock graphs reject Core and
  static-only providers rather than treating them as replaceable releases.

### 3.3 Platform Content-Addressed Artifact Store

`ArtifactRuntime` reads the verified digest-pinned payload from platform CAS
for every execution. The external OCI registry is an admission-time
distribution source only, so registry availability does not affect execution
of an admitted blob.

- [x] Introduce an `ArtifactBlobStore` port addressed only by verified digest.
- [x] Use `stage -> durable CAS publish -> DB transaction + outbox ->
  reconciler` for admission. PostgreSQL does not claim atomicity with external
  object storage; `StorageArtifactBlobStore` publishes digest-derived durable
  keys and `ArtifactAdmissionReconciler` removes orphans only after reference
  and retention-policy checks.
- [x] Commit admission metadata, dependency lock, installation/composition
  revision, and the existing transactional-outbox envelope in one database
  transaction; do not introduce a module-specific second event journal.
- [x] During admission, stream the selected payload into a platform-controlled
  CAS, verify digest/size while streaming, then atomically publish the blob and
  installation record. OCI preserves its bounded verified temporary file as an
  explicit payload source; the installer stages it through the durable CAS
  file path and removes it after admission.
- [x] Execute from the admitted CAS blob; external OCI is a distribution source,
  not the per-request runtime store.
- [x] Bound descriptor/config/layer size before allocation and support streaming
  reads rather than unbounded `Vec<u8>` downloads. The OCI adapter rejects
  oversized config and declared layer sizes before `pull_blob`, streams bytes
  through temporary storage with size and digest checks, then stages that file
  directly into platform CAS without a post-verification payload buffer.
- [x] Store verification evidence and blob metadata separately from executable
  bytes; do not copy large payloads into PostgreSQL. The admission record now
  persists the signer, policy revisions, required-check outcomes, and redacted
  evidence references alongside the CAS identity.
- [x] Define local/node caches keyed by digest with verified reads, atomic fill,
  corruption detection, and safe eviction. `VerifiedArtifactNodeCache` fills
  only from durable CAS, rehashes every hit, discards corrupt entries, and uses
  bounded LRU eviction; an oversized artifact is never cached.
- [x] Define reference counting/retention for active, rollback, quarantined,
  audit-retained, and unreferenced blobs. The reconciler first excludes every
  currently referenced digest, then evaluates a durable retention snapshot;
  legal hold, rollback protection, audit retention, or an unexpired deadline
  deny deletion.
- [x] Support execution during an external registry outage when the admitted
  blob is present; fail closed with `BlobNotFound` before sandbox execution
  when it is not. `ArtifactRuntime` has no registry client or fallback path.
- [x] Re-verification after trust-policy/root changes updates admission evidence
  and status through an expected-revision CAS without changing the immutable
  blob, descriptor, or CAS identity. Incomplete evidence moves the admission to
  `failed`.

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
- [x] Store capability grant revision separately from artifact declaration and
  policy revision. The owner supplies it explicitly when constructing the
  installer, and the admission transaction persists it with the installation.
- [x] Store migration/application checkpoint and irreversible migration flags.
  The owner records an object checkpoint through an expected-revision CAS in
  the scoped installation transaction and emits a revisioned transactional-outbox
  event without exposing checkpoint contents. Checkpoints are bounded to 16 KiB
  before the transaction begins. An irreversible-migration fact is monotonic
  and cannot be cleared by a later command.
- [x] Add optimistic revision and idempotency key. Lifecycle and selection
  transitions use expected-revision CAS. Immutable admission accepts an
  actor-scoped `ArtifactAdmissionCommand`; its canonical reference/scope/lock
  digest is durably reserved in the same transaction as installation,
  admission, and outbox state. A matching retry returns the original
  installation ID, while key reuse for a different command fails closed.

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
  installation; no local or legacy verifier exists as a fallback. The worker
  now executes fixed Cosign verification commands and fails closed unless its
  complete typed allow-list accepts the signed in-toto subject digest, SLSA
  builder/build type/source/ref, and CycloneDX JSON version, component-license, and
  vulnerability evidence. The worker listener requires deployment-provided
  mTLS identity/client-CA material and bounds concurrency, duration, and
  message size. Its same mTLS-protected listener exposes a readiness RPC only
  after fail-closed startup validation, so deployment supervision uses the
  authenticated transport rather than a plaintext health port. The transport
  supports mTLS client configuration and readiness probing. The mounted typed
  policy selects either keyless Sigstore identities/issuers or a
  first-party KMS key reference; neither mode falls back to the other.
  Fixture-backed tests cover accepted statements and denied digest, license,
  vulnerability, keyless-policy, and KMS-policy cases.
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
  secret-reference handles. The durable data owner provides bounded structured
  JSON values and a private object broker through a host-owned
  tenant/module/revision namespace with optimistic revisions and durable
  idempotency results. Object metadata exposes logical name, content type, size,
  digest, and revision only; its SeaORM/storage adapter generates and retains
  the physical key privately, derives the digest from accepted bytes, and
  re-hashes every private read before returning bytes. Secret references now
  have a separate owner-owned scoped binding table with revision CAS,
  idempotency, actor/reason audit data, and a redacted transactional-outbox fact;
  the injected `acquire_handle` broker returns only the logical handle and
  revision after per-execution host authorization. `platform.data.objects` now
  admits owner-scoped `get_metadata`, `read`, `put`, and `list` calls only under
  separately declared object-prefix/operation grants. Its JSON/base64 bridge is
  deliberately capped at 44 KiB of decoded bytes per call. Larger objects use
  durable owner-owned upload sessions with ordered chunks, final owner-side
  size/digest verification, expiry reaping, and retention-GC hand-off; a true
  streaming WIT protocol remains future work. The audit enforces canonical
  lowercase `sha256:` digests at the API and DDL boundaries, rejects
  non-canonical content types, and keys upload idempotency by immutable policy
  scope. The owner enforces the 32 MiB object quota across the entire durable
  chunk set before storing each chunk. Completion explicitly claims the upload before publication and the
  reaper atomically abandons only expired open/completing sessions before it
  queues chunks, so the two paths cannot publish or collect the same session
  concurrently. The immutable persistence contract now admits at most sixteen
  named scalar indexes with a narrow logical JSON pointer and declared scalar
  type; it never admits a physical index, database JSON path, or query
  expression. The owner computes canonical scalar projections in Rust and
  persists them in a separate tenant-RLS table within the same write/purge
  transaction. The first indexed write binds a namespace to the exact
  declaration digest; indexed reads validate that binding without mutating the
  namespace. Changing that declaration requires a new data-contract
  revision and owner-mediated upgrade. A legacy namespace that contains data
  but has no index-contract binding fails closed rather than returning a
  partial indexed result. `platform.data.query_index` requires its own typed grant
  operation and an exact granted logical-key prefix; it accepts only one
  declared index, scalar equality, and bounded keyset pagination. It cannot
  express sorting, ranges, joins, offsets, or query plans.
  Value-consuming secret-use remains pending.
- [ ] Scope every operation by tenant, module slug, data-contract revision, and
  policy; the guest cannot choose a physical schema/table/bucket path. The
  structured-data validator is host-constructed with the immutable installation
  ID, so it resolves only that RLS-scoped admitted descriptor and persistence
  revision, never a latest-release lookup; the ID never crosses the artifact
  capability boundary. The structured-value adapter requires a host-owned
  authorizer for every logical read/write and a separate destructive-purge
  authorizer. The object adapter applies the same immutable scope and per-object
  authorization while hiding storage keys. Authorized namespace purge removes
  object metadata and transactionally queues now-unreferenced private bytes for
  retention/GC. The tenant-scoped GC owner deletes only queued keys approved by
  an explicit snapshot rule after legal-hold, audit-hold, rollback-hold, and
  expiry checks; a missing rule fails closed. The remaining broker capability
  kinds still need the same boundary.
- [ ] Validate data/settings/action payloads with bundled JSON Schema using the
  maintained `jsonschema` validator and bounded regular-expression settings.
  Structured-value writes now require a host-owned schema-validation port before
  persistence. `SeaOrmArtifactDataSchemaValidator` resolves the exact admitted
  installation descriptor and persistence schema under tenant RLS, then uses
  Draft 2020-12 with strict formats and bounded regular expressions. Settings
  and action payload adapters remain unfinished.
- [ ] Define quotas, pagination, transactions/batches, optimistic revisions,
  idempotency, backup/export, retention, and deletion semantics.
  Structured-value writes currently have a 256-byte logical-key bound, a
  64 KiB JSON-payload bound, per-record optimistic revisions, and durable
  idempotency. Their namespace lifecycle serializes writes against explicit
  purge, retains a tombstone after purge, and requires a host authorization
  port for lifecycle/retention/legal-hold checks before the audited outbox
  operation. Authorized keyset pagination is bounded to 100 records and uses
  only a logical-key continuation. `put_batch` accepts at most 32 distinct
  logical keys and idempotency keys, validates every schema and authorization
  decision before opening its transaction, and commits all writes and their
  durable idempotency facts atomically. Object overwrite and authorized purge
  queue replaced/unreachable private keys for retention-guarded collection.
  The owner now also provides an audited bounded export page: a separate host
  authorizer, active namespace revision CAS, lifecycle lock, audit row, and
  redacted outbox fact protect each export. It never appears as a sandbox
  capability or returns physical storage identity. This is intentionally a
  keyset page rather than a cross-page backup snapshot; durable snapshot/restore
  export and the remaining capability types are still pending.
- [ ] Keep secret values outside settings and module data; store only brokered
  secret references. The secret-binding store persists only a host-authorized
  resolver reference in its separate owner table; structured data, sandbox
  inputs, artifact handles, and outbox evidence never include a secret value.
  The host handle-acquisition broker exposes only logical name and revision;
  value-consuming secret use remains unfinished.
- [x] Define data-contract upgrade hooks that transform through bounded sandbox
  commands without holding control-plane transactions. Descriptor v4 reserves
  the dedicated `data_upgrade` binding kind, unavailable through the generic
  dispatcher. Its owner bridge invokes only that admitted binding through the
  existing artifact executor after one validated keyset read, validates each
  transformed value against the higher target contract, and returns only a
  non-durable plan with source revisions. `ArtifactDataUpgradeApplier` then
  rechecks source revisions, uses create-only target writes with deterministic
  per-record idempotency derived from the owner plan ID, and records a redacted
  installation checkpoint through the existing revision CAS/outbox path only
  after the page completes. It holds no control-plane transaction across the
  page. Uncertain-outcome recovery, distributed rollout, rollback, and
  quarantine policies remain pending.
- [x] Before allowing declarative DDL migrations, create a separate ADR and
  threat model covering allowed operations, schema isolation, locks, rollback,
  backup, cross-module references, tenant rollout, and failure recovery.
  [`2026-07-18-artifact-declarative-ddl-boundary`](../../DECISIONS/2026-07-18-artifact-declarative-ddl-boundary.md)
  keeps v1 declarative DDL prohibited and records the required future admission
  conditions without creating a descriptor escape hatch.
- [x] Static-promoted modules continue to use reviewed module-owned
  `MigrationSource` migrations in distribution builds. The shared migrator
  aggregates only declared module migration sources and their dependency
  descriptors; dynamic artifact descriptors expose no migration path.

### 3.7 Rollback, Uninstall, and Purge

The owner boundary is fixed by the [module artifact rollback ADR](../../DECISIONS/2026-07-13-module-artifact-rollback-boundary.md): an explicit CAS-revision command selects the durable predecessor, re-evaluates grants, audits actor/reason, and writes an outbox event in one transaction. Runtime activation and tenant enablement remain downstream operations.

- [x] Rollback selects a previously admitted immutable release; it never edits
  the failed release.
- [x] Capability grants are re-evaluated for the target release.
- [x] Data migrations declare whether rollback is reversible, compensating, or
  prohibited. A recorded irreversible checkpoint accepts only an explicit
  compensating rollback; prohibited rollback is rejected before state changes.
- [x] Runtime activation and tenant enablement rollback remain distinct. The
  artifact rollback command changes only durable selection/admission state;
  tenant toggles and lifecycle hooks use the separate lifecycle owner path.
- [x] Every rollback is a new audited operation with actor and reason.
- [x] Define disable, deactivate, uninstall, and purge as distinct operations:
  - disable preserves installation and data;
  - deactivate removes runtime bindings but preserves admitted release/rollback;
  - uninstall removes the scope's selection after dependent checks;
  - purge deletes retained module data only through an explicit destructive,
    authorized, audited operation.
- [x] Artifact deactivation is an owner-owned, revision-guarded and idempotent
  binding removal. It requires an active installation, rejects an active direct
  dependent in the same scope, transitions the admission to `inactive`, and
  writes its audit/outbox fact atomically without deleting CAS, data, or
  rollback evidence. A replay must match the full immutable command
  (installation, revision, actor, and reason), never merely an idempotency key.
- [x] Artifact tenant enablement and disable preserve installation and data.
  They write only tenant intent in a separate scoped lifecycle record through
  expected-revision CAS, record actor/reason/idempotency metadata, and publish
  a revisioned transactional-outbox event without changing admission or runtime
  bindings. They accept only an admitted Optional artifact visible in the
  requesting tenant scope. Their durable lifecycle row also records the
  command's expected revision and requested enabled state, making replays fail
  closed unless actor, reason, revision, state, and key all match.
- [x] Artifact uninstall is an owner-owned, revision-guarded and idempotent
  scope-selection removal. It requires an inactive installation, rejects an
  active direct dependent in the same scope, writes audit/outbox atomically,
  and only releases the CAS reference; it does not purge retained data or
  evidence. Its replay contract likewise matches the complete immutable
  command rather than accepting a reused key alone.
- [x] Structured artifact data purge is a separate destructive operation. It
  is tenant/module/data-contract scoped, revision-guarded and idempotent,
  serializes against data writes, records actor/reason and the deleted-record
  count, emits a transactional-outbox fact, and leaves a durable namespace
  tombstone. A host-owned authorizer must approve lifecycle, retention, and
  legal-hold policy before the operation begins.
- [x] Uninstall never silently deletes tenant data, logs, evidence, or rollback
  artifacts. It removes only the scoped selection and its CAS reference;
  retention, legal-hold, audit, and rollback policy remain responsible for any
  later reclamation.
- [x] Garbage collection runs only after reference, retention, legal-hold,
  rollback, and audit checks.

#### Implemented Atomic Work Package: Owner-Owned Artifact Uninstall

`apps/server` still has a static-manifest uninstall flow only; it is not an
artifact uninstall path and must not be reused for marketplace artifacts. The
owner command requires an inactive selection, a scope-bound expected revision,
actor/reason/idempotency metadata, and an absence of active direct dependents.
Its transaction records the uninstall audit fact, removes the installation's
CAS reference, and emits an outbox event. It does not delete CAS bytes, tenant
data, evidence, logs, or rollback history; the reconciler may reclaim an
unreferenced blob only after retention and legal-hold policy permits it. Purge
remains a separate destructive command.

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

- [x] Keep build request/result/domain orchestration in `rustok-modules`.
  The immutable request/result protocol and a tenant-RLS durable submission
  queue now live there. Submission is tenant/project/idempotent, writes
  `module.build.queued` through the transactional outbox, and cannot invoke a
  worker inline. Terminal results must correlate to the immutable request under
  the same tenant RLS; their idempotent persistence writes
  `module.build.completed` through the transactional outbox. The dedicated
  `rustok-module-build-transport` crate now maps this owner port onto versioned
  mTLS gRPC with authenticated readiness and no in-process fallback.
  The owner also exposes `load_queued`/`dispatch_queued` for a dedicated
  outbox-consumer host: it releases tenant-scoped database state before the
  remote call and persists only an immutable validated result. The external
  production event-consumer host wiring, worker deployment, and later
  release-governance completion remain unfinished.
- [x] Define the worker protocol before creating another crate or service.
  `ModuleBuildRequest` and `ModuleBuildResult` bind source/dependency/toolchain/
  WIT evidence, bounded resources, network policy, validation profiles, and
  canonical terminal outcomes. `ModuleBuildWorker` is a transport port only;
  it does not permit a server or runtime implementation to invoke Cargo. The
  v1 result derives toolchain and WIT digests from domain-separated immutable
  request fields, so a worker result cannot substitute a different contract.
  Its retryability bit must exactly match the `retry_build` next action.
- [x] Initially implement the worker as a separately deployable binary/process;
  split a package only when the protocol and operational lifecycle justify it.
  The transport boundary is fixed by
  [`2026-07-16-module-build-worker-transport`](../../DECISIONS/2026-07-16-module-build-worker-transport.md):
  it serializes only the owner-owned request/result protocol, requires mTLS in
  production, and exposes readiness on the same authenticated listener.
  `rustok-module-build-worker` now provides the separately deployable process:
  it invokes only a fixed image-owned non-symlink OCI job launcher in a fixed
  workdir with a cleared environment, request-derived deadline, and aggregate
  streamed stdout/stderr output limit. Startup requires a gVisor or Kata job
  runtime plus a digest-pinned OCI job image; the launcher receives those fixed
  identities and must create the corresponding isolated OCI job. Its v1 source contract is a digest-addressed
  `cas://sha256/<hex>` archive from a deployment-mounted read-only root; the
  worker rehashes and safely materializes it into a request-scoped directory
  before the runner starts. Source digest, archive-safety, and extraction-limit
  violations become terminal owner-validated build results rather than
  retryable broker transport failures. The delivery host must consume the
  outbox-published event through a real external broker consumer group, perform
  mTLS delivery, and persist the result through the owner without sharing the
  worker process or competing with the global outbox relay.
  `rustok-module-build-dispatcher` owns the broker-neutral process-and-ack
  contract and its Iggy adapter. The adapter uses a dedicated `module-build`
  topic and one persistent remote consumer-group cursor; it commits the exact
  Iggy offset only after owner-side result persistence. Before dispatch it
  validates the broker envelope identities, event type/schema metadata,
  queued-event payload, and tenant equality; malformed or cross-tenant
  messages fail closed without an acknowledgement. Its separately
  deployable binary owns only the database owner adapter, Iggy credentials, and
  mTLS worker client; the external Iggy transport requires an explicit TLS=true
  deployment setting and has no plaintext downgrade. It validates worker
  readiness before consuming. Broker
  topology provisioning and deployment configuration remain explicit
  operational prerequisites; neither has a server-local fallback. A processing,
  acknowledgement, or broker-receive failure terminates the dispatcher without
  committing its outstanding offset; deployment supervision restarts it with
  bounded backoff so the persistent cursor redelivers rather than leaving a
  pending message stuck in process memory. The worker
  now implements source materialization, policy/metadata checks, verified
  Component/WIT/evidence inspection, scoped dependency materialization, and
  scoped OCI publication. The publication path uses a short-lived
  repository-scoped lease with a bounded publication/signing window, clears the
  Cosign environment, validates its deployment-owned target at worker
  construction, rechecks the lease before OCI publication, and records a
  digest-pinned signature-manifest receipt.
  Deployment evidence that the launcher creates the hardened job, and later
  release-governance admission, remain unfinished.
  `rustok-build` remains a reviewed static platform-release composition service
  used only by installer/CLI operations; its server background executor has
  been removed. It has no path from `module.build.queued` and is not an
  implementation of `ModuleBuildWorker`. It must not be repurposed as a
  server-local fallback for untrusted module builds. No server-local fallback
  or dual module-build path is permitted. The static worker-isolation verifier
  also rejects module-build worker/transport dependencies and direct delivery
  symbols in `apps/server`; it requires the dedicated dispatcher to use the
  mTLS remote worker and readiness check without a worker-crate dependency, and
  requires fixed Cosign execution to clear its environment and receive only the
  private Docker configuration.
- [ ] Run builds as isolated OCI jobs. Production untrusted builds use a
  hardened runtime such as gVisor or Kata where available. The worker now
  requires an explicit fixed OCI job launcher and `gvisor` or `kata` runtime,
  and readiness probes the worker-owned launcher/runtime configuration rather
  than returning an unconditional success. Every launched job must also emit a
  bounded immutable-request-matching OCI receipt, including its opaque job ID,
  fixed image digest, build attempt, dependency-lock digest, toolchain digest,
  WIT digest, and a domain-separated digest of the exact request JSON delivered
  to the launcher, before the worker accepts its terminal result. Startup and
  readiness also require the deployment-owned
  `RUSTOK_MODULE_BUILD_ISOLATION_ATTESTATION` file: a bounded, regular JSON
  attestation must match the fixed runtime/image and prove unprivileged,
  host-mount-free, socket-free, host-network/PID-isolated, resource-limited,
  ephemeral-job settings. This is configuration-review evidence and does not
  replace deployment evidence that the launcher enforces the corresponding
  controls.
  Deployment evidence that the launcher actually creates the hardened job
  remains required before this item can close.
- [ ] The worker has no tenant database access and no general platform secrets.
  `verify-module-build-worker-isolation.mjs` rejects direct tenant-database,
  platform-storage, and general-secret dependencies or APIs in the worker crate
  and verifies that the untrusted runner is environment-cleared without
  database or credential forwarding. The worker also fails closed without the
  bounded isolation attestation, while deployment isolation evidence remains
  required before this item can close.

### 4.2 Build Request Contract

The immutable request contains:

- request, tenant/project, actor, and correlation IDs;
- source artifact reference and source digest;
- expected module slug and version;
- target runtime ABI and WIT world/version;
- pinned Rust toolchain and component target;
- independently versioned author SDK and template inputs;
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
3. Bind the raw `Cargo.lock` bytes to the request lock digest and reject
   source-local or ancestor-workdir Cargo config, patches/replacements, path dependencies,
   unapproved registry sources, forbidden Git sources, build scripts, and
   native links before starting the runner. The worker now implements this
   fail-closed preflight, including bounded resolved-lock graph inspection,
   registry checksums, and pinned Git revisions. The worker now also runs the
   fixed image-owned Cargo executable as `cargo metadata --locked --offline`
   against a trusted deployment-materialized cache, then verifies the returned
   package/source graph, custom-build/native-link facts, workspace paths, and
   resolve-node closure under request deadline/output limits. Scoped egress now
   invokes only a fixed materializer adapter for a separately isolated OCI
   network sandbox. Its receipt must bind the source, raw lock digest, and the
   exact ordered endpoint list; its fresh Cargo home is checked for symlinks and
   Cargo config or credentials before worker Cargo remains forced offline. The
   fixed OCI job receives only that verified cache, a fixed Cargo executable,
   request-scoped home/target/output paths, and `CARGO_NET_OFFLINE=true`; it
   cannot inherit worker credentials or use a deployment Cargo configuration.
4. Inspect the graph using `cargo metadata`/`cargo_metadata`.
5. Reject disallowed sources, Git revisions, build scripts, native links,
   unsafe policy violations, or dependency limits according to policy.
6. Run `cargo deny`, advisory checks, and `cargo vet` policy where configured.
7. Format/check/lint/test using pinned commands and locked dependencies.
8. Build the component with `cargo component build --locked`.
9. Validate and inspect exports/imports using `wasm-tools`. The worker now
   additionally binds a successful result to a fixed `output/component.wasm`,
   rehashes it, validates Component Model bytes, and compares the root
   imports/exports with runner evidence. A deployment-owned `wasm-tools` stage
   extracts WIT from that same fixed payload; the worker parses it and requires
   the requested package, world, version, and complete import/export surface to
   match exactly.
10. Require the configured WIT world and reject undeclared imports. This is
    enforced from Component-derived WIT rather than runner-provided text.
11. Generate CycloneDX SBOM. The worker now requires and rehashes a fixed
    `output/sbom.cdx.json` file before it accepts success, and checks bounded
    CycloneDX JSON structure.
12. Produce provenance containing source, toolchain, command, dependency, SDK,
    template, WIT, and output digests. The worker now requires and rehashes fixed
  `output/provenance.intoto.json` SLSA in-toto JSON with a component-digest
  subject and a RusToK external-parameters envelope binding source, lock,
  toolchain, and WIT digests plus exact independently versioned SDK/template
  inputs, expected module slug/version, runtime ABI, build attempt, and exact
  requested validation-profile list.
13. Emit payload, SBOM, provenance, logs, metrics, and structured result to the
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
- [x] Provide a local sandbox harness with the same request/policy/error contract
  and fixture capability broker as production, but no production credentials.
  `LocalSandboxHarness` delegates directly to `SandboxRuntime`; its
  `FixtureCapabilityBroker` resolves only exact caller-provided deterministic
  responses and default-denies every unregistered fixture. The harness has no
  deployment configuration or infrastructure clients.
- [x] Emit machine-readable diagnostics and build evidence usable by Alloy,
  CLI, CI, and admin without parsing human logs. `ModuleBuildResult` protocol
  v7 carries bounded typed diagnostic `(stage, code)` facts and ordered
  validation-profile outcomes in its evidence;
  every failed result must include its canonical code at its owner-canonical
  stage, while success
  cannot include failure diagnostics. The worker synthesizes those facts from
  its owner failure taxonomy and retains raw runner output only behind the
  separately authorized log reference. A successful result must report every
  requested profile as `passed`; `validation_failed` must identify an ordered
  requested profile with a `failed` outcome. The verified SLSA provenance
  envelope carries the same requested-profile and outcome lists.
- [x] Version SDK/templates independently and record their versions in build
  provenance. `ModuleBuildRequest` v7 requires SemVer `sdk_version` and
  `template_version`; publication-side SLSA verification requires exact
  `sdkVersion` and `templateVersion` values in the request-bound RusToK
  external-parameters envelope.

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

- [x] Freeze media types for descriptor/config, Rhai source, WASM Component,
  sidecar metadata, static-promotion source reference, SBOM, provenance, test
  evidence, and release lineage. `rustok-modules` now exposes stable v1 media
  types for descriptor config, every payload kind, and the four evidence
  referrers. The OCI reader rejects a config media type, declared config size,
  or raw config digest that does not match this contract and accepts exactly one
  descriptor-selected executable layer. `OciDistributionArtifactPublisher`
  now emits the descriptor-configured executable layer and OCI 1.1 SBOM and
  provenance referrer manifests, each with an exact subject descriptor.
- [x] Publish by content digest; tags point to immutable releases but are never
  accepted as installation identity. The current adapter derives deterministic
  write tags only to satisfy registry mutation APIs, immediately resolves the
  registry manifest digest, verifies it against the raw bytes, and returns only
  digest-pinned receipts. The worker now supplies only fixed inspected output
  files and carries that receipt in its terminal result; owner persistence
  rejects successful results without it. Release-governance promotion remains
  separate work.
- [x] Attach SBOM/provenance/signature evidence using OCI referrers or a
  documented compatible layout. The adapter uploads bounded verified SBOM and
  provenance as OCI 1.1 subject referrers. After publication, the isolated
  build worker signs only the returned digest-pinned artifact through Cosign
  with a deployment-owned KMS URI, resolves Cosign's standard compatible OCI
  signature manifest to its digest, and returns that digest-pinned identity in
  the immutable result. The lookup tag is never installation identity.
- [x] Ensure exactly one executable layer matches descriptor payload kind and
  digest. `OciDistributionArtifactRegistry` rejects a manifest unless exactly
  one layer has both the descriptor payload digest and its frozen payload media
  type before it streams and rehashes that layer into staging.
- [x] Use short-lived, least-privilege registry credentials through the host
  secret/provider boundary; credentials never enter descriptors, build inputs,
  logs, Alloy tools, or sandbox requests. The build worker now invokes only a
  fixed deployment-owned credential broker for its configured repository. Its
  bounded v1 response must match that registry/repository and remain valid for
  the complete bounded publication/signing window (at most 15 minutes). The
  credential is retained only in worker memory for OCI and a private temporary
  Cosign Docker configuration, then removed; the worker no longer reads direct
  registry username/password environment variables.
- [ ] Define registry redirect, cross-host auth, TLS, proxy, timeout, retry,
  maximum-size, and decompression policies explicitly. The enforced client
  subset now uses the typed `OciRegistryTransportPolicy`: HTTPS only, verified
  TLS, redirects and cross-host authentication disabled, deployment-boundary
  proxy mode, bounded request/retry/transfer/decompression ceilings, disabled
  platform resolver, and serialized uploads/downloads. A weaker policy is
  rejected before client construction and the policy is the only public
  construction path for the distribution reader and publisher. Registry
  transport source is covered by
  `verify-oci-registry-transport-policy.mjs`; deployment egress still must
  provide the corresponding redirect/proxy/retry/decompression enforcement.
  references are host/repository/digest identities rather than URLs, and the
  publisher receives credentials only after the worker has obtained a
  repository-bound lease. The current `oci-distribution` client exposes no
  redirect, proxy, retry, per-request timeout, or decompression hooks, so the
  deployment egress boundary must deny redirects and cross-host credential
  forwarding, restrict proxy use to that boundary, enforce connection and
  request deadlines, and apply retry and transfer/decompression ceilings. The
  adapter independently bounds complete descriptor/layer admission to five
  minutes, streams config only after its declared descriptor-size check, and
  cancellation-safely deletes a partial staging file. Config and payload streams
  reject bytes beyond their OCI-declared size before extending memory or disk
  staging, and require an exact final size before parsing or digest acceptance.
  The current client still buffers manifests, so manifest and transfer ceilings
  remain egress controls. The adapter also cancels a complete artifact-and-referrer
  publication after ten minutes, leaving bounded time for Cosign inside the
  worker's separate 15-minute credential window. This item remains open until
  the remaining egress controls are configured and verified.

### 5.2 Signing

- [x] Use `cosign`/Sigstore-compatible signing rather than custom cryptography.
  The build worker executes only an absolute, image-owned Cosign binary with an
  approved KMS provider URI after artifact publication, suppresses command
  output, and removes `COSIGN_REPOSITORY` before invocation. It retains only
  the resolved signature-manifest digest in the build result; raw keys and
  signing credentials never enter request data, descriptors, runner output, or
  logs.
- [x] Define accepted trust roots, signer identities, certificate constraints,
  transparency-log policy, offline verification behavior, and key rotation.
  The isolated verifier has keyless-Sigstore and KMS root modes, identity/OIDC
  allow-lists, optional transparency-bundle offline verification, policy
  revisions, and fail-closed SLSA/CycloneDX allow-lists. `VerificationTrustRoots`
  requires one active root and permits only one explicit retiring root, which is
  evaluated with the same mode-specific checks strictly before its configured
  Unix-second expiry; it is never an unbounded fallback.
- [x] Separate author signature, build-service attestation, marketplace approval,
  and platform admission decisions. `ModuleBuildPublicationReceipt` v6 now
  records only `build_service` as its signature authority; author signature and
  marketplace approval remain independent governance evidence and admission
  continues to require its separate trust decision. The owner now persists an
  append-only `registry_publication_evidence` ledger keyed by exact subject
  digest and one of `author_signature`, `build_service_attestation`,
  `marketplace_approval`, or `platform_admission`; repeat submission of the
  same fact is idempotent through a domain-separated evidence digest and a
  database uniqueness constraint. Promotion/admission must still require the
  applicable distinct facts before this item can close. Marketplace approval
  is not accepted through the generic evidence command: the owner creates it
  only in the atomic final-publication transaction, bound to the canonical
  staged artifact SHA-256 and the approving principal. Build-service
  attestation is also reserved: only `ModuleBuildServiceAttestationCommand`
  can record it, after validating the complete `ModuleBuildPublicationReceipt`,
  its `build_service` authority, and its co-located digest-pinned OCI
  payload/SBOM/provenance/signature identities. Platform admission is reserved
  too: `ModulePlatformAdmissionCommand` accepts only an admitted immutable
  verification decision for the exact OCI manifest, binds signature/SLSA/SBOM
  outcomes, signer, policy revisions, and evidence-reference fingerprint, and
  records the platform decision without exposing verifier output. The owner
  now fails publication closed unless author-signature evidence is bound to the
  staged artifact SHA-256 and build-service attestation plus platform-admission
  evidence share one exact OCI manifest subject; marketplace approval is added
  only inside that same final-release transaction. A reupload invalidates prior
  evidence for promotion: the owner accepts only facts recorded after the
  current staged-artifact timestamp.
- [x] Do not equate a valid signature with a trusted module; policy must verify
  who signed what under which build/provenance conditions. Admission accepts a
  decision only when its exact policy revisions match and signature, SLSA
  provenance, and CycloneDX SBOM verification all succeed. The verifier also
  requires a configured signer identity plus builder, build-type, source,
  license, and vulnerability-policy facts.

### 5.3 Publication Governance

- [x] Stage release from an immutable source/build result. The prerequisite
  owner read is now `SeaOrmModuleBuildService::load_completed`: it returns only
  a tenant-RLS-scoped durable request/result pair after revalidating the result
  against its immutable stored request. `stage_platform_build` now consumes the
  pair, validates the expected slug/version and component digest against the
  submitted artifact, and appends the source, component, and OCI receipt
  identities in `registry_publish_build_staging`. Publication requires a stage
  newer than the current upload. `artifact_origin` is now explicit and legacy
  rows are `unclassified`, which fails closed. External prebuilts use the
  separate immutable `registry_publish_external_staging` record with either a
  reproducible source identity or an explicit absence reason, an approved
  provenance-policy revision, and an independent quarantine review. The final
  owner transaction requires the current origin-specific stage. The server now
  exposes an operator-only external-prebuilt staging adapter that derives both
  actor and quarantine approver from `modules.manage` authority. The parallel
  platform build-stage adapter derives the tenant exclusively from the
  authenticated session, requires `modules.manage`, and passes only a completed
  build ID plus idempotency key to the owner RLS reload. Both staging paths
  persist and compare their full authenticated immutable command fingerprints
  on replay: platform builds include tenant, build, source, component, and
  actor; external prebuilts include source/provenance/quarantine facts and
  both authenticated principals. Any conflicting reuse fails closed.
- [ ] Run automated descriptor, compatibility, dependency, signature, SBOM,
  provenance, license, vulnerability, and sandbox smoke checks.
- [x] Record review decisions, required changes, holds, approvals, rejections,
  yanks, and reasons as owner events. `SeaOrmModuleGovernanceService` writes
  the transition and its reason in the same owner transaction. It also records
  an append-only `publication_evidence_recorded` audit event for every
  authority-scoped immutable publication fact, without treating the stored
  reference contents as trusted display or prompt content.
- [x] Publish creates a release once; retry resumes idempotent stages instead of
  duplicating a release. The owner locks an approved request during finalization
  on PostgreSQL. The live approval transport requires a non-nil external UUID
  key, and the owner persists its complete command fingerprint with the release.
  A replay of a terminal `published` request succeeds only for that exact
  fingerprint and durable release; a missing legacy record or conflicting key
  reuse fails closed without inserting another release, marketplace-approval
  fact, or audit event.
- [x] Yanking prevents new resolution but does not mutate existing installed
  artifact identity. The owner changes only the release lifecycle to `yanked`
  and records the reason/audit fact; the immutable resolver snapshot excludes
  yanked releases while storage key, checksum, and size remain unchanged.
- [x] Distinguish platform-built and externally-built artifacts. The owner now
  persists immutable origin on both requests and releases and fails closed for
  `unclassified` history. Platform-built releases require the current build
  stage plus build-service and platform-admission facts for that stage's exact
  OCI manifest. External prebuilts
  require a current external stage with an approved provenance-policy revision,
  quarantine review, explicit source/reproducibility classification, author
  signature, and platform admission whose verified payload digest matches that
  stage; they cannot claim a build-worker
  attestation. The server transport accepts only evidence fields and an
  idempotency key, deriving the actor and quarantine approver from authenticated
  `modules.manage` authority. The parallel platform build-stage adapter accepts
  no caller-supplied tenant identifier and derives its owner RLS scope from the
  authenticated session.
- [ ] Treat marketplace README, metadata, source comments, test output, and
  artifact text as untrusted content for both UI rendering and AI prompts. The
  registry bundle validator caps the complete upload at 2 MiB before JSON
  parsing, bounds embedded manifest parsing, and emits content-free diagnostics,
  preventing raw artifact/request strings from flowing into governance events
  through that path. Rendering and prompt-boundary policy remain to be completed
  across all publication inputs.

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
  `AlloyDraftRequestBuilder` already carries draft ID and source revision into
  `SandboxSubject::AlloyDraft`; tenant-scoped Alloy storage now also prevents
  cross-tenant single-script reads and mutations. Single-script persistence now
  uses the stored version as a durable CAS predicate, advances it for every
  storage mutation, and rejects stale saves with `RevisionConflict`. Durable
  source-revision rows record a bounded canonical workspace, digest, author,
  and parent lineage in the
  same transaction, including a baseline row when a pre-ledger draft first
  changes. Owner storage exposes the immutable evidence through tenant-scoped
  `(script_id, revision)` lookup and revision-ascending history, rather than
  direct ledger queries. REST and GraphQL draft updates require the caller's
  expected revision before mutation. REST and GraphQL manual runs also require
  that revision and execute the selected snapshot without a second lookup. The
  canonical JSON workspace now replaces the single `code: String` model. It
  is persisted and revisioned with bounded file/path/content limits, carried as
  immutable sandbox payload bytes, and decoded only by an Alloy extension to
  select the entry source; no guest filesystem is mounted. Rhai imports resolve
  only from exact in-memory `src/*.rhai` workspace paths, through a
  request-private static resolver assembled in dependency order with cycle and
  depth rejection. Durable owner review decisions now bind the exact source
  digest, expected revision, policy revision, actor, reason, and idempotency
  fingerprint. GraphQL and host HTTP review/history transports require the
  verified `scripts.manage` actor. Workspace tests now select only a declared
  immutable `tests/*.rhai` entrypoint from the same canonical workspace digest
  and revision, resolve imports through the bounded in-memory source resolver,
  run without capability grants, reject entity changes, and return a boolean
  result. Test commands now reserve a durable revision-pinned source digest,
  test path, verified actor, and request fingerprint before sandbox execution;
  exact replays return terminal evidence, concurrent callers see a bounded
  pending lease, and only an expired lease can be reclaimed against the same
  immutable source snapshot. Host HTTP and GraphQL derive `scripts.manage`
  authority from authentication. Build-command idempotency remains pending.
  Alloy release staging now selects the current immutable source revision and
  latest approved review, then delegates an idempotent `alloy_authored` stage
  to `rustok-modules`. The owner records source/review evidence together with
  the Alloy tenant/script identity and remains the only marketplace writer.
  Final promotion also requires matching platform admission for the attached
  artifact. Origin-aware owner upload and the isolated validation worker now
  accept only a bounded canonical Alloy workspace, and release staging requires
  its checksum to equal the reviewed source digest. Authenticated HTTP and
  GraphQL release-stage adapters now derive the actor from auth, require the
  current revision and module authority, verify that the authenticated tenant
  matches the resolved request tenant on both transports, and delegate
  idempotent staging to the owner service with typed conflict/not-found
  transport outcomes; final marketplace promotion remains an owner governance
  operation. The canonical Rhai workspace
  payload media type is retained by admission and runtime resolution, so a
  multi-file release cannot be reinterpreted as a single-source artifact after
  publication.
- Alloy lifecycle status mutations now require the expected revision on both
  REST activate/pause and GraphQL activate/pause/disable/archive/reset-errors
  transports, so stale status writes fail closed with a revision conflict.
- Alloy deletion now also requires the expected revision on direct REST,
  host-composed REST, and GraphQL transports; owner storage applies the same
  version predicate atomically before removing the script.
- The owner-owned MCP delete tool carries the same expected revision and uses
  the owner CAS path, so management transport cannot bypass stale-write guards.
- MCP Alloy create/update/run tools now use the canonical workspace contract;
  update and manual execution require expected revisions and pin the loaded
  workspace snapshot.
- [ ] Reject execution/publish commands for stale revisions.
- [ ] Execute validation, tests, manual runs, hooks, schedules, and preview
  scenarios through `SandboxRuntime`.
- [ ] Convert Alloy entity/parameter behavior into explicit request-scoped
  bindings without adding generic Alloy concepts to `rustok-sandbox`.
- [ ] Persist execution evidence linked to revision and policy revision.
- [x] Replace the former single `code: String` model with a revisioned workspace
  contract for sources, imports/modules, tests, fixtures, schemas, policy, and
  generated artifacts. DB/object storage remains the source of truth; guests do
  not receive direct filesystem access.
- [x] Resolve Rhai imports through an Alloy-owned bounded static in-memory
  resolver keyed by the request workspace/revision, with cycle, depth, size,
  and path validation.

### 6.2 Release Creation

- [x] Stage and package immutable Rhai descriptors with source digest/lineage
  and preserve the exact admitted workspace media type through runtime
  resolution.
- [ ] Validate declared capabilities from observed/declared tool use.
- [x] Complete release source/descriptor publication through `rustok-modules`;
  Alloy does not write marketplace tables. The revision-pinned reviewed-source
  staging gate, origin-aware owner artifact upload, and authenticated HTTP /
  GraphQL staging adapters are complete; final marketplace promotion remains
  an owner governance operation.
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
