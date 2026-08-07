# rustok-product

## Purpose

`rustok-product` is the default catalog submodule of the `Ecommerce` family.

## Responsibilities

- Product entities, translations, options, variants, and product-owned migrations.
- PostgreSQL target-schema enforcement removes unused compatibility columns,
  requires Media-owned image identifiers, preserves decimal variant weights,
  and maintains an indexed globally visible storefront page path.
- One Product-owned public-error policy redacts internal database details across
  GraphQL and native admin/storefront transports while preserving stable codes
  and correlation references.
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
- Own positive monotonic `index_revision` storage columns for Product and
  ProductVariant. Product updates and Product-translation changes advance the
  Product revision; every ProductVariant update advances the variant revision.
  Variant insert/delete/move membership also advances the affected parent
  Product revision. These are owner-input watermarks, not the complete Product
  Index mutation clock.
- Own retained hard-delete replay state in `product_index_tombstones` and
  `product_variant_index_tombstones`. Physical Product deletion retains every
  current translation locale, direct translation deletion retains the exact
  removed locale, and ProductVariant deletion retains its non-localized key.
  Recreated identities receive an `index_revision` strictly above the retained
  delete before the tombstone is cleared.
- Own append-only `product_sales_channel_index_relation_snapshots` with a
  dedicated monotonic relation epoch, bounded current/change readers,
  idempotent complete-membership replacement, live-Product delete fencing, and
  retained empty membership after Product hard delete. Product still does not
  resolve Channel slugs or depend on `rustok-channel`/`rustok-index`.
- Own append-only `product_index_graph_projection_snapshots`, whose
  `projection_epoch` advances when either retained Product state or resolved
  SalesChannel membership advances. This is the complete Product graph mutation
  clock used by the selected Index source.
- Publish only a neutral `ProductRuntimeSelected` marker for selected
  cross-module composition. The Product crate does not depend on `rustok-index`
  and does not construct generic Index mutations.
- The selected `rustok-distribution` bridge publishes exactly one current
  Product Index contract and one current ProductVariant contract. Product replay
  is locale-aware and enumerates stable `(product_id, locale)` identities;
  ProductVariant replay is non-localized and enumerates stable `variant_id`
  identities.
- The current Product Index graph carries Product identity/scalars,
  `variant_ids` with a many `variants` link, and `sales_channel_ids` with a many
  `sales_channels` link. Product visibility slugs remain Product-owned resolver
  input and are not duplicated as transitional Index fields.
- Product replay uses `projection_epoch` as the full-record mutation
  `source_version`; ProductVariant replay uses its owner `index_revision`.
  Product locale absence uses the same Product projection clock and fails closed
  until the projection matches the current Product watermark.
- The selected distribution composition resolves Product channel visibility to
  current tenant Channel UUID membership and writes only through the
  Product-owned relation store. Unrestricted Product visibility resolves
  against the current tenant Channel identity universe; Channel runtime active
  state remains Channel-owned. This resolver is bounded and convergent, not an
  atomic cross-owner snapshot or continuous event consumer.
- The selected bridge emits `IndexMutation::Upsert` for live owner rows and
  `IndexMutation::Delete` for retained hard-delete identities. The canonical
  Product graph has no parallel Product compatibility source/schema branches.
- The Index-owned bounded multi-pass reconciliation runner can restart these
  sources from the beginning and catch live or retained identities inserted
  behind an earlier cursor. It is explicit and crash-resumable, but it does not
  establish a repeatable-read owner snapshot or close the final-pass
  concurrent-write window. Durable relation convergence triggering, retained
  freshness/equivalence evidence, event-contract admission, tombstone
  retention/purge admission, and authoritative Index cutover remain open.
- Effective visibility is resolved as tri-state overrides with precedence
  `attribute defaults < schema/category overrides < channel settings`.
