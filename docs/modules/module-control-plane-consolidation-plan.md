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
- the isolated module build worker.

Local component plans describe their own implementation details. If a local
plan conflicts with this document, update both documents in the same change and
resolve the conflict in favor of the accepted architecture decisions.

The ownership decision is fixed by
[`DECISIONS/2026-07-11-neutral-sandbox-foundation.md`](../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md).
Production update, direct-predecessor recovery, finalization, and destructive
data-action semantics are additionally governed by
[Module release rollback safety](../../DECISIONS/2026-08-06-module-release-rollback-safety.md)
and its [implementation plan](module-release-rollback-plan.md). The accepted
release-safety contract supersedes the older generic rollback/purge wording
below wherever they differ.

## Execution Checkpoint

- Current phase: `sandbox_worker_and_build_closure`.
- Last updated: 2026-08-24.
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
  1. complete isolated sandbox-worker profiles and deterministic resource
     metrics;
  2. close retained isolated OCI build-job enforcement and supervisor evidence;
  3. close the remaining registry transport boundary evidence;
  4. implement Alloy authoring/release evolution on the admitted runtime;
  5. complete GraphQL/native/admin marketplace parity;
  6. finish desired-state activation reconciliation and operational gates.
- Targeted verification is current through the dirty-worktree boundary:
  `rustok-forum`, `rustok-pricing`, `rustok-commerce`, `rustok-groups`,
  `rustok-server --no-default-features`, and
  `rustok-admin --no-default-features --features ssr` all pass `cargo check`
  (warnings only). No workspace-wide compile or test claim is made.
- Open architecture blockers: none.
- Latest platform-composition scope evidence: the global `platform_state`
  owner now rejects tenant-scoped commands, reserves exact retries in the
  shared platform receipt namespace, and retains full command evidence. Its
  GraphQL install/uninstall/upgrade transport requires a direct SuperAdmin
  plus `modules:manage`, while the post-commit platform build notification
  retains the same actor, correlation, and trace fields. `rustfmt --edition
  2024`, `git diff --check`, and the module-control-plane owner-boundary
  verifier passed. The current targeted Cargo verification is recorded below;
  no workspace-wide compile or test suite is claimed.
- Latest registry platform-build staging evidence: the REST adapter derives
  one tenant-scoped command context from authenticated session, idempotency,
  and telemetry trace. The owner rejects a principal whose UUID does not match
  that context, persists expected revision plus full context and privilege in
  the append-only staging receipt, and rejects every changed replay fact.
- Latest registry external-prebuilt staging evidence: the global registry
  aggregate uses a platform-scoped command context (`tenant_id` absent), even
  though the authenticated session tenant remains authorization evidence. The
  owner binds both the operator and quarantine approver user UUIDs to the
  context actor, persists expected revision plus full context and privilege in
  the immutable staging receipt, and rejects conflicting replay evidence.
- Current external-prebuilt verification: the focused owner receipt/replay test
  and SQLite migration-schema test passed, as did `cargo check --locked -p
  rustok-server` after its GraphQL composition-error mapper was aligned to the
  canonical platform-scope error. `rustfmt --edition 2024`, `git diff --check`,
  `verify-module-control-plane-write-path.mjs`, and
  `verify-module-build-worker-isolation.mjs` passed. The scoped Cargo commands
  reported only a pre-existing unused import in an untouched `rustok-forum`
  migration and Windows linker informational warnings; no workspace-wide
  compile or test suite was run.
- Latest artifact-node materialization evidence: the separately deployable
  `rustok-artifact-node-agent` authenticates only to the narrow mTLS controller
  port, reads the owner-issued admitted digest directly from durable CAS, and
  atomically rehashes the canonical node-local payload cache. It reports
  `prepared` after non-executing Rhai/Wasm preparation and `healthy` only after
  the corresponding isolated-worker or local component readiness check. The
  agent has no owner database, topology, release-selection, policy, capability,
  tenant, AI, Alloy, or application-server dependency. Targeted checks passed:
  `cargo test --locked -p rustok-artifact-node-agent --all-targets` (8),
  `cargo test --locked -p rustok-runtime --lib` (19), and
  `cargo test --locked -p rustok-sandbox --features "rhai wasm-component" --lib`
  (32). No workspace-wide compile or test claim is made.
- Latest bounded artifact-data evidence: namespace quota tests cover projected
  structured/object usage, atomic batch rollback, capacity release through
  logical deletion, active upload-session/staging aggregation, and guarded
  restore rejection. The production `platform.secrets` route now uses an owner
  policy that rechecks exact installation identity, active lifecycle,
  capability revision, grant, and derived namespace immediately before a
  logical handle is read. Its durable-state test covers allow, stale scope,
  foreign installation, deactivation, and grant removal. Phase 3 is complete;
  no workspace-wide compile or test claim is made.
- Latest sandbox-placement evidence: `rustok-sandbox-transport` now supplies the
  current-only bidirectional tonic adapter and `rustok-sandbox-worker` supplies
  the separately deployable neutral Rhai process. Artifact server composition
  requires an mTLS connection plus exact readiness and registers Rhai only as
  `isolated_worker`; worker loss has no in-process fallback. Seven loopback
  transport tests cover broker callback, serialized Rhai scope input/output,
  cancellation, deadline/hang, typed error preservation, disconnect, readiness
  loss, and protocol mismatch.
  Six worker policy/observation tests cover exact revalidation, unbounded
  attestation rejection, request limits above the attested envelope, observed
  peak memory, execution failure without measurement, and readiness failure
  without measurement. The sandbox-worker isolation guard also passes
  and rejects product/infrastructure dependency drift or server embedding.
  Thirteen neutral runtime-contract tests also pass. Alloy and artifact
  production composition now share one readiness-checked mTLS Rhai worker
  client with no production in-process constructor or fallback. Canonical
  workspace/import resolution, standard functions, brokered HTTP helpers, and
  serialized scope records live in the neutral sandbox, so the worker retains
  no Alloy or product-infrastructure dependency. Seven focused Rhai executor
  tests cover raw source, workspace imports, mutable record changes,
  immutable-record denial, brokered HTTP allow/default-deny, instruction
  pressure, and deadline mapping; three scope-contract tests cover bounded,
  duplicate, and reserved bindings. The focused Alloy command did
  not finish compiling inside its bounded 60-second window. A canonical
  digest-pinned gVisor/Kata Kubernetes renderer and exact mTLS RPC probe now
  define the production worker profile, but retained cluster enforcement and
  supervisor evidence remain open, so Phase 1 is not complete. No
  server, Alloy, or workspace-wide compile/test claim is made.

## Quality and Isolation Audit Checkpoint (2026-07-22)

The owner boundary was rechecked after the marketplace and lifecycle cutovers.
`rustok-modules` has no direct dependency or production import for AI, product,
commerce, MCP, Alloy, Leptos, Axum, or Async-GraphQL. A transitive leak was
found in the neutral runtime foundation: `HostRuntimeContext` was hidden behind
the same `rustok-api/server` feature as HTTP/GraphQL. The current architecture
now exposes a separate `rustok-api/runtime` feature for the SeaORM-backed host
context; `server` includes it and adds only the transport frameworks. The
standalone module profile is verified by Cargo dependency-tree inspection and
the repository guard.

Targeted evidence from this checkpoint:

- `cargo check -p rustok-api --lib --features runtime` passed;
- `cargo check -p rustok-api --lib --features server` passed;
- `cargo check -p rustok-runtime --lib` passed;
- `cargo check -p rustok-modules --no-default-features --lib` passed;
- `cargo check -p rustok-modules --lib` passed;
- `cargo test -p rustok-api --lib --features runtime`: 25 passed;
- `cargo test -p rustok-runtime --lib`: 3 passed;
- `cargo test -p rustok-modules --no-default-features --lib`: 152 passed;
- `cargo check -p rustok-forum --lib`, `cargo check -p rustok-pricing --lib`,
  `cargo check -p rustok-commerce --lib`, `cargo check -p rustok-groups --lib`,
  `cargo check -p rustok-server --lib --no-default-features`, and
  `cargo check -p rustok-admin --lib --no-default-features --features ssr`
  passed (warnings only);
- `node scripts/verify/verify-module-control-plane-write-path.mjs` passed,
  including concrete admin transport backend-logic and runtime feature
  isolation checks. The native admin and GraphQL build active/history/release
  reads and rollback now use the host-composed `rustok_build::SharedBuildControl`;
  remaining Phase 7 work is canonical transport parity and the other resolver
  families.

The SHA-256 digest formatting now uses an explicit byte-to-hex helper compatible
with `sha2` 0.11, and product/inventory status and projection boundaries use
owner-neutral DTOs. The admin lifecycle mapper has explicit `ServerFnError`
closure types for optional governance records. Existing warnings remain
tracked separately; no workspace-wide compile or test claim is made.

## Verification and Isolation Hardening Checkpoint (2026-08-14)

The server and control-plane test boundary was rechecked after the artifact
node, durable CAS, and lifecycle changes. PostgreSQL-native product and channel
migrations are now explicitly excluded only from SQLite unit-test composition;
the production `Migrator` remains complete and no production schema path falls
back to SQLite. `SqliteTestMigrator` applies the declared portable schema, and
narrow component fixtures use the same named-migration predicate rather than
silently accepting unsupported database semantics.

Two cross-database migrations were corrected to issue one SQLite-compatible
`ALTER TABLE` operation per column. GraphQL mutation mapping now labels manifest
validation and composition revision conflicts as `BAD_USER_INPUT`, while
database and policy failures in module toggles return a generic internal error.
Index drift diagnostics validate canonical module/entity identifiers in a stable
field order after authorization, and the Flex REST round-trip fixture supplies
the required event transport and unique field positions.

Current targeted evidence:

- `cargo test --locked -p rustok-migrations --lib
  sqlite_test_migrator_tests::sqlite_test_migrator_applies_the_portable_schema
  -- --exact` passed;
- focused `rustok-server` tests passed for the auth lifecycle default-status
  path, product field event path, RBAC cache path, independent Blog/Forum/Pages/
  Commerce OpenAPI construction, GraphQL composition/toggle error mapping, and
  Index drift authorization-before-parsing paths;
- the broad `cargo test --locked -p rustok-server --lib -j 1` audit was started
  but intentionally stopped after it exposed additional unrelated failures and
  a hanging test. It is not counted as a passing server-wide run.

No workspace-wide compile or test claim is made.

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
| Rust component build | Native pinned Cargo targeting `wasm32-wasip2`, `wit-bindgen`, Rust's Component Model linker, `wasm-tools` |
| Cargo graph inspection | `cargo metadata` / `cargo_metadata` |
| OCI transport | Platform-owned strict OCI Distribution transport over `reqwest`; `oci-distribution` is limited to OCI data-model and auth DTOs |
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
- [x] Define revision/CAS fields for every mutable aggregate:
  - platform composition revision;
  - publish-request revision;
  - installation revision;
  - tenant settings revision;
  - build attempt/revision.
  Build requests now start at revision `1`. Before dispatch, the owner atomically
  transitions a queued request to `running`, increments its revision, and issues
  an opaque durable claim with a lease longer than the maximum admitted worker
  deadline. Terminal persistence requires that exact still-live claim and its
  observed revision, clears the lease, and increments the revision again;
  exact terminal redelivery replays, while a recovered expired claim cannot be
  completed by its former dispatcher. This prevents concurrent deliveries from
  executing or publishing the same immutable build. Publish-request work is in
  progress: its durable row now begins at revision
  `1`, owner-derived REST, GraphQL, and native-admin status projections expose
  that value. Reject, request-changes, hold, resume, final-publication,
  artifact-attach, validation-enqueue, and validation-worker result commands
  compare it atomically with their state transition before advancing it. A
  claimed validation work item carries the observed request revision; the
  owner therefore rejects a delayed running-job result, while an exact
  terminal redelivery remains idempotent. Platform-build, external-prebuilt,
  and Alloy-authored staging commands now use the same request CAS and return
  the resulting owner revision. Generic, build-service, and platform-admission
  evidence commands now use the same CAS: a newly recorded immutable fact
  advances the aggregate, while an exact replay returns the locked revision.
  The platform evidence producer chains the source, build-evidence, and
  admission revisions. Manual validation-stage reports/requeues use the same
  CAS and one platform-scoped `ModuleCommandContext`; their durable receipt
  rejects any idempotency reuse with changed actor, trace, correlation, stage,
  status, reason, or requeue evidence. Remote claim and expired-lease requeue
  advance the aggregate, and the
  claim carries the resulting revision which a terminal result must present.
  Heartbeat only renews an existing operational lease. All current
  request-state and validation-stage transitions now use the compare-and-swap
  contract.
- [ ] Define actor, tenant, trace, idempotency, and correlation context required
  by every command. The canonical `ModuleCommandContext` validates non-nil UUID
  actor, correlation, and idempotency identities, plus a bounded non-empty
  trace; a
  tenant context is either absent for platform scope or a non-nil UUID. The
  artifact lifecycle family (activation, deactivation, tenant intent,
  uninstall, rollback, migration checkpoints, tenant data purge, admission
  reverification, and artifact admission),
  settings recovery, data snapshots, owner-only artifact-data export, artifact secret binding, global
  artifact-security transitions, static promotion, and static-distribution
  bootstrap/admission/revocation, and tenant-scoped registry platform-build
  staging now carry this one context through their owner validation, durable
  receipts, and owner-created outbox envelopes where the operation emits an
  event. Artifact admission persists the complete context in its scoped
  durable idempotency receipt and derives its outbox envelope from that exact
  evidence. Platform static-distribution rollout and recovery do the same;
  static-distribution build intent does so before an immutable snapshot is
  queued;
  isolated tenant build requests bind the complete context into their immutable
  request/replay hash and use it for both queued and completed outbox envelopes;
  node-agent reports remain separately authenticated deployment observations.
  Each receipt rejects an idempotency reuse with different context
  evidence. GraphQL and REST adapters carry the context where those surfaces
  are exposed.
  The remaining mutable owner families still require atomic caller cutover to
  this contract. The accepted target is recorded in
  [ADR 2026-08-22](../../DECISIONS/2026-08-22-module-command-context-evidence.md).
- [x] Document GraphQL/native compatibility and versioning rules. The central
  UI transport contract requires one current repository-owned GraphQL/native
  surface, atomic caller cutover, and no version-suffixed routes, fields,
  types, server functions, or old/new adapters. An independently deployed
  external API version maps at its boundary to the same canonical owner DTO;
  it cannot fork authorization or domain semantics.
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
- [x] Preserve Alloy entity, parameter, validation, and HTTP behavior through
  neutral serialized scope records, the shared standard library, and the
  capability broker. The isolated worker does not import Alloy or product
  infrastructure.
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
- [x] Add deterministic metrics for fuel/instructions, memory, output size,
  capability calls, queue time, and execution time. The neutral runtime now
  records queue time, executor duration, output size, Rhai instructions or
  Wasmtime fuel consumption, and policy-admitted capability-call count for
  success and terminal failure evidence. Artifact audit persists queue time and
  capability calls alongside the existing metrics. Wasmtime now reports actual
  aggregate non-shared guest linear-memory peak through its resource limiter,
  excluding failed growth rather than reporting a configured limit. Rhai runs
  one request per isolated worker process; the worker samples its cgroup v2
  memory and reports the observed cgroup peak, while missing measurement makes
  startup, readiness, admission, and execution fail closed.
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
  after host authorization. Value consumption is deliberately not a sandbox
  `get_value` operation: `SeaOrmArtifactSecretUseService` requires a stronger
  use-specific host authorization, reloads the exact expected binding revision
  under tenant RLS, resolves the `SecretString` only after the transaction is
  closed, and lends it only to a host-composed fixed-purpose consumer. The
  consumer can return no arbitrary payload; callers receive only a redacted
  logical-reference/revision/purpose receipt, and resolver failures are mapped
  to content-free owner errors.
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
  discovery input. Server composition now binds that port to the stable
  deployment-owned `rustok` alias and the MCP owner's transport-neutral
  registry-tool invoker. The adapter derives a service identity from the exact
  artifact installation, applies `McpAccessContext`, and requires redacted
  durable audit before invocation. Unknown aliases, unsupported tools, invalid
  scope, and audit failure all fail closed.
- [x] Add per-execution payload-size, call-count, and rolling rate limits before
  broker invocation.
- [x] Ensure denied calls emit redacted audit evidence without protected input.
- [x] Ensure host adapters receive scoped handles, never platform-global clients
  or raw credentials.

### 1.4 Execution Deployment Profiles

The crate is the contract owner; executor placement is a deployment decision.
It does not create a second sandbox API.

- [x] Define `in_process` and `isolated_worker` executor adapters behind the same
  `SandboxExecutor`/runtime contract. The registry contract is now explicit:
  callers must use `register_in_process` or `register_isolated_worker`, the
  former ambiguous registration API is removed, duplicate kinds are rejected
  across placements, and `SandboxRuntime::executor_placement` exposes the
  selected adapter for readiness checks. `GrpcRhaiExecutor` now implements the
  real generated streaming worker adapter and never falls back locally.
- [x] Permit in-process Wasmtime where its threat model and resource controls are
  accepted. Artifact server composition keeps only Wasmtime in-process and
  names that placement explicitly.
