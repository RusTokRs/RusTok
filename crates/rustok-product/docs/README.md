# `rustok-product` Documentation

`rustok-product` — default catalog submodule of the `ecommerce` family.

## Purpose

- product catalog;
- variants, options, translations and publishing;
- taxonomy-backed product tags via shared `rustok-taxonomy` and product-owned relation `product_tags`;
- product-owned migrations;
- `ProductModule`, `CatalogService`, module-owned admin UI package `rustok-product/admin` and module-owned storefront UI package `rustok-product/storefront`.

## Scope

- `rustok-product` owns module-owned admin/storefront UI packages, native `#[server]` internal paths and product-owned GraphQL contract types; the umbrella `rustok-commerce` remains the ecommerce composition layer, but does not mask the product owner boundary.
- The storefront read-side for published catalog now lives in `rustok-product/storefront` and uses native Leptos server functions over `CatalogService`, keeping the GraphQL storefront contract as a fallback.
- Product CRUD in the admin UI has been moved out of `rustok-commerce-admin`
  into the module-owned route `product`; native server functions are the primary internal path,
  and GraphQL is maintained in parallel without returning ownership to the umbrella `rustok-commerce`;
- The generic GraphQL roots `product` / `storefrontProduct`, which are still used by
  module-owned product UI packages, are considered a catalog-authoritative surface:
  `variants.prices` in them remains a compatibility snapshot without explicit
  currency/region/price-list/channel resolution and must not be treated as a
  pricing source of truth alongside `adminPricingProduct` / `storefrontPricingProduct`;
- Module-owned `rustok-product/admin` and `rustok-product/storefront` are now also
  synchronized with this split: the UI no longer shows generic catalog
  `variants.prices` as resolved price, but keeps a separate pricing-module preview
  hook for `adminPricingProduct` / `storefrontPricingProduct`; admin list/status/filter,
  shipping-profile, pricing-preview and pricing deep-link helpers live in
  framework-agnostic `admin/src/core.rs` (including `SelectedProductSummaryViewModel`),
  admin GraphQL operations go through the module-owned facade `admin/src/transport.rs`,
  and the Leptos render/effect adapter is isolated in `admin/src/ui/leptos.rs`;
- The storefront FFA slices have moved route/query normalization, typed fetch request shape,
  shell copy, selected-product view-model composition, selected-card labels/empty
  state, catalog rail presentation, pricing/seller labels, pricing-context
  sanitization/defaulting and pricing deep-link state into `storefront/src/core.rs`;
  native/GraphQL storefront fetch paths are structured as `storefront/src/transport/`
  adapters, and Leptos `ProductView`/`SelectedProductCard`/`CatalogRail` live in
  `storefront/src/ui/leptos.rs` as a thin host-context/render layer over
  prepared core state;
- Shared DTOs, entities and error surface come from `rustok-commerce-foundation`.
- FBA boundary is published as `ProductCatalogReadPort` / `product.catalog_read.v1`.
  Registry `contracts/product-fba-registry.json`, static matrix,
  no-compile runtime contract smoke and source-locked runtime fallback smoke maintain status
  `boundary_ready`; `transport_verified` remains closed until live provider execution evidence.
- Canonical vocabulary and attach semantics for product tags live in
  `rustok-taxonomy` + `product_tags`, and the public contract uses a first-class
  `tags` field instead of legacy `metadata.tags`.
- Shipping profile for product and variant now has a first-class typed surface in
  product DTO (`shipping_profile_slug`) and typed persistence in
  `products.shipping_profile_slug` / `product_variants.shipping_profile_slug`; metadata-backed
  `shipping_profile.slug` remains only a backward-compatible normalization form for old
  read/write-path consumers.
- Multivendor foundation now also starts at the product boundary: the create/update/read contract
  includes nullable `seller_id`, which is the canonical seller identity key for downstream
  cart/order/fulfillment orchestration; merchandising/display fields like `vendor` must not
  be used as seller identity.
- Effective shipping profile for deliverability is resolved as
  `variant.shipping_profile_slug -> product.shipping_profile_slug -> default`, and omission
  of the first-class field on the write-path must not overwrite an existing typed binding/compatibility
  normalization.
- Transport-level validation for `shipping_profile_slug` now lives in the
  `rustok-commerce` facade and checks the reference against active shipping profiles from the typed
  registry `shipping_profiles`, so that the product write-path does not accept arbitrary slugs.

## Native catalog attributes

