# `rustok-product` Remediation Register

**Reviewed:** 2026-07-26

**Scope:** `crates/rustok-product` and its product GraphQL and migration boundaries.
**Status terms:** `resolved` is implemented and source-verified; `open` remains a valid
engineering task; `partial` mitigates the risk but does not yet meet the target contract;
`blocked` needs an external dependency or production data audit.

This register replaces the stale, incorrectly encoded draft. Every original item is retained
below with its current disposition. Items are deliberately not marked resolved solely from
source markers or no-compile evidence.

## Architecture

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Move product DTOs, entities, and errors out of `rustok-commerce-foundation` | resolved | `rustok-product` owns its DTOs, product ORM entities, and Product-specific error enum. Inventory bootstrap returns neutral database errors, while pricing ORM state and transaction-aware initial-price lifecycle operations have one owner source in `rustok-pricing-persistence`. Product has no `rustok-commerce-foundation` dependency; Pricing and Commerce convert Product errors explicitly at their consumer boundaries. |
| Split `CatalogService` into commands, queries, inventory, tags, and projection components | resolved | `services/catalog/commands.rs`, `queries.rs`, `projection.rs`, and `tags.rs` now hold the corresponding `CatalogService` implementations. Inventory persistence and public-channel availability are called through `rustok-inventory`; initial price create/read/delete operations use the transaction-aware pricing persistence owner contract. `catalog.rs` retains only service construction and shared wiring. |
| Split `ProductCatalogSchemaService` into attributes, schemas, categories, values, and virtual categories | resolved | Attribute, schema, category, typed-value, effective-form, and virtual-category responsibilities now live in `attributes.rs`, `schemas.rs`, `categories.rs`, `values.rs`, `effective_forms.rs`, and `virtual_categories.rs`. The parent file retains shared records, validation, and service construction. |
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
| Storefront ordering index | resolved | The migration adds `(tenant_id, status, published_at DESC, created_at DESC)` as a partial index for published products with an empty channel allowlist. Live plans use it for the globally visible storefront page path at 10k, 100k, and 1M rows. |
| Remove transitional columns from products, options, images, translations, and variants | resolved | The audited target-schema migration removes unused product dimensions/legacy flags, translation subtitle/material, option name/values, image URL/metadata/timestamp, and obsolete variant shipping/metadata columns. It also makes the Media-owned image UUID non-null and aligns variant weight with the owner ORM's `NUMERIC(20,6)` contract. `inventory_management` and `inventory_quantity` remain because current Pricing/Inventory/Commerce consumers still use that owner bridge; they are not classified as unused transitional storage. The owner-local PostgreSQL lifecycle fixture verifies column absence, type/nullability, exact decimal persistence, null-media rejection, and `up/down/up`. |
| Migrate and remove `manage_inventory`, `allow_backorder`, and `variant_rank` | resolved | The migration maps them to `inventory_management`, `inventory_policy`, and `position`, then drops the old columns. |
| Product-tag tenant integrity | resolved | Product tags are backfilled from their product; composite product/tag-term FKs and `(tenant_id, product_id)` index are added. The migration depends on taxonomy storage. |
| Automated schema check for every tenant-bearing table | resolved | `verify-product-catalog-schema` verifies the registered Product migrations, catalog tenant constraints, translation/product-tag composite tenant keys, target column cleanup, and EAV/primary-category/channel-visibility invariants. Its fixture suite proves that removal of representative constraints, indexes, or cleanup markers fails the guardrail. Owner-local and central Product PostgreSQL fixtures provide the execution evidence. |

## Code and ORM

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Add taxonomy `Term` relation to `product_tag` | resolved | `product_tag::Relation::Term` and its `Related` implementation now target `rustok_taxonomy::taxonomy_term`. |
| Replace `SELECT → INSERT` uniqueness checks with constraint-conflict handling | resolved | Product handle and SKU inserts rely on the new unique indexes; in-process duplicate input detection remains only to report duplicate values in one request. |
| Bulk-insert translations, options, option values, variants, and prices where safe | resolved | Product-option rows, option translations, option values, option-value translations, and variant translations use batched inserts after dependent ids are allocated; pricing-owned bootstrap batches initial prices in the same transaction. Product translations remain per-row to preserve tenant/locale handle conflict attribution, and variants remain per-row because each write performs owner inventory bootstrap and conflict mapping. |
| Extract a common entity-and-outbox transaction helper | resolved | Product entity write paths with domain events now use `services/write_transaction.rs::ProductWriteTransaction`. It owns the SeaORM transaction, exposes only transactional event publication, and commits only after the entity and outbox writes succeed. The source guardrail rejects direct `self.db.begin()` in the catalog and schema write services. |
| Replace SEO provider-registration `expect` with a controlled module-init error | resolved | `RusToKModule::register_runtime_extensions` and `ModuleRegistry::build_runtime_extensions` are fallible. Product, Pages, Blog, Forum, AI, notification factory materialization, and server bootstrap now propagate contextual initialization errors; Product maps SEO provider conflicts without `expect` or `panic`. |

