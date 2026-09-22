---
id: doc://docs/architecture/matryoshka.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Matryoshka / Composition Model

This document preserves Matryoshka as a conceptual model of RusToK layers, but
captures it in terms of the current state, without detaching from the current code.

## Why the Model Is Needed

Matryoshka helps avoid mixing different levels of the platform:

- foundation and runtime host
- platform modules
- shared/support/capability crates
- UI and interaction surfaces
- future federation/network layers

It is not a separate runtime contract and not a replacement for `modules.toml`. It is an architectural
frame that helps explain composition.

## Current State Layers

### Layer 1. Foundation Platform

The foundation layer includes:

- `apps/server` as composition root
- `rustok-core`
- `rustok-api`
- `rustok-events`
- `rustok-storage`
- `rustok-test-utils`

This layer provides base contracts, runtime wiring and host-level policy.

### Layer 2. Platform Modules

Platform modules are declared in `modules.toml` and are divided into:

- `Core`
- `Optional`

This layer describes domain/runtime modules, not arbitrary crates in `crates/`.

### Layer 3. Shared Domain Families and Support Slices

This layer covers shared family crates and module-adjacent support pieces,
for example:

- `rustok-commerce-foundation`
- shared read/index/event helpers
- module-specific support surfaces that are not standalone platform modules

The purpose of this layer is to provide reuse without blurring ownership of platform modules.

### Layer 4. Capability Crates

Capability crates add separate platform capabilities:

- `rustok-mcp`
- `rustok-ai`
- `alloy`
- `flex`
- `rustok-telemetry`
- `rustok-iggy`
- `rustok-iggy-connector`

They participate in composition, but are not part of the `Core/Optional` taxonomy until
declared as platform modules.

### Layer 5. Unified UI

The UI layer combines:

- Leptos hosts
- Next.js hosts
- module-owned UI packages
- common UI/runtime contract

Important rule here: UI remains module-owned, and hosts only mount and
compose surfaces.

### Layer 6. Interaction / Read Layer

This layer describes:

- denormalized read models
- index/search layer
- event-driven projections
- live transport surfaces, if needed for interaction

It is not a separate domain taxonomy, but a layer of aggregation and interaction flows.

### Layer 7. Federation / Future Network Layer

This layer remains a vision-level direction:

- inter-instance interaction
- federation protocols
- mesh/network scenarios

It is not considered a current runtime baseline and must not be mixed with the live
contract layer of the current platform.

## What Matters in the Current State

- `modules.toml` is more important than conceptual-layer names when it comes to the source of truth for runtime
- a platform module is defined through manifest and registry, not through an abstract
  model layer
- a capability crate does not become a platform module just because it is important
- central docs must describe the current code, not just the vision

## How to Use This Model

Use Matryoshka when you need to:

- explain the place of a new component in the overall architecture
- avoid confusing host, module, support and capability roles
- understand at which level a new contract should live

Do not use Matryoshka as a replacement for:

- `modules.toml`
- `rustok-module.toml`
- local component docs
- verification contract

## Related Documents

- [Platform Architecture Overview](./overview.md)
- [Module Architecture](./modules.md)
- [Platform Diagrams](./diagram.md)
- [Module Platform Overview](../modules/overview.md)
- [Module and Application Registry](../modules/registry.md)
