# Implementation Plan for `rustok-product`

## Current state

`rustok-product` owns the catalog, variants, category-bound attribute schemas,
typed attribute values, and product admin/storefront packages. Product UI uses
owner-owned core, transport, and Leptos adapter layers. Native server functions
use `HostRuntimeContext` and a typed event bus; GraphQL remains the parallel
selected path. The product packages contain no package-local framework or
framework-specific outbox adapter dependency.

`ProductCatalogReadPort` / `product.catalog_read.v1` is implemented by
`CatalogService`. Its in-process profile has live PostgreSQL execution evidence.
The Product-owned `ProductCatalogReadRuntime` gives host composition one typed
profile selector for `embedded_native` or `external` execution. The server
preserves a runtime already installed in `HostRuntimeContext` or
`ServerRuntimeContext`, otherwise composes the embedded provider once. AI and
Marketplace Listing consume the same selected port rather than constructing
parallel `CatalogService` instances. The Order storefront native checkout now
reads the selected runtime from `HostRuntimeContext` and enters Commerce through
`complete_storefront_checkout_with_product_port`; that composed function passes
the selected port directly to `CheckoutPlanBuilder`. Backward-compatible HTTP and
GraphQL wrappers still construct the embedded Product provider, so checkout
transport cutover remains open for those two surfaces. A concrete external
transport adapter has not yet been executed, so the provider remains
`boundary_ready` rather than `transport_verified`.

The composed `rustok-ai` consumer has live unavailable/deadline degraded-path
evidence. Commerce checkout treats Product as a hard dependency and must not
claim a cart-snapshot fallback that does not exist. The port resolves
variant-first consumer input to the owning product projection, so consumers do
not query product or variant entities. The compiled commerce checkout
channel-inventory regression executes the in-process product projection provider
before inventory preflight; it is bounded consumer evidence only and does not
close the external transport gate.

Product runtime contract, commerce transport, and module metadata remain synchronized.
The category-bound admin transport keeps native server functions as the
internal path and parallel GraphQL operations for the public/headless path.
The DB-level tenant consistency audit, `VARCHAR(32)` locale storage, catalog
search-option discovery, detached-value marker contract, and no-compile schema
guardrail are source-locked. The complete storefront/admin catalog-controls
contract carries snake_case `search`, `category_id`, `sort_by`,
`sort_direction`, and `attribute_filters` through typed UI state, native and
GraphQL adapters, Product-owned request models, and shared server-side
execution. Storefront and admin accept at most eight semicolon-separated
`code=value` attribute predicates. Product resolves each code against an active,
product-scoped, filterable definition and executes exact typed EAV equality for
localized/plain text, integer, decimal, boolean, date, datetime, select, and
multiselect storage while excluding detached values. JSON attributes are
explicitly rejected because this contract does not claim unindexed JSON
comparison semantics. Recheck on 2026-07-29.

Product write GraphQL derives tenant and actor exclusively from authenticated
contexts. Product-owned `map_product_public_error` is shared by GraphQL and
native admin/storefront transports; it keeps internal errors in structured logs
and exposes only a safe message, stable code, retryability, and correlation id.
Entity writes that publish product domain events use
`ProductWriteTransaction` to keep the outbox write and database commit in one transaction.
Admin and storefront product roots reject an explicit tenant that differs from the
host-provided `TenantContext` before accessing storage.