## API and access control

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Derive tenant and actor exclusively from trusted GraphQL contexts | resolved | Product write mutations no longer accept tenant/user GraphQL arguments; they derive both from `TenantContext` and `AuthContext`. The owner admin GraphQL operations were updated accordingly. |
| Preserve RBAC and bind it to the tenant | resolved | Each product mutation now performs the existing permission check plus authenticated tenant/actor scope validation. |
| Prevent DB error strings from reaching GraphQL clients | resolved | Product owns one exhaustive `CommerceError -> ProductPublicError` mapper used by GraphQL plus native admin/storefront transports. It logs the internal error with boundary/operation and returns only a safe message, stable code, retryability, and generated correlation id. Host-local handle lookup was removed in favor of the Product owner service; remaining direct Product list/count/translation/inventory queries map `DbErr` through the same policy. Unit coverage proves database password/host details are absent. |
| Map every Product error to a stable API code | resolved | Product owns an exhaustive `CommerceError -> ProductPublicError` mapping with stable codes, retryability, redacted messages, and correlation identifiers. GraphQL and native consumers call that owner mapper rather than converting through the foundation error family. |
| Apply one pagination validation rule and remove service-level clamping | resolved | Product service and commerce GraphQL storefront paths reject page `0` and per-page values outside `1..=48`; neither silently clamps client input. |

## Performance

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Push channel visibility, count, and pagination into SQL | resolved | Product and commerce storefront list paths filter, count, order, and page in SQL; neither materializes a tenant catalog before pagination. |
| Normalize channel visibility or add an indexed JSONB predicate | resolved | Product metadata canonicalizes allowlist slugs with a PostgreSQL trigger; storefront uses JSONB containment backed by a GIN `jsonb_path_ops` index. |
| Reduce sequential queries in `get_product_with_locale_fallback` | resolved | Independent base projections, tag/metadata resolution, option projections, and variant price/translation/inventory reads execute in bounded parallel groups; dependent option/image lookups remain batched by ids. |
| Run `EXPLAIN (ANALYZE, BUFFERS)` at 10k/100k/1M products | resolved | `storefront_queries_use_indexes_at_representative_scales` incrementally seeds ten tenants to 10k, 100k, and 1M Product rows, analyzes the table, and captures JSON plans for storefront page and count SQL. The 2026-07-26 local run proved the page path uses `idx_products_storefront_published` at every scale and the count path uses it at 100k/1M. Recorded page/count execution times were 1.067/0.378 ms, 9.867/3.841 ms, and 0.135/66.726 ms respectively. |

## Testing

