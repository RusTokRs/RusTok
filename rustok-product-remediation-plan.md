# `rustok-product` Remediation Register

**Reviewed:** 2026-07-21

**Scope:** `crates/rustok-product` and its product GraphQL and migration boundaries.
**Status terms:** `resolved` is implemented and source-verified; `open` remains a valid
engineering task; `partial` mitigates the risk but does not yet meet the target contract;
`blocked` needs a live PostgreSQL environment or production data audit.

This register replaces the stale, incorrectly encoded draft. Every original item is retained
below with its current disposition. Items are deliberately not marked resolved solely from
source markers or no-compile evidence.

## Architecture

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Move product DTOs, entities, and errors out of `rustok-commerce-foundation` | partial | `rustok-product` now owns concrete DTO and product ORM source files under `src/dto/` and `src/entities/`; its public module no longer re-exports foundation DTO/entity collections. A source guard fixes that owner-local boundary. The remaining dependency is intentionally narrow: pricing-owned `price` ORM access and the shared `CommerceError` identity still bridge through foundation until transaction-aware Pricing/Inventory owner ports replace those direct dependencies. Foundation copies cannot be deleted safely before that inversion, so this item is not marked resolved. |
| Split `CatalogService` into commands, queries, inventory, tags, and projection components | partial | Tag reads/writes are isolated in `src/services/catalog/tags.rs`. Product no longer owns inventory persistence helpers: it calls inventory-owned `BootstrapService` for initial records, cleanup, and available-quantity reads inside its transaction under a documented native-only bootstrap exception. Commands, queries, and product projection still share `src/services/catalog.rs`. |
| Split `ProductCatalogSchemaService` into attributes, schemas, categories, values, and virtual categories | partial | Category creation, groups, bindings, schema modes, and listing are isolated in `src/services/catalog_schema_service/categories.rs`; schema creation/listing/groups/bindings are isolated in `src/services/catalog_schema_service/schemas.rs`; attribute reads/writes are isolated in `src/services/catalog_schema_service/attributes.rs`; values, virtual-category validation, and effective-form projection remain in the main service file. |
| Keep a single owner of product migrations and remove commerce copies | resolved | `rustok-commerce/src/migrations/` no longer creates product tables; `ProductModule` exports the product migration set. |
| Enforce PostgreSQL-only product migrations | resolved | New product migrations return an explicit error for a non-PostgreSQL backend instead of silently succeeding. |
| Move the product GraphQL surface to `rustok-product` or use the `product` module slug | resolved | Catalog GraphQL roots remain schema-composed by commerce, but every product read/write root is gated by `PRODUCT_MODULE_SLUG` (`product`), not the commerce umbrella slug. |

## Database and schema

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Move `product_status_enum` creation and `products.status` conversion to the product owner | resolved | `m20260711_000001_product_status_enum` now owns it. The server migration retains only the content enum, preventing a clean install from altering `products` before the table exists. |
| Tenant-scope translation handles | resolved | `m20260711_000002_enforce_product_tenant_integrity` backfills `product_translations.tenant_id`, adds a composite FK and `UNIQUE (tenant_id, locale, handle)`. Writes now supply the tenant id. |
| Tenant-scoped unique SKU and `DuplicateSku` mapping | resolved | The migration adds partial index `uq_product_variants_tenant_sku`; catalog inserts map that constraint to `CommerceError::DuplicateSku`. |
| Unique root category slug | resolved | The migration adds partial unique index `(tenant_id, slug) WHERE parent_id IS NULL`. |
| EAV value, detached-value, and option-type constraints | resolved | `m20260711_000003_enforce_catalog_value_invariants` adds scalar-value checks, type/tenant triggers, option ownership validation, and serialized single-select enforcement. Detached state is now derived from the effective schema rather than persisted as an independently writable timestamp. |
| One canonical primary-category source | resolved | `products.primary_category_id` is canonical. The migration fails on multiple legacy primary assignments, backfills a missing canonical value, converts legacy assignment rows to navigation, and prohibits new `primary` assignments. |
| Storefront ordering index | resolved | The migration adds `(tenant_id, status, published_at DESC, created_at DESC)` for non-deleted products. |
| Remove transitional columns from products, options, images, translations, and variants | open | Only obsolete variant inventory fields are safely migrated here. Other legacy columns need an audited consumer inventory. |
| Migrate and remove `manage_inventory`, `allow_backorder`, and `variant_rank` | resolved | The migration maps them to `inventory_management`, `inventory_policy`, and `position`, then drops the old columns. |
| Product-tag tenant integrity | resolved | Product tags are backfilled from their product; composite product/tag-term FKs and `(tenant_id, product_id)` index are added. The migration depends on taxonomy storage. |
| Automated schema check for every tenant-bearing table | resolved | `verify-product-catalog-schema` now verifies the registered product migrations, the catalog tenant constraints, translation/product-tag composite tenant keys, and the EAV/primary-category/channel-visibility invariants. Its fixture suite proves that removal of representative constraints or indexes fails the guardrail. This is a source-level check; PostgreSQL execution remains separately open. |

