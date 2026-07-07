# rustok-product

## Purpose

`rustok-product` is the default catalog submodule of the `Ecommerce` family.

## Responsibilities

- Product entities, translations, options, variants, and product-owned migrations.
- Native catalog categories, category-bound product forms, reusable product
  attribute schemas, product attribute dictionaries, typed product/variant
  attribute values, and highload-ready category/value projections.
- Typed product attribute value reads and transactional patches validate the
  effective category schema and option ownership, keep localized text in the
  requested translation row, preserve detached values, and publish one outbox
  event per patch request.
- Product publication validates required effective attributes before a product
  becomes active; localized text-like required values need an explicit non-empty
  translation row, option attributes need stored option relations, and
  create-with-publish is rejected when required typed attributes cannot yet be
  populated.
- Detached values remain visible to product operators after a primary-category
  change and can be explicitly cleared through owner-owned native server
  functions with parallel GraphQL; the service rejects attempts to clear values
  that are still effective.
- Effective product forms expose localized option dictionaries and localized
  group labels with bounded reads; schema/category group creation and
  `group_code` bindings are available over native server functions plus
  parallel GraphQL, and the module-owned admin renders grouped typed editors
  with dirty-field patch semantics.
- `rustok-index` materializes tenant- and locale-scoped category assignments and
  normalized attribute rows for facets, full-text input, and sorting. The
  projection resolves effective category membership through the product-owned
  schema reader, so detached values are excluded without becoming write-side
  state.
- `rustok-search` consumes those materialized category and attribute projections
  for category/virtual-category filters, channel-scoped attribute facets and
  attribute sorting, while `rustok-product` remains the write-model owner.
- Effective visibility is resolved as tri-state overrides with precedence
  `attribute defaults < schema/category overrides < channel settings`.
  Attribute facet/search/sort rows are emitted per active channel; tenants with
  no active channel receive one explicit global projection scope. The rows also
  retain effective storefront, comparison, and admin-grid visibility flags.
- Virtual categories use a validated, bounded V1 rule contract over product
  status, primary-category subtree, intersecting price range, stock state, and
  effective locale-neutral product attribute equality/ranges. The indexer
  replaces materialized assignments idempotently before category projections.
- Product-owned relation storage for taxonomy-backed tags (`product_tags`).
- Product write-side services and publication lifecycle.
- Product-side synchronization of first-class `tags` contract fields with the
  taxonomy-backed dictionary.
- Product-side normalization of first-class `shipping_profile_slug` onto the
  temporary metadata-backed shipping profile contract, without erasing an
  existing metadata-backed profile when the typed field is omitted.
- Product-side ownership of nullable `seller_id` as the canonical marketplace
  identity key that downstream cart/order/fulfillment flows consume; merchandising
  fields such as `vendor` remain display-only and are not used as seller identity.
- Product-side split and locale-aware resolution of Flex attached custom-field
  values, using shared `flex` attached localized storage while preserving
  non-Flex operational metadata in `products.metadata`.
- Publish a module-owned Leptos admin UI package in `admin/` for catalog CRUD,
  publication lifecycle, and shipping-profile selection.
- Publish a module-owned Leptos storefront UI package in `storefront/` for
  published catalog discovery, handle-based product selection, and
  channel-aware inventory visibility.
- Keep generic catalog price snapshots available for product-owned CRUD and
  discovery flows, while treating pricing-authoritative reads as the
  responsibility of `rustok-pricing` surfaces (`adminPricingProduct` /
  `storefrontPricingProduct`).
- Keep product-owned admin/storefront UI aligned with that split by rendering
  catalog snapshot pricing separately from pricing-module previews instead of
  using generic `variants.prices` as resolved pricing.
- Keep storefront shell copy, typed fetch request shape, selected-card labels,
  empty state, and rail presentation state in the framework-agnostic storefront
  core so Leptos remains a host-context/render adapter over native + GraphQL
  transport parity.
- Publish the owner `ProductCatalogReadPort` / `product.catalog_read.v1`
  boundary for catalog-read consumers. The in-process `CatalogService`
  implementation is `boundary_ready` on no-compile runtime fallback evidence;
  `transport_verified` still requires live provider execution evidence.
- Product module metadata for runtime registration.
- Product-owned catalog search metadata for optional category filters and
  filterable/sortable attribute controls in admin/storefront search UI. Hosts
  inject those options through composition; search UI does not import product
  internals or negotiate locale itself.
- Product translation title search predicates are not owned by `apps/server`;
  shared ecommerce readers use the owner/foundation search helper instead of a
  host-local `product_search` service.

## Interactions

- Depends on `rustok-commerce-foundation` for shared commerce DTOs/entities/errors.
- Depends on `flex` for shared attached localized-value storage helpers used by
  product custom-field multilingual flows.
- Depends on `rustok-taxonomy` for shared scope-aware tag dictionary while keeping `product_tags`
  module-owned.
- Depends on `rustok-outbox` and `rustok-events` for transactional event publishing.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.
- Consumed by `apps/admin` through manifest-driven module UI composition.
- Consumed by `apps/storefront` through manifest-driven module UI composition.
- Consumed by `rustok-index` through the read-only effective-form resolver for
  highload product projections.
- Consumed by `rustok-search` UI hosts through product-owned catalog search
  metadata helpers for category filters and attribute filter/sort controls.
- Consumed by commerce, pricing, and `rustok-ai-product` through the
  `ProductCatalogReadPort` catalog-read contract instead of umbrella product
  service re-exports.

## Entry points

- `ProductModule`
- `CatalogService`
- `ProductCatalogReadPort`
- `services::catalog_schema::resolve_effective_product_form`
- `ProductCatalogSchemaService`
- `admin::ProductAdmin`
- `storefront::ProductView`

See also `docs/README.md`.
