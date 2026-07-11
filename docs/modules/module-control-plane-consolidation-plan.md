---
id: doc://docs/modules/module-control-plane-consolidation-plan.md
kind: implementation_plan
language: markdown
last_verified_snapshot: snap_jsonl_00000040
source_language: markdown
status: verified
---
# Module Control Plane Consolidation Plan

## Execution checkpoint

- Current phase: `architecture_fixed`
- Last checkpoint: neutral sandbox ownership and the Alloy/module-platform boundary were accepted in the 2026-07-11 ADR.
- Next step: introduce `rustok-sandbox` contracts and move the generic Rhai execution kernel behind them.
- Open blockers: none for the foundation slice; OCI and Wasmtime dependency selection belongs to the artifact-executor stage.
- Hand-off notes for next agent: Alloy and `rustok-modules` are peer consumers of `rustok-sandbox`; neither may implement a parallel production sandbox.
- Last updated at (UTC): 2026-07-11T00:00:00Z

## Problem

Module management has several legitimate levels, but their orchestration and
transport implementation are distributed more widely than necessary:

- `rustok-core` owns basic module contracts and `ModuleRegistry`;
- `apps/server/src/modules` assembles the runtime registry and validates the manifest;
- `ModuleLifecycleService` manages tenant enable/disable and recovery;
- `PlatformCompositionService` manages platform snapshot and build enqueue;
- `RegistryGovernanceService` manages publishing, releases and ownership;
- GraphQL publishes the server API;
- `apps/admin/src/features/modules/transport` additionally contains its own manifest
  DTOs, hashing, SQL, build/release and marketplace orchestration.

Main boundary defect: the admin host partially performs backend/control-plane
duties instead of consuming a unified server-owned API. This creates multiple
sources of taxonomy, validation and state mapping.

## Goal

Form a single `rustok-modules`-owned module control plane with explicit
subdomains and one set of transport-neutral contracts. Admin and other hosts
only call APIs and display canonical payloads without SQL, manifest parsing,
hashing, executor selection or lifecycle taxonomy.

Sandboxed execution is owned by the neutral `rustok-sandbox` foundation. Alloy
and `rustok-modules` consume the same execution request, capability broker,
policy, limits, audit envelope and outcome taxonomy.

## Out of Scope

- migration of module-owned business logic or UI into the platform host;
- merging platform composition and tenant enablement into one state;
- removing GraphQL when adding native `#[server]` functions;
- turning capability crates into tenant-toggled modules;
- rewriting existing native modules in Rhai or WebAssembly;
- implementing the sidecar executor before the common sandbox and WebAssembly executor are stable.

## Target Model

The control plane maintains four separate areas of state:

1. build/runtime composition — which modules are included in the platform snapshot;
2. registry governance — package/release/owner/publish lifecycle;
3. tenant lifecycle — enable/disable/settings/recovery for `Optional` modules;
4. effective policy — final availability considering platform, tenant and channel.

One `rustok-modules` facade coordinates these areas without mixing their tables
and invariants. Infrastructure adapters are supplied explicitly by the host.
The neutral sandbox executes admitted payloads but does not own marketplace,
module identity, installation or Alloy authoring state.

## Stages

### 0. Architectural Fixation

- [x] Compile an inventory of all read/write entrypoints, SQL and manifest DTOs.
- [x] Fix an ADR: ownership, dependency direction, neutral sandbox and immutable release lineage.
- [ ] Define canonical error taxonomy and revision/CAS semantics.
- [ ] Fix compatibility policy for GraphQL and native server functions.

### 0A. Neutral Sandbox Foundation

- [ ] Introduce one execution subject/envelope, capability broker, sandbox policy and typed outcome taxonomy.
- [ ] Register `rhai` and `wasm_component` executors behind one executor interface; reserve `sidecar` without a placeholder implementation.
- [ ] Move generic Rhai execution and resource limits from Alloy into `rustok-sandbox`; retain Alloy-specific bridges in Alloy adapters.
- [ ] Route Alloy draft execution and installed module execution through the same sandbox with distinct typed subjects and grants.
- [ ] Preserve immutable marketplace releases: Alloy changes create a new version and digest with explicit source lineage.

### 1. Shared Contracts

- [ ] Extract transport-neutral snapshots for catalog, platform composition,
  tenant lifecycle, recovery and governance.
- [ ] Keep pure manifest/registry validation in the shared contract layer.
- [ ] Remove duplicate DTOs and local taxonomy mappings after migrating consumers.
- [ ] Add contract tests for serialization, error codes and revision conflicts.

### 2. Module-Owned Orchestration

- [ ] Introduce a single `rustok-modules` facade over lifecycle, installation, composition and governance services.
- [ ] Fix one write entrypoint per operation.
- [ ] Maintain atomic boundaries for platform CAS + build enqueue and tenant journal + state.
- [ ] Prohibit direct writes to control-plane tables outside owner services via static guardrail.

### 3. Transport Surfaces

- [ ] Migrate GraphQL queries/mutations to the canonical facade.
- [ ] Add native `#[server]` adapters for Leptos without removing GraphQL.
- [ ] Ensure identical payload/error/recovery semantics on both transports.
- [ ] Add parity tests GraphQL ↔ native adapters.

### 4. Admin Host Simplification

- [ ] Remove direct SQL to platform/build/registry tables from admin SSR.
- [ ] Remove admin-owned manifest loading, canonical hashing and build planning.
- [ ] Keep in admin the transport facade, view models and UI effects.
- [ ] Prohibit local remapping of lifecycle taxonomy and recovery metadata.

### 5. Effective Policy and Consumers

- [ ] Reduce module availability checks to a single typed effective-policy contract.
- [ ] Separate platform installed, tenant enabled and channel bound in API and UI.
- [ ] Verify Core/Optional invariants and dependency graph on all write paths.
- [ ] Add tenant/channel isolation and stale revision tests.

### 6. Atomic Cutover and Removal

- [ ] Migrate every internal caller to the owner facade in the same change set.
- [ ] Delete server/admin SQL, manifest and lifecycle paths after caller migration.
- [ ] Do not retain dual reads, compatibility aliases or fallback-to-legacy execution.
- [ ] Update central/local docs and operational runbook.

### 7. Artifact Executors and Alloy Evolution

- [ ] Package Rhai modules as immutable artifacts with source lineage and canonical descriptors.
- [ ] Add the Wasmtime component executor under the same sandbox policy and capability broker.
- [ ] Add OCI publication, signature/SBOM verification and digest-pinned installation.
- [ ] Support importing or forking a marketplace Rhai release into Alloy and publishing changes as a new version.
- [ ] Move trusted native composition into the explicit static-promotion adapter.

## Verification Gates

- [ ] One production write path for each control-plane operation.
- [ ] `apps/admin` has no direct SQL and canonical manifest/build algorithms.
- [ ] GraphQL/native parity confirmed by contract tests.
- [ ] Platform CAS/build enqueue and tenant lifecycle journal/state remain atomic.
- [ ] Core cannot be disabled; Optional dependencies cannot be violated.
- [ ] Recovery/retry/compensation preserve canonical taxonomy.
- [ ] `cargo check -p rustok-server --lib` and targeted module/admin tests pass.

## Definition of Done

The plan is complete when `rustok-modules` is the sole owner of module-management
orchestration, `rustok-sandbox` is the sole sandbox contract used by Alloy and
installed artifacts, admin/server have no backend bypass paths, and all
transports confirm identical lifecycle, revision and recovery semantics.
