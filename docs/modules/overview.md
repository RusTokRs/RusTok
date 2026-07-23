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
- Capability extension — a deployment-scoped runtime capability declared with `runtime = "extension"` and composed globally when compiled.

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

| Slug | Crate | Depends on |
|---|---|---|
| `modules` | `rustok-modules` | — |
| `auth` | `rustok-auth` | — |
| `cache` | `rustok-cache` | — |
| `channel` | `rustok-channel` | — |
| `email` | `rustok-email` | — |
| `index` | `rustok-index` | — |
| `search` | `rustok-search` | — |
| `outbox` | `rustok-outbox` | — |
| `events` | `rustok-events-module` | `outbox` |
| `tenant` | `rustok-tenant` | — |
| `rbac` | `rustok-rbac` | — |

### Optional

| Slug | Crate | Depends on |
|---|---|---|
| `content` | `rustok-content` | — |
| `cart` | `rustok-cart` | — |
| `customer` | `rustok-customer` | — |
| `product` | `rustok-product` | `taxonomy` |
| `profiles` | `rustok-profiles` | `taxonomy` |
| `social_graph` | `rustok-social-graph` | — |
| `groups` | `rustok-groups` | — |
| `region` | `rustok-region` | — |
| `pricing` | `rustok-pricing` | `product` |
| `inventory` | `rustok-inventory` | `product` |
| `order` | `rustok-order` | — |
| `payment` | `rustok-payment` | — |
| `fulfillment` | `rustok-fulfillment` | — |
| `commerce` | `rustok-commerce` | `cart`, `customer`, `product`, `region`, `pricing`, `inventory`, `order`, `payment`, `fulfillment` |
| `marketplace_seller` | `rustok-marketplace-seller` | — |
| `marketplace_listing` | `rustok-marketplace-listing` | `marketplace_seller`, `product` |
| `marketplace_allocation` | `rustok-marketplace-allocation` | `order`, `marketplace_seller`, `marketplace_listing` |
| `marketplace_commission` | `rustok-marketplace-commission` | `marketplace_allocation` |
| `marketplace_ledger` | `rustok-marketplace-ledger` | `marketplace_commission` |
| `marketplace_payout` | `rustok-marketplace-payout` | `marketplace_ledger` |
| `marketplace` | `rustok-marketplace` | `marketplace_seller`, `marketplace_listing`, `marketplace_allocation`, `marketplace_commission`, `marketplace_ledger`, `marketplace_payout` |
| `moderation` | `rustok-moderation` | — |
| `blog` | `rustok-blog` | `content`, `comments`, `taxonomy` |
| `forum` | `rustok-forum` | `content`, `taxonomy`, `page_builder` |
| `notifications` | `rustok-notifications` | `outbox` |
| `comments` | `rustok-comments` | — |
| `pages` | `rustok-pages` | `content`, `page_builder` |
| `navigation` | `rustok-navigation` | `channel` |
| `page_builder` | `rustok-page-builder` | — |
| `taxonomy` | `rustok-taxonomy` | `content` |
| `media` | `rustok-media` | — |
| `seo` | `rustok-seo` | `content` |
| `workflow` | `rustok-workflow` | — |
| `alloy` | `alloy` | — |
| `flex` | `flex` | — |

### Capability Extensions

| Slug | Crate | Runtime |
|---|---|---|
| `ai` | `rustok-ai` | `extension` |
| `iggy_connector` | `rustok-iggy-connector` | `extension` |

Capability extensions are deployment-scoped, globally active when compiled and are
not tenant-toggled through the regular `Core` / `Optional` module lifecycle.

## What Lives Next to Modules

Not every crate in `crates/` is a platform module.

### Shared Libraries

- `rustok-core`
- `rustok-api`
- `rustok-events`
- `rustok-storage`
- `rustok-test-utils`
- `rustok-commerce-foundation`

### Infrastructure and Support Crates

- `rustok-iggy`
- `rustok-iggy-connector`
- `rustok-telemetry`
- `rustok-mcp`

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

## Alloy, Flex and Capability Extensions

`alloy` and `flex` are capability-oriented in product meaning, but both are declared
in `modules.toml` and participate in `ModuleRegistry` as regular optional modules.

`rustok-ai` is different: it is declared with `runtime = "extension"`. It may publish
operator/admin UI surfaces and runtime workers, but it is composed as a deployment
capability rather than as a tenant-toggled optional module.

`rustok-mcp` remains an infrastructure/support capability and is not declared as a
platform module in `modules.toml`.

## Related Documents

- [Module and Application Registry](./registry.md)
- [Module Documentation Index](./_index.md)
- [Module Platform Crate Registry](./crates-registry.md)
- [`rustok-module.toml` Contract](./manifest.md)
- [Rich Text Implementation Plan](./rich-text-implementation-plan.md)
- [Page Builder Implementation Plan](./page-builder-implementation-plan.md)
- [Module Architecture](../architecture/modules.md)