| Item | Status | Review result and evidence |
| --- | --- | --- |
| PostgreSQL `up/down/up` migration integration tests | resolved | `product_postgres_migrations_support_up_down_up` creates an isolated PostgreSQL database, installs the minimal tenant/taxonomy/Flex owner prerequisites, applies every Product migration, tears the Product schema down, and reapplies it. The 2026-07-26 local run passed. The full platform clean-install smoke separately stops later in an unrelated `ledger_reversals` foreign-key migration. |
| Persistence-backed tests for read projection and published listing | resolved | `product_catalog_read_port_executes_against_postgres` executes all three `ProductCatalogReadPort` operations against isolated PostgreSQL. It proves product and variant-first owner projections, price/inventory enrichment, tenant isolation with typed `NotFound`, locale fallback, published/channel filtering, count, and two-page pagination. |
| Concurrent duplicate-handle and duplicate-SKU tests | resolved | `product_postgres_constraints_reject_invalid_and_racing_writes` issues each duplicate pair concurrently through separate pooled PostgreSQL operations and proves that exactly one write succeeds while the tenant-scoped unique index rejects the other. |
| Tenant-isolation tests for product/catalog storage | resolved | The GraphQL runtime suite rejects a substituted tenant on every current Product read root. `tenant_storage_constraints_reject_cross_tenant_catalog_writes` additionally proves PostgreSQL rejection of mixed-tenant product translations, category parents, schema attributes, category-schema assignments, EAV values, and product-category joins. Category/attribute/schema translations intentionally derive tenancy from their owner row; the fixture inserts identical locale/copy under two tenants and proves each owner-scoped join returns only its own row. The separate tag fixture covers taxonomy relations. |
| Cross-tenant `product_tags` rejection test | resolved | The isolated PostgreSQL constraint fixture creates a product and taxonomy term under different tenants and proves that `fk_product_tags_term_tenant` rejects the mixed-tenant relation. |
| EAV corruption, category cycle, closure drift, multiple-primary, and root-slug tests | resolved | The PostgreSQL constraint fixture proves rejection of a corrupt multi-scalar integer EAV value, a new `primary` join-table assignment, a duplicate tenant root slug, a parent cycle, and a missing canonical closure edge. Deferred constraint triggers validate the complete tenant tree/closure projection at commit. |
| Migration test for pre-existing duplicates | resolved | `product_tenant_integrity_migration_rejects_dirty_data_and_maps_inventory` pauses immediately before the tenant-integrity migration and proves that legacy duplicate handles, SKUs, and root slugs each block migration through the expected unique index before a cleaned fixture can proceed. |
| Migration test for legacy inventory mapping and column removal | resolved | The same PostgreSQL fixture proves `manage_inventory=true`, `allow_backorder=true`, and `variant_rank=7` map to `manual`, `continue`, and `position=7`, then verifies that all three legacy columns are absent. |
| Native/GraphQL parity tests for admin and storefront | resolved | `admin_native_and_graphql_product_reads_are_equivalent` compares the Product owner service with the admin GraphQL projection on one database. `storefront_graphql_filters_channel_hidden_products` compares native and GraphQL list/detail visibility, locale fallback, and hidden-product behaviour on the same seeded catalog. |

## Security

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Prevent tenant/user substitution through GraphQL variables | resolved | Product write mutations no longer expose tenant/user GraphQL variables. |
| Prevent internal DB message leakage through the API | resolved | GraphQL and native Product transports use the Product-owned exhaustive public-error descriptor. Database/internal detail stays in structured logs; client output contains a safe message, stable code and correlation reference. A unit test injects secret-bearing `DbErr` text and proves it is redacted. |
| Bound `metadata`, validation, rule, snapshot, and other JSONB inputs | resolved | Product schema inputs now require bounded JSON (64 KiB, depth 32); metadata/override/rule payloads must be objects, and JSON attribute values use the same bound. Clone snapshots are server-generated. |
| Negative tenant-substitution tests for all read/write flows | resolved | `graphql_runtime_parity_test` rejects substituted tenant input on every Product read root. `admin_graphql_rejects_foreign_actor_for_every_product_mutation` executes all 15 Product mutations with an authenticated actor bound to another tenant and proves identical pre-storage denial; the schema guardrail separately verifies that no mutation exposes tenant/user arguments. |

## Documentation and FBA status

| Item | Status | Review result and evidence |
| --- | --- | --- |
| Product ER diagram with keys, constraints, and indexes | resolved | Product documentation now contains the storage ER summary and identifies the schema-level tenant/composite/partial constraints. |
| Table ownership and canonical-source documentation | resolved | Product documentation now names every storage owner class and `products.primary_category_id` as the canonical category source. |
| ADRs for PostgreSQL-only, tenant isolation, EAV, closure table, and product/commerce ownership | resolved | [ADR 2026-07-11](DECISIONS/2026-07-11-product-storage-integrity-and-request-trust.md) records these decisions and is indexed. |
| Promote FBA to `boundary_ready` / `transport_verified` after live tests | resolved | The audited outcome is `boundary_ready`, not a false promotion. PostgreSQL verifies the embedded provider; Commerce executes it as a hard dependency; composed `rustok-ai --features server` tests verify unavailable/deadline prompt-only degradation. The registry no longer invents Pricing, GraphQL compatibility, cart-snapshot, refresh-required, or pricing-enrichment fallback profiles. `transport_verified` is correctly reserved for a future concrete external adapter and any Commerce degraded policy introduced with live evidence. |

## Verification performed