- Virtual categories use a validated, bounded V1 rule contract over product
  status, primary-category subtree, intersecting price range, stock state, and
  effective locale-neutral product attribute equality/ranges. Materialized
  assignments remain reserved storage until an owner-owned runtime is implemented.
- Catalog category writes preserve a canonical closure projection; deferred
  PostgreSQL constraints reject parent cycles and closure drift at commit.
- Product-owned relation storage for taxonomy-backed tags (`product_tags`).
- Product write-side services and publication lifecycle.
- Responsibility-specific catalog components: commands, queries, product
  projection, tags, typed values, and effective-form resolution remain behind
  the stable `CatalogService` and `ProductCatalogSchemaService` entry points.
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
  discovery flows through the transaction-aware `rustok-pricing-persistence`
  owner contract, while treating pricing-authoritative reads as the
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
  implementation has live PostgreSQL execution evidence. The composed
  `rustok-ai` consumer has live unavailable/deadline degraded-path evidence;
  Commerce checkout currently treats Product as a hard dependency. The module
  therefore remains honestly `boundary_ready` until an external adapter and
  any declared Commerce degraded policy have live execution evidence.
- Product module metadata for runtime registration.
- Product-owned catalog search metadata for optional category filters and
  filterable/sortable attribute controls in admin/storefront search UI. Hosts
  inject those options through composition; search UI does not import product
  internals or negotiate locale itself.
- Product translation title search predicates are not owned by `apps/server`;
  shared ecommerce readers use the owner/foundation search helper instead of a
  host-local `product_search` service.
- `StorefrontProductListQuery` owns storefront filters, sorting, and validated
  page/per-page input as one Product query contract across native and GraphQL
  transports.

## Interactions

- Owns Product DTOs, ORM entities, errors, tables, and migration history without
  a `rustok-commerce-foundation` or `rustok-index` dependency.
- Depends on `rustok-pricing-persistence` for pricing-owned ORM projections and
  atomic initial-price lifecycle operations.
- Depends on `rustok-inventory` for inventory-owned bootstrap and availability
  operations.
- Depends on `flex` for shared attached localized-value storage helpers used by
  product custom-field multilingual flows.
- Depends on `rustok-taxonomy` for shared scope-aware tag dictionary while keeping `product_tags`
  module-owned.
- Depends on `rustok-outbox` and `rustok-events` for transactional event publishing.
- Is consumed by the selected distribution bridge for generic Index schema/source
  composition and Product-to-Channel relation resolution; Index core and server
  replay composition remain Product-agnostic.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.
- Consumed by `apps/admin` through manifest-driven module UI composition.
- Consumed by `apps/storefront` through manifest-driven module UI composition.
- Consumed by `rustok-search` UI hosts through product-owned catalog search
  metadata helpers for category filters and attribute filter/sort controls.
- Consumed through `ProductCatalogReadPort` by Commerce checkout and the
  `rustok-ai-product` support adapter composed by `rustok-ai`. Pricing uses
  Product owner services directly in the embedded runtime and is not falsely
  declared as a Product read-port fallback consumer.

## Entry points

- `ProductModule`
- `ProductRuntimeSelected`
- `CatalogService`
- `ProductCatalogReadPort`
- `ProductSalesChannelIndexRelationStore`
- `services::catalog_schema::resolve_effective_product_form`
- `ProductCatalogSchemaService`
- `admin::ProductAdmin`
- `storefront::ProductView`

See also `docs/README.md`, the Index
[M7 canonical Product graph contract](../rustok-index/docs/m7-product-graph-source.md),
[M7 Product-SalesChannel resolver contract](../rustok-index/docs/m7-product-sales-channel-resolver.md),
[M7 Product tombstone replay contract](../rustok-index/docs/m7-product-tombstone-source.md),
[M7 Product graph projection ledger](docs/index-graph-projection-ledger.md), and
[M7 bounded Product reconciliation contract](../rustok-index/docs/m7-product-reconciliation.md).
