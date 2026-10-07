# rustok-product-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-product` module.

## Responsibilities

- Exposes the product catalog admin root view used by `apps/admin`.
- Keeps product list/create/edit/publish/archive workflow inside the product-owned package.
- Keeps admin shell copy, profile-panel state, list/status/filter, list-card view-model, editor shell view-model, shipping-profile, selected-summary, pricing-preview and pricing deep-link presentation helpers in framework-agnostic `src/core.rs`, leaving Leptos as the render/effect adapter.
- Isolates Leptos rendering in `src/ui/*`: `src/ui/root.rs` owns the mounted router shell, `src/ui/product_grid.rs`, `src/ui/product_editor.rs`, `src/ui/attributes.rs` and `src/ui/categories.rs` own the routed pages, and `src/ui/leptos.rs` owns the shared sections those pages mount (typed attribute values, variant axes, category-schema authoring). The crate root re-exports the mounted `ui::root::ProductAdmin`.
- Routes admin data operations through `src/transport.rs`, with GraphQL operations in `src/transport/graphql_adapter.rs` and native server functions in `src/transport/native_server_adapter.rs`.
- Builds native catalog schema services from `HostRuntimeContext` DB and typed `TransactionalEventBus` host handles without a package-local framework runtime or framework-specific outbox adapter.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses registry-backed shipping-profile selection so catalog operators work with typed product bindings instead of raw slug text.
- Ships package-owned `admin/locales/en.ftl` and `admin/locales/ru.ftl` bundles declared through `[provides.admin_ui.i18n]`.
- Embeds owner-side product SEO editing through `rustok-seo-panel` so product metadata stays inside the product screen.

## Entry Points

- `ProductAdmin` - mounted root admin view re-exported from `ui::root` and rendered from the host admin registry.
- `ProductGridPage`, `ProductEditorPage`, `AttributesPage`, `CategoriesPage` - routed pages of the mounted surface.
- `core::*` helpers for product admin shell copy, profile-panel state, product list/status/filter labels, list-card view-models, editor shell view-models, selected-summary view-models, pricing previews and pricing deep links.
- `transport::*` facade functions for product admin native and GraphQL operations.

### Non-mounted reference compositions

`ui::leptos::ProductAdmin` (single-screen composition) and `ui::catalog_admin::ProductAdmin` (the same screen behind the catalog-controls query shell, 113 lines) are **not mounted**: the host code generator mounts `ui::root::ProductAdmin`. They are kept as the canonical catalog-controls reference pinned by the catalog verification suite, and they are not advertised as entry points.
Every capability they expose is reachable on the mounted surface: list controls and search in `ui/product_grid.rs` + `ui/catalog_admin.rs`, editor shell and media in `ui/product_editor.rs`, typed attribute values through `ProductAttributeValuesSection`, variant axes through `ProductVariantAxesSection`, attribute/schema authoring in `ui/attributes.rs`, and category work in `ui/categories.rs`. New feature work belongs on the mounted pages and the shared sections; the reference compositions must not grow.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses the `rustok-commerce` GraphQL contract for product CRUD while ownership moves to module-owned UI.
- Treats `product -> variants.prices` as a catalog compatibility snapshot and now
  renders pricing-authoritative preview through a separate `adminPricingProduct`
  hook instead of presenting catalog snapshot rows as resolved prices.
- Links directly into `rustok-pricing/admin` with prefilled product id and
  pricing context so operators can move from catalog editing to pricing control
  without reselecting the product.
- Uses the shared `rustok-seo` GraphQL contract through `rustok-seo-panel`
  for explicit product SEO authoring.
- Accepts product edit deep links through query `id=` so neighboring
  module-owned admin routes can return to the exact catalog item without using
  display fields as identity.
- Reads the effective UI locale from `UiRouteContext.locale`; product translation edits and edit-form hydration both resolve against that host-owned locale without a package-local locale override.

## Documentation

- See [platform docs](../../../../docs/index.md).