Product migrations enforce PostgreSQL-only execution, tenant-scoped
translation/SKU/tag identity, canonical primary categories, typed EAV option
relations, bounded JSON inputs, normalized/indexed channel visibility, and a
target schema without unused compatibility columns. The owner-local migration
fixture also verifies non-null Media-owned image identifiers and exact decimal
variant weights through `up/down/up`.
The isolated `product_postgres_migrations_support_up_down_up` fixture verifies
the complete Product migration lifecycle against PostgreSQL with owner
prerequisites and schema/constraint/index assertions.
The isolated `product_postgres_constraints_reject_invalid_and_racing_writes`
fixture proves concurrent tenant-scoped handle and SKU uniqueness as well as
database rejection of cross-tenant tags, corrupt typed EAV rows, legacy
primary-category assignments, duplicate root slugs, parent cycles, and closure
drift. Deferred database triggers validate the exact tree/closure projection at
transaction commit.
The pre-integrity `product_tenant_integrity_migration_rejects_dirty_data_and_maps_inventory`
fixture proves dirty handle, SKU, and root-slug data blocks migration and verifies
the legacy inventory-field backfill and physical column removal after cleanup.
The owner-local tenant-storage fixture rejects mixed-tenant product
translations, category parents, schema/category/attribute relations, EAV
values, and product-category joins, and verifies owner-derived translation
isolation for category, attribute, and schema copy.

`product_catalog_read_port_executes_against_postgres` exercises product,
variant-first, and published-list operations with live price/inventory
enrichment, tenant isolation, locale fallback, channel filtering, count, and
pagination. A concrete external adapter execution remains required before
promoting the transport status.
`storefront_queries_use_indexes_at_representative_scales` seeds ten tenants at
10k, 100k, and 1M total products and captures live
`EXPLAIN (ANALYZE, BUFFERS)` plans for storefront page and count SQL. The
specialized published/global-visibility index is used at all three page scales;
the count path uses it at 100k and 1M.

`CatalogService` is separated by responsibility across
`services/catalog/commands.rs`, `admin_queries.rs`, `attribute_filters.rs`,
`queries.rs`, `projection.rs`, and `tags.rs` while the public service contract
remains unchanged. Inventory state uses the owner-owned native
`rustok_inventory::BootstrapService` inside product's transaction for variant
initialization, cleanup, and available-quantity reads; this is a
documented bootstrap exception because no GraphQL/REST bootstrap contract exists
yet. Public inventory availability/reservation contracts remain inventory-owned;
the exception must be replaced if a public bootstrap transport is introduced.
Initial price creation, projection reads, and cleanup use the transaction-aware
`rustok_pricing_persistence::BootstrapService`, keeping pricing ORM ownership
outside Product without creating a `rustok-pricing -> rustok-product ->
rustok-pricing` dependency cycle.
`ProductCatalogSchemaService` is separated across `attributes.rs`,
`schemas.rs`, `categories.rs`, `values.rs`, `effective_forms.rs`, and
`virtual_categories.rs`; the parent retains shared records and validation.

## FFA/FBA status

- FFA status: `in_progress` — both owner UI surfaces exist and must preserve
  the core/transport/UI split and native/GraphQL parity.
- FBA status: `boundary_ready` — the read port, in-process persistence profile,
  Product-owned runtime selector, AI/Marketplace host composition, and native
  checkout source cutover are complete. External transport execution plus HTTP
  and GraphQL checkout cutover remain open.
- Structural shape: `core_transport_ui`
- Evidence: `crates/rustok-product/contracts/product-fba-registry.json`,
  `crates/rustok-product/contracts/evidence/product-runtime-contract-smoke.json`,
  `crates/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json`,
  `scripts/verify/verify-product-runtime-fallback-smoke.mjs`,
  `scripts/verify/verify-product-catalog-read-runtime-composition.mjs`,
  `scripts/verify/verify-product-native-checkout-catalog-runtime.mjs`,
  `scripts/verify/verify-product-admin-boundary.mjs`,
  `scripts/verify/verify-product-admin-category-sort.mjs`,
  `scripts/verify/verify-product-storefront-boundary.mjs`,
  `scripts/verify/verify-product-storefront-category-sort.mjs`,
  `scripts/verify/verify-product-catalog-attribute-filters.mjs`,
  `scripts/verify/verify-product-catalog-controls-plan-sync.mjs`, and
  `scripts/verify/verify-ai-product-fba.mjs` for the AI consumer contract.

## Open results

