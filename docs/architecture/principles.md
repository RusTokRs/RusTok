---
id: doc://docs/architecture/principles.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Architecture Principles

> Status: guidance on the current state

This is a short set of rules that hold the RusToK architecture together.

## 1. RusToK is a Modular Monolith

RusToK is built as a modular monolith, not as a set of independent services.

Consequences:

- The composition root is in `apps/server`
- Platform modules live in a single runtime
- Boundaries between modules are defined by contracts, not by processes

## 2. Platform Modules Have Only `Core` and `Optional`

A platform module is defined through `modules.toml` and can belong only to:

- `Core`
- `Optional`

`Core` modules always participate in the runtime.
`Optional` modules participate in build/runtime composition and can be managed at
the tenant level.

Support/capability crates do not form a third taxonomy of platform modules.

## 3. Role, Taxonomy and Crate-packaging Must Not Be Mixed

Three axes must be distinguished:

- Architectural role: module / shared library / capability crate / host
- Runtime taxonomy: `Core` / `Optional`
- Technical packaging: `crate`

It follows that:

- `crate != platform module`
- `ModuleRegistry != architectural taxonomy`
- Bootstrap wiring != domain logic ownership

## 4. Source of Truth for Platform Composition is `modules.toml`

The composition of platform modules, dependency graph and composition contract are defined
through `modules.toml`.

For a path module, this must be aligned with:

- `rustok-module.toml`
- Runtime registration
- Local docs
- Verification flow through `xtask`

## 5. Source of Truth for Documentation Lives in the Component

For every first-party component:

- Root `README.md` in English captures the public contract
- `docs/README.md` in English captures the live runtime/app/module contract
- `docs/implementation-plan.md` in English captures the live development plan

Central docs in `docs/` provide the map and navigation, but must not replace
local component docs.

## 6. Server is a Host, Not a Domain Logic Dump

`apps/server` owns:

- Transport layer
- Runtime wiring
- Auth/session integration
- RBAC enforcement path
- Operational endpoints

`apps/server` must not become a place for accumulating module-owned domain
logic if that logic already has an owning crate.

## 7. Write-side Correctness Over Convenience

Write-side operations must be:

- Transactional
- Tenant-safe
- RBAC-aware
- Consistent with the event contract

Cross-module events are published through a transactional path where
atomicity of write + event persistence is needed.

## 8. Read-side is Separated from Write-side

RusToK maintains separation:

- Write-side for domain changes
- Read-side for projections, indexes and fast query paths

This allows:

- Not dragging heavy join-paths into storefront/read flows
- Building downstream consumers independently
- Evolving indexing and projections separately from write-side models

## 9. UI Remains Module-owned

If a module provides UI:

- Leptos surfaces are published through `admin/` and `storefront/` sub-crates
- Host applications only mount surfaces through manifest-driven wiring
- Internal Leptos data layer uses `#[server]` functions by default
- GraphQL remains a parallel transport contract
- Locale is selected by the host/runtime layer, not by a package-local fallback chain

## 10. Capability Crates Do Not Replace Module Taxonomy

Capability/support crates like:

- `alloy`
- `rustok-mcp`
- `rustok-ai`
- `rustok-telemetry`
- `flex`

must not be described as regular tenant-toggled platform modules if they are not
declared as platform modules in `modules.toml`.

And the reverse is also true: if a component is declared as a platform module, it must
live in the `Core/Optional` taxonomy.

## 11. Documentation Must Reflect Code

If code and docs diverge, the current code takes priority, and the documentation must
be updated synchronously.

This especially applies to:

- Module taxonomy
- Event flow
- API surface
- Host wiring
- Tenant and RBAC boundaries

## 12. Boundary Changes Require Synchronous Updates

When changing architectural boundaries, the following must be updated simultaneously:

1. Local docs of the affected component
2. Central docs in `docs/`
3. `docs/index.md`
4. Verification docs if the verification contract changes
5. ADR if the change is non-trivial

## Related Documents

- [Platform Architecture Overview](./overview.md)
- [Module Architecture](./modules.md)
- [Platform Diagrams](./diagram.md)
- [Module Platform Overview](../modules/overview.md)
- [`rustok-module.toml` Contract](../modules/manifest.md)
