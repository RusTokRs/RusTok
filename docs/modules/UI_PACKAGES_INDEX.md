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
the current contract layer. It does not replace the local docs of the modules themselves and does not
duplicate their runtime/UI details.

## Basic Rule

- UI packages belong to the module itself, not to the host application;
- Leptos admin/storefront UI surfaces are published through `admin/` and
  `storefront/` sub-crates inside the module crate;
- Next.js host applications only mount module-owned UI surfaces and must not
  become their canonical owner;
- The source of truth for UI wiring lives in `rustok-module.toml`, the local
  `README.md` and `docs/README.md` of the module itself.

## What Counts as a UI Package

For a platform module, a UI surface is considered correctly structured if there is:

- Root `README.md` of the module in English;
- Local `docs/README.md` in English;
- Local `docs/implementation-plan.md` in English;
- `rustok-module.toml` with correct `[provides.admin_ui]` and/or
  `[provides.storefront_ui]` if the module actually provides UI;
- `admin/Cargo.toml` and/or `storefront/Cargo.toml` if such UI is declared in
  manifest wiring.

The mere presence of an `admin/` or `storefront/` folder is not considered proof of
integration. The canonical source of truth here is only manifest wiring.

## Runtime Contract for UI Packages

- Leptos module-owned UI uses the host-provided locale contract and does not
  invent its own locale fallback chain;
- For internal Leptos data layer, `#[server]` functions are used by default,
  while GraphQL remains the target parallel transport contract. Current single-adapter
  packages are valid only as documented module-local exceptions;
- Next.js hosts work through server/API contracts and do not duplicate module-owned
  domain logic in the application;
- Host applications are responsible only for mount/wiring/navigation, not for
  ownership of module UI functionality.
- In `apps/next-admin`, module-owned navigation does not live in core nav: each Next UX package
  is registered from `apps/next-admin/packages/*` or mounted `@rustok/*-admin`,
  receives `moduleSlug` through the registry and is hidden if the module is not enabled for the tenant.
  This preserves the scenario where a tenant uses only `blog`, without showing product/commerce UX.

## Where to Look

### General Contract

- [`rustok-module.toml` Contract](./manifest.md)
- [Module and Application Registry](./registry.md)
- [Module Documentation Index](./_index.md)
- [Module Documentation Template](../templates/module_contract.md)

### UI and Host Applications

- [UI Overview](../UI/README.md)
- [GraphQL and Leptos Server Functions](../UI/graphql-architecture.md)
- [Storefront Contract](../UI/storefront.md)
- [Admin ↔ Server Quick Start](../UI/admin-server-connection-quickstart.md)
- **Module UI Package Guides** (read when working on `admin/` or `storefront/` sub-crates):
  - [Architecture Guide](../UI/module-package-architecture.md) — FFA, `core/transport/ui` split, Dioxus-readiness
  - [Implementation Guide](../UI/module-package-implementation.md) — file structure, internal crates, i18n, forbidden patterns
  - [Verification Guide](../UI/module-package-verification.md) — verification commands, common errors

### Local Application Docs

- [Admin Documentation](../../apps/admin/docs/README.md)
- [Storefront Documentation](../../apps/storefront/docs/README.md)
- [Next Admin Documentation](../../apps/next-admin/docs/README.md)
- [Next Frontend Documentation](../../apps/next-frontend/docs/README.md)

## Module UI Examples

### Core/Admin Surfaces

- `rustok-channel` admin UI: [README](../../crates/rustok-channel/admin/README.md)
- `rustok-index` admin UI: [README](../../crates/rustok-index/admin/README.md)
- `rustok-outbox` admin UI: [README](../../crates/rustok-outbox/admin/README.md)
- `events` admin UI: [README](../../crates/rustok-events-module/admin/README.md);
  the sibling Next package is
  `crates/rustok-events-module/next-admin`.
- `iggy_connector` admin UI:
  [README](../../crates/rustok-iggy-connector/admin/README.md); the sibling
  Next package is `crates/rustok-iggy-connector/next-admin`.
- `rustok-auth` admin UI: [README](../../crates/rustok-auth/admin/README.md), fast boundary gate `npm run verify:auth:admin-boundary`
- `rustok-tenant` admin UI: [README](../../crates/rustok-tenant/admin/README.md)
- `rustok-rbac` admin UI: [README](../../crates/rustok-rbac/admin/README.md)

### Optional/Admin Surfaces

- `rustok-product` admin UI: [README](../../crates/rustok-product/admin/README.md)
- Ecommerce admin UI routes `rustok-product` <-> `rustok-pricing` support
  stable deep links through product `id`; display fields are not used as identity.