- [x] Run AI-generated or otherwise untrusted Rhai in an isolated sandbox worker
  in production so interpreter/runtime defects and hard memory/process limits do
  not affect the server process. Alloy and admitted artifacts share one
  readiness-checked mTLS client stored in server runtime composition.
- [x] Keep in-process Rhai only in explicit test/local harnesses; it is not a
  production fallback. Production Alloy constructors require an injected
  `AlloyDraftRuntime`, and server composition registers only the isolated Rhai
  worker. Artifact composition uses the same client; duplicate-kind placement
  cannot create a fallback.
- [x] Use a versioned framed RPC over a local channel; reject raw stdin/stdout
  ambiguity, oversized frames, unsolicited output, and protocol drift. Prefer
  the workspace `tonic`/`prost` generated contract over a custom codec. The
  exact revision and execution UUID are checked on every streaming protobuf
  frame; metadata uses the current JSON model and artifact bytes stay native.
- [x] Route worker capability requests back through the same host broker without
  giving the worker network, database, filesystem, secret, or MCP clients. The
  neutral worker depends on none of those clients and the host re-applies the
  original `SandboxHost` identity, grant, constraint, budget, audit, and
  cancellation checks.
- [ ] Apply process/container CPU, memory, process-count, file, disk, and time
  limits through the deployment runtime rather than hand-writing a platform OS
  sandbox in Rust. The worker now fails startup/readiness/execution without an
  exact digest-pinned gVisor/Kata attestation and rejects request limits outside
  its finite envelope. The canonical Kubernetes renderer pins the selected
  RuntimeClass and image digest, applies portable pod limits and hardening,
  restricts ingress, and denies egress. This item remains open until retained
  cluster evidence also proves RuntimeClass-specific PID/file enforcement and
  the other declared controls.
- [ ] Supervise crash, cancellation, forced kill, restart/backoff, and complete
  cleanup with execution audit evidence. The rendered Deployment has at least
  two replicas, bounded rolling replacement, disruption protection, and exact
  mTLS RPC startup/readiness/liveness probes; retained crash/OOM/kill and
  capacity-recovery evidence is still required.

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
  `SeaOrmArtifactSecretCapabilityBrokerResolver`, and the facade-constructed
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
  request-digest/replay/lease coordinator for every externally routed binding.
  Its durable receipt binds one tenant-matched `ModuleCommandContext`, so a
  replay cannot substitute actor, trace, correlation, or UUID idempotency
  evidence. A crashed pending request can be reclaimed after its lease instead of
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
  Deactivation, tenant disable/enable, uninstall, and migration checkpoints
  reject nil installation, actor, idempotency, and tenant-scope identities
  before opening that transaction, keeping lifecycle audit and idempotency
  records attributable.

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
  - `PromotionService`;
  - `StaticDistributionService`.
  `ModuleControlPlane` now provides the owner composition root for the extracted
  catalog, lifecycle, composition, build, installation, release, publication,
  current static-promotion, and immutable static-distribution selection services.
  No versioned or compatibility promotion path exists. Server lifecycle,
  composition, artifact runtime/HTTP,
  registry release/publication/validation adapters, the independent validation
  worker, module-build dispatcher, and installer persistence adapter now consume
  those services through the facade. The facade also supplies the exact artifact
  data/object capability resolvers, redacted execution-audit observer, durable
  event-subscription projector, and binding idempotency store; server runtime,
  outbox projection, and routed artifact HTTP no longer construct those owner
  adapters directly. It also owns construction of the logical secret-binding
  service, dynamic `platform.secrets` capability resolver, host-only
  exact-revision secret-use service, and default secret-handle policy, so
  callers cannot bypass their distinct
  authorization ports or create a sandbox-visible secret-value broker directly.
  RBAC permission evaluation remains a separate RBAC-owner
  authorization adapter. `EffectivePolicyService` likewise owns the
  tenant override read and Core/default composition shared by server guards,
  GraphQL, and installer adapters. It also supplies the tenant-scoped
  policy-revision cursor required for commit-time lifecycle serialization.
  Lifecycle recovery-plan reads are likewise tenant-bound inside the owner;
  GraphQL cannot load a global operation then filter its tenant after reading
  owner state. The static write-path verifier rejects direct construction of these extracted
  SeaORM services outside the owner crate.
  Promotion request and approval are a separate platform-scoped owner
  subservice. It accepts only an active `platform_built` release and reloads the
  completed build request/result, source identity, dependency-lock digest, and
  publication receipt before recording any request or approval.
- [x] Register the mandatory `ModulesModule` migration source in the shared
  server/installer migrator. Control-plane tables are no longer fixture-only:
  `rustok-migrations::Migrator` now includes the owner migration source before
  schema application, so fresh installations receive artifact admission,
  lifecycle, rollback, and subsequent owner migrations.
- [x] Define infrastructure ports for registry transport, artifact blob access,
  signature verification, SBOM/provenance verification, build scheduling,
  transactional storage, events, audit, clock, and ID generation. The runtime
  boundaries now expose `ArtifactRegistry`, `ArtifactBlobStore` /
  `DurableArtifactBlobStore`, `TrustVerifier`, `OciArtifactPublisher`,
  `ModuleCompositionBuildEnqueuer`, and `ModuleBuildWorker`. The owner-owned
  `ControlPlaneInfrastructure` supplies injected clock and UUID ports;
  admission, installation lifecycle, build, governance, binding-idempotency,
  event/schedule delivery, and identity-allocating data operations use it for
  installation, operation, outbox, verification, commit-evidence, publication
  aggregate, validation-stage, validation-claim, delivery, work-lease,
  data-upload, private-object, GC-candidate, export, and lease-time identities
  instead of process globals. Schedule work materialization also receives the
  injected owner time. Secret outbox facts, generated lifecycle correlations,
  durable/in-memory CAS stage identities, and OCI temporary staging paths now
  use the same context. A crate-wide production-source audit leaves direct
  system clock and random UUID access only inside the default infrastructure
  adapters; test fixtures remain free to create their own identities.
  Database-expression timestamps remain a transactional storage concern. The
  caller-supplied SeaORM connection and owner-opened `DatabaseTransaction` are
  the explicit transactional storage adapter. `rustok-outbox` now exposes the
  object-safe `TransactionalEventWriter`; `ControlPlaneInfrastructure` composes
  its `OutboxTransport` adapter once and owner operations append through that
  port without constructing a transport or publishing outside their
  transaction. Runtime audit uses the existing redacted `ExecutionObserver`
  port, while governance, lifecycle, data, secret, and installation audit facts
  remain owner rows/outbox facts in the same transaction; no second audit
  journal or fire-and-forget audit sink is introduced.
- [x] Keep transaction boundaries inside owner services while accepting a
  caller-provided database/transaction adapter where required. Composition,
  build, governance, installation, lifecycle, data, secret, and delivery
  services open and complete their own transactions. The composition build
  enqueuer is the explicit exception: it receives only the owner-opened
  `DatabaseTransaction`, cannot commit it, and a failed enqueue rolls the
  composition CAS mutation back.
- [x] Add idempotency keys for install, publish, build, retry, rollback, and
  promotion commands. Artifact admission now requires a non-nil actor and
  idempotency UUID, reserves the complete immutable request digest inside the
  installation transaction, replays the original installation identity for an
  exact retry, and rejects conflicting reuse. Build submission applies the same
  tenant/project request-fingerprint rule. Post-hook retry
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
  at the live approval endpoint; the session-backed transport derives a
  platform-scoped `ModuleCommandContext`, and the owner binds its actor UUID to
  the structured principal before it stores actor, trace, correlation,
  idempotency, and approval facts with the resulting release. Only an exact
  retry replays a published request, while a changed context, a missing receipt,
  or conflicting key reuse fails closed. Live review transitions (reject,
  request-changes, hold, and resume) require the same non-nil header and
  platform-scoped context. One immutable owner receipt ledger binds their
  operation kind, revision, actor, trace, correlation, reason, and reason code;
  exact retry succeeds after the request state has changed, while every changed
  fact fails closed. Static-promotion request and approval reserve a global
  operation key with the complete command digest and actor, persist the original
  status/revision receipt, replay only an exact retry, and reject any conflicting
  reuse. Future distribution-selection commands must use the same operation
  contract before they are admitted.

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
  Composition is a platform aggregate: every mutation carries a
  platform-scoped `ModuleCommandContext`, the shared receipt ledger uses its
  separate `platform` namespace instead of a tenant or sentinel UUID, and the
  owner rejects a tenant-scoped context before it reads `platform_state`.
  GraphQL permits that command only for a direct, tenant-matched SuperAdmin
  holding `modules:manage`; the routed tenant anchors authorization but never
  enters the composition command or receipt. The post-commit `build.requested`
  event is emitted at platform scope with the original actor, correlation, and
  trace evidence, so notification delivery does not create a second command
  identity.
- [x] Move registry ownership, publish-request, release, validation-stage,
  yanking, and governance rules from `RegistryGovernanceService`. Release
  yanking, ownership binding, owner transfer, publish-request rejection,
  request-changes, hold, resume, and final publication have moved: the host
  supplies authenticated actor and privilege facts, and live yanking/review/
  publication/owner-transfer/publish-request-create commands supply one platform-scoped context with a required
  idempotency key, while
  `SeaOrmModuleGovernanceService` locks durable rows, derives the applicable
  authorization, updates the relevant state, and writes its governance audit
  facts in one transaction. Publication atomically writes
  the release projection and translations, owner binding or authorized rebind,
  optional approval-override evidence, and request finalization. Initial
  binding and authorized rebind have no parallel direct owner-bind command;
  later ownership changes use the separately context-bound transfer command.
  Validation
  stages are owner-owned: manual report/requeue transitions, remote lease claim,
  heartbeat, terminal completion, expired-lease requeue, validation-job enqueue,
  job claim, stale-job recovery, worker retry telemetry, and automated result
  materialization. Live enqueue binds the platform-scoped command context,
  expected revision, actor principal, and rejected-retry policy to a durable
  receipt, so only the exact retry returns the original queue result. A later authorized enqueue marks a validation job still
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
  request, default-locale metadata, and creation audit fact together after it
  derives authorization from the authenticated privilege fact and current
  owner binding. Artifact object storage remains a host adapter; its immutable
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
  The registry catalog adapter and router now expose only the current `/catalog`
  and `/catalog/{slug}` contracts. Version-suffixed compatibility routes, client
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
must pass through `ModuleControlPlane` in `rustok-modules`. The static
distribution worker is additionally constrained to immutable build-completion
evidence: it cannot import release/rollout owner types or owner persistence, so
deployment receipts remain a distinct rollout-owner concern.

The guard covers raw SQL, protected aggregate `ActiveModel` construction, and
direct SeaORM `Entity` mutation methods. The unused server runtime database
truncation helper was removed because it deleted owner-owned tenant-module state
outside `ModuleControlPlane`. Database reset remains an explicit operational
tooling concern and is not exposed as a privileged server runtime API.

The owner event boundary now builds every module-control-plane envelope through
`ControlPlaneInfrastructure`: event identity, correlation identity, timestamp,
tenant scope, and available actor identity are assigned explicitly before the
transactional outbox write. This replaces calls that accidentally placed an
event ID in the envelope tenant field and a tenant ID in the actor field. The
same verifier rejects direct `EventEnvelope::new` calls elsewhere in the owner
crate so that metadata cannot silently drift again.

The root event contract encodes platform scope as the nil tenant sentinel only
for an explicit allow-list of platform-capable module events. Every other event
remains tenant-scoped and rejects the sentinel at both root and typed-contract
envelope validation boundaries.

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
  admission path leaves the pointer unset because admission is inert. The
  owner activation operation serializes one `(scope, slug)`, selects at most
  one active non-uninstalled predecessor, rejects ambiguity, and writes the
  pointer in the same transaction that makes the candidate active and the
  predecessor inactive. A later rollback command advances it with its status
  transition.
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
  transitions use expected-revision CAS. Immutable admission accepts a
  scope-matched `ArtifactAdmissionCommand` with a complete
  `ModuleCommandContext`; its canonical context/reference/scope/lock/policy
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
  message size. The shared process-wide admission semaphore bounds work across
  all connections and sheds after a deployment-bounded wait, while readiness
  remains available without a permit. SIGTERM/Ctrl+C performs tonic graceful
  shutdown and cancelled Cosign futures kill their child process. Its same
  mTLS-protected listener exposes a readiness RPC only
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

- [x] Provide brokered namespaced storage capabilities for structured values,
  objects/files, indexes/query patterns supported by the platform, and
  secret-reference handles. The durable data owner provides bounded structured
  JSON values and a private object broker through a host-owned
  tenant/module/revision namespace with optimistic revisions and durable
  idempotency results. Structured `delete` requires an exact positive record
  revision and UUID idempotency key, removes matching materialized indexes in
  the same transaction, and stores a policy-revision-scoped replay receipt
  under tenant RLS. Object metadata exposes logical name, content type, size,
  digest, and revision only; its SeaORM/storage adapter generates and retains
  the physical key privately, derives the digest from accepted bytes, and
  re-hashes every private read before returning bytes. Secret references now
  have a separate owner-owned scoped binding table with revision CAS,
  idempotency, actor/reason audit data, and a redacted transactional-outbox fact;
  the injected `acquire_handle` broker returns only the logical handle and
  revision after per-execution host authorization. `platform.data.objects` now
  admits owner-scoped `get_metadata`, `read`, `put`, `delete`, and `list` calls
  only under separately declared object-prefix/operation grants. Logical delete
  requires an exact positive object revision plus a UUID idempotency key,
  persists an immutable replay receipt under tenant RLS, removes metadata with
  revision CAS, and queues the unreachable private key for retention-aware GC
  in the same transaction. Its JSON/base64 bridge is
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
  Value-consuming secret use now passes through the host-only exact-revision
  service described below; `platform.secrets` itself remains handle-only so it
  cannot become a value-exfiltration capability. The production server
  registers that route with `ModuleControlPlane::artifact_secret_handle_policy`, which repeats
  exact installation, lifecycle, policy-revision, explicit-grant, and
  owner-derived-scope validation immediately before the binding read. Event
  delivery remains durable owner-queued ingress into admitted bindings rather
  than an outbound guest capability. `platform.mcp` is now composed through
  `ModuleControlPlane` and the MCP owner with the stable `rustok` alias, exact artifact-derived service
  identity, existing access-policy contract, and fail-closed durable audit. It
  exposes only the read-only registry tool surface; external aliases require
  explicit host-owned adapters and cannot be supplied by artifacts.
- [x] Add revision-guarded logical object deletion to the brokered data
  capability. The guest receives only logical name and deleted revision;
  physical bytes remain private and cannot be collected until the independent
  retention policy approves their durable GC candidate.
- [x] Add revision-guarded logical structured-record deletion. The owner
  authorizes `Delete` separately from writes, removes the record and every
  materialized scalar index atomically, and returns only its logical key and
  deleted revision through an exact policy-scoped idempotency replay.
- [x] Scope every operation by tenant, module slug, data-contract revision, and
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
  expiry checks; a missing rule fails closed. Individual object deletion uses
  the same immutable scope, a distinct `ObjectDelete` authorization decision,
  and revision CAS. Structured/object put and delete idempotency identities,
  purge receipts, export evidence, upload sessions, snapshot operations, GC
  candidates, and secret bindings all carry the exact current policy scope.
  Future capability kinds must use the same owner-derived boundary before they
  can be registered in the fail-closed router.
- [x] Validate data/settings/action payloads with bundled JSON Schema using the
  maintained `jsonschema` validator and bounded regular-expression settings.
  Structured-value writes now require a host-owned schema-validation port before
  persistence. `SeaOrmArtifactDataSchemaValidator` resolves the exact admitted
  installation descriptor and persistence schema under tenant RLS, then uses
  the same bounded Draft 2020-12 compiled-validator cache as runtime dispatch
  and settings, with strict formats, linear-time regular expressions, and no
  external resolvers. Artifact
  settings now have a distinct owner write entrypoint: the immutable definition
  retains only its admitted settings-schema digest plus schema bundle,
  `ModuleLifecycleDbWriter::persist_artifact_settings` resolves that exact
  document and validates the object before persistence, and the static
  pre-normalized entrypoint rejects artifact definitions. The lower-level
  tenant settings store is no longer exported, so a host cannot bypass this
  split. `ModuleControlPlane::artifact_lifecycle` composes that dynamic owner
  with the same infrastructure and admitted lifecycle executor. Binding inputs
  and decoded outputs, including command/action payloads,
  already use the same bounded compiled-validator cache in `ArtifactRuntime`.
  The Phase 7 declarative admin-action presentation/transport adapter remains
  separately tracked; it must route through this admitted binding path and
  cannot introduce another payload-validation implementation.
