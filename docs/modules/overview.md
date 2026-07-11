---
id: doc://docs/modules/overview.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Module Platform Overview

This document captures the current modular model of RusToK without mixing
architectural roles and technical packaging.

Central docs in `docs/modules/` describe the platform map, taxonomy and composition
rules, but do not replace the local docs of the components themselves.

## Base Model

For platform modules, there are only two categories:

- `Core`
- `Optional`

The source of truth for platform module composition and dependencies is `modules.toml`.

It is important to distinguish:

- Platform module — a crate declared in `modules.toml` that belongs to the `Core` or `Optional` runtime taxonomy;
- Support/library crate — a shared dependency or infrastructure layer that lives in `crates/` but is not a platform module;
- Capability crate — a separate runtime capability layer that can be connected to the platform, but is not required to belong to the `Core/Optional` taxonomy.

## Documentation Sources of Truth

- Root `README.md` of the component in English captures the public contract:
  `Purpose`, `Responsibilities`, `Entry points`, `Interactions`;
- Local `docs/README.md` in English captures the live runtime/module/app contract;
- Local `docs/implementation-plan.md` in English captures the live development plan;
- Central docs in `docs/modules/` tie this picture together and must not
  duplicate local documents line by line.

## Where to Look in the Code

- Platform module composition: `modules.toml`
- Runtime registry: `apps/server/src/modules/mod.rs`
- Manifest wiring: `apps/server/src/modules/manifest.rs`
- Base module contracts: `crates/rustok-core/src/module.rs`
- `Core` / `Optional` taxonomy: `crates/rustok-core/src/registry.rs`

## Platform Modules

### Core

| Slug | Crate |
|---|---|
| `modules` | `rustok-modules` |
| `auth` | `rustok-auth` |
| `cache` | `rustok-cache` |
| `channel` | `rustok-channel` |
| `email` | `rustok-email` |
| `index` | `rustok-index` |
| `search` | `rustok-search` |
| `outbox` | `rustok-outbox` |
| `tenant` | `rustok-tenant` |
| `rbac` | `rustok-rbac` |

### Optional

| Slug | Crate | Depends on |
|---|---|---|
| `content` | `rustok-content` | — |
| `cart` | `rustok-cart` | — |
| `customer` | `rustok-customer` | — |
| `product` | `rustok-product` | `taxonomy` |
| `profiles` | `rustok-profiles` | `taxonomy` |
| `region` | `rustok-region` | — |
| `pricing` | `rustok-pricing` | `product` |
| `inventory` | `rustok-inventory` | `product` |
| `order` | `rustok-order` | — |
| `payment` | `rustok-payment` | — |
| `fulfillment` | `rustok-fulfillment` | — |
| `commerce` | `rustok-commerce` | `cart`, `customer`, `product`, `region`, `pricing`, `inventory`, `order`, `payment`, `fulfillment` |
| `blog` | `rustok-blog` | `content`, `comments`, `taxonomy` |
| `forum` | `rustok-forum` | `content`, `taxonomy` |
| `comments` | `rustok-comments` | — |
| `pages` | `rustok-pages` | `content` |
| `taxonomy` | `rustok-taxonomy` | `content` |
| `media` | `rustok-media` | — |
| `workflow` | `rustok-workflow` | — |
| `alloy` | `alloy` | — |

## What Lives Next to Modules

Not every crate in `crates/` is a platform module.

### Shared Libraries

- `rustok-core`
- `rustok-api`
- `rustok-events`
- `rustok-storage`
- `rustok-test-utils`
- `rustok-commerce-foundation`

### Infrastructure / Capability Crates

- `rustok-iggy`
- `rustok-iggy-connector`
- `rustok-telemetry`
- `rustok-mcp`
- `rustok-ai` with large operator/admin UI surfaces in `crates/rustok-ai/admin` and
  `apps/next-admin/packages/rustok-ai`
- `flex`

This is why "any crate in `crates/`" cannot be automatically equated with a platform module.

When changing ownership, runtime contract or component boundaries, first
update local docs of that component, then `overview.md`, `registry.md`,
`_index.md` and other central registry docs.

## UI Composition Policy

If a module provides UI, that UI must remain module-owned:

- Leptos UI surfaces are published through `admin/` and `storefront/` sub-crates;
- Next.js UI surfaces are published through packages in `apps/next-admin/packages/*` and
  `apps/next-frontend/packages/*`;
- Host applications mount these UI surfaces through manifest-driven wiring, not
  through hard-coded module-specific branches.

## Alloy and Capability Crates

`rustok-ai`, `rustok-mcp` and `flex` do not belong to the `Core/Optional` taxonomy
as regular platform modules.

This means:

- They may be part of runtime composition;
- They may have their own docs, UI and capability surface;
- `rustok-ai` remains a capability crate, but already publishes large
  operator/admin UI surfaces for both Leptos host and Next.js host;
- But their role is described as a support/capability layer, not as a tenant-toggled
  module category.

`alloy` is a separate case here: it remains capability-oriented in meaning, but
is declared in `modules.toml` and participates in `ModuleRegistry` as a regular
optional module.

## Related Documents

- [Module and Application Registry](./registry.md)
- [Module Documentation Index](./_index.md)
- [Module Platform Crate Registry](./crates-registry.md)
- [`rustok-module.toml` Contract](./manifest.md)
- [Rich Text Implementation Plan](./rich-text-implementation-plan.md)
- [Page Builder Implementation Plan](./page-builder-implementation-plan.md)
- [Module Architecture](../architecture/modules.md)
