---
id: doc://docs/architecture/overview.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Platform Architecture Overview

RusToK evolves as a modular monolith with an explicit composition root in
`apps/server`, platform modules, host applications and a separate layer of
shared/support/capability crates.

This document provides a top-level architecture map. Detailed rules for
module contract, registry and local docs are in `docs/modules/*`.

## Main Layers

### 1. Host Applications

- `apps/server` — main runtime host, HTTP/GraphQL entry point and composition root
- `apps/admin` — Leptos admin host
- `apps/storefront` — Leptos storefront host
- `apps/next-admin` — Next.js admin host
- `apps/next-frontend` — Next.js storefront host

Host applications assemble the runtime, mount module-owned surfaces and must not
take ownership of module domain logic.

### 2. Platform Modules

A platform module is defined through `modules.toml` and belongs to only one of
two runtime categories:

- `Core`
- `Optional`

Platform modules publish their own runtime contracts, transport surfaces,
RBAC ownership and local documentation. For path modules, the following are required:

- `rustok-module.toml`
- root `README.md`
- `docs/README.md`
- `docs/implementation-plan.md`

### 3. Shared / Support Crates

Shared crates provide foundation contracts and reusable infrastructure:

- `rustok-core`
- `rustok-api`
- `rustok-events`
- `rustok-storage`
- `rustok-test-utils`
- `rustok-commerce-foundation`

They may be critical for runtime, but they do not become platform
modules on their own without a slug in `modules.toml`.

### 4. Capability Crates

Capability crates provide separate runtime capabilities and integration layers:

- `rustok-mcp`
- `rustok-ai`
- `alloy`
- `flex`
- `rustok-telemetry`
- `rustok-iggy`
- `rustok-iggy-connector`

They participate in composition, but are not considered tenant-toggled `Core/Optional`
modules.

## Runtime Composition

The top-level runtime contract is assembled as follows:

1. `modules.toml` defines platform composition and dependency graph.
2. `apps/server/src/modules/mod.rs` builds the runtime registry.
3. `apps/server/src/modules/manifest.rs` validates the manifest/runtime contract.
4. `apps/server` and other hosts mount surfaces through manifest-driven wiring.
5. Shared/capability crates are connected as support layers, not as a separate
   module taxonomy.

## Sources of Truth

### Runtime

- `modules.toml`
- `apps/server/src/modules/mod.rs`
- `apps/server/src/modules/manifest.rs`
- `crates/rustok-core/src/module.rs`

### Documentation

- Root `README.md` in English captures the public contract of the component
- `docs/README.md` in English captures the live runtime/app/module contract
- `docs/implementation-plan.md` in English captures the live development plan
- Central docs in `docs/` link the platform map and must not duplicate
  local docs line by line

## UI and Transport Policy

- Module-owned UI stays with the module itself
- Leptos surfaces are published through `admin/` and `storefront/` sub-crates
- Internal Leptos data layer uses `#[server]` functions by default
- GraphQL remains a parallel transport contract
- Host applications only mount surfaces and routes
- Locale is selected by the host/runtime layer and passed to UI packages as effective locale

## Event Flow and Read Model

The basic write/read scheme of the platform:

1. Request arrives at the host/runtime layer.
2. Tenant/auth/RBAC policy is applied before calling domain logic.
3. The module performs a write-side operation.
4. Cross-module events are published through a transactional outbox.
5. Read-side and indexing are updated through an event-driven flow.
6. UI and APIs read consistent read models and transport surfaces.

`rustok-outbox` is considered a `Core` platform module, not just a support crate.

## Tenant Lifecycle

Tenant-level enable/disable applies only to `Optional` modules and works
on top of the already assembled platform composition.

It must not:

- Disable `Core` modules
- Turn capability crates into platform modules
- Bypass the dependency graph from `modules.toml`

## Readiness Criteria for Architecture Changes

A change is considered complete if:

1. The runtime contract is reflected in code and manifest wiring;
2. Local docs of affected components are updated;
3. Central docs in `docs/modules/*`, `docs/architecture/*` and `docs/index.md`
   are synchronized;
4. If necessary, the decision is captured in an ADR.

## Related Documents

- [Module Architecture](./modules.md)
- [Platform Diagram](./diagram.md)
- [Architecture Principles](./principles.md)
- [Module Platform Overview](../modules/overview.md)
- [Module and Application Registry](../modules/registry.md)
- [`rustok-module.toml` Contract](../modules/manifest.md)