1. Implement a concrete external `ProductCatalogReadPort` transport adapter in a
   separate transport crate and execute all three operations through it. Preserve
   serialized `PortContext`, deadlines, typed `PortError`, tenant/locale/channel
   semantics, variant-to-product resolution, count, and pagination. Do not promote
   above `boundary_ready` from source markers alone.
2. Complete Commerce HTTP and GraphQL checkout cutover. Both compositions must
   receive `ProductCatalogReadRuntime::read_port()` and call the composed staged
   entrypoint. Execute unavailable/deadline behavior as an explicit
   hard-dependency failure; do not invent a degraded cart-snapshot fallback.
3. Keep Product richtext adoption explicitly deferred until the owner approves
   a typed storage/API/index migration. `product_translations.description` and
   catalog attributes currently named `richtext` are scalar text, so replacing
   their textarea alone would create a false contract. When approved, use the
   shared [Richtext plan](../../../docs/modules/rich-text-implementation-plan.md),
   assign an owner profile, migrate both transports, and keep short/meta
   descriptions plain text.

## Verification

- [x] Compose one host-selected `ProductCatalogReadRuntime` and reuse it for AI and Marketplace Listing.
- [x] Cut Order storefront native checkout over to the composed Product runtime.
- [ ] Cut Commerce HTTP and GraphQL checkout over to the composed Product runtime.
- [ ] Execute a concrete external Product catalog read adapter.
- [x] Connect storefront/admin UI controls to optional catalog filters/sorts.
- [x] Connect storefront title search through typed UI state, native/GraphQL transports, and Product-owned server-side filtering.
- [x] Connect storefront category and deterministic date sorting through typed UI state, native/GraphQL transports, and Product-owned server-side execution.
- [x] Connect admin search/status/category and deterministic date sorting through typed UI state, native/GraphQL transports, and Product-owned server-side execution.
- [x] Connect typed attribute_filters through storefront/admin UI state, native/GraphQL transports, filterable-definition validation, and Product-owned typed EAV execution.
- `node scripts/verify/verify-product-catalog-read-runtime-composition.mjs`
- `node scripts/verify/verify-product-catalog-read-runtime-composition.test.mjs`
- `node scripts/verify/verify-product-native-checkout-catalog-runtime.mjs`
- `node scripts/verify/verify-product-native-checkout-catalog-runtime.test.mjs`
- `node scripts/verify/verify-product-catalog-attribute-filters.mjs`
- `node scripts/verify/verify-product-catalog-attribute-filters.test.mjs`
- `node scripts/verify/verify-product-admin-category-sort.mjs`
- `node scripts/verify/verify-product-admin-category-sort.test.mjs`
- `node scripts/verify/verify-product-storefront-category-sort.mjs`
- `node scripts/verify/verify-product-storefront-category-sort.test.mjs`
- `node scripts/verify/verify-product-catalog-controls-plan-sync.mjs`
- `node scripts/verify/verify-product-catalog-controls-plan-sync.test.mjs`
- `npm run verify:product:runtime-fallback-smoke`
- `npm run verify:product:admin-boundary`
- `npm run verify:product:storefront-boundary`
- `npm run verify:ecommerce:fba`
- `cargo test -p rustok-ai --features server --lib direct_product_attributes_`

## Boundaries

- Product owns catalog data, `ProductCatalogReadPort`, and
  `ProductCatalogReadRuntime` profile selection.
- The host selects and shares one Product read runtime; consumers receive the
  public port and must not construct parallel owner services.
- Order native checkout, Marketplace Listing, and AI consume Product's public
  read contract through host composition. Commerce HTTP/GraphQL checkout remain
  explicit embedded compatibility paths until their cutover PR. Pricing uses
  Product's public embedded service contract and does not claim a read-port
  fallback profile. None regain Product DTO, entity, or storage ownership.
- Hosts compose product UI packages and pass the effective locale and runtime
  context without adding a package-local locale or transport fallback.