- [x] Define quotas, pagination, transactions/batches, optimistic revisions,
  idempotency, backup/export, retention, and deletion semantics.
  `ArtifactDataQuota` is an owner-selected immutable policy snapshot; artifact
  payloads cannot provide or raise it. The platform hard ceilings are 10,000
  structured records and 64 MiB of canonical structured JSON, 1,024 logical
  objects and 256 MiB of live object bytes, sixteen active upload sessions,
  and 64 MiB of staged chunks per tenant/module/data-contract namespace.
  Production composition may only tighten those limits. Structured and object
  writes compute projected count/byte usage under the same namespace lock and
  transaction as their revision mutation; overwrites replace the prior byte
  contribution, batch writes observe earlier writes in the same transaction,
  and a rejected batch rolls back in full. Logical delete releases live
  capacity while private object bytes remain retention-GC governed. Upload
  session and staging-byte limits cover every active session in the namespace,
  not only one guest execution or policy revision. Restore authorization now
  returns the exact target quota snapshot, which is revalidated against the
  canonical manifest inside the guarded restore transaction before rows become
  live.
  Structured-value writes currently have a 256-byte logical-key bound, a
  64 KiB JSON-payload bound, per-record optimistic revisions, and durable
  idempotency. Per-record deletion requires exact revision CAS, durable
  idempotency, and atomic materialized-index cleanup. Their namespace lifecycle
  serializes writes and deletes against explicit
  purge, retains a tombstone after purge, and requires a host authorization
  port for lifecycle/retention/legal-hold checks before the audited outbox
  operation. Authorized keyset pagination is bounded to 100 records and uses
  only a logical-key continuation. `put_batch` accepts at most 32 distinct
  logical keys and idempotency keys, validates every schema and authorization
  decision before opening its transaction, and commits all writes and their
  durable idempotency facts atomically. Object overwrite, logical object
  deletion, and authorized namespace purge queue replaced/unreachable private
  keys for retention-guarded collection. Individual deletion requires exact
  revision CAS and durable idempotency; it never removes private bytes inline.
  The owner now also provides an audited bounded export page: a separate host
  authorizer, active namespace revision CAS, lifecycle lock, audit row, and
  redacted outbox fact protect each export. It never appears as a sandbox
  capability or returns physical storage identity. It remains intentionally a
  keyset page rather than a backup primitive. Durable namespace backup is now a
  separate owner operation: `SeaOrmArtifactDataSnapshotService` locks the exact
  active namespace revision, captures bounded structured records, object
  metadata, materialized indexes, and the index-contract digest under tenant
  RLS, then copies immutable object bytes to snapshot-owned private storage and
  re-hashes every copy before publishing a canonical logical manifest digest.
  A crash leaves a resumable `staging` snapshot; only a fully copied `ready`
  snapshot emits `module.artifact.data_snapshot_created`. Snapshot creation is
  bounded to 1,000 structured records, 64 objects, 8,192 index projections, and
  256 MiB of object bytes per operation. Restore is separately authorized and
  idempotent, re-verifies the manifest and every object, and atomically restores
  values, object metadata, index projections, the index contract, namespace
  revision CAS, audit operation, and
  `module.artifact.data_snapshot_restored`. It accepts only the same logical
  tenant/module/data-contract namespace and an empty active target; it never
  clears a purge tombstone or overwrites live data. Staging source references
  also block retention GC under the namespace lifecycle lock. Snapshot
  retention is now independently revisioned: an authorized idempotent command
  may extend (never shorten) `retain_until` and may apply or release legal hold,
  with audit and `module.artifact.data_snapshot_retention_updated` committed in
  the same transaction. The bounded collector scans at most 1,000 candidates
  and collects at most 100 per pass. A ready snapshot must be expired, free of
  legal hold, separately authorized as a destructive owner command, and
  explicitly approved by a supplied durable policy snapshot
  with no audit or rollback hold; a missing rule retains it. Approval first
  commits an immutable `collecting` operation with its tenant-matched typed
  command context, reason, and policy snapshot identity. Blob deletion is
  idempotent, interrupted collection resumes with the original
  actor/trace/correlation/idempotency identity rather than that of the
  resuming worker, and the final transaction
  deletes manifest-owned rows while preserving retention/restore/collection
  audit facts and emits `module.artifact.data_snapshot_collected`.
- [x] Keep secret values outside settings and module data; store only brokered
  secret references. The secret-binding store persists only a host-authorized
  resolver reference in its separate owner table; structured data, sandbox
  inputs, artifact handles, and outbox evidence never include a secret value.
  Binding is a tenant-matched typed command: its immutable operation receipt
  retains actor, trace, correlation, and idempotency facts, rejects a changed
  replay, and its outbox envelope preserves the same evidence.
  The sandbox handle-acquisition broker exposes only logical name and revision.
  `SeaOrmArtifactSecretUseService` is the separate host-only value boundary: a
  caller must present the exact nonzero handle revision and immutable execution
  scope, `ArtifactSecretUseAuthorizer` is distinct from handle authorization,
  the resolver alias/key stay inside the owner, and a fixed
  `ArtifactSecretValueConsumer` receives only a short-lived `SecretString`
  borrow. The consumer result is `()` and the owner returns only a redacted
  receipt. Resolution and consumption happen after the binding read transaction
  closes; concrete consumers own their operation-specific idempotency and
  redacted audit requirements.
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
  after the page completes. The apply command supplies authenticated
  actor/reason/idempotency facts; the checkpoint receipt binds their immutable
  request digest and replays an uncertain successful command without another
  revision advance or outbox event. It holds no control-plane transaction
  across the page. Distributed rollout, rollback, and quarantine policies
  remain pending. Focused SQLite lifecycle and data-upgrade replay tests plus
  the lifecycle command validation test passed after the durable checkpoint
  receipt was added; no workspace-wide test run is claimed.
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

The owner boundary is fixed by the [module artifact rollback ADR](../../DECISIONS/2026-07-13-module-artifact-rollback-boundary.md) as amended by release safety: an explicit CAS-revision command may select only the exact durable direct predecessor fixed from then-serving state, re-evaluates current grants, audits actor/reason, and writes an outbox event in one transaction. Runtime activation and tenant enablement remain downstream operations.

- [x] The current selection slice can select a previously admitted immutable
  release; that broad target is an explicit atomic-cutover gap, not accepted
  production rollback behavior.
- [x] Replace broad historical selection with exact durable
  direct-predecessor selection fixed from then-serving state. The scoped owner
  activation operation holds one `(scope, slug)` lock, accepts only an
  admitted/installed/inactive candidate at its expected revision, captures the
  sole active non-uninstalled predecessor, makes that predecessor inactive,
  records the candidate pointer and a replayable operation result, makes the
  candidate active, and writes `module.artifact.activated` in one transaction.
  It rejects an ambiguous serving state; arbitrary older admitted releases are
  new admitted updates and never rollback targets.
- [x] Bind dynamic artifact settings to one stable `data_owner_id` and exact
  `settings_instance_id`, never to a `(tenant_id, module_slug)` row. Admission
  creates opaque identities; activation carries them from the direct
  predecessor only when the admitted registry/repository continuity and the
  immutable settings-schema digest both match. Dynamic settings reads and
  writes hold the activation fences, resolve one active scoped installation,
  validate against that installation's descriptor-bundled schema, and persist
  only in the tenant-RLS settings-instance owner table. Native/static manifest
  settings remain the separate `tenant_modules` contract. Schema-changing
  activation fails closed until the distinct guarded settings-migration path is
  implemented.
- [x] Capability grants are re-evaluated for the target release.
- [x] Data migrations declare whether rollback is reversible, compensating, or
  prohibited. A recorded irreversible checkpoint accepts only an explicit
  compensating rollback; prohibited rollback is rejected before state changes.
- [x] Runtime activation and tenant enablement rollback remain distinct. The
  artifact rollback command changes only durable selection/admission state;
  tenant toggles and lifecycle hooks use the separate lifecycle owner path.
- [x] Every rollback is a new audited operation with actor and reason.
- [x] Define disable, deactivate, and uninstall as distinct current operations;
  `purge` alone is only a non-callable category in the accepted target:
  - disable preserves installation and data;
  - deactivate removes runtime bindings but preserves admitted release/rollback;
  - uninstall removes the scope's selection after dependent checks;
  - `dynamic_artifact_data_purge` deletes only the exact retained
    structured/index/object boundary through its explicit destructive,
    authorized, audited operation;
  - `dynamic_artifact_settings_purge` is a separate target operation with its
    own settings recovery point, tombstone, receipt, and retention lifecycle.
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
  requesting tenant scope that has not been uninstalled. An uninstall operation
  rejects a later tenant lifecycle command before it can write a new intent
  record. A separate tenant-scoped immutable receipt ledger records the
  installation, requested state, expected and committed revisions, actor,
  reason, and idempotency key. Exact retries replay their original committed
  revision after later commands or uninstall without another event; divergent
  key reuse fails closed. The mutable lifecycle row retains only current intent
  and its latest audit metadata.
- [x] Artifact uninstall is an owner-owned, revision-guarded and idempotent
  scope-selection removal. It requires an inactive installation, rejects an
  active direct dependent in the same scope, writes audit/outbox atomically,
  and only releases the CAS reference; it does not purge retained data or
  evidence. Its replay contract likewise matches the complete immutable
  command rather than accepting a reused key alone; a new command against the
  terminal uninstalled selection rejects before it can reach persistence.
- [x] Expose the current dynamic installation lifecycle through tenant-scoped
  GraphQL commands only. `activateTenantArtifact`, `deactivateTenantArtifact`,
  `uninstallTenantArtifact`, and `rollbackTenantArtifact` derive scope from the
  authenticated `TenantContext` and require `modules:manage`; no client input
  can select tenant or platform scope. Rollback also accepts no target
  installation selector: the owner selects only the retained direct
  predecessor after its capability-grant and migration-policy checks. The
  platform-scope GraphQL surface is deliberately absent because a
  tenant-derived permission is not a platform-operator authorization contract;
  it remains fail-closed until that distinct authority exists. The shared
  GraphQL document guard classifies the tenant lifecycle snapshot as read and
  every lifecycle mutation as manage before resolver execution; the owner
  facade repeats the authorization check at the command boundary. The same
  guard classifies the composition snapshot as read and marketplace registry
  freshness as manage before their resolver/owner checks. The admin-navigation
  `enabledModules` tenant availability projection is also read-gated at both
  layers.
- [x] The current structured artifact-data purge is a separate destructive
  operation. Its generic command/tenant-module attach identity is an explicit
  cutover gap; the target callable is `dynamic_artifact_data_purge`. It
  is tenant/module/data-contract scoped, revision-guarded and idempotent,
  serializes against data writes, carries a tenant-matched typed command
  context, records actor/trace/correlation/reason and the deleted-record count
  in its immutable receipt, emits a transactional-outbox fact with that same
  command identity, and leaves a durable namespace tombstone. A host-owned
  authorizer must approve lifecycle, retention, and legal-hold policy before
  the operation begins.
- [x] Complete `dynamic_artifact_settings_purge` as the independently
  authorized settings-owner lifecycle. The implemented core has exact
  encrypted recovery points, immutable KMS key-version/schema/descriptor/value
  roots, unresolved-secret-handle digest, policy/retention/hold-aware
  authorization context, authenticated decrypt/revalidation, idempotent
  tombstoned purge, fresh-instance restore, and transactional outbox facts. It
  rejects combined data/settings apply and retains no generic `purge` command.
  Recovery retention has its own revision-CAS receipt and may only extend
  expiry or add holds; the host KMS port rewraps authenticated ciphertext under the current
  approved key; collection records durable `ready`/`collecting`/`collected`
  state before terminally clearing ciphertext while preserving evidence and
  the original tenant-matched typed command context. A crash resume reloads
  that stored actor/trace/correlation/idempotency identity before it emits the
  terminal outbox event; and an
  intentionally unbound restore has a separate one-time continuity-authorized
  bind command. Direct restore pins its selected target admission revision;
  binding requires exact data owner, registry/repository lineage,
  slug, schema, and inactive successor compatibility and never clears the
  source tombstone. Each lifecycle action emits an owner outbox fact.
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

Focused verification on 2026-08-06 passed `cargo test --locked -p rustok-modules --lib`
(186 tests), `cargo test --locked -p rustok-modules --test policy_commit_guard_sqlite`
(one test), and `cargo test --locked -p rustok-events --test canonical_contracts`
(14 tests), including platform-scope envelope validation and the reviewed
event-contract release artifact. The control-plane write-path, strict OCI
transport, and lifecycle-bypass verifiers also pass; the last now proves the
obsolete direct toggle helper is absent rather than allowing a migration-only
exception.

Focused verification on 2026-08-08 passed `cargo test --locked -p
rustok-modules --lib` (191 tests) after the owner facade took over production
construction of the policy-revision cursor, secret-handle policy,
`platform.mcp` capability resolver, exact request-scoped governance follow-up
projection, and the public publish-status projection. Its regression tests
cover durable failed-stage facts, approval override, actor filtering, request
identity, semantic next-action selection, final-publication guidance after the
required stages pass, host-only attached-artifact delivery facts, and the
owner-authorized content-addressed artifact-upload slot including exact replay
and actor rejection, owner-issued remote runner transition results,
actor-specific authorization, and owner-derived rejected-request retry and
publisher facts. `cargo check --locked -p rustok-modules` also passed for the
current owner/server cutover. Both default and `--no-default-features`
`cargo check --locked -p rustok-server` attempts exceeded ten minutes without
diagnostic output after the local build cache was cleared, so they do not count
as passing server checks. The unrelated dirty `rustok-translation` source still
contains its earlier unresolved `hash_manifest` reference in
`crates/rustok-translation/src/progress.rs`; no foreign translation code was
changed here. The matching `cargo test` target still cannot reach its test
binary on this host: the unrelated `rustok-storefront` and `rustok-admin`
cdylib links exhaust linker memory (`LNK1102`). These are environment/worktree
constraints, not passing server tests.

Focused verification on 2026-08-09 passed `cargo test --locked -p
rustok-modules --lib` (198 tests) after owner-derived authorization was added
for publish-request creation, platform/external staging, release yanking, and
owner transfer. The regression coverage includes denied owner transfer before
any durable write. The initial default server check then stopped at a dirty
`rustok-translation` import error; that foreign worktree change was not edited
in this control-plane slice.

The subsequent 2026-08-09 lifecycle transport closure removed the remaining
post-command lifecycle ORM rereads. Owner state records now return persisted
enablement, module slug, and settings, while toggle results carry the exact
settings fact used by the lifecycle command. GraphQL maps those owner results;
retry returns the owner operation record and inherited compensation availability
comes from the owner policy service. Lifecycle state snapshots retain the
owner-issued operation identity for transition and replay evidence. The focused
owner snapshot test passed,
as did the 198-test owner library suite, touched-file `rustfmt --edition 2024`,
both default and `--no-default-features` `cargo check --locked -p
rustok-server`, `git diff --check`, `cargo metadata --locked --no-deps`, and
the module control-plane write-path and lifecycle-bypass guardrails. Server
checks emitted pre-existing warnings in unrelated packages; no workspace-wide
compile or test suite is claimed.

The subsequent 2026-08-09 remote validation observability closure moved
runtime guardrail lease counts behind the governance owner. The owner-issued
`ModuleRemoteValidationRunnerSnapshot` includes only `running` remote leases,
so terminal or locally owned stages cannot cause false degradation; a failed
owner snapshot is critical rather than synthetic zero work. The focused owner
snapshot test, touched-file `rustfmt --edition 2024`, `git diff --check`,
`cargo metadata --locked --no-deps`, and the module control-plane write-path
and lifecycle-bypass guardrails passed. The current `cargo test --locked -p
rustok-modules --lib` run also passed all 198 tests. The current
`cargo check --locked -p rustok-server --no-default-features` run also
completed successfully, with pre-existing warnings outside this slice. The
focused `remote_executor_guardrail_tests` server unit test passed with
`--no-default-features`, as did the owner validation-stage normalization and
server schema regression tests. The `module_lifecycle` integration target was
attempted with both feature selections, but the host exhausted Windows virtual
memory while compiling the default UI graph; the subsequent no-default attempt
inherited an inconsistent target cache. Neither attempt reached the test
runner, so no lifecycle integration success is claimed.

The follow-up owner-boundary cleanup removed the last server-local registry
persistence models and moved registry error classification into the
`ModuleGovernanceError` contract. The focused category/code test passed, as
did `cargo check --locked -p rustok-server --no-default-features`, touched-file
`rustfmt --edition 2024`, `git diff --check`, `cargo metadata --locked
--no-deps`, and both module-control-plane static guards. The server check emits
pre-existing warnings outside this slice; no workspace-wide compile or test
suite is claimed.
The focused remote-transition regression also passed after classifying a
runner-mismatched remote lease as owner-issued permission denial rather than a
state conflict. Both remote HTTP paths now preserve that same canonical
category/code contract.

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
  `rustok-module-build-transport` crate now maps this owner port onto the single
  current mTLS gRPC service with authenticated readiness, no generation suffix,
  and no in-process fallback.
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
  current result derives toolchain and WIT digests from domain-separated immutable
  request fields, so a worker result cannot substitute a different contract.
  Its retryability bit must exactly match the `retry_build` next action.