## Code and ORM

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Add taxonomy `Term` relation to `product_tag` | resolved | `product_tag::Relation::Term` and its `Related` implementation now target `rustok_taxonomy::taxonomy_term`. |
| Replace `SELECT → INSERT` uniqueness checks with constraint-conflict handling | resolved | Product handle and SKU inserts rely on the new unique indexes; in-process duplicate input detection remains only to report duplicate values in one request. |
| Bulk-insert translations, options, option values, variants, and prices where safe | partial | Product-option rows, option translations, option values, option-value translations, variant translations, and prices now use batched inserts after their dependent ids are allocated. Product translations and variants remain per-row because their conflict mapping and inventory/outbox side effects require per-record handling. |
| Extract a common entity-and-outbox transaction helper | resolved | Product entity write paths with domain events now use `services/write_transaction.rs::ProductWriteTransaction`. It owns the SeaORM transaction, exposes only transactional event publication, and commits only after the entity and outbox writes succeed. The source guardrail rejects direct `self.db.begin()` in the catalog and schema write services. |
| Replace SEO provider-registration `expect` with a controlled module-init error | resolved | `RusToKModule::register_runtime_extensions` and `ModuleRegistry::build_runtime_extensions` are fallible. Product, Pages, Blog, Forum, AI, notification factory materialization, and server bootstrap now propagate contextual initialization errors; Product maps SEO provider conflicts without `expect` or `panic`. |

## API and access control

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Derive tenant and actor exclusively from trusted GraphQL contexts | resolved | Product write mutations no longer accept tenant/user GraphQL arguments; they derive both from `TenantContext` and `AuthContext`. The owner admin GraphQL operations were updated accordingly. |
| Preserve RBAC and bind it to the tenant | resolved | Each product mutation now performs the existing permission check plus authenticated tenant/actor scope validation. |
| Prevent DB error strings from reaching GraphQL clients | partial | Product catalog and schema-service errors on GraphQL reads and writes are mapped to a safe message and stable code; failures and invalid enum inputs are logged on the server without being reflected to the client. Direct SeaORM helper paths, correlation-id propagation, and complete read-transport coverage remain open. |
| Map every `CommerceError` to a stable API code | resolved | Product GraphQL maps the existing exhaustive `CommerceError → RichError` conversion; every `RichError::new` initializes a stable error-kind code and explicit product conflicts retain their named codes. |
| Apply one pagination validation rule and remove service-level clamping | resolved | Product service and commerce GraphQL storefront paths reject page `0` and per-page values outside `1..=48`; neither silently clamps client input. |

## Performance

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Push channel visibility, count, and pagination into SQL | resolved | Product and commerce storefront list paths filter, count, order, and page in SQL; neither materializes a tenant catalog before pagination. |
| Normalize channel visibility or add an indexed JSONB predicate | resolved | Product metadata canonicalizes allowlist slugs with a PostgreSQL trigger; storefront uses JSONB containment backed by a GIN `jsonb_path_ops` index. |
| Reduce sequential queries in `get_product_with_locale_fallback` | resolved | Independent base projections, tag/metadata resolution, option projections, and variant price/translation/inventory reads execute in bounded parallel groups; dependent option/image lookups remain batched by ids. |
| Run `EXPLAIN (ANALYZE, BUFFERS)` at 10k/100k/1M products | blocked | Requires representative live PostgreSQL datasets; no such environment is available in this workspace. |

## Testing

