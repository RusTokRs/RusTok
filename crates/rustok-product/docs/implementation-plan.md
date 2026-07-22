# Implementation Plan for `rustok-product`

## Current state

`rustok-product` owns the catalog, variants, category-bound attribute schemas,
typed attribute values, and product admin/storefront packages. Product UI uses
owner-owned core, transport, and Leptos adapter layers. Native server functions
use `HostRuntimeContext` and a typed event bus; GraphQL remains the parallel
selected path. The product packages contain no package-local framework or
framework-specific outbox adapter dependency.

`ProductCatalogReadPort` / `product.catalog_read.v1` is implemented by
`CatalogService`. `boundary_ready` on no-compile runtime fallback evidence is
supported by the provider registry, static contract matrix, and fallback smoke.
`transport_verified` still requires live provider execution evidence.
The port also resolves variant-first consumer input to the owning product
projection, so checkout consumers do not query product or variant entities.
The compiled commerce checkout channel-inventory regression executes the
in-process product projection provider before inventory preflight; it is a
bounded consumer proof only and does not close the module transport gate.
Product runtime contract, commerce transport, and module metadata remain synchronized.
The category-bound admin transport keeps native server functions as the
internal path and parallel GraphQL operations for the public/headless path.
The DB-level tenant consistency audit, `VARCHAR(32)` locale storage, optional catalog filters/sorts, detached-value marker contract, and no-compile schema guardrail are source-locked.
Product write GraphQL derives tenant and actor exclusively from authenticated
contexts, and product-service GraphQL reads/writes map internal errors to safe
public messages and stable codes. Entity writes that publish product domain events use
`ProductWriteTransaction` to keep the outbox write and database commit in one transaction.
Admin and storefront product roots reject an explicit tenant that differs from the
host-provided `TenantContext` before accessing storage.
Product migrations enforce PostgreSQL-only execution, tenant-scoped
translation/SKU/tag identity, canonical primary categories, typed EAV option
relations, bounded JSON inputs, and normalized/indexed channel visibility.
Storefront product lists filter, count, and paginate in SQL; live PostgreSQL
execution evidence remains required before promoting the transport status.
`CatalogService` is being separated by responsibility; product-tag reads and
writes now live in `services/catalog/tags.rs` while the public service contract
remains unchanged. Inventory state uses the owner-owned native
`rustok_inventory::BootstrapService` inside product's transaction for variant
initialization, cleanup, and available-quantity reads; this is a
documented bootstrap exception because no GraphQL/REST bootstrap contract exists
yet. Public inventory availability/reservation contracts remain inventory-owned;
the exception must be replaced if a public bootstrap transport is introduced.
`ProductCatalogSchemaService` is also being split by responsibility: category
creation, category groups, category bindings, category schema modes, and category listing now live in
`services/catalog_schema_service/categories.rs` without changing closure-table
validation or category outbox semantics.
Schema creation and schema listing now live in
`services/catalog_schema_service/schemas.rs` with the existing schema outbox
event, translation writes, schema groups, and schema-attribute bindings preserved.
Attribute and attribute-option reads and writes now live in
`services/catalog_schema_service/attributes.rs`, including option-type
validation and attribute outbox events.

## FFA/FBA status

- FFA status: `in_progress` — both owner UI surfaces exist and must preserve
  the core/transport/UI split and native/GraphQL parity.
- FBA status: `boundary_ready` — read-port policy, metadata, and fallback
  profiles are source-locked; no persistence-backed execution has been shown.
- Structural shape: `core_transport_ui`
- Evidence: `crates/rustok-product/contracts/product-fba-registry.json`,
  `crates/rustok-product/contracts/evidence/product-runtime-contract-smoke.json`,
  `crates/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json`,
  `scripts/verify/verify-product-runtime-fallback-smoke.mjs`,
  `scripts/verify/verify-product-admin-boundary.mjs`,
  `scripts/verify/verify-product-storefront-boundary.mjs`, and
  `scripts/verify/verify-ai-product-fba.mjs` for the AI consumer contract.

## Open results

1. Execute `ProductCatalogReadPort` against persistence for
   `read_product_projection`, `read_variant_product_projection`, and
   `list_published_products`. Done when real
   calls prove read-policy ordering, tenant and locale handling, bounded
   pagination, and typed `PortError` mapping rather than source markers.
   Dependency: a runnable product persistence environment. Verification:
   `npm run verify:product:runtime-fallback-smoke` plus targeted port tests.
2. Prove the consumer profiles with observed fallback behaviour before changing
   FBA status to `transport_verified`. Done when commerce checkout/storefront,
   pricing enrichment, and `rustok-ai` product context each exercise their
   declared fallback or degraded mode against the live provider.
   Dependency: priority 1 and the respective consumer composition. Verification:
   `npm run verify:ecommerce:fba` and `npm run verify:ai-product:fba`.
3. Keep Product richtext adoption explicitly deferred until the owner approves
   a typed storage/API/index migration. `product_translations.description` and
   catalog attributes currently named `richtext` are scalar text, so replacing
   their textarea alone would create a false contract. When approved, use the
   shared [Richtext plan](../../../docs/modules/rich-text-implementation-plan.md),
   assign an owner profile, migrate both transports, and keep short/meta
   descriptions plain text.

## Verification

- [x] Connect storefront/admin UI controls to optional catalog filters/sorts.
- `npm run verify:product:runtime-fallback-smoke`
- `npm run verify:product:admin-boundary`
- `npm run verify:product:storefront-boundary`
- `npm run verify:ecommerce:fba`

## Boundaries

- Product owns catalog data and the `ProductCatalogReadPort` implementation.
- `rustok-commerce`, pricing, and AI consume public product contracts; they do
  not regain catalog service, DTO, or entity ownership.
- Hosts compose product UI packages and pass the effective locale and runtime
  context without adding a package-local locale or transport fallback.