- [x] Initially implement the worker as a separately deployable binary/process;
  split a package only when the protocol and operational lifecycle justify it.
  The transport boundary is fixed by
  [`2026-07-16-module-build-worker-transport`](../../DECISIONS/2026-07-16-module-build-worker-transport.md):
  it serializes only the owner-owned request/result protocol, requires mTLS in
  production, and exposes readiness on the same authenticated listener.
  `rustok-module-build-worker` now provides the separately deployable process:
  its process-wide admission bound applies across all authenticated connections,
  readiness remains permit-free, and SIGTERM/Ctrl+C starts bounded tonic
  graceful shutdown. Cancellation drops and kills every worker-owned child
  process rather than leaving a build running after its RPC future ends. The
  worker invokes only a fixed image-owned non-symlink OCI job launcher whose
  SHA-256 digest is configured and rehashed at construction, readiness, and
  immediately before spawning in a fixed workdir with a cleared environment,
  request-derived deadline, and aggregate streamed stdout/stderr output limit.
  Startup requires a gVisor or Kata job
  runtime plus a digest-pinned OCI job image; the launcher receives those fixed
  identities and must create the corresponding isolated OCI job. Its current
  source contract is a digest-addressed `cas://sha256:<hex>` archive from a
  deployment-mounted read-only root. The worker rehashes and materializes it
  through the shared `rustok-build-source` strict USTAR parser into a
  request-scoped directory
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
  `rustok-build` remains a reviewed static role-plan/build foundation used only
  by trusted preparation operations; its server background executor has
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
  requires an explicit SHA-256-pinned OCI job launcher and `gvisor` or `kata` runtime,
  and readiness probes the worker-owned launcher/runtime configuration rather
  than returning an unconditional success. Every launched job must also emit a
  bounded immutable-request-matching OCI receipt, including its opaque job ID,
  fixed image digest, build attempt, source scenario digest, dependency-lock
  digest, toolchain digest, WIT digest, exact component target, and a
  domain-separated digest of the exact request JSON delivered to the launcher,
  before the worker accepts its terminal result. The receipt schema rejects
  unknown fields, so a launcher cannot smuggle unreviewed controls into
  evidence consumed by later code.
  Startup and
  readiness also require the deployment-owned
  `RUSTOK_MODULE_BUILD_ISOLATION_ATTESTATION` file: a bounded, regular JSON
  attestation must match the fixed runtime/image and prove unprivileged,
  host-mount-free, socket-free, host-network/PID-isolated, resource-limited,
  ephemeral-job settings. It also binds the exact launcher digest and requires
  false tenant-database and general-platform-secret access facts. The
  attestation schema rejects unknown fields, there is no attestation-free
  constructor, and every build rechecks readiness and reloads the
  deployment-owned file before accepting work. This is
  configuration-review evidence and does not
  replace deployment evidence that the launcher enforces the corresponding
  controls. The canonical module-build-worker Kubernetes renderer now pins the
  selected RuntimeClass plus independently pinned worker and OCI-job image digests, deploys two hardened replicas with
  mTLS readiness probes, mounts only a read-only source PVC plus attestation
  and mTLS material, and permits ingress only from the dispatcher while
  default-denying egress. Its probe executes the generated authenticated
  readiness RPC rather than accepting a listening port. Retained cluster proof
  that the launcher actually creates the corresponding hardened OCI jobs is
  still required.
  Deployment evidence that the launcher actually creates the hardened job
  remains required before this item can close.
- [ ] The worker has no tenant database access and no general platform secrets.
  `verify-module-build-worker-isolation.mjs` rejects direct tenant-database,
  platform-storage, and general-secret dependencies or APIs in the worker crate
  and verifies that the untrusted runner is environment-cleared without
  database or credential forwarding. The worker also fails closed without the
  bounded isolation attestation, which binds the launcher digest, requires
  explicit false tenant-database/general-secret access facts, and accepts only
  positive bounded PID and open-file ceilings, while deployment
  isolation evidence remains required before this item can close.

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
   Load the bounded regular `module-artifact.json` source declaration before
   any author code runs. It must omit the build-derived component digest and
   exactly match the immutable request's module slug, version, runtime ABI,
   optional WASM kind, and `run` entrypoint.
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
8. Build the component with the pinned native Rust toolchain using
   `cargo build --locked --target wasm32-wasip2`. The former
   `cargo-component` path is not retained: current Rust emits WASI P2
   components natively, while `rustok-module-sdk` owns generated guest
   bindings from the single canonical WIT source. The fixed OCI launcher is
   given that exact component target, and its independently deployed receipt
   must claim the same target before the worker accepts any result.
9. Validate and inspect exports/imports using `wasm-tools`. The worker now
   additionally binds a successful result to a fixed `output/component.wasm`,
   rehashes it, validates Component Model bytes, and compares the root
   imports/exports with runner evidence. A deployment-owned `wasm-tools` stage
   extracts WIT from that same fixed payload; the worker parses it and requires
   the requested package, world, version, and complete import/export surface to
   match exactly.
10. Require the configured WIT world and reject undeclared imports. This is
    enforced from Component-derived WIT rather than runner-provided text.
    After this inspection, the worker combines the validated source
    declaration with the independently verified component digest and creates
    `module-artifact-descriptor.json` exactly once. Runner-provided descriptor
    output is rejected.
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

- [x] Generate Rust guest bindings from the frozen WIT contract with maintained
  Bytecode Alliance tooling; do not hand-maintain duplicate ABI structs.
  `rustok-module-sdk` now packages the sole canonical
  `rustok:module@1.0.0/module-runtime` WIT source and uses `wit-bindgen` to
  generate the guest import, `Guest` trait, and public export macro. The
  Wasmtime host binding consumes that exact file rather than retaining an
  inline duplicate. The SDK is product-neutral and has no server, database,
  marketplace, AI, Alloy, network-client, or platform-runtime dependency.
- [x] Add `rustok module` CLI flows for init, validate, test, build, package,
  inspect, and publish through existing CLI provider contracts.
  `rustok-modules-cli` now implements the canonical init/validate/test/build/
  package/publish/inspect command set:
  create-new project writes, pinned Cargo lock generation with bounded timeout,
  rollback of only the newly created root on failure, and read-only validation
  of source manifest, SDK/template/toolchain provenance, native target,
  fail-closed dependency policy, and checksummed lock graph. Package creates a
  deterministic file-only USTAR source archive outside the project through the
  shared bounded writer and returns its SHA-256/CAS identity; inspect validates
  either a source project or a standalone archive through the same owner/parser
  contracts. Test runs sanitized bounded offline Cargo stages, rehashes the
  native WASI P2 Component, and executes it through the real neutral Wasmtime
  executor with a bounded `LocalSandboxScenario`: exact typed grants/limits,
  capability/operation fixture responses, input, and success/error expectation.
  A completed local run emits the scenario's domain-separated digest with only
  a redacted `success` or `expected_error` comparison result; it never exposes
  fixture payload through that comparison projection. Local results are never
  trusted build/admission evidence. `module build`
  requires explicit tenant, actor, project, trace, correlation, and idempotency
  identity, creates a private deterministic archive, and submits it only through
  `ModuleAuthoringBuildControl` as a non-serializable
  `PreparedModuleSourceArchive`, never a transport-supplied filesystem path.
  `ModuleAuthoringSourceArchiveBuilder` is the single host-materializer path:
  it writes the private archive using the same fixed profile the owner later
  applies during source-CAS scanning, preventing CLI or Alloy limit drift. The
  shared `SourceTreeMaterializer` first creates template and future reviewed
  host source trees from data-only files under the same strict path/resource
  boundary; callers never recursively write untrusted file paths themselves.
  owner strictly scans and rehashes the
  archive, atomically publishes `<sha256-hex>.tar` into the deployment source
  CAS, selects the fixed limits, dependency egress, validation profiles, WIT,
  ABI, and target, then commits the immutable request and outbox fact. The CLI
  never invokes the worker, OCI registry, or signing service; the external
  dispatcher retains remote delivery ownership. `module publish` accepts an
  exact completed build ID and creates the current metadata bundle from
  `module-artifact.json` plus `Cargo.toml`. The owner reloads the build
  under tenant scope, fixes platform-built/third-party/sandboxed policy,
  validates and content-addresses the bundle, creates the publish request under
  a full-command deterministic identity, attaches the bundle, binds the exact
  source/Component/OCI receipt stage, and queues registry validation. Bundle,
  Component payload, and OCI manifest digests remain distinct identities. The
  CLI cannot record build-service attestation, platform admission, marketplace
  approval, or final publication. The owner manifest selects the provider
  through the generated distribution registry.
- [x] Provide module templates containing descriptor, WIT bindings, tests,
  locked toolchain, dependency policy, settings/action schemas, localization,
  and example brokered capabilities. The independently versioned
  `rustok-module-template` pure renderer emits a standalone Rust 2024 `cdylib`
  project for native `wasm32-wasip2`, pins the exact SDK and Rust toolchain,
  validates its digest-free source manifest through the owner contract, and
  includes a declared Events broker example plus a matching typed local sandbox
  scenario. The guest publishes `{topic, payload}` under the exact scenario
  topic/operation grant; the renderer validates the scenario before returning
  files. The CLI owns create-new writes
  and `Cargo.lock` generation; isolated component compilation remains a
  verification-gate requirement rather than template-renderer behavior.
- [x] Provide a local sandbox harness with the same request/policy/error contract
  and fixture capability broker as production, but no production credentials.
  `LocalSandboxHarness` delegates directly to `SandboxRuntime`; its
  `FixtureCapabilityBroker` resolves only exact caller-provided deterministic
  responses and default-denies every unregistered fixture. The harness has no
  deployment configuration or infrastructure clients. Its bounded scenario
  contract rejects unknown/oversized input, duplicate or ungranted fixtures,
  invalid operations and authoring limits, and evaluates exact outputs or stable
  expected error codes through the real Component executor for local tests.
- [x] Emit machine-readable diagnostics and build evidence usable by Alloy,
  CLI, CI, and admin without parsing human logs. `ModuleBuildResult` protocol
  v9 carries bounded typed diagnostic `(stage, code)` facts and ordered
  validation-profile outcomes in its evidence;
  every failed result must include its canonical code at its owner-canonical
  stage, while success
  cannot include failure diagnostics. The worker synthesizes those facts from
  its owner failure taxonomy and retains raw runner output only behind the
  separately authorized log reference. A successful result must report every
  requested profile as `passed`; `validation_failed` must identify an ordered
  requested profile with a `failed` outcome. The verified SLSA provenance
  envelope carries the same requested-profile and outcome lists plus the
  request-bound scenario digest and redacted scenario result.
- [x] Version SDK/templates independently and record their versions in build
  provenance. `ModuleBuildRequest` v9 requires SemVer `sdk_version` and
  `template_version`; publication-side SLSA verification requires exact
  `sdkVersion` and `templateVersion` values in the request-bound RusToK
  external-parameters envelope. Its canonical request also carries any exact
  Rhai predecessor reference for a reviewed Rust/WASM rewrite; the worker
  receives provenance only, while the governance owner verifies predecessor
  activity and runtime kind during staging and finalization.

### Verification Gate

- Identical request inputs reproduce the same logical output digest or emit a
  documented nondeterminism failure.
- Malicious archives, dependency graphs, build scripts, infinite builds,
  oversized output, network access, and undeclared WIT imports are rejected.
- Worker termination cannot affect server or sandbox runtime availability.
- The server never invokes Cargo directly for runtime marketplace installation.
- Generated guest bindings and local harness compatibility are tested against
  the exact host WIT/runtime ABI version.

Lightweight verification for the 2026-08-02 source-manifest, native template,
and module init/validate/package/inspect slice used touched-file
`rustfmt --edition 2024`, `git diff --check`, `cargo metadata --no-deps`, the CLI
registry freshness check, and the module build-worker, SDK WIT, template,
authoring CLI, and source-archive Node guardrails. All of those checks passed.
Three earlier narrow Rust test invocations for the new owner/worker boundaries
exceeded the 60-second dependency-compilation limit and were terminated without
a test result. The focused `rustok-build-source --lib` suite now passes all
five deterministic writer/strict reader/publisher tests. A filtered
`rustok-modules-cli` provider test still exceeded the 60-second limit on both
attempts and was terminated, so package/inspect CLI compilation is not yet
proved. The new bounded local-scenario harness tests pass three of three with
the real `wasm-component` feature enabled. The focused template scenario test
and the earlier five-command CLI provider test each exceeded their bounded
compilation window and were terminated, so `module test` and template
integration still lack direct crate compile evidence. The first focused owner
authoring-request attempt also exceeded that window, but after the dependency
cache completed both owner request/policy tests passed. The expanded
six-command CLI provider test again exceeded 60 seconds while compiling
dependencies and was terminated, so the new CLI command still lacks direct
crate compile evidence. `cargo test --locked -p rustok-module-build-worker` now
passes all eight unit tests, including immutable `Cargo.lock` policy fixtures
parsed as TOML documents rather than scalar values; its isolation verifier also
passes. No workspace-wide compile or test suite was run.

Focused verification on 2026-08-14 repeated `cargo test --locked -p
rustok-module-build-worker --lib`: all eight unit tests passed, including the
strict OCI-job receipt, isolation-attestation, dependency-policy, provenance,
and descriptor-finalization contracts. The build-worker isolation verifier also
passed. This proves the repository-local worker contract only; the two Phase 4
checkboxes remain open until deployment evidence proves that the pinned launcher
creates the attested hardened OCI job.

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
- [x] Define and enforce registry redirect, cross-host authentication, TLS, proxy, timeout, retry,
  maximum-size, and decompression policy. The typed
  `OciRegistryTransportPolicy` is applied by the platform-owned OCI
  Distribution transport: HTTPS-only routing, verified TLS, redirects disabled,
  no process/system proxy, connection/request deadlines, bounded manual retry,
  transfer and decompressed-response ceilings, identity-only response encoding,
  and request-wide semaphore bounds. The transport disables `reqwest` retries
  and automatic decompression, rejects cross-origin upload locations, and
  permits a cross-host bearer-token lookup only without forwarding Basic
  credentials. It owns digest/tag manifest reads, streaming blob reads,
  monolithic blob uploads, and manifest writes; unsupported workflows fail
  closed.
  OCI identities remain host/repository/digest values rather than URLs, and the
  publisher receives credentials only after the worker has obtained a
  repository-bound lease. `oci-distribution` remains only for OCI data-model
  and registry-auth DTOs; repository-owned production code does not construct
  its network client. The adapter independently bounds complete descriptor/layer
  admission to five minutes, streams config only after its declared
  descriptor-size check, cancellation-safely deletes partial staging, and
  rejects received bytes beyond declared size before parsing or digest
  acceptance. Source and unit coverage are provided by
  `verify-oci-registry-transport-policy.mjs` and `rustok-modules` transport
  tests. See [ADR: Platform-Owned OCI Registry Transport Boundary](../../DECISIONS/2026-08-06-oci-registry-transport-boundary.md).

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
  database uniqueness constraint. The operator-facing author-signature route
  accepts only context-bound signature facts; while holding the request and
  current owner-binding locks, `ModuleAuthorSignatureEvidenceCommand` rechecks
  the authenticated `modules:manage` fact or current requester/publisher/owner
  principal before an idempotent replay or write, then derives the canonical attached-artifact
  SHA-256, persists the signature digest in the immutable publication-evidence
  row used by finalization (whose schema rejects an author-signature row without
  that digest), and persists an exact-replay receipt over the
  actor/context, signature digest, signer, policy revision, reference, and
  resulting evidence. Promotion/admission requires the applicable
  distinct facts. Marketplace approval
  is not accepted through a transport evidence command: the owner creates it
  only in the atomic final-publication transaction, bound to the canonical
  staged artifact SHA-256 and the approving principal. Build-service
  attestation is also reserved: only `ModuleBuildServiceAttestationCommand`
  can record it, after validating the complete `ModuleBuildPublicationReceipt`,
  its `build_service` authority, and its co-located digest-pinned OCI
  payload/SBOM/provenance/signature identities. Platform admission is reserved
  too: `ModulePlatformAdmissionCommand` accepts only an admitted immutable
  verification decision for the exact OCI manifest, binds signature/SLSA/SBOM
  plus independent license/vulnerability policy outcomes, signer, policy
  revisions, and evidence-reference fingerprint, and
  records the platform decision without exposing verifier output. The
  verification decision carries three typed, digest-bound evidence identities
  instead of an unclassified string list. The owner requires one unique
  signature, provenance, and SBOM identity, persists the complete descriptor,
  descriptor digest, logical registry identity, OCI repository, runtime/media
  type, and those typed evidence references in a create-once
  platform-admission contract, and rejects conflicting replay. The owner
  now fails publication closed unless author-signature evidence and its
  immutable signature digest are bound to the staged artifact SHA-256 and
  build-service attestation plus platform-admission
  evidence share one exact OCI manifest subject; marketplace approval is added
  only inside that same final-release transaction. A reupload invalidates prior
  evidence for promotion: the owner accepts only facts recorded after the
  current staged-artifact timestamp.
- [x] Do not equate a valid signature with a trusted module; policy must verify
  who signed what under which build/provenance conditions. Admission accepts a
  decision only when its exact policy revisions match and signature, SLSA
  provenance, CycloneDX SBOM, license-policy, and vulnerability-policy
  verification all succeed. The verifier also requires a configured signer
  identity plus builder, build-type, source, license, and vulnerability-policy
  facts. Missing license or vulnerability outcome fields fail decoding rather
  than receiving compatibility defaults.
- [x] Compose the production publication-evidence producer. The independent
  registry-validation worker reloads the immutable completed build and current
  publication stage through the owner, acquires a short-lived registry lease
  from its deployment-owned credential broker, fetches the exact digest-pinned
  OCI artifact, and revalidates the package before calling the isolated verifier
  through mTLS with a readiness gate. It then records the reserved build-service
  attestation and complete platform-admission contract through owner operations.
  Registry credentials and trust roots never enter the server, Alloy, MCP, or
  module runtime. Partial persistence is retry-safe because both immutable
  evidence records reject conflicting replay and accept exact replay.

