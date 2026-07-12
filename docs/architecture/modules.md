---
id: doc://docs/architecture/modules.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Module Architecture

This document captures the architectural view of the RusToK modular platform:
what counts as a platform module, where the boundary lies between module crate,
support/capability crate and host application, and where the source of truth
for runtime and documentation contract lives.

## Base Model

In RusToK, platform modules are divided into only two runtime categories:

- `Core`
- `Optional`

The source of truth for platform module composition and dependencies is `modules.toml`.

This means:

- A platform module is defined not by the crate name, but by the presence of a slug in
  `modules.toml`;
- Runtime taxonomy is defined through `Core` and `Optional`, not through arbitrary
  local categories;
- Support/shared/capability crates may participate in composition, but do not
  automatically become tenant-toggled modules because of it.

## Sources of Truth

### Runtime

- Composition root: `modules.toml`
- Runtime registration: `apps/server/src/modules/mod.rs`
- Manifest/runtime validation: `apps/server/src/modules/manifest.rs`
- Base module contracts: `crates/rustok-core/src/module.rs`
- Module artifact control plane: `crates/rustok-modules`
- Neutral sandbox execution foundation: `crates/rustok-sandbox`

### Documentation

- Root `README.md` of the component in English captures the public contract:
  `Purpose`, `Responsibilities`, `Entry points`, `Interactions`
- Local `docs/README.md` in English captures the live runtime/module/app contract
- Local `docs/implementation-plan.md` in English captures the live development plan
- Central docs in `docs/modules/*` provide the map and navigation, but do not replace
  local component docs

When changing ownership, runtime contract or module boundaries, first
update local component docs, then central docs.

## Component Types

### Platform Modules

A platform module:

- Is declared in `modules.toml`
- Has `rustok-module.toml` if it is `source = "path"`
- Passes scoped validation through `cargo xtask module validate <slug>`
- Participates in runtime/module lifecycle as `Core` or `Optional`

The current scope of platform modules can be found in:

- [Module Platform Overview](../modules/overview.md)
- [Module and Application Registry](../modules/registry.md)

### Shared / Support Crates

These crates provide foundation or shared contracts for platform modules and the host layer:

- `rustok-core`
- `rustok-api`
- `rustok-runtime`
- `rustok-sandbox`
- `rustok-web`
- `rustok-fba`
- `rustok-cli-core`
- `rustok-events`
- `rustok-storage`
- `rustok-test-utils`
- `rustok-commerce-foundation`

They may have their own `README.md` and local docs, but are not required to have a slug in
`modules.toml`.

### Capability Crates

Capability crates add separate runtime capabilities, but are not considered
tenant-toggled platform modules:

- `rustok-mcp`
- `rustok-ai`
- `alloy`
- `flex`
- `rustok-iggy`
- `rustok-iggy-connector`
- `rustok-telemetry`

Their role is described as a support/capability layer, not as a `Core`/`Optional`
module category.

### Host Applications

Host applications assemble the runtime and mount module surfaces:

- `apps/server` — composition root and main runtime host
- `apps/admin` — Leptos admin host
- `apps/storefront` — Leptos storefront host
- `apps/next-admin` — Next.js admin host
- `apps/next-frontend` — Next.js storefront host

A host application must not become the canonical owner of module-owned domain logic
or UI surfaces.

The same rule applies to transport types: a host can combine owner-provided
GraphQL roots through `MergedObject`, but concrete connection types, resolvers and DTOs
remain in the owner/support crate. Cross-module integration surface belongs to a separate
support crate, not to `apps/server`.

Backend module implementation must follow the backend module guides:

- [Backend Module Architecture](../backend/module-backend-architecture.md)
- [Backend Module Implementation Guide](../backend/module-backend-implementation.md)
- [Backend Module Verification Guide](../backend/module-backend-verification.md)

The key foundation split is stable: `rustok-api` owns API contracts,
`rustok-runtime` owns executable runtime helpers, `rustok-web` owns Axum boundary helpers,
`rustok-fba` owns FBA metadata, and `rustok-cli-core` owns CLI provider contracts.

Backend module file ownership follows the same split:

- `crates/rustok-<module>/src` owns domain/application code, services, ports, events,
  migrations and owner-owned GraphQL/REST entrypoints;
- `crates/rustok-<module>/contracts` owns published OpenAPI/GraphQL/FBA evidence artifacts;
- `crates/rustok-<module>/docs` owns local implementation plan and readiness evidence;
- `crates/rustok-<module>/cli` owns optional external command adapters and uses
  `rustok-cli-core`;
- `apps/server` mounts and composes owner-owned entrypoints, but must not become the owner
  of module services, DTOs, command providers or business policy.

## UI Composition Policy

If a module provides UI, that UI remains module-owned:

- Leptos surfaces are published through `admin/` and `storefront/` sub-crates
- Host applications only mount these surfaces through manifest-driven wiring
- Internal Leptos data layer is built on `#[server]` functions by default
- GraphQL remains a parallel transport contract and is not removed
- Locale comes from the host-provided contract, not from a package-local fallback chain

The mere presence of an `admin/` or `storefront/` folder is not considered proof of
integration. The canonical source of truth here is `rustok-module.toml`.

## Outbox and Capability Layers

Several components are important to read correctly:

