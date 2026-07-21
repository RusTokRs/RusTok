---
id: doc://docs/modules/UI_PACKAGES_INDEX.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Module UI Packages Documentation

This document provides navigation to module-owned UI surfaces and captures only
the current contract layer. Local module docs and `rustok-module.toml` remain the
sources of truth.

## Basic Rule

- UI packages belong to the module, not the host application.
- Leptos admin/storefront surfaces are published through owner-local `admin/` and
  `storefront/` sub-crates.
- Hosts mount module-owned surfaces and provide route, tenant, auth, and locale
  context only.
- The source of truth for wiring is `rustok-module.toml` plus the module's local
  docs.

## Runtime Contract for UI Packages

- Effective locale comes from host `UiRouteContext.locale`.
- Module packages do not implement package-local locale negotiation.
- Native `#[server]` and GraphQL transports remain explicit parallel paths.
- UI consumes only the package transport facade and never raw adapters.
- Another module's UI or business logic is not copied into the package.

## Contract References

- [`rustok-module.toml` Contract](./manifest.md)
- [Module and Application Registry](./registry.md)
- [Module Documentation Index](./_index.md)
- [FFA Architecture Guide](../UI/module-package-architecture.md)
- [FFA Implementation Guide](../UI/module-package-implementation.md)
- [FFA Verification Guide](../UI/module-package-verification.md)

## Core/Admin Surfaces

- `rustok-channel`: [README](../../crates/rustok-channel/admin/README.md)
- `rustok-index`: [README](../../crates/rustok-index/admin/README.md)
- `rustok-outbox`: [README](../../crates/rustok-outbox/admin/README.md)
- `rustok-auth`: [README](../../crates/rustok-auth/admin/README.md)
- `rustok-tenant`: [README](../../crates/rustok-tenant/admin/README.md)
- `rustok-rbac`: [README](../../crates/rustok-rbac/admin/README.md)

## Optional/Admin Surfaces

- `rustok-product`: [README](../../crates/rustok-product/admin/README.md)
- `rustok-fulfillment`: [README](../../crates/rustok-fulfillment/admin/README.md)
- `rustok-customer`: [README](../../crates/rustok-customer/admin/README.md)
- `rustok-groups`: [README](../../crates/rustok-groups/admin/README.md)
- `rustok-region`: [README](../../crates/rustok-region/admin/README.md)
- `rustok-order`: [README](../../crates/rustok-order/admin/README.md)
- `rustok-inventory`: [README](../../crates/rustok-inventory/admin/README.md)
- `rustok-pricing`: [README](../../crates/rustok-pricing/admin/README.md)
- `rustok-commerce`: [README](../../crates/rustok-commerce/admin/README.md)
- `rustok-page-builder`: [README](../../crates/rustok-page-builder/admin/README.md)
- `rustok-pages`: [README](../../crates/rustok-pages/admin/README.md)
- `rustok-seo`: [README](../../crates/rustok-seo/admin/README.md)
- `rustok-blog`: [README](../../crates/rustok-blog/admin/README.md)
- `rustok-forum`: [README](../../crates/rustok-forum/admin/README.md)
- `rustok-search`: [README](../../crates/rustok-search/admin/README.md)
- `rustok-media`: [README](../../crates/rustok-media/admin/README.md)
- `rustok-comments`: [README](../../crates/rustok-comments/admin/README.md)
- `rustok-workflow`: [README](../../crates/rustok-workflow/admin/README.md)

`rustok-groups-admin` owns the group directory/control-room surface. Forum,
Blog, Pages, Marketplace, Media, and future social modules continue to own their
feature screens; the host composes their entrypoints into the group shell.

## Optional/Storefront Surfaces

- `rustok-blog`: [README](../../crates/rustok-blog/storefront/README.md)
- `rustok-cart`: [README](../../crates/rustok-cart/storefront/README.md)
- `rustok-commerce`: [README](../../crates/rustok-commerce/storefront/README.md)
- `rustok-fulfillment`: [README](../../crates/rustok-fulfillment/storefront/README.md)
- `rustok-payment`: [README](../../crates/rustok-payment/storefront/README.md)
- `rustok-order`: [README](../../crates/rustok-order/storefront/README.md)
- `rustok-forum`: [README](../../crates/rustok-forum/storefront/README.md)
- `rustok-groups`: [README](../../crates/rustok-groups/storefront/README.md)
- `rustok-pages`: [README](../../crates/rustok-pages/storefront/README.md)
- `rustok-pricing`: [README](../../crates/rustok-pricing/storefront/README.md)
- `rustok-product`: [README](../../crates/rustok-product/storefront/README.md)
- `rustok-region`: [README](../../crates/rustok-region/storefront/README.md)
- `rustok-search`: [README](../../crates/rustok-search/storefront/README.md)

`rustok-groups-storefront` owns public group discovery and the group shell. Its
public directory never requests non-public groups. Secret-group visibility must
remain fail-closed in owner ports and downstream projections.

## Capability/Admin Surfaces

- `rustok-ai` Leptos operator/admin UI: [README](../../crates/rustok-ai/admin/README.md)
- `rustok-ai` Next.js operator/admin UI: `apps/next-admin/packages/rustok-ai/`

## What Not to Do

- Do not duplicate module-owned UI in host applications.
- Do not introduce package-local locale negotiation.
- Do not call raw GraphQL/native adapters from Leptos UI.
- Do not embed another module's workflow into Groups UI.
- Do not treat source presence as FFA parity evidence without executing the
  documented verification gates.

## Related Documents

- [UI Packages Quick Start](./UI_PACKAGES_QUICKSTART.md)
- [Module Platform Overview](./overview.md)
- [Module Platform Crate Registry](./crates-registry.md)
- [Groups implementation plan](../../crates/rustok-groups/docs/implementation-plan.md)