### 5.3 Publication Governance

- [x] Stage release from an immutable source/build result. The prerequisite
  owner read is now `SeaOrmModuleBuildService::load_completed`: it returns only
  a tenant-RLS-scoped durable request/result pair after revalidating the result
  against its immutable stored request. `stage_platform_build` now consumes the
  pair, validates the expected slug/version and successful Component/OCI
  receipt identities independently from the submitted metadata-bundle
  checksum, and appends the source, component, and OCI receipt identities in
  `registry_publish_build_staging`. The exact immutable source
  reference is retained with its digest, so final publication does not
  reconstruct lineage from a tenant-scoped build request. The component/payload digest
  and registry-returned OCI manifest digest are separate immutable identities:
  staging validates both but never compares them for equality, while final
  signature/admission joins use the manifest identity. Publication requires a
  stage newer than the current upload. `artifact_origin` is now explicit and legacy
  rows are `unclassified`, which fails closed. External prebuilts use the
  separate immutable `registry_publish_external_staging` record with either a
  reproducible source identity or an explicit absence reason, an approved
  provenance-policy revision, and an independent quarantine review. The final
  owner transaction requires the current origin-specific stage. The server now
  exposes an external-prebuilt staging adapter that derives a platform-scoped
  `ModuleCommandContext`, actor and quarantine approver plus an authenticated
  `modules.manage` fact. The owner rejects a tenant-scoped context, binds both
  canonical user principals to its actor UUID, and enforces that capability.
  The parallel platform build-stage
  adapter derives a complete tenant-scoped `ModuleCommandContext` from the
  authenticated session, telemetry trace, and idempotency key, then supplies
  only that context, a completed build ID, and the authenticated privilege fact
  to the owner RLS reload. The owner binds the context actor UUID to the
  canonical user principal and derives the current request manager from
  binding/requester facts. Both staging paths persist and compare their full
  authenticated immutable command fingerprints on replay: platform builds
  include expected revision, tenant, actor, trace, correlation, privilege,
  build, source, and component; external prebuilts include platform scope,
  expected revision, actor, trace, correlation, privilege,
  source/provenance/quarantine facts, and both authenticated principals. Any
  conflicting reuse fails closed. Alloy-authored staging uses the corresponding
  tenant-scoped context derived by authenticated HTTP and GraphQL adapters;
  those adapters pass the authenticated `modules:manage` fact, and the owner
  rechecks it or the current requester/publisher/owner principal while holding
  the request and owner-binding locks before a replay or write. Its receipt
  binds expected revision, Alloy tenant/script, reviewed source and sandbox
  facts, actor, trace, correlation, and idempotency. The owner requires the
  staged user principal to equal the context actor UUID and rejects a
  changed-context replay.
- [x] Run automated descriptor, compatibility, dependency, signature, SBOM,
  provenance, license, vulnerability, and sandbox smoke checks. The owner
  validates the claimed canonical bundle against the exact SHA-256, crate,
  publish metadata, current `module-artifact.json`, and `Cargo.toml`;
  fixture substitutions and undeclared UI manifests fail closed without
  echoing artifact content.
  Build-worker lock fixtures cover checksummed allow-listed registries,
  credential rejection, exact git revisions, and dangling dependency denial.
  Platform-built gates consume only durable passed `check`, `test`, dependency
  policy, and vulnerability profiles from the exact completed build. The
  verification-worker fixture matrix covers Cosign envelope shape, exact SLSA
  subject/builder/build-type/source/ref, CycloneDX schema and subject binding,
  component licenses, vulnerability ratings, and severity policy. Its typed
  decision exposes signature, provenance, SBOM, license-policy, and
  vulnerability-policy outcomes independently; the owner requires and
  fingerprints every outcome. External-prebuilt reconciliation additionally
  requires exact provenance/quarantine staging and author signature. Alloy
  staging executes the fixed capability-free `tests/publication_smoke.rhai`
  entrypoint through the production neutral sandbox, requires `true` without
  entity mutations, and binds execution ID, executor, runtime ABI, and effective
  policy digest. Origin-specific stages accept only their owner evidence and
  reconcile idempotently regardless of arrival order. Lightweight structural
  verification on 2026-07-20 passed `rustfmt --edition 2024`,
  `git diff --check`, and `cargo metadata --no-deps`; compile and test suites
  were intentionally not run in this worktree.
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
  idempotency key, deriving a platform-scoped command context, the actor and
  quarantine approver, plus the authenticated `modules.manage` fact. The owner
  enforces the external operator capability and binds both user principals to
  the context actor UUID. The parallel platform build-stage
  adapter accepts no caller-supplied tenant identifier and derives its owner
  RLS scope from the authenticated session; its owner command authorizes the
  current request manager from durable binding/requester facts.
- [x] Treat marketplace README, metadata, source comments, test output, and
  artifact text as untrusted content for both UI rendering and AI prompts. The
  registry bundle validator caps the complete upload at 2 MiB before JSON
  parsing, bounds embedded manifest parsing, and emits content-free diagnostics.
  Publisher name and description now pass through the owner-owned bounded
  plain-text projection, which rejects control, invisible, and bidirectional
  override characters. Marketplace category and tags are bounded canonical
  identifier tokens with a bounded, duplicate-free tag set. Catalog responses
  declare `content_format = plain_text`
  and `content_trust = untrusted_publisher_content`; the current React and
  Leptos marketplace surfaces render both fields through framework text nodes,
  never HTML/Markdown injection APIs. The same projection exposes AI input only
  as a trust-tagged structured data object with no instruction field; no current
  AI runtime consumes marketplace content, and future adapters must place that
  object in a non-system data/tool boundary rather than concatenate it into
  instructions. README, source comments, and artifact text have no catalog or
  prompt projection. Manual and remote stage reports discard caller/runner
  detail, and validation-delivery retry events replace raw errors with stable
  owner-owned diagnostics, so test output and artifact-derived failure strings
  cannot enter governance events through those paths.

### 5.4 Federated Catalog Consumption

- [x] Harden the configured remote catalog boundary. Production configuration
  accepts only an absolute HTTPS base URL without embedded credentials, query,
  or fragment and requires a stable logical registry identity independent from
  the endpoint; the identity participates in the cache namespace. The client
  rejects redirects and invalid certificates. Remote
  list failure preserves the local manifest catalog but marks the non-critical
  `marketplace_providers` readiness check degraded. Remote detail failure
  returns unavailable rather than false not-found. Slug collisions between
  non-local providers fail closed, while the local compiled manifest is the
  explicit authority for its own slug.
- [x] Define and enforce the canonical federated release consumption contract.
  Each active third-party artifact release must carry stable registry identity,
  immutable OCI manifest/payload/descriptor digests, exact source reference and
  digest, runtime kind, and durable signature/SBOM/provenance/admission evidence
  references. Remote providers fail closed when an active release omits that
  contract or claims another registry identity; `crate_name`, `git`, `rev`,
  `path`, and a checksum alone are not an installable external artifact
  identity.
- [x] Project the same canonical artifact contract from final owner publication
  rows and serve it from the registry catalog. Platform admission preserves the
  complete immutable descriptor, logical registry/repository identity, exact
  manifest and payload digests, runtime/media type, and typed
  signature/SBOM/provenance/admission evidence. Final publication joins those
  facts with exact source lineage, author signature, optional build-service
  attestation, and the transaction-local marketplace approval, validates the
  shared consumer contract, and stores it create-once beside the release.
  Publication and idempotent replay fail closed when that projection is missing
  or conflicts. The server catalog reads it through the owner service, attaches
  it to active release versions, and refuses an active release with no complete
  contract. Source-unavailable external prebuilts consequently cannot enter the
  public installable catalog. The remaining Alloy work is the digest-pinned
  CAS/OCI source materializer and authenticated import transports, not
  publication-fact reconstruction. The production evidence producer now makes
  this projection reachable once the independently required author signature
  and final governance decision are present.
- [x] Compose multiple explicitly configured registries. Registry identity is
  independent from endpoint URL, participates in cache and release identity,
  and has deterministic namespace/collision rules. An endpoint move must not
  change release identity, duplicate configured identities fail startup, and
  two registries cannot silently claim the same unqualified module slug. The
  current `RUSTOK_MARKETPLACE_REGISTRIES` JSON contract composes no implicit
  remote provider when absent and rejects non-canonical IDs, non-HTTPS
  endpoints, credentials, query/fragment endpoint state, and redirects.
- [x] Require reproducible source identity for publicly marketable third-party
  releases. The federated catalog rejects every active third-party release
  without an immutable artifact contract, and that contract requires an exact
  source reference plus canonical source digest. Source-unavailable external
  prebuilts remain private quarantine sideloads and cannot appear as normally
  installable public catalog releases.
- [x] Expose freshness and last-success evidence per configured registry to
  operator transports and declarative admin UI, not only aggregate readiness.
  `MarketplaceRegistryFreshness` is the framework-neutral API DTO and excludes
  endpoint URLs and remote error content. The modules owner catalog port
  projects one record per configured logical registry while excluding the local
  manifest provider. GraphQL and native Leptos transports require
  `modules.manage`; both Leptos and Next module operator surfaces show the same
  status, last-success timestamp, and consecutive-failure count and refresh the
  evidence after catalog access. The aggregate readiness check remains a
  non-critical availability signal, not the operator detail contract.
  `verify-marketplace-registry-freshness.mjs` locks owner-port use, permission
  enforcement, content minimization, and cross-admin parity.

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

- [x] Represent every execution with draft ID and monotonic revision.
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
  exact replays return terminal evidence only while the owning draft remains
  current, concurrent callers see a bounded pending lease, and only an expired
  lease can be reclaimed against the same immutable source snapshot. A test
  completion racing with deletion settles the held lease for retention, then
  returns `NotFound` instead of test evidence. Host HTTP and GraphQL derive `scripts.manage`
  authority from authentication. Canonical Alloy authoring now uses only
  host-composed HTTP routes and GraphQL: every operator source/history read,
  validation, manual run, lifecycle, review, and test operation requires a
  matching authenticated tenant plus `scripts.manage`; source-revision author
  and manual-execution actor are derived from that principal. The generic
  in-memory Axum router was removed, so it cannot bypass tenant, permission,
  or provenance policy. Prompt/tool provenance uses an optional canonical
  prompt digest only; raw prompts, tool inputs/results, review reasons, and
  test diagnostics have no post-expiry retention path. Build-command
  idempotency remains pending.
  Alloy release staging now selects the current immutable source revision and
  latest approved review, then delegates an idempotent `alloy_authored` stage
  to `rustok-modules`. The owner records source/review evidence together with
  the Alloy tenant/script identity and remains the only marketplace writer.
  Both authenticated adapters construct one tenant-scoped
  `ModuleCommandContext`; the owner persists its expected revision and complete
  actor/trace/correlation/idempotency receipt, binds the staged principal to
  that actor UUID, and rejects conflicting replay evidence.
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
  publication. Production execution history now also persists the exact source
  revision/digest, sandbox policy digest, executor kind, and runtime ABI and
  exposes that redacted evidence through REST and GraphQL.
- Alloy lifecycle status mutations now require the expected revision on both
  REST activate/pause and GraphQL activate/pause/disable/archive/reset-errors
  transports, so stale status writes fail closed with a revision conflict.
- Alloy deletion now also requires the expected revision on direct REST,
  host-composed REST, and GraphQL transports; owner storage applies the same
  version predicate atomically before removing the script.
- Generic MCP Alloy script CRUD, validation, and execution have been removed:
  the generic adapter cannot compose an owner-scoped Alloy runtime, so it must
  not simulate tenant or actor binding. Canonical authoring stays on
  host-composed HTTP, GraphQL, and authenticated remote MCP; no generic stdio
  or in-process MCP path can bypass owner CAS or immutable snapshot checks.
- [x] Compose remote MCP script authoring from the same owner-scoped Alloy
  runtime as HTTP and GraphQL. The authenticated JSON/SSE remote transport
  derives tenant and actor identity from its durable MCP binding, verifies that
  the identity tenant equals the binding tenant before its audit decision, and requires
  `scripts.manage` through the standard tool policy before it constructs
  `AlloyAuthoringService` from `SharedAlloyRuntime::scoped`. The typed service
  owns script read/create/update/delete, source-revision history, validation,
  manual execution, review, workspace tests, and lifecycle CAS commands; it
  returns source-redacted script, revision, execution, review, and test
  evidence only. Its operation audit replaces caller metadata with a fixed
  redaction marker, so source, tests, and diagnostics have no audit persistence
  path. Its production SeaORM test proves a second tenant cannot update the
  first tenant's script, and the control-plane verifier prevents these
  remote-only tool names from entering generic stdio/in-process MCP. Generic
  MCP remains a design-scaffold-only tool: it records requested transports in
  documentation and never generates fake GraphQL or REST handlers.
- [x] Reject execution/publish commands for stale revisions. Caller-driven
  manual, test, review, lifecycle, deletion, and release-stage commands use
  explicit revision CAS; hook and schedule dispatch execute the exact current
  owner-selected snapshot passed to `ScriptExecutor`.
- [x] Execute validation, tests, manual runs, hooks, schedules, and preview
  scenarios through `SandboxRuntime`.
- [x] Convert Alloy entity/parameter behavior into explicit request-scoped
  bindings without adding generic Alloy concepts to `rustok-sandbox`.
- [x] Persist execution evidence linked to revision and policy revision.
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
- [x] Validate declared capabilities from observed source tool use. Alloy scans
  every executable `src/*.rhai` file before descriptor staging and packaging;
  `http_*` helpers map to `platform.http`, while `capability_call` requires a
  literal valid capability name. The descriptor set must match exactly: missing
  and unused declarations, dynamically selected names, and attempts to shadow
  a reserved helper fail closed before owner admission.
- [x] Complete release source/descriptor publication through `rustok-modules`;
  Alloy does not write marketplace tables. The revision-pinned reviewed-source
  staging gate, origin-aware owner artifact upload, and authenticated HTTP /
  GraphQL staging adapters are complete; final marketplace promotion remains
  an owner governance operation.
- [x] Preserve author, prompt/tool provenance, tests, and review evidence under
  explicit retention/redaction rules. The first durable provenance slice now
  records the authenticated author plus owner-generated HTTP, GraphQL, remote
  MCP, release-import, or internal origin and normalized tool name on each
  immutable source revision. It accepts only an optional canonical SHA-256
  prompt digest from a separately governed owner; raw prompts, tool arguments,
  model completions, and tool results have no Alloy persistence field. Remote
  MCP and its audit metadata remain source-redacted, and deleted scripts hide
  retained source/review/test evidence from owner reads and idempotency replay.
  Deletion itself now requires an authenticated owner-derived actor, a bounded
  reason, and an idempotency key on every HTTP, GraphQL, and remote MCP command.
  The tenant-scoped tombstone records actor, reason, request digest, key, and
  deletion time in the same deletion transaction; only an exact digest replay
  succeeds after removal, while a mismatched reuse fails closed. A racing test
  completion settles its retained lease and returns `NotFound`; a durable
  tombstone makes the deleted script ID non-reusable until retention policy
  purges it. `rustok-core::RetentionPolicy` now defines the shared closed
  `owner_lifecycle` / `retain_until` / `legal_hold` vocabulary, deadline
  invariant, and legal-hold collection guard used by Translation Memory. Alloy
  deletion now assigns a fixed 30-day `retain_until` policy in the tombstone;
  its global owner scheduler atomically collects an expired tombstone, source
  revisions, reviews, and test runs, then preserves only a content-free receipt
  with tenant/script identity, timing, counts, and request digest. The collector
  selects only `retain_until`, so a legal hold is excluded from automatic
  collection. Authenticated owner HTTP, GraphQL, and remote MCP now expose only
  source-free retention state and apply tenant-scoped, deletion-digest-bound,
  retention-revision CAS commands to place or release a legal hold. Applying a
  hold clears the collection deadline; releasing it starts a fresh fixed
  30-day `retain_until` window. The durable command receipt records actor,
  action, policy, revision, and request digests without retaining the reason.
  At expiry the collector irreversibly erases review reasons and test
  diagnostics rather than retaining a redacted copy.

### 6.3 Marketplace Fork and Continued Development

- [x] Persist an imported Rhai workspace as a new tenant-scoped draft with its
  exact immutable parent release. Import storage now reserves a durable
  `(tenant_id, idempotency_key)` receipt before creating the draft and first
  source revision in the same transaction. Exact replay returns the same
  draft; conflicting replay and duplicate tenant-scoped names fail closed.
- [x] Compose the production owner source provider and authenticated HTTP /
  GraphQL import adapters. The host resolves an exact active release only
  through `rustok-modules`' publication projection, verifies the admitted Rhai
  workspace media type and source digest, then reads and canonicalizes the
  digest-pinned workspace from verified CAS bytes. `POST
  /api/alloy/releases/import` and GraphQL `importPublishedRelease` derive the
  tenant and actor from host authentication and require both `scripts.manage`
  and `modules.manage`; neither accepts catalog DTO data or a mutable OCI tag.
- [x] Compose the tenant-bound remote MCP import adapter. The authenticated
  `alloy_import_published_release` tool on `/api/mcp/runtime/tools/call` and
  `/api/mcp/runtime/tools/stream` derives the tenant and actor from the durable
  MCP runtime binding, requires both `scripts.manage` and `modules.manage`,
  creates a tenant-scoped Alloy registry, and injects the same owner-backed
  source provider used by HTTP and GraphQL. Its redacted result exposes only
  draft identity and immutable parent release lineage; generic stdio MCP does
  not advertise an import tool without this host composition.