- `rustok-outbox` is a `Core` platform module, not just a support crate
- `rustok-core` and `rustok-events` are shared contract crates, not tenant-toggled modules
- `rustok-installer` is a support crate for installer-core contracts, not a
  tenant-toggled module and not a module lifecycle registry entry
- `alloy`, `rustok-ai`, `rustok-mcp`, `flex` are capability layers, not `Core/Optional`
  modules

This distinction is important for registry, lifecycle, RBAC ownership and documentation flow.

## Install/Uninstall and Tenant Lifecycle

Three levels must be distinguished:

### Platform-level Composition

Platform-level composition is defined through `modules.toml` and build/runtime
registration. This determines:

- Which modules are included in the build
- What dependency edges they have
- Which path-modules must have `rustok-module.toml`
- What scoped contract must pass `xtask`

### Schema Composition

Schema composition in the current version is defined by the server `Migrator` in
`rustok-migrations`, which combines platform-core and module-owned
migrations into a single globally sorted list. Installer v1 must not
promise physical exclusion of optional module schema artifacts from the database just
because the module is disabled at the tenant level.

### Tenant-level Enable/Disable

Tenant lifecycle applies only to `Optional` modules and works on top of the already
assembled platform composition. It must not:

- Switch `Core` modules
- Turn capability crates into platform modules
- Break the dependency graph described in `modules.toml`
- Delete or hide already applied module-owned schema artifacts

## Related Documents

- [Module Platform Overview](../modules/overview.md)
- [Module and Application Registry](../modules/registry.md)
- [Module Documentation Index](../modules/_index.md)
- [`rustok-module.toml` Contract](../modules/manifest.md)
- [Module Platform Crate Registry](../modules/crates-registry.md)
- [Backend Module Architecture](../backend/module-backend-architecture.md)
- [Backend Module Implementation Guide](../backend/module-backend-implementation.md)
- [Module Documentation Template](../templates/module_contract.md)

## Runtime Control Plane and Lifecycle

The active runtime composition of modules is stored in `platform_state`; `modules.toml` is used as a bootstrap/dev input.
Composition changes are performed as atomic control-plane operations: the manifest is validated against the registry, `platform_state`
is updated via revision/CAS, and the build job receives `manifest_ref = platform_state:<revision>` in the same DB transaction.
The manifest hash is a SHA-256 of the canonical JSON of the full snapshot, including settings/build/source/dependency metadata.

Tenant enable/disable must go through `ModuleLifecycleService::toggle_module_with_actor()`: the operation journal
is written before changing tenant state, compat `on_enable`/`on_disable` hooks are executed as pre-hooks, and the successful state
change and operation transition to `committed` are fixed in one commit. GraphQL and Leptos SSR/admin surfaces
do not own lifecycle taxonomy or journal metadata: the server GraphQL mapper publishes canonical error codes
(`BAD_USER_INPUT`, `MODULE_HOOK_FAILED`, `INTERNAL_ERROR`) and recovery/journal fields, while Leptos SSR/admin
layers only pass through the payload without local remap.

Module-owned migrations with cross-module foreign keys or order assumptions must declare ordering metadata next to their exporter through `migration_dependencies()` and pass it through `MigrationSource::migration_dependencies()`. The server migrator aggregates these descriptors through the module `MigrationSource` contract for all module crates whose migrations are included in the server migrator; modules without cross-module ordering metadata use default empty descriptors. The migrator then performs a topological sort and treats missing dependency/cycle as a runtime/test contract error. The current aggregated baseline covers explicit boundaries `channel -> auth`, `pricing/inventory -> product variants`, `commerce collections/categories -> product`, `blog/forum taxonomy joins -> taxonomy`.

### Current Control-plane/Module Lifecycle Contract

- Composition updates are performed only through server-owned orchestration: validation, `platform_state` CAS/revision update and build enqueue are in one transaction boundary; `manifest_ref` has the form `platform_state:<revision>`, and `manifest_hash` is computed through a shared SHA-256 canonical snapshot helper.
- Tenant module lifecycle has one production entrypoint — `ModuleLifecycleService::toggle_module_with_actor()`. Direct model-level toggle and admin-side SQL/bypass are not contract surfaces.
- Lifecycle journal uses statuses `validated/running/committed/failed`; post-hook failure does not roll back committed tenant state, but creates a failed operation with recovery metadata (`status`, `issue`, `retryable`, `recommended_action`, `correlation_id`, `requested_by`, `error_message`).
- Recovery is performed only through canonical GraphQL/service surface: `moduleOperationRecoveryPlan`, `failedModuleOperationRecoveryPlans`, `retryFailedModuleOperationPostHook`, `compensateFailedModuleOperation`. Compensation is allowed only for `post_hook_failed` operations when the current effective state still matches the committed requested state.
- GraphQL mapper owns error taxonomy (`BAD_USER_INPUT`, `MODULE_HOOK_FAILED`, `INTERNAL_ERROR`); Leptos SSR/admin layers must pass through and must not remap taxonomy/journal/recovery fields.
- Module migration ordering is fixed by descriptor contract: module crates export `migration_dependencies()` next to `migrations()` and return it through `MigrationSource::migration_dependencies()`, the server migrator aggregates descriptors through the module contract for all module crates whose migrations are included in the server migrator, performs topological sort, preserves deterministic lexical tie-breaker for independent migrations and fails on missing dependency/cycle.