- Source audit of product migrations, service writes, GraphQL mutations and storefront listing.
- `git diff --check` passed for the earlier remediation change set.
- `cargo check -p rustok-inventory -p rustok-product --offline` and `cargo check -p rustok-commerce --offline` passed for the earlier remediation change set.
- `cargo test -p rustok-product --lib --offline` passed (16 tests) for the current remediation change set.
- `npm run verify:product:catalog-schema` and `npm run test:verify:product:catalog-schema` passed for the earlier remediation change set.
- The 2026-07-21 owner-local DTO/entity source move was reviewed through GitHub diff and source markers only; tests and CI were not run in that slice.
- On 2026-07-25, the ignored PostgreSQL zero-migration smoke connected to the local RusToK development database and advanced past Product's former `media` and `product_tags` ordering failures. It currently stops in the unrelated `ledger_reversals` migration because the referenced target key has no unique constraint.
- `RUSTOK_MIGRATION_SMOKE_ADMIN_URL=<local-dev-admin-url> cargo test -p rustok-migrations --test postgres_zero_migration_smoke product_ --offline -- --ignored` passed 4/4 on 2026-07-26: migration lifecycle, concurrent/invariant constraints, dirty-data/inventory preflight, and live Product read-port persistence.
- `RUSTOK_MIGRATION_SMOKE_ADMIN_URL=<local-dev-admin-url> cargo test -p rustok-product --test postgres_migrations target_schema_supports_lifecycle_and_removes_transitional_columns --offline -- --ignored --exact` passed on 2026-07-26, proving the owner-local target-column and migration lifecycle contract.
- `RUSTOK_MIGRATION_SMOKE_ADMIN_URL=<local-dev-admin-url> cargo test -p rustok-product --test postgres_migrations storefront_queries_use_indexes_at_representative_scales --offline -- --ignored --exact --nocapture` passed on 2026-07-26 and recorded the 10k/100k/1M query-plan baseline.
- `RUSTOK_MIGRATION_SMOKE_ADMIN_URL=<local-dev-admin-url> cargo test -p rustok-product --test postgres_migrations tenant_storage_constraints_reject_cross_tenant_catalog_writes --offline -- --ignored --exact` passed on 2026-07-26, proving the remaining catalog/EAV/translation tenant-storage matrix.
- `cargo test -p rustok-product --lib --offline` passed 16 tests, including secret-bearing database-error redaction. The combined `rustok-pricing-persistence`, foundation, Product, Pricing, Inventory, and Commerce library compile gate passes without owner-code warnings.
- `cargo test -p rustok-product -p rustok-product-admin -p rustok-product-storefront --all-targets --all-features --offline` exposed and then closed a storefront feature-profile assertion drift; the corrected SSR/all-features storefront suite passes 15 tests and the preceding Product/admin suites pass 16/37 tests with three PostgreSQL tests intentionally ignored in the default matrix.
- Product GraphQL runtime evidence passes for native/admin read parity, native/storefront visibility parity, and all 15 mutation tenant-substitution checks.
- `cargo test -p rustok-commerce --test graphql_runtime_parity_test catalog:: --offline` passes all 8 Product catalog GraphQL tests. The unfiltered Commerce suite is 63/80; its 17 remaining failures are Cart/Checkout/Pricing/Shipping contract-fixture drift outside this Product remediation scope.
- `cargo test -p rustok-pricing --test pricing_read_port_runtime --offline` passes 3/3.
- `cargo test -p rustok-ai --features server --lib direct_product_attributes_ --offline` passes 3/3, including owner-port success, unavailable degradation, and deadline degradation.
- Product catalog-schema, runtime-fallback, admin/storefront boundary, and AI-product FBA verifiers pass with their fixture suites. The ecommerce-wide aggregates still stop outside Product on Payment contract-version drift and missing Order checkout-completion runtime evidence.
- The final all-features compile gate for Product, Product admin/storefront, Pricing persistence/Pricing, Inventory, Commerce foundation, and Commerce passes with no owner-code warnings. Rust still reports only the external `proc-macro-error2` future-incompatibility notice and localized MSVC linker informational messages.

## Ongoing release gates

1. Run a production-data preflight for duplicate handles/SKUs/root slugs, cross-tenant tags,
   conflicting primary categories, and legacy inventory values.
2. Keep the PostgreSQL lifecycle, target-schema, invariant, and 10k/100k/1M query-plan fixtures in the release gate.
3. Keep FBA at `boundary_ready` until a concrete external adapter is executed; add any future Commerce degraded policy only together with live unavailable/deadline evidence.