- [x] Fork contracts record the parent release and never mutate or overwrite
  it. Imported parent identity is persisted on the draft and every immutable
  source revision, and storage rejects replacement or removal.
- [x] Require a newer semantic version and new source/artifact digest in the
  immutable `ArtifactRelease::fork` contract.
- [x] Allow tests and preview against the same WIT/capability policy as the
  installed parent. Alloy persists only immutable parent release lineage; for
  every imported draft execution and revision-pinned workspace test, the host
  resolves that exact release through `rustok-modules`' active tenant
  installation and sandbox-policy resolvers. The resolver revalidates
  admission, lifecycle, descriptor runtime ABI, and policy revision. Missing,
  disabled, stale, or mismatched parent state fails closed without a default
  policy fallback; the sandbox test phase remains explicit for broker-side
  phase constraints. Publication smoke remains deliberately zero-grant while
  retaining the resolved parent limits.
- [x] Publish the fork through the same governance pipeline as any release.
  The revisioned Alloy stager carries the immutable imported parent through the
  zero-grant smoke and the owner-only stage command. The owner verifies the
  exact active predecessor, unchanged slug, and strictly newer semantic
  version, then records direct parent lineage beside the final immutable
  artifact contract in the same publication transaction. The next published
  Rhai import reads that owner-persisted lineage; catalog DTOs, mutable tags,
  and caller-supplied predecessor data are never authoritative.

### 6.4 Rhai-to-Rust Evolution

This is an AI-assisted rewrite and validation workflow, not a transparent AST
transpiler.

- [ ] Generate typed Rust against the versioned WIT guest contract.
- [x] Preserve the Rhai parent release and source lineage. The immutable
  platform build request carries the optional exact predecessor, and the owner
  stores it in the platform-build staging receipt before it derives the final
  marketplace artifact contract. Staging and finalization revalidate a
  same-slug, strictly older, active published Rhai predecessor; idempotent
  replay compares the complete reference and cannot replace it.
- [x] Dispatch an approved Rust Component candidate through the owner build
  control without accepting a caller filesystem path. The host-composed Alloy
  evolution service rechecks candidate tenant, approved source/scenario review,
  parent-release identity, and manifest version before it materializes source
  only in a correlation-bound private work directory. The shared authoring
  archive builder derives the source-CAS digest; the durable Alloy receipt
  binds that archive digest and its exact `cas://` reference to the candidate,
  authenticated command context, and returned build-request ID. Exact replay
  returns the receipt before materialization, while changed idempotency
  evidence, non-approved candidates, invalid source references, and deleted
  parents fail closed. Receipt evidence is deleted with candidate retention.
- [ ] Verify generated Rust build execution only through the isolated build
  worker, including deployment evidence for the hardened OCI job. The completed
  owner handoff above is deliberately not execution evidence.
- [ ] Compare scenario/contract evidence between Rhai and WASM versions. The
  owner now defines, validates, persists, and security-reconciles a
  domain-separated digest for the canonical zero-input, zero-grant Rhai
  publication-smoke scenario. The neutral sandbox also derives a separate
  domain-separated canonical digest for every validated bounded
  `LocalSandboxScenario` and emits only that digest plus a redacted
  `success`/`expected_error` result for a completed local run. Candidate
  adapters must use that comparison tuple without exposing fixture payload.
  Build protocol v9 additionally pins the candidate source-local scenario path
  and canonical digest, and the isolated worker reopens, parses, digest-checks,
  and capability-subsets it against the source manifest before job launch.
  A successful runner result is accepted only with the matching redacted
  comparison tuple. This binds the execution contract but is not itself proof
  of the hardened OCI job deployment or Rhai/WASM semantic parity.
  The current Alloy publication smoke is still its fixed Rhai-only evidence;
  generated candidate execution and complete Rhai/WASM parity remain open.
- [x] Persist immutable Rust Component candidate input before source
  preparation. Alloy accepts data-only UTF-8 source files only after the
  shared source-tree validation; its durable candidate record pins tenant,
  approved current Rhai revision/source digest, exact published Rhai parent,
  canonical candidate source digest, scenario digest, actor, and idempotency
  receipt. SeaORM and in-memory owners both fail closed on stale, unapproved,
  release-unpinned, conflicting, or cross-tenant requests; source is not
  returned by the operator response. Candidate source and receipt are retained
  and GC-collected with the owning deleted draft's evidence lifecycle. The
  candidate manifest must retain the parent slug and declare a strictly newer
  semantic version before it is admitted.
- [x] Record immutable Rust Component candidate review decisions. The candidate
  owner uses the canonical review transition state machine with decision rows
  bound to exact candidate source/scenario digests, policy revision, actor,
  reason, and idempotency receipt. Tenant-scoped reads and deletion retention
  hide then collect both candidates and their reviews. A candidate approval is
  deliberately not an implicit build enqueue; the next owner build operation
  must revalidate it.
- [ ] Publish the WASM implementation as a new release after review.
- [x] Never emit or runtime-load a native dynamic library. Rust Component
  authoring fixes the only build target to `wasm32-wasip2`; source metadata,
  CLI commands, immutable build requests, OCI-job environment, and the
  independently emitted job receipt must all repeat that exact target. The
  component worker accepts only the inspected Component output and its runtime
  executes it through the Wasm Component sandbox. The isolation verifier scans
  Alloy, authoring, worker, sandbox, SDK, and template production sources for
  dynamic loader APIs/dependencies and fails on any native loader path.

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
- [x] Move the `tenantModules` override/settings query off direct
  `tenant_modules` SQL. The modules owner returns a bounded
  `TenantModuleOverrideSnapshot` list through `EffectivePolicyService`, while
  GraphQL performs transport mapping only. Broader resolver migration remains
  open under the aggregate item above.
- [x] Remove lifecycle mutation post-command ORM rereads. Toggle, post-hook
  retry, compensation, and settings writes return owner-issued operation or
  state facts. The server maps those facts to GraphQL and resolves inherited
  availability through the owner policy service; it no longer loads
  `tenant_modules` or `module_operations` models after a lifecycle command.
- [x] Bind lifecycle recovery to the authenticated tenant in the owner writer.
  Retry and compensation commands reject a foreign operation as not found
  before dispatch or state change. Retry returns the completed owner recovery
  plan, while compensation returns its exact module identity; GraphQL no longer
  preloads an operation for tenant authorization or reloads its plan after the
  command.
- [x] Route GraphQL platform-native install, uninstall, and upgrade through one
  typed composition adapter. Resolvers require a direct SuperAdmin principal
  whose authenticated tenant matches the routed tenant and whose effective
  permissions include `modules:manage`; a tenant administrator cannot mutate
  global `platform_state`. The routed tenant is authorization evidence only:
  resolvers construct a platform-scoped `ModuleCommandContext` with no tenant
  identity, then provide that context, revision, idempotency key, and requested
  module change to the adapter. The owner admits the command in the platform
  receipt namespace before the adapter obtains the durable snapshot or applies
  the static host-manifest adapter, then controls the
  composition-CAS/build/receipt transaction. Resolvers no longer load, mutate,
  validate, serialize, or hash a manifest directly. The
  `installedModules` query also consumes the adapter's owner-backed installed
  projection rather than inspecting the manifest in GraphQL.
- [x] Move remote validation lease observability behind the registry owner. The
  runtime guardrail receives the active and expired running-remote-lease counts
  from `SeaOrmModuleGovernanceService`; it no longer queries
  `registry_validation_stages` through a server model. A failed owner snapshot
  is a critical guardrail condition rather than a synthetic zero-count success.
  Manual validation-stage reports likewise pass raw stage/status/reason inputs
  to the owner, which canonicalizes and validates them before state mutation.
  The obsolete server stage model, status parser, and ignored `detail` request
  field were removed; the request rejects unknown fields.
- [x] Move platform build/release history, active-release, and rollback
  precondition reads behind `rustok-build::BuildService`. The owner enforces
  bounded history pages; GraphQL no longer imports the corresponding SeaORM
  entities. The server now exposes the host-composed
  `rustok_build::SharedBuildControl`, so native admin active/history reads and
  rollback also use the owner port and the event-aware server implementation.
  Module build-worker and registry-release transports remain open under the
  aggregate item above.
- [x] Move marketplace list/detail reads to the host-composed
  `SharedModuleMarketplaceCatalog`. GraphQL and native admin consume the same
  owner DTO, and detail lifecycle metadata is mapped directly from the owner
  snapshot without transport-local stage or moderation fallbacks. The durable
  registry-release projection that enriches static catalog entries with
  localized active metadata, canonical artifact references, yanked versions,
  and publisher identity is now also an owner query on
  `SeaOrmModuleGovernanceService`. GraphQL and the public registry HTTP adapter
  map those canonical facts into their transport shapes and no longer query
  registry release or translation tables. The public publish-status projection
  is likewise owner-scoped by exact request ID: request identity,
  warnings/errors, acceptance, approval-override guidance, semantic next
  action, validation stages, gates, and actor-visible actions are derived from
  durable registry facts in `rustok-modules`; the HTTP adapter supplies
  authenticated actor facts and maps the semantic action to its route/text
  without publish-request SQL or policy reconstruction. The same exact status
  projection serves approval previews, removing the duplicate focused
  follow-up read path. After all required stages pass, an approved request
  resolves to final publication rather than repeating a completed stage
  operation.
  External-prebuilt and platform-build staging similarly dispatch directly to
  owner commands and reuse the same exact status snapshot for dry-run and
  committed response identity/status, rather than a server-local
  publish-request existence or post-command model read; the canonical owner
  `not_found` error reaches the HTTP mapper.
  Creation and artifact upload also return the canonical exact status
  projection. Creation carries only authenticated principal and privilege
  facts; the owner checks the current binding before its write or idempotent
  replay. Upload derives its destination through an owner-authorized,
  SHA-256 content-addressed slot; the host conditionally creates the object,
  rehashes an existing collision, and may never select a storage key or delete
  a prior artifact inline. A live attach receives the same authenticated command
  context and records its revision, metadata, storage result, actor, trace,
  correlation, principal, and privilege facts in the transition transaction.
  Exact replay returns that committed result; changed input or an attachment
  without a receipt fails closed. The platform-authoring producer uses the same
  slot contract, leaving retention-aware owner policy as the only
  historical-object cleanup authority.
  Release yanking also dispatches directly to the owner. The command carries
  only authenticated principal/privilege facts; the owner locks the exact
  release, derives authorization from `modules.manage`, the current owner
  binding, or publisher identity, and returns a minimal owner-issued mutation
  result without server release-model reads before or after the transition.
  Owner transfer follows the same path: the owner locks the existing binding,
  authorizes `modules.manage` or the bound owner, and records the transfer in
  that transaction. The former server owner/release, publish-request,
  validation-job, and governance-event SeaORM models, their server-local status
  mapper, and the unmounted adapter test module were deleted after all
  production callers moved to owner projections.
  The registry access middleware now asks only for an
  owner-derived request authorization snapshot before forwarding to the
  controller; it no longer loads a publish request or owner binding itself.
  Validation enqueue, manual validation-stage reporting, and approve/reject/
  request-changes/hold/resume responses all consume the exact owner status
  projection after their mutation. The HTTP adapter no longer reads an updated
  request model to reconstruct acceptance, errors, or next-step guidance.
  Remote-runner heartbeat and terminal-completion responses likewise use the
  owner-issued `ModuleRemoteValidationStageTransition` and never reread a
  server validation-stage model. The duplicate registry-governance remote
  runner mutation adapter was deleted; only the owner-routed transition path
  remains. Both remote transition HTTP paths also propagate the owner-issued
  governance error category/code rather than maintaining a lease-specific
  error taxonomy; not-found detail remains content-free.
  That exact owner status projection now carries authenticated `can_manage`
  and `can_review` facts. Live validation, validation-stage reporting, and
  moderation authorize through them rather than server-local publish-request
  or owner-binding reads; unauthenticated status projections expose no
  governance actions. The projection also supplies rejected-request retry
  eligibility, effective publisher identity, and latest validation-stage facts,
  so every live operation on an existing request avoids server
  publish-request-model reads after its owner status lookup.
  The artifact-download adapter uses a separate host-only owner projection for
  attached storage key and content type, treating missing/unattached artifacts
  as unavailable without exposing storage topology through the public status
  contract or reading a server request model.
  Validation-queue and validation-stage dry-run previews also obtain request
  identity only from the authenticated exact owner status snapshot; their live
  commands remain owner-authorized mutation paths rather than server-local
  request preflights.
  Approve, reject, request-changes, hold, and resume previews follow the same
  path; approval override warning text and pending-stage facts remain owner
  derived rather than HTTP-local policy.
  Owner transfer sends only authenticated actor/privilege facts to the owner.
  The owner locks the binding, derives authorization, and records the transition
  without a server owner-table preflight or post-command reread.
- [x] Map canonical codes/details without reconstructing issue/retry taxonomy.
  `ModuleGovernanceError` now owns a stable category and code contract. HTTP
  maps only that category to its envelope status, preserves the owner detail
  where safe, and keeps `not_found` detail content-free; it no longer contains
  a parallel lifecycle-error taxonomy.
- [x] Require typed actor, tenant, permission, idempotency, and revision inputs.
  The canonical static-module lifecycle target is the owner-owned
  `module_static_tenant_lifecycle` aggregate from
  [ADR 2026-08-20](../../DECISIONS/2026-08-20-static-module-lifecycle-revision.md):
  it has revision `0` for inherited/default state, is distinct from the
  `tenant_modules` override projection, and accepts a write only after exact
  idempotency admission, expected-revision CAS, and an execution claim. The
  claim covers pre/post hooks as well as the transaction that commits override
  or settings state; different commands fail closed while it is held. Toggle,
  normalized settings, post-hook retry, and compensation carry the same
  authenticated actor/tenant/trace/correlation/idempotency/revision context, expose the resulting
  revision through owner snapshots and GraphQL, and use the same aggregate.
  Hook operations retain `module_operations` for recovery evidence, while
  settings retain an exact result in the shared owner-operation receipt ledger.
  `moduleRegistry.lifecycleRevision` and `tenantModules.revision` expose the
  aggregate read model; `toggleModule`, `updateModuleSettings`,
  `compareAndSwapModuleSettings`, post-hook retry, and compensation require a
  non-nil idempotency UUID plus a non-negative expected revision. The settings
  owner completes its receipt in the same transaction as normalized settings,
  aggregate advancement, and claim release; exact replays return the retained
  result, while retained terminal failures replay their original typed outcome:
  reviewed-snapshot conflict, disabled-module validation, revision conflict,
  idempotency conflict, or active-operation failure. Admin and Next Admin
  forward the owner revision and refresh it from every mutation result; settings
  writes have no native/server-function fallback.
- [x] Static-composition GraphQL mutations derive tenant, actor, and
  `modules:manage` permission from the authenticated context, require a
  non-nil idempotency UUID and positive expected revision, and never accept
  caller-controlled actor text. The composition owner admits the canonical
  command before the host reads or adapts a manifest, scopes the durable receipt
  to the tenant and owner operation, completes it in the same transaction as
  composition CAS and build enqueue, and replays the original immutable build
  after later composition changes. The admin fetches only the owner revision,
  forwards it with a fresh UUID, and does not calculate a manifest hash or parse
  build execution identity. Broader typed-context coverage remains open under
  the aggregate item.
- [x] Artifact tenant-lifecycle GraphQL derives tenant, actor, and
  `modules:manage` permission from authenticated context and exposes the
  owner-issued lifecycle snapshot for one admitted Optional installation. The
  snapshot returns inherited enabled intent as revision `0` with expected
  revision `1`; explicit intent returns its current revision. The single
  enablement mutation requires installation UUID, boolean intent, positive CAS
  revision, reason, and idempotency UUID, then delegates to the existing
  owner-held revision-CAS/exact-replay/audit/outbox transaction. Owner conflict
  and storage details are not exposed through GraphQL. Broader typed-context
  coverage remains open under the aggregate item.
- [x] Static module-lifecycle GraphQL toggle derives tenant, actor, and
  `modules:manage` permission from authenticated context and requires a
  non-nil idempotency UUID plus a non-negative aggregate revision. Its
  owner-only `ModuleLifecycleToggleCommand` carries one tenant-matched
  `ModuleCommandContext`, rejects invalid context evidence, and persists actor,
  trace, correlation, and idempotency in its operation journal before evaluating
  a no-op transition. A no-op therefore persists explicit intent and returns
  the committed original operation on an exact retry; a changed context maps to
  non-retryable `IDEMPOTENCY_CONFLICT`. It uses the same aggregate as settings
  and recovery.
- [x] Static lifecycle recovery GraphQL mutations derive tenant, actor, and
  `modules:manage` permission from authenticated context and require non-nil
  idempotency UUIDs plus a non-negative aggregate revision. Retry and
  compensation now enter the owner only through
  `ModuleLifecycleRecoveryCommand`; it rejects nil identity, derives persisted
  actor text, trace, correlation, and idempotency evidence from the authenticated
  command context, and cannot accept transport-controlled actor labels or
  correlation. It claims the same aggregate before hook dispatch and
  releases it on every terminal path, including configuration failure.
