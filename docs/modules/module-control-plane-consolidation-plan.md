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

- Current phase: `not_started`
- Last checkpoint: current fragmentation of module control plane between foundation contracts, server services, GraphQL and admin SSR has been recorded.
- Next step: prepare ADR with target ownership boundary and inventory of production entrypoints.
- Open blockers: target boundary of shared contracts/server orchestration must be approved before code migration.
- Hand-off notes for next agent: do not mix this work with a temporary production remediation plan; do not start by creating a new crate without ADR.
- Last updated at (UTC): 2026-06-27T00:00:00Z

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

Form a single server-owned module control plane with explicit subdomains and
one set of transport-neutral contracts. Admin and other hosts should only
call APIs and display canonical payloads without SQL, manifest parsing, hashing
or lifecycle taxonomy.

## Out of Scope

- migration of module-owned business logic or UI into the platform host;
- merging platform composition and tenant enablement into one state;
- removing GraphQL when adding native `#[server]` functions;
- turning capability crates into tenant-toggled modules;
- creating a new foundation crate before an architectural decision.

## Target Model

The control plane maintains four separate areas of state:

1. build/runtime composition — which modules are included in the platform snapshot;
2. registry governance — package/release/owner/publish lifecycle;
3. tenant lifecycle — enable/disable/settings/recovery for `Optional` modules;
4. effective policy — final availability considering platform, tenant and channel.

One server facade coordinates these areas, but does not mix their tables and
invariants. The shared layer contains only DTOs, pure validation and error taxonomy;
DB, transactions, build jobs and hooks remain in the server-owned implementation.

## Stages

### 0. Architectural Fixation

- [ ] Compile an inventory of all read/write entrypoints, SQL and manifest DTOs.
- [ ] Fix an ADR: ownership, dependency direction and transaction boundaries.
- [ ] Define canonical error taxonomy and revision/CAS semantics.
- [ ] Fix compatibility policy for GraphQL and native server functions.

### 1. Shared Contracts

- [ ] Extract transport-neutral snapshots for catalog, platform composition,
  tenant lifecycle, recovery and governance.
- [ ] Keep pure manifest/registry validation in the shared contract layer.
- [ ] Remove duplicate DTOs and local taxonomy mappings after migrating consumers.
- [ ] Add contract tests for serialization, error codes and revision conflicts.

### 2. Server-Owned Orchestration

- [ ] Introduce a single facade over existing lifecycle/composition/governance services.
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

### 6. Migration and Removal of Legacy Paths

- [ ] Migrate consumers incrementally with dual-read comparison without dual-write.
- [ ] Add telemetry on legacy entrypoint usage.
- [ ] Remove legacy paths after zero-usage window.
- [ ] Update central/local docs and operational runbook.

## Verification Gates

- [ ] One production write path for each control-plane operation.
- [ ] `apps/admin` has no direct SQL and canonical manifest/build algorithms.
- [ ] GraphQL/native parity confirmed by contract tests.
- [ ] Platform CAS/build enqueue and tenant lifecycle journal/state remain atomic.
- [ ] Core cannot be disabled; Optional dependencies cannot be violated.
- [ ] Recovery/retry/compensation preserve canonical taxonomy.
- [ ] `cargo check -p rustok-server --lib` and targeted module/admin tests pass.

## Definition of Done

The plan is complete when the server is the sole owner of module-management
orchestration, the shared layer contains only reusable contracts/pure
validation, admin has no backend bypass paths, and all transports confirm
identical lifecycle, revision and recovery semantics.
