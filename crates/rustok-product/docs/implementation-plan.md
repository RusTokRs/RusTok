# Implementation Plan for `rustok-product`

## Current state

`rustok-product` owns the catalog, variants, category-bound attribute schemas,
typed attribute values, and product admin/storefront packages. Product UI uses
owner-owned core, transport, and Leptos adapter layers. Native server functions
use `HostRuntimeContext` and a typed event bus; GraphQL remains the parallel
selected path. The product packages contain no package-local framework or
framework-specific outbox adapter dependency.

`ProductCatalogReadPort` / `product.catalog_read.v1` is implemented by
`CatalogService`. Its in-process profile has live PostgreSQL execution evidence,
while the provider registry, static contract matrix, and fallback smoke retain
the module at `boundary_ready` until consumer fallback profiles are observed.
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
`product_catalog_read_port_executes_against_postgres` exercises product,
variant-first, and published-list operations with live price/inventory
enrichment, tenant isolation, locale fallback, channel filtering, count, and
pagination. Consumer fallback execution remains required before promoting the
transport status.
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
Virtual-category rule reference validation now lives in
`services/catalog_schema_service/virtual_categories.rs`; category creation
delegates structural-subtree, attribute-scope, localization, and value-type
checks to that component.

## FFA/FBA status

- FFA status: `in_progress` — both owner UI surfaces exist and must preserve
  the core/transport/UI split and native/GraphQL parity.
- FBA status: `boundary_ready` — read-port policy, metadata, and fallback
  profiles are source-locked and the in-process profile is persistence-backed;
  declared consumer fallback profiles are not yet live-verified.
- Structural shape: `core_transport_ui`
- Evidence: `crates/rustok-product/contracts/product-fba-registry.json`,
  `crates/rustok-product/contracts/evidence/product-runtime-contract-smoke.json`,
  `crates/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json`,
  `scripts/verify/verify-product-runtime-fallback-smoke.mjs`,
  `scripts/verify/verify-product-admin-boundary.mjs`,
  `scripts/verify/verify-product-storefront-boundary.mjs`, and
  `scripts/verify/verify-ai-product-fba.mjs` for the AI consumer contract.

## Open results

1. Prove the consumer profiles with observed fallback behaviour before changing
   FBA status to `transport_verified`. Done when commerce checkout/storefront,
   pricing enrichment, and `rustok-ai` product context each exercise their
   declared fallback or degraded mode against the live provider.
   Dependency: the respective consumer composition. Verification:
   `npm run verify:ecommerce:fba` and `npm run verify:ai-product:fba`.
2. Keep Product richtext adoption explicitly deferred until the owner approves
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