- `rustok-fulfillment` admin UI: [README](../../crates/rustok-fulfillment/admin/README.md)
- `rustok-customer` admin UI: [README](../../crates/rustok-customer/admin/README.md)
- `rustok-region` admin UI: [README](../../crates/rustok-region/admin/README.md)
- `rustok-order` admin UI: [README](../../crates/rustok-order/admin/README.md)
- `rustok-inventory` admin UI: [README](../../crates/rustok-inventory/admin/README.md)
- `rustok-pricing` admin UI: [README](../../crates/rustok-pricing/admin/README.md)
- `rustok-commerce` admin UI: [README](../../crates/rustok-commerce/admin/README.md)
- `rustok-page-builder` admin UI: [README](../../crates/rustok-page-builder/admin/README.md)
- `rustok-pages` admin UI: [README](../../crates/rustok-pages/admin/README.md)
- `rustok-seo` admin UI: [README](../../crates/rustok-seo/admin/README.md)
- `rustok-blog` admin UI: [README](../../crates/rustok-blog/admin/README.md)
- `rustok-forum` admin UI: [README](../../crates/rustok-forum/admin/README.md)
- `rustok-search` admin UI: [README](../../crates/rustok-search/admin/README.md)
- `rustok-media` admin UI: [README](../../crates/rustok-media/admin/README.md)
- `rustok-comments` admin UI: [README](../../crates/rustok-comments/admin/README.md)
- `rustok-workflow` admin UI: [README](../../crates/rustok-workflow/admin/README.md)

### Optional/Storefront Surfaces

- `rustok-blog` storefront UI: [README](../../crates/rustok-blog/storefront/README.md)
- `rustok-cart` storefront UI: [README](../../crates/rustok-cart/storefront/README.md)
- `rustok-commerce` storefront UI: [README](../../crates/rustok-commerce/storefront/README.md)
- `rustok-fulfillment` storefront UI: [README](../../crates/rustok-fulfillment/storefront/README.md)
- `rustok-payment` storefront UI: [README](../../crates/rustok-payment/storefront/README.md)
- `rustok-order` storefront UI: [README](../../crates/rustok-order/storefront/README.md)
- `rustok-forum` storefront UI: [README](../../crates/rustok-forum/storefront/README.md)
- `rustok-pages` storefront UI: [README](../../crates/rustok-pages/storefront/README.md)
- `rustok-pricing` storefront UI: [README](../../crates/rustok-pricing/storefront/README.md)
- `rustok-product` storefront UI: [README](../../crates/rustok-product/storefront/README.md)
- Ecommerce storefront UI routes `rustok-product` <-> `rustok-pricing`
  preserve navigation context through `handle` and pricing query fields, and locale
  continues to be taken only from host `UiRouteContext`.
- Storefront product/pricing UI shows stable `seller_id` as seller boundary;
  `vendor` remains a merchandising/display label and is not used as identity.
- `rustok-region` storefront UI: [README](../../crates/rustok-region/storefront/README.md)
- `rustok-search` storefront UI: [README](../../crates/rustok-search/storefront/README.md)
- `rustok-seo` remains `admin_only`: storefront SEO runtime lives in `apps/storefront` and `apps/next-frontend`
  through shared SEO contract, not through a separate module-owned storefront package.
- Entity-specific SEO UI is not centralized in `rustok-seo-admin`: page/product/blog/forum SEO
  panels belong to owner modules, while `rustok-seo-admin` remains a cross-cutting infrastructure UI.
- Reusable owner-side SEO widgets and transport helpers live in the support crate
  `rustok-seo-admin-support`, not in host application code.

### Large Capability/Admin Surfaces

These entries are visibility/scaffold references for large capability work. They are not
manifest-backed integration proof unless the capability crate also has `rustok-module.toml`
with `[provides.admin_ui]`; the manifest remains the canonical source for mounted UI.

- `rustok-ai` Leptos operator/admin UI: [README](../../crates/rustok-ai/admin/README.md)
- `rustok-ai` Next.js operator/admin UI: `apps/next-admin/packages/rustok-ai/`

## What Not to Do

- Do not describe UI package contract only in `docs/modules/*` without updating
  local docs of the module itself;
- Do not duplicate module-owned UI in `apps/admin` or `apps/storefront`;
- Do not introduce package-local locale negotiation;
- Do not consider old installation and deployment instructions as the source of truth for current UI
  wiring.

## Related Documents

- [UI Packages Quick Start](./UI_PACKAGES_QUICKSTART.md)
- [Module Platform Overview](./overview.md)
- [Module Platform Crate Registry](./crates-registry.md)
### Next.js Admin Showcase

- `rustok-blog`: `apps/next-admin/packages/blog/`
- `rustok-product`: `apps/next-admin/packages/rustok-product/` as the current Next UX package over GraphQL product read-side; `apps/next-admin/src/features/products/` remains only a registration shim.
- `rustok-search`: `apps/next-admin/packages/search/`
- `rustok-workflow`: `apps/next-admin/packages/workflow/`
- `rustok-rbac`: `apps/next-admin/packages/rbac/`
- `rustok-email`: `apps/next-admin/packages/email/`
- `rustok-cache`: `apps/next-admin/packages/cache/`
- `rustok-events`: `apps/next-admin/packages/events/`

- `rustok-navigation`: [storefront README](../../crates/rustok-navigation/storefront/README.md)