- [x] Keep subscriptions/build events as transport adapters over owner events.
  Build completion contains build facts only. Static admission, rollout,
  activation, and recovery events come exclusively from `rustok-modules` and
  are not synthesized from `BuildCompleted`.

### 7.2 Native Leptos Server Functions

- [x] Add owner-backed native operations for the current Leptos marketplace and
  registry lifecycle reads. Both resolve the host-composed
  `SharedModuleMarketplaceCatalog`; lifecycle policy, stages, gates, events,
  and action availability come from the modules owner. Remaining native
  mutation/parity coverage is tracked by the aggregate Phase 7 gate.
- [ ] Reuse canonical DTOs through the approved framework-neutral contract
  layer; do not duplicate GraphQL types in the UI package. The artifact UI
  portion uses `rustok-api::ArtifactUiContributionView` and its typed
  content/surface/confirmation contract plus the redacted
  `ArtifactBindingExecutionAuditEntry` contract: `rustok-modules` creates
  those owner projections and HTTP/native/GraphQL clients consume them without
  descriptor-local duplicates. Other control-plane DTO families remain under
  this aggregate item.
- [x] Reuse canonical framework-neutral build/release snapshots across
  `SharedBuildControl`, GraphQL, and native admin. `rustok-api` owns the typed
  status/stage/profile contract, `rustok-build` alone maps SeaORM persistence,
  and the admin no longer defines or populates parallel build/release DTOs.
  The aggregate item remains open for the other control-plane DTO families.
- [ ] Preserve GraphQL as the public/headless surface. The artifact UI read
  slice is available through `artifactUiContributions(installationId)`: it
  adapts the one `rustok-api::ArtifactUiContributionView` projection, takes
  its locale only from the resolved request context, and shares the HTTP
  adapter's per-contribution dynamic-RBAC and exact-locale fail-closed rules.
  Its `executeArtifactUiAction(installationId, contributionId, input,
  idempotencyKey)` mutation resolves an admitted action/form contribution to
  its exact Command binding, then shares REST's effective-policy, dynamic-RBAC,
  durable-idempotency, sandbox-dispatch, and audit path without exposing a raw
  binding selector. `artifactUiActionAudit(installationId, contributionId)`
  provides its redacted execution evidence through the same contribution
  resolution and dynamic-RBAC path, again without a raw binding selector.
  Tenant dynamic-installation lifecycle also has owner-backed GraphQL commands:
  `activateTenantArtifact`, `deactivateTenantArtifact`,
  `uninstallTenantArtifact`, and `rollbackTenantArtifact`. They derive tenant,
  actor, scope, and `modules:manage` from authenticated context and expose no
  platform-scope or arbitrary rollback-target selector. The aggregate remains
  open for the other control-plane operations.
- [ ] Add GraphQL/native parity fixtures for success, validation, conflict,
  policy denial, recovery, and build failure.

### 7.3 Dynamic Marketplace UI Boundary

Compile-time module-owned Leptos/Next/Flutter packages cannot be the normal UI
delivery mechanism for a runtime-installed artifact. The marketplace therefore
uses an explicit UI trust boundary.

- [x] The current marketplace contract requires host-rendered declarative
  contributions for settings, commands/actions, status, help, navigation
  metadata, tables/forms supported by the shared UI schema, and storefront
  slots. `ArtifactUiContribution` now uses a typed surface/content vocabulary,
  rather than arbitrary JSON or executable host metadata; result/error
  presentation remains a host responsibility over canonical binding outcomes.
- [x] Define one framework-neutral UI contribution schema and validate it with
  bundled JSON Schema. `rustok-modules/contracts/ui-contribution.schema.json`
  is validated at descriptor admission in addition to strict typed decoding.
  The contract has no component source, markup, CSS, URL, iframe, query,
  authentication, or module-controlled locale fallback field. Leptos, Next,
  and Flutter hosts must adapt that one contract rather than receive
  host-specific artifacts.
- [x] Bind every action to an admitted runtime binding, permission, input/output
  schema, confirmation/destructive flag, idempotency, and audit policy. The
  descriptor contract rejects an action/form unless it references the exact
  admitted `Command` binding with the same module-owned permission, bundled
  input/output schemas, required idempotency, a consistent destructive
  confirmation, and required audit policy. The platform-owned
  `POST /api/artifacts/{installation_id}/ui/contributions/{contribution_id}/execute`
  route accepts only that reviewed Action/Form identity, then delegates to the
  existing RBAC, schema, durable idempotency, and audited sandbox command path;
  it cannot be used to select an arbitrary binding. The runtime writes the
  host-selected admitted binding ID as a redacted neutral sandbox audit label;
  `GET /api/artifacts/{installation_id}/ui/contributions/{contribution_id}/audit`
  resolves and authorizes that same contribution before reading only its exact
  tenant/installation/binding evidence. The response contains no payload,
  output, actor, trace, credential, capability, or grant data.
- [x] Resolve route, navigation, child-page, and storefront slot collisions in
  the owner control plane before activation. `SeaOrmArtifactInstallationStore`
  derives only typed navigation-route (including child-page) and storefront-slot
  identities, acquires durable global resource locks in deterministic order, and
  revalidates every candidate descriptor inside the lifecycle transaction. A
  tenant activation compares its candidate with its platform baseline and its
  own overlay; a platform activation compares it with every active tenant
  overlay. The transaction-local PostgreSQL control-plane owner context permits
  that platform-wide safety check without widening tenant-scoped queries. The
  same guard runs before rollback can reactivate a predecessor, and a conflict
  leaves the candidate admission revision unchanged.
- Earlier focused verification on 2026-08-22 passed all 227
  `rustok-modules` library tests after aligning the Alloy-fork SQLite fixture
  with the owner-required publish-request `updated_at` field. After adding the
  audit reader, the focused binding-evidence and canonical SQLite-migration
  tests both passed. Exact-locale projection and binding-identity redaction
  tests also passed, as did the package-scoped `cargo check --locked -p
  rustok-server`. The shared GraphQL contribution, action, and audit adapters
  then passed that same package check plus focused `rustok-api artifact_ui`
  (3 passed), server `artifact_ui` (1 passed), and `module_security` (4
  passed) library tests. The tenant lifecycle GraphQL transport then passed
  `cargo check --locked -p rustok-server`, six `module_security` tests
  covering the lifecycle snapshot, all five lifecycle mutations, composition
  snapshot, registry freshness, and enabled-module availability, and its
  focused sanitized-conflict test. `rustfmt --edition 2024`, `git diff
  --check`, and `cargo metadata --locked --no-deps` also passed. No
  workspace-wide compile or test run is claimed.
- [x] Use the host-provided effective locale and signed/admitted localization
  catalogs; reject module-owned locale fallback chains and unsafe markup. The
  admitted contract has digest-verified, bounded plain-text catalogs with an
  identical key set per declared locale and exact-locale lookup only; unsafe
  markup is rejected. `GET /api/artifacts/{installation_id}/ui/contributions`
  receives its locale exclusively from the server middleware's
  `ResolvedRequestLocale`, filters each contribution through its dynamic RBAC
  permission, and returns only the framework-neutral
  `rustok-api::ArtifactUiContributionView` localized projection. The headless
  GraphQL `artifactUiContributions(installationId)` read uses that exact same
  server adapter and its middleware-resolved `RequestContext.locale`; it has
  no locale argument or fallback. The headless
  `executeArtifactUiAction(installationId, contributionId, input,
  idempotencyKey)` mutation resolves the admitted contribution to its exact
  Command binding and uses the same effective-policy, dynamic-RBAC,
  durable-idempotency, sandbox-dispatch, and audit path as REST. It has no raw
  binding selector. Its `artifactUiActionAudit(installationId, contributionId)`
  companion uses that same contribution-resolution and dynamic-RBAC path to
  return only `rustok-api::ArtifactBindingExecutionAuditEntry` facts. The
  client cannot select a locale; an unavailable exact locale omits that
  contribution rather than falling back. Catalogs, localization keys, binding
  IDs, permissions, executable UI material, payloads, outputs, actors, traces,
  credentials, capabilities, and grants never enter either response.
- [ ] If custom untrusted web UI is introduced, run it from an isolated origin in
  a sandboxed iframe with strict CSP and a versioned, origin-checked,
  schema-validated message SDK. Do not provide platform cookies, bearer tokens,
  DOM access, arbitrary navigation, or direct APIs.
- [ ] Native Leptos, Next, and Flutter code packages are allowed only through
  reviewed static promotion/distribution composition.
- [ ] Add a dedicated ADR before enabling iframe/custom UI artifacts; the
  current declarative path must not grow ad-hoc executable expressions.

### 7.4 Admin Simplification

- [x] Remove direct SQL to `platform_state`, build, registry, release, publish,
  installation, and lifecycle tables from the admin module transport. The
  native adapter now calls owner services or the authenticated server
  governance transport and performs DTO mapping only.
- [x] Remove client-side success fallbacks that synthesized module registry,
  installed-module, tenant-intent, and marketplace state from the generated
  admin navigation registry after GraphQL failures. The transport now preserves
  owner errors; native owner cutover remains open under the surrounding items.
- [x] Remove admin-owned module/Cargo manifest scanning and filesystem loading.
- [x] Remove admin-owned canonical hashing, dependency solving, build planning,
  and marketplace synthesis. The active-composition DTO is retained only as a
  transport-neutral snapshot shape.
- [x] Remove local lifecycle/governance/status/retry policy derivation. The
  remaining admin helpers are presentation-only labels and command rendering
  over owner-provided facts.
- [x] Keep transport facade, route/query state, view models, optimistic UI keyed
  by revision/idempotency, and presentation effects.
  Both Leptos Admin and Next Admin now maintain full parity on the module control
  plane: Next Admin consumes the canonical server GraphQL queries/mutations and REST
  catalog governance endpoints directly through `src/shared/api/modules.ts`,
  renders observation windows, monotonic security epochs, and single-attempt emergency
  rollback via `TransitionControlCard`, renders registry quality and platform compatibility
  checks via `MetadataChecklistView`, provides dry-run and live moderation through
  `GovernanceForm`, and edits tenant-scoped configuration via `ModuleSettingsDialog`
  with CAS revision checks and idempotency UUIDs. Integration parity is tested by
  `apps/server/tests/module_graphql_native_parity.rs`.
- [x] Add a static verifier preventing backend logic from returning to the admin
  host. The module control-plane guard now scans the admin module transport for
  SQL, filesystem, hashing, dependency, build-planning, and direct
  `BuildService` APIs while allowing owner-backed DTO and command mapping.
  Native build reads and rollback now use `SharedBuildControl`; broader
  transport parity and canonical-error contracts remain separate Phase 7 work.

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

- [x] Return decision, contributing facts, policy revision, and denial reasons.
  `ModuleEffectivePolicyDecision` now covers every declared Phase 8 input with
  typed facts and stable denial reasons under one deterministic `sha256:`
  policy revision. Focused owner tests cover catalog/default/tenant intent,
  artifact installation and capability evidence, dependencies, channel,
  maintenance, node readiness, registry state, quarantine, revocation, and
  unknown modules.
- [x] Return the current catalog/default/tenant-intent decision slice as one
  serializable `ModuleEffectivePolicy`: its deterministic `sha256:` revision
  covers the exact definition catalog, normalized platform defaults, and
  persisted tenant overrides; every known module carries typed contributing
  facts and stable denial reasons, and unknown modules are explicitly denied.
  Artifact runtime evidence is covered by the following slices; channel,
  quarantine, revocation, maintenance, and node readiness are owner inputs
  described by the following slices.
- [x] Extend that same decision with exact artifact runtime availability:
  selected artifact definitions resolve only through the existing tenant-RLS
  active-installation owner, require the exact durable capability-policy
  revision, require an injected isolated executor, and fail closed across the
  complete dependency closure. The policy revision includes these redacted
  facts; grant contents and resolver error text are excluded. Channel and
  node-readiness inputs, quarantine, and emergency revocation are covered by
  the following owner slices.
- [ ] Use the same decision in lifecycle writes, runtime dispatch, routing,
  events, scheduler, APIs, and admin UI.
- [x] Wire lifecycle tenant-toggle writes to the same effective-policy
  transition boundary. The owner computes current/next policy revisions from
  the canonical catalog and tenant overrides, checks the durable lifecycle
  predecessor cursor, and appends the explicit transition event in the state
  transaction through `ModuleEffectivePolicyTransitionCoordinator`. A
  concurrent stale policy transition rolls back the lifecycle mutation. The
  lifecycle journal and derived transition event retain the authenticated
  tenant command's actor, correlation, and trace evidence;
  runtime, routing, scheduler, transport, and UI consumers remain open until
  they use the same decision directly.
- [x] Make the server artifact HTTP/command transport resolve the canonical
  effective policy before dispatch and fail closed when the exact module is not
  enabled. The transport uses the shared registry/facade and does not rebuild
  tenant enablement or inspect owner tables directly; runtime revalidation and
  other transport surfaces remain open.
- [x] Require the sandbox-backed artifact runtime executor to re-resolve the
  same host-owned effective policy before every non-lifecycle binding. The
  server supplies the shared registry/facade resolver, so event, scheduled,
  manual, command, and HTTP calls fail closed after a policy change instead of
  relying on a stale transport decision; lifecycle hooks remain governed by
  the owner toggle transaction.
- [x] Keep durable event and scheduled delivery on that same shared executor
  path. Their delivery adapters do not construct a second runtime; the
  executor-side policy revalidation therefore applies before event and
  scheduled artifact bindings as well.
- [x] Keep server module route guards and module-list GraphQL projections on
  `EffectiveModulePolicyService`; they consume the canonical owner decision and
  no longer reconstruct enablement from `tenant_modules` in routing code.
- [x] Invalidate/cache decisions using explicit revision dependencies.
  `crates/rustok-modules` implements the canonical `ModuleEffectivePolicyCache`
  with fail-closed validation bound to `EffectivePolicyCacheIdentity::matches`.
  `ServerRuntimeContext` and `EffectiveModulePolicyService` expose cached policy
  resolution (`resolve_snapshot_cached`, `resolve_cached`) and tenant invalidation.
  Outbox transition events (`module.effective_policy_revision_changed`) trigger
  cache invalidation to prevent stale policy reads across cluster nodes. Verified by
  `crates/rustok-modules/tests/policy_cache_tests.rs` (5 passed).
- [x] Define the first fail-closed cache identity slice. A resolved owner
  decision produces an `EffectivePolicyCacheIdentity` containing the exact
  tenant and content-addressed policy revision; neither tenant identity, TTL,
  nor a process generation can authorize a cache hit alone. The server policy
  snapshot carries this identity for downstream consumers.
- [x] Quarantine blocks new execution without silently changing tenant intent.
  The effective-policy decision records the still-enabled tenant override as a
  contributing fact while returning `quarantined` and denying new execution.
- [x] Revocation policy distinguishes emergency stop from ordinary yanking.
  Terminal security revocation returns the distinct `revoked` denial, while a
  clear already-installed release whose registry lifecycle is `yanked`
  remains executable; yanking affects discovery and new installation only.
- [x] Add the current artifact security owner aggregate: explicit quarantine,
  authorized quarantine clear, and terminal emergency revoke are persisted by
  immutable release identity with revision CAS, exact idempotency receipts, and
  transactional outbox events. Registry yanking remains a discovery/install
  state; effective policy consumes the redacted security snapshot and blocks
  new execution without changing tenant intent.
- [x] Define the neutral channel-policy input boundary: the channel owner or
  host adapter supplies a tenant-safe channel id, surface, immutable
  `sha256:` channel revision, active state, and module bindings. The modules
  owner evaluates those facts through `EffectivePolicyService::resolve_for_channel`;
  inactive channels, missing optional bindings, and disabled bindings are
  explicit denials and the channel snapshot contributes to the same policy
  revision. Channel resolution and channel-table access remain owned by
  `rustok-channel`/the host adapter.
- [x] Add a neutral revisioned maintenance input to the same effective-policy
  aggregate. Operational owners can block all selected modules or an explicit
  module subset with a bounded reason code; active maintenance emits a typed
  denial and never rewrites tenant enablement intent. The snapshot is included
  in the deterministic policy revision and is forwarded through the owner and
  server context facades.
- [x] Define node-readiness evidence as a host-owned snapshot carrying Core
  readiness, active artifact graph revision, CAS availability, executor ABI,
  node revision, and observed base policy revision. The node must observe the
  deterministic base policy before the final policy revision is materialized;
  stale observations fail closed and unready affected modules receive an
  explicit denial. The final policy revision includes the validated snapshot.