- `product_attributes` is the unified reference for ecommerce attributes.
- `catalog_categories` stores structural, collection and virtual categories; `products.primary_category_id` determines product form only through structural category.
- `product_attribute_schemas` are optional reusable templates, and category bindings/groups provide inheritance, clone snapshot, custom and local override scenarios.
- `product_categories` stores additional navigation/storefront bindings and does not change the product form.
- Values live in typed product/variant attribute value tables; localized labels and text-like values are in translation tables.
- Product-level values are read and modified through an owner-owned typed read/patch contract: an omitted attribute does not change, `clear` deletes the value, an empty multiselect clears the value, options and effective schema are validated before transactional write, and detached values are preserved and returned with a separate marker.
- Detached values are displayed in the product admin as a separate review block and are cleared only through the owner-owned `clear_detached_product_attribute_values`; the service verifies that each deleted attribute is indeed outside the current effective schema, native `#[server]` remains the primary path, GraphQL is maintained in parallel.
- Publish validation is performed in the owner-owned `ProductCatalogSchemaService`: required effective attributes must be filled before the product transitions to `Active`, localized text-like values require an explicit non-empty translation row, option attributes require saved option relations, and create-with-publish is rejected for categories with required typed attributes.
- Effective form loads localized options in a single bounded query by effective attribute ids; schema/category groups return a localized `group_label` by host locale, binding uses stable `group_code`, and the product admin groups fields by label/code, displays typed controls and sends only dirty patches after saving the product and its primary category.
- `rustok-index` when indexing a product materializes tenant/locale-scoped category strings and normalized facet/search/sort values. Multiselect expands to one row per option, localized labels are taken only from the explicit row of the requested locale, and effective attribute ids are computed by a read-only resolver in `rustok-product`, so detached values do not enter the read model.
- `rustok-search` reads these projections directly for category/virtual-category filters, channel-scoped attribute facets and attribute sorting. The write model remains with `rustok-product`; search does not recompute schema inheritance and does not read detached values as effective.
- Visibility flags are computed with priority `global attribute defaults < schema/category overrides < channel settings`. Overrides are tri-state: a missing field inherits the previous value, explicit `false` disables the behavior. The resolver preserves overrides through live inheritance and clone snapshots, and the indexer creates separate rows for each active channel with effective facet/search/sort, comparison, storefront and admin-grid flags. If a tenant has no active channels, one global row is created with `channel_id = null`.
- Virtual category uses a bounded V1 rule contract. All filled predicates are combined with AND: `statuses`, `primary_category_subtree_id`, overlapping range `price_min`/`price_max`, `in_stock` and a list of `attributes` with `eq`/`range` operators. Attribute rules work only with stable option codes and locale-neutral product values; localized and variant-only attributes are rejected on the write-side.

```json
{
  "version": 1,
  "statuses": ["active"],
  "primary_category_subtree_id": "00000000-0000-0000-0000-000000000000",
  "price_min": 1000,
  "price_max": 5000,
  "in_stock": true,
  "attributes": [
    { "code": "brand", "operator": "eq", "value": "rustok" },
    { "code": "weight", "operator": "range", "min": "1.0", "max": "2.5" }
  ]
}
```

When reindexing a product, `rustok-index` first completely replaces its rows in `virtual_category_product_assignments`, then builds localized category projections. Invalid old rule payloads do not stop reindex: the category is skipped with a warning; the creation service rejects new invalid payloads.

## Integration

- the module is part of the ecommerce family and must maintain its own storage/runtime boundary without returning responsibility to the umbrella `rustok-commerce`;
- product-owned admin/storefront UI, native server functions, GraphQL contract types and FBA read port are published by the owner `rustok-product`; `rustok-commerce` only composes the ecommerce family and does not reclaim product service/DTO ownership;
- cross-module contract changes must be synchronized with `rustok-commerce` and neighboring split modules.

## Search metadata

- The Leptos product admin package exports `fetch_catalog_search_options` and neutral option DTOs. The helper requires the host effective locale, uses a current-tenant native `#[server]` endpoint with parallel GraphQL fallback and is already connected in `apps/admin` through host-owned `SearchAdminComposition` without a direct dependency from search UI on `rustok-product`.
- The Leptos product storefront package publishes a separate public-safe `fetch_catalog_search_options`: native `product/storefront/catalog-search-options` is the primary path, GraphQL `storefrontCatalogSearchOptions(locale: String!)` remains parallel. The payload contains only category ids/labels and filterable/sortable attribute codes/labels; `apps/storefront` connects it through `SearchStorefrontComposition` with host effective locale.
- The Next storefront has a mirror product-owned helper `apps/next-frontend/packages/rustok-product::fetchCatalogSearchOptions` which reads the same public GraphQL contract and returns safe DTOs to the host search composition without a direct product dependency inside the search package.

- The Next admin product package exposes owner-owned search metadata helpers: category options return `catalogCategories.id`, attribute options return filterable/sortable `productAttributes.code`, labels use the host effective locale, and the search UI consumes only host-provided options without importing product internals.

## SEO ownership

- `rustok-product/admin` already keeps an owner-side product SEO panel via
  `rustok-seo-admin-support`, without moving product metadata editing into `rustok-seo-admin`.

## Verification

- `npm.cmd run verify:product:runtime-fallback-smoke`
- `npm.cmd run test:verify:product:runtime-fallback-smoke`
- `npm.cmd run verify:ecommerce:fba`
- `npm.cmd run test:verify:ecommerce:fba`
- cargo xtask module validate product
- cargo xtask module test product
- targeted commerce tests for the product domain when changing runtime wiring

Cargo checks remain a targeted/live evidence step before promoting product FBA to `transport_verified`; the current fast gate for boundary evidence does not require Rust compilation.
## Related documents

- [README crate](../README.md)
- [README admin UI](../admin/README.md)
- [Commerce split plan](../../rustok-commerce/docs/implementation-plan.md)