| Item | Status | Review result and evidence |
| --- | --- | --- |
| PostgreSQL `up/down/up` migration integration tests | open | Existing product tests are module/source tests, not a full PostgreSQL migration lifecycle. |
| Persistence-backed tests for read projection and published listing | open | Current FBA evidence is source/no-compile fallback evidence; local implementation plan explicitly says live persistence execution is absent. |
| Concurrent duplicate-handle and duplicate-SKU tests | open | Constraints exist, but race tests have not been added. |
| Tenant-isolation tests for product/catalog storage | partial | The GraphQL runtime suite now rejects a substituted tenant on every current product read root, including schema/EAV and storefront roots, before any storage access. A complete persistence suite for products, categories, schemas, attributes, values, translations, and tags is still required. |
| Cross-tenant `product_tags` rejection test | open | Constraints are now present; a PostgreSQL negative test is still required. |
| EAV corruption, category cycle, closure drift, multiple-primary, and root-slug tests | open | No complete invariant suite exists. |
| Migration test for pre-existing duplicates | open | Required before applying the new uniqueness constraints to production data. |
| Migration test for legacy inventory mapping and column removal | open | The data mapping is defined but untested against a real PostgreSQL fixture. |
| Native/GraphQL parity tests for admin and storefront | open | Boundary checks exist, but no live parity suite proves equal behaviour. |

## Security

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Prevent tenant/user substitution through GraphQL variables | resolved | Product write mutations no longer expose tenant/user GraphQL variables. |
| Prevent internal DB message leakage through the API | partial | Product service GraphQL read/write mapper is safe and logs internal failures; direct SeaORM helper paths, correlation-id propagation, and complete read-transport coverage remain open. |
| Bound `metadata`, validation, rule, snapshot, and other JSONB inputs | resolved | Product schema inputs now require bounded JSON (64 KiB, depth 32); metadata/override/rule payloads must be objects, and JSON attribute values use the same bound. Clone snapshots are server-generated. |
| Negative tenant-substitution tests for all read/write flows | partial | `graphql_runtime_parity_test` proves substituted `tenantId` is rejected for every current product read root (`product`, `products`, schema/EAV reads, `storefrontProduct`, and `storefrontProducts`) and rejects `createProduct` when `AuthContext.tenant_id` differs from `TenantContext`. The schema guardrail verifies that every one of the 15 product mutations binds the trusted actor and exposes neither tenant nor user arguments. Every remaining mutation still needs equivalent negative runtime coverage. |

## Documentation and FBA status

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Product ER diagram with keys, constraints, and indexes | resolved | Product documentation now contains the storage ER summary and identifies the schema-level tenant/composite/partial constraints. |
| Table ownership and canonical-source documentation | resolved | Product documentation now names every storage owner class and `products.primary_category_id` as the canonical category source. |
| ADRs for PostgreSQL-only, tenant isolation, EAV, closure table, and product/commerce ownership | resolved | [ADR 2026-07-11](DECISIONS/2026-07-11-product-storage-integrity-and-request-trust.md) records these decisions and is indexed. |
| Promote FBA to `boundary_ready` / `transport_verified` after live tests | blocked | `boundary_ready` already has source-locked evidence; `transport_verified` must wait for persistence-backed execution and consumer fallback evidence. |

## Verification performed

- Source audit of product migrations, service writes, GraphQL mutations and storefront listing.
- `git diff --check` passed for the earlier remediation change set.
- `cargo check -p rustok-inventory -p rustok-product --offline` and `cargo check -p rustok-commerce --offline` passed for the earlier remediation change set.
- `cargo test -p rustok-product --lib --offline` passed (13 tests) for the earlier remediation change set.
- `npm run verify:product:catalog-schema` and `npm run test:verify:product:catalog-schema` passed for the earlier remediation change set.
- The 2026-07-21 owner-local DTO/entity source move was reviewed through GitHub diff and source markers only; tests and CI were not run in that slice.

## Required next execution order

1. Run a production-data preflight for duplicate handles/SKUs/root slugs, cross-tenant tags,
   conflicting primary categories, and legacy inventory values.
2. Run the new migrations against an isolated PostgreSQL fixture and add the migration/invariant tests.
3. Complete the GraphQL owner-boundary move; the tenant/user variables have been removed from the public mutation contract.
4. Run indexed SQL plans on representative PostgreSQL data and capture `EXPLAIN (ANALYZE, BUFFERS)` evidence.
5. Replace the narrow pricing/error bridge with transaction-aware Pricing/Inventory owner ports before deleting the remaining foundation copies. Active implementation work now hands off to Forum.
