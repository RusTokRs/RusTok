# rustok-product-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-product` module.

## Responsibilities

- Exposes the product catalog admin root view used by `apps/admin`.
- Keeps product list/create/edit/publish/archive workflow inside the product-owned package.
- Keeps admin shell copy, profile-panel state, list/status/filter, list-card view-model, editor shell view-model, shipping-profile, selected-summary, pricing-preview and pricing deep-link presentation helpers in framework-agnostic `src/core.rs`, leaving Leptos as the render/effect adapter.
- Isolates Leptos rendering in `src/ui/leptos.rs`, with crate root re-exporting `ProductAdmin`.
- Routes admin data operations through `src/transport.rs`, with GraphQL operations in `src/transport/graphql_adapter.rs` and native server functions in `src/transport/native_server_adapter.rs`.
- Builds native catalog schema services from `HostRuntimeContext` DB and typed `TransactionalEventBus` host handles without a package-local Loco runtime or outbox Loco adapter.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses registry-backed shipping-profile selection so catalog operators work with typed product bindings instead of raw slug text.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.
- Embeds owner-side product SEO editing through `rustok-seo-admin-support` so product metadata stays inside the product screen.

## Entry Points

- `ProductAdmin` - root admin view re-exported from `ui::leptos` and rendered from the host admin registry.
- `core::*` helpers for product admin shell copy, profile-panel state, product list/status/filter labels, list-card view-models, editor shell view-models, selected-summary view-models, pricing previews and pricing deep links.
- `transport::*` facade functions for product admin native and GraphQL operations.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses the `rustok-commerce` GraphQL contract for product CRUD while ownership moves to module-owned UI.
- Treats `product -> variants.prices` as a catalog compatibility snapshot and now
  renders pricing-authoritative preview through a separate `adminPricingProduct`
  hook instead of presenting catalog snapshot rows as resolved prices.
- Links directly into `rustok-pricing/admin` with prefilled product id and
  pricing context so operators can move from catalog editing to pricing control
  without reselecting the product.
- Uses the shared `rustok-seo` GraphQL contract through `rustok-seo-admin-support`
  for explicit product SEO authoring.
- Accepts product edit deep links through query `id=` so neighboring
  module-owned admin routes can return to the exact catalog item without using
  display fields as identity.
- Reads the effective UI locale from `UiRouteContext.locale`; product translation edits and edit-form hydration both resolve against that host-owned locale without a package-local locale override.

## Documentation

- See [platform docs](../../../docs/index.md).