- [ ] Implement a durable desired-state/observed-state reconciler for every
  server/sandbox node; in-memory registries and caches are never control-plane
  sources of truth. The first current slice now implements the topology-bound
  native distribution reconciler: durable desired/observed rollout pointers,
  per-assignment observation revisions, prepare/health/activate/converged/degraded
  transitions, exact replay, stale-report rejection, and transactional outbox
  events. `ModuleDesiredObservedState`, `ModuleReconciliationPhase`,
  `ModuleReconciliationEvidence`, and `ModuleReconciliationFailure` now supply
  its shared owner contract; every future artifact/sandbox reconciler must use
  this vocabulary rather than introduce a parallel desired/observed model. The
  accepted [module-node reconciliation ledger ADR](../../DECISIONS/2026-08-14-module-node-reconciliation-ledger.md)
  fixes the target owner, assignment identity, fenced node-agent protocol, and
  static-rollout separation for the implementation. The new
  `SeaOrmModuleArtifactNodeReconciliationService` implements the dynamic owner
  aggregate: trusted sorted topology resolution selects node/installation pairs,
  the owner transaction reloads and freezes every admitted release/payload
  digest, payload kind, admitted payload media type, admission/dependency/
  capability revisions, ABI, and policy; agents
  receive and report only their fenced exact assignment; lifecycle identity
  changes make the previous set stale; and desired/observed heads, reports,
  receipts, status changes, and outbox events commit atomically. The owner, not
  an agent, converts an entirely healthy set to active and advances the
  observed head; post-convergence failure clears that head into `degraded`.
  The server's non-lifecycle artifact executor now consumes the read-only
  `SeaOrmArtifactNodeReadiness` gate before CAS/sandbox execution. It requires
  a configured stable node UUID and compares the selected installation, live
  admission, and current canonical policy revision, including payload kind and
  media type, with the converged observed assignment, so a cache hit or
  sandbox readiness response cannot re-enable stale payloads. The neutral
  sandbox remains free of owner database, policy, AI, and product dependencies.
  `rustok-worker-transport` now derives a canonical SHA-256 fingerprint only
  from the verified mTLS leaf certificate. The current
  `rustok-artifact-node-transport` uses it for claim/heartbeat/report gRPC
  framing, binds reports to an injected fingerprint-to-agent/node map, and
  exposes no plaintext or in-process client constructor. It gives the agent no
  topology, policy, database, CAS, sandbox, AI, or product ownership. The
  separately deployed `rustok-artifact-node-controller` now owns only the
  immutable deployment certificate map and the narrow
  `ModuleControlPlane::artifact_node_agent()` owner port; it cannot author a
  reconciliation or resolve topology. The separate
`rustok-artifact-node-reconciler` composes the other authenticated service:
a verified deployment-operator certificate maps to one audited actor and
explicit allowed-node set, while its bounded request contains a platform-scoped
`ModuleCommandContext` (trace, correlation, and idempotency evidence), topology,
expected durable-state revision, and canonical policy revision. It never
accepts a caller-selected actor or artifact identity; the owner
  validates the topology and reloads each selected admitted installation under
  transaction before it persists a desired set. Its canonical topology digest
  is bound to the owner idempotency identity and must match the resolver output,
  so a replay cannot substitute a target set. The durable reconciliation
  operation ledger records trace and correlation for operator requests only;
  agent reports retain null values because the verified mTLS identity remains
  their sole command evidence. Server composition now uses the bounded
  `VerifiedArtifactNodeCache` read-through CAS adapter and rehashes every cache
  hit before sandbox execution. The independently deployed
  `rustok-artifact-node-agent` now claims only those mTLS-authenticated
  assignments, reads the exact admitted payload from durable CAS, atomically
  materializes and rehashes its canonical node-local cache entry, and records a
  runtime-fingerprint-bound preparation marker. It validates Rhai
  source/workspace bytes without executing guest code, requires authenticated
  isolated Rhai-worker readiness before reporting `healthy`, and compiles Wasm
  Components locally without instantiation or guest execution. Static-promotion
  and sidecar assignments fail closed; CAS, filesystem, and sandbox outages
  remain retryable. The agent has no owner database, topology,
  release-selection, policy, capability, tenant, AI, Alloy, or application
  server dependency. The old host-supplied effective-policy node-readiness
  snapshot family was removed: the canonical base policy is followed by this
  exact durable assignment gate before every non-lifecycle artifact dispatch,
  so a host readiness value cannot affect artifact routing or availability.
  Authenticated target-topology input is complete; deployment-supervisor
  evidence and traffic wiring remain open.
  Focused verification on 2026-08-14 passed `cargo test --locked -p
  rustok-modules --lib` (218 tests), `cargo test --locked -p rustok-events
  --lib` (63 tests), and `cargo clippy --locked -p rustok-modules --lib --tests
  -- -D warnings`. `cargo check --locked -p rustok-server --no-default-features
  --features mod-alloy` also passed with `CARGO_INCREMENTAL=0`; the default
  incremental attempt hit a Rust compiler cache panic, while the successful
  scoped retry emitted only 41 existing warnings in unrelated server/generated
  files. No workspace-wide compile or test suite was run.
  The mTLS transport foundation then passed 10 focused tests and the artifact
  node transport passed ten focused tests, including unknown-fingerprint,
  report-principal, operator-scope, and certificate-actor denial. The
  independently composed `rustok-artifact-node-reconciler` passed two focused
  configuration tests. The owner reconciliation test rejects a mismatched
  topology digest before persistence and a changed topology digest on an
  idempotency replay. The owner/agent/reconciler boundary still requires
  deployment-supervisor evidence and traffic wiring before this checkbox can
  close. `cargo clippy -p rustok-artifact-node-transport --lib --tests -- -D
  warnings` and `cargo clippy -p rustok-artifact-node-reconciler --all-targets
  -- -D warnings` pass for the current topology service.
- [x] Implement the current topology-bound native distribution rollout slice
  with durable rollout/assignment/state/idempotency records, exact release and
  composition identity, full-topology convergence, stale-report rejection,
  and release-revocation invalidation in the same transaction as release-head
  CAS. Each assignment binds `(node, role)`, the candidate role digest, and the
  operation-bound predecessor role digest when one exists; first install keeps
  that predecessor explicitly absent. The predecessor is the then-observed
  serving rollout, never a merely desired or failed candidate; rollout
  revisions still advance from the latest desired operation. Operator rollout
  and recovery receipts retain one platform-scoped `ModuleCommandContext` and
  derive their outbox envelopes from it; release-revocation-derived rollout
  status events retain that same command context. This does not claim the generic
  artifact/sandbox reconciler or the retained-predecessor recovery command is
  complete.
- [ ] Publish composition, installation, activation, grant, quarantine,
  revocation, and binding changes through the existing transactional outbox.
- [ ] Make consumers idempotent and revision-aware because delivery is
  at-least-once; stale/out-of-order events cannot reactivate old state.
- [x] Define the reusable predecessor-bound `ModulePolicyRevisionGate` for
  outbox consumers. It treats exact replays as duplicates, rejects divergent
  or out-of-order transitions as stale, and never infers ordering from opaque
  digest values. Wiring every individual consumer to this gate remains part of
  the broader delivery cutover.
- [x] Add the durable `module_policy_revision_cursors` owner table and
  `SeaOrmModulePolicyRevisionConsumer`. It row-locks one cursor per tenant and
  consumer key under RLS, applies the predecessor gate transactionally, and
  never turns consumer state into a second event journal. The consumer also
  exposes an owner-transaction adapter so a future concrete producer can commit
  its state mutation, outbox append, and cursor advancement atomically; wiring
  individual producers remains open until they carry an explicit predecessor
  and successor effective-policy revision.
- [x] Add the explicit `module.effective_policy_revision_changed` producer
  contract and `ModuleEffectivePolicyTransitionPublisher`. It validates a
  real `sha256:` predecessor/successor pair and appends the event on the same
  owner transaction as the state mutation. Existing security and native
  distribution command revisions are intentionally not treated as effective
  policy transitions; concrete producer wiring remains open until those
  owners compute and supply both policy revisions.
- [x] Define node readiness: required Core/static definitions, active artifact
  graph revision, CAS availability, executor ABI, and the observed base policy
  revision are validated before serving affected traffic. Generic durable
  artifact/sandbox fleet reconciliation remains open; this slice defines the
  owner policy boundary and fail-closed evidence contract.
- [ ] Define prepare -> health/smoke -> activate transitions and optional
  tenant/cohort canary rollout for upgrades. Native fleet transitions are now
  owner-enforced for the full topology; cohort/canary policy remains open.
- [x] Enforce prepare -> health -> activate -> converged transitions for the
  native distribution fleet, with failed/degraded terminal handling and
  revisioned `(node, role, artifact digest)` observations. Tenant/cohort canary orchestration remains
  open.
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

- [x] Define promotion request, review, approval, build, release, rollback, and
  revocation records. Request and approval records now exist in
  `module_static_promotions`, `module_static_promotion_reviews`, and the durable
  idempotency operation journal. Immutable predecessor-linked distribution-build
  intents, their normalized full selection, CAS head, and exact-replay operation
  journal now also exist. Each worker claim has a durable attempt, bounded lease,
  heartbeat and terminal result; expired leases close the old attempt before
  reclaim. A separate immutable distribution-release ledger, admission record,
  CAS head, and exact-replay operation journal now activate only the current
  successfully completed build. The currently implemented direct-predecessor
  rollback has its own durable request/operation records and queues a new
  immutable build from the target release snapshot. Revocation records actor,
  reason, policy, exact replay, and
  release-head CAS; revoking either side cancels a pending rollback request.
- [ ] Replace the rebuild-on-rollback incident path with the accepted
  [module release rollback safety](./module-release-rollback-plan.md) contract.
  `rustok-modules` must retain and revalidate the complete direct-predecessor
  role bundle before rollout and recover it through the normal desired/observed
  deployment reconciler. Rebuild remains admission/reproducibility evidence or
  a separately admitted maintenance update through the same owner lifecycle;
  it is never a rollback fallback, and missing predecessor bytes makes
  automatic mode ineligible.
- [ ] Require source availability, trusted ownership, dependency audit, tests,
  static review, and platform-team approval. Approval now requires immutable
  ownership, dependency-audit, test, and static-review evidence references and
  digests plus an explicit policy identity, non-nil platform actor, and a
  mandatory host authorization decision through the separate promotion
  authorizer port. Request authorization and approval authorization are distinct
  methods and both fail closed. The owner also rejects self-approval: the
  approving actor must differ from the persisted requester. Distribution-build
  selection has its own mandatory fail-closed authorization port and cannot be
  reached by constructing its SeaORM owner outside `ModuleControlPlane`.
  Release activation has a separate authorization decision and external
  verifier port. Its decision must echo the requested policy revision and
  independently admit signature, provenance, SBOM, test, and dependency-policy
  evidence for the exact immutable build.
- [x] Pin the promoted release and source/dependency digests. The request owner
  accepts only an active platform-built release and revalidates its published
  component against the completed tenant-scoped build request/result before
  persisting the exact release, publish request, source reference/digest, and
  dependency-lock digest. It also loads the Cargo package and native entry type
  from the registry release, rejects missing or unsafe Rust identities,
  normalizes a crate-local entry type, and persists both without caller input.
  Promoted and platform source references must exactly match their
  `cas://sha256:<hex>` identity. Approval repeats that verification under
  revision CAS.
  Release identity is read from the current `checksum_sha256` schema column;
  there is no legacy checksum alias or compatibility query path.
- [ ] Generate distribution composition through build tooling; runtime install
  never edits the server Cargo graph. The owner now queues a complete immutable
  composition build intent only from approved promotions, pins platform source,
  toolchain and target digests, and rejects duplicate module slugs or unchanged
  snapshots. Every item now carries the registry-verified Cargo package and
  native entry type; both participate in the composition digest and are
  revalidated for activation and rollback. The separately authorized worker
  owner now provides atomic
  claim/reclaim, heartbeat, and terminal completion. Its current-only,
  transport-neutral `ModuleStaticDistributionExecutor` port and
  `dispatch_next` orchestration run the external call only after the claim
  transaction commits, renew the lease while it is in flight, and persist only
  the owner-validated terminal outcome. Executor/transport failure leaves the
  immutable build reclaimable rather than recording a false build failure.
  `rustok-module-build-transport` now maps that port to the separate current-only
  `rustok.static_distribution` mTLS service with authenticated readiness and no
  plaintext constructor. `rustok-distribution::generate_static_distribution`
  now validates the complete running claim and emits deterministic Cargo
  dependency, promoted-registry Rust, and canonical JSON manifest outputs. The
  output digest binds the claim, composition, platform/toolchain/target,
  reviewed sources, output destinations, and exact generated Cargo/Rust byte
  sequences. A baseline
  no-promotion registry hook is replaced only in the isolated materialized CI
  workspace. `rustok-static-distribution-worker` now hosts the current mTLS
  service as a separate trusted process. It re-hashes its deployment-pinned
  launcher and job configuration at startup/readiness/execution, accepts only
  the pinned toolchain and target, stages bounded create-only generated inputs
  in an idempotent claim-attempt directory, runs the fixed launcher with an
  empty environment and bounded lifetime, and accepts only a receipt bound to
  the exact request, composition, output, launcher, config, toolchain, and
  target. Missing or mismatched output remains reclaimable. Deployment-owned
  signing/publication credentials, publisher configuration, and worker
  deployment remain.
  The static launcher and untrusted module worker share only
  `rustok-build-source` for exact CAS identity and bounded strict USTAR
  extraction. The former worker-local parser was removed atomically; no
  permissive or compatibility extraction route remains.
  The static worker now also parses one strict digest-pinned job config that
  fixes CAS, Cargo, Rustc, publisher, toolchain, target, and resource identities
  and revalidates them during readiness. Its launcher library regenerates the
  complete generated bundle, materializes a new job-local platform workspace,
  verifies every promoted Cargo package/version and raw lock digest, rejects
  dependency alias collisions, and applies the Cargo/registry/manifest outputs
  only there. Its fixed launcher binary resolves the final composed workspace
  lock offline, binds its raw digest into test and publication evidence, runs
  only locked workspace tests and release compilation, invokes the
  digest-pinned publisher, validates its fully bound receipt, and writes the
  terminal owner receipt. Reclaim removes and regenerates only the derived
  workspace while immutable inputs remain create-only. The concrete current
  publisher now uploads the fixed native executable and publishes CycloneDX
  SBOM, SLSA provenance, and raw test evidence as subject-bound OCI referrers.
  It obtains only a short-lived repository lease through the shared
  digest-pinned credential broker, signs the exact artifact digest with the
  shared KMS-only Cosign adapter, resolves the signature manifest, and writes a
  create-only receipt that distinguishes the raw test payload digest from its
  OCI referrer manifest digest. Worker deployment, deployment-owned credentials
  and configuration, and integration evidence remain.
- [ ] Compile promoted crates in CI/distribution builds, not the running server.
  The owner worker protocol now accepts only claimed build intents and records
  successful artifact, SBOM, provenance, signature-manifest, and test evidence,
  and the transport-neutral dispatcher maintains the owner lease around an
  external executor call. The current mTLS client/server adapter and separate
  digest-pinned worker process, concrete launcher, materialization/apply/build
  orchestration, and production OCI evidence publisher are present. CI
  credentials and configuration, worker deployment, and integration evidence
  are not wired yet, so this item remains open.
- [x] Map the native module to the same module/release identity and lifecycle
  facts while marking executor mode as static/native. Every immutable
  distribution item persists `static_native`; that field participates in the
  composition digest and generated build manifest. Verified release reads
  reload and validate the complete succeeded build before exposing its exact
  items. Runtime catalog construction then distinguishes `platform_native`
  definitions from `promoted_native` definitions and binds each promotion to
  its registry release, promotion revision, distribution release/revision,
  native artifact digest, and executor mode. The owner lifecycle and effective
  policy services can consume that exact catalog while implementation handles
  still come only from the compiled registry. A durable native rollout owner
  now handles topology-bound desired/observed convergence; generic
  artifact/sandbox reconciliation remains separate Phase 8 work.
- [x] Require a new distribution build for promotion, upgrade, or removal.
  Promotion selection, replacement, and removal are represented only by a new
  full-snapshot build intent. The currently implemented rollback accepts only the active
  release's non-revoked direct predecessor, rejects a pending desired build,
  revalidates the target admission/build/composition/promotion evidence, and
  queues a new predecessor-linked build. It never reactivates old binary bytes;
  worker completion and verified release activation are mandatory again, and
  activation requires the rebuilt artifact digest to reproduce the target.
  A superseding selection, failed/cancelled build, or revocation of either
  involved release atomically cancels the pending rollback request.
  Activation, rollback, and revocation share one durable idempotency-key
  namespace, preventing cross-command key reuse.
- [x] Remove the superseded rebuild rollback and direct `rustok-build`
  operator rollback without a compatibility or fallback path.
- [ ] Complete the retained direct-predecessor rollout surface:
  distribution contract above. Bind every deployed server and worker role,
  embedded Leptos SSR/hydration artifact, generated registry, and browser asset;
  expose the complete composition blast radius; and report success only after
  the predecessor is observed healthy.
- [x] Do not claim sandbox isolation for native execution. The current
  request/approval and distribution-selection services have no compiler,
  active-composition mutation, or native loader dependency. The worker owner
  only leases and records external build results. The release owner records a
  verified release head but has no native loader or runtime-composition writer,
  so queued, completed, and activated release records remain inert until the
  separate deployment boundary consumes them.

The build-worker transport now exposes the single current
`rustok.module_build` and `rustok.static_distribution` services. The old
generation-suffixed module-build package was removed atomically; there is no
compatibility service, plaintext connection constructor, or fallback route.

Lightweight verification for the current Phase 10 slices on 2026-07-22 used
touched-file `rustfmt --edition 2024 --check`, `git diff --check`, and
`cargo metadata --no-deps`. The formatter check still reports pre-existing
dirty-worktree import/layout drift in untouched portions of the touched files;
`git diff --check` and metadata passed. Targeted crate checks are recorded in
the quality checkpoint above; no workspace-wide compile or test claim is made.

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
