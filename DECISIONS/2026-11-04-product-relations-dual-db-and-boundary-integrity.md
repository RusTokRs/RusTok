# Product Relations Dual-Database Support and Boundary Integrity

- Date: 2026-11-04
- Decision status: Accepted
- Implementation status: Implemented
- Owners: rustok-product-relations, rustok-product
- Extends: [2026-03-25-commerce-module-split-product-pricing-inventory.md](./2026-03-25-commerce-module-split-product-pricing-inventory.md)
- Supersedes: None
- Superseded by: None

## Context

The `rustok-product-relations` module manages product associations (cross-sells, up-sells, related products, accessories, alternatives) and their display ordering.
During architectural code review against platform standards (`AGENTS.md`), several critical defects and platform rule violations were identified:

1. **Direct Mutation of Foreign Module Schema (Table Sovereignty Violation):**
   In migration `m20260921_000002_enforce_product_foreign_keys`, the relations module executed:
   `ALTER TABLE products ADD CONSTRAINT uq_products_tenant_id_for_product_relations UNIQUE (tenant_id, id);`
   Modifying the schema of `products` directly inside a satellite module's migration violates module autonomy and schema isolation. Furthermore, `products` already possessed its canonical tenant consistency constraint `uq_products_tenant_id`, rendering the added key duplicate and redundant (eventually necessitating a later cleanup migration `m20261017_000005`).

2. **Hard-coded PostgreSQL Backend Checks (Dual-DB Support Broken):**
   All migrations in `rustok-product-relations` contained hard-coded checks:
   `if manager.get_database_backend() != DatabaseBackend::Postgres { return Err(DbErr::Custom("rustok-product-relations migrations require PostgreSQL")); }`
   This made it impossible to run the module's migrations on SQLite, violating the platform requirement that core and domain modules support both PostgreSQL and SQLite. Tests in `relation_service_tests.rs` were consequently forced to bypass `ProductRelationsModule::migrations()` and manually define schema tables with raw SQL strings.

3. **Frontend Packaging and UI Parity Gap:**
   While Leptos provided `rustok-product-relations-admin`, the Next.js admin UI embedded relations components (`ProductRelationsCard`, `ProductRelationAddDialog`) directly inside `@rustok/product-admin`. `rustok-module.toml` declared only `leptos-admin` and omitted `next_package = "@rustok/product-admin"`, creating a declared parity violation in module linting.

4. **Module Source Layout Conformance:**
   The source tree contained non-canonical root files (`ports.rs`, `dto.rs`, `contract_tests.rs`) and exposed internal entities publicly in `lib.rs` instead of keeping them private to the crate.

## Decision

1. **Remove Foreign Table Mutations:**
   `rustok-product-relations` migrations MUST NOT alter the `products` table. The composite foreign keys to `(tenant_id, product_id)` and `(tenant_id, related_product_id)` link cleanly to the existing canonical `uq_products_tenant_id` on PostgreSQL, and are handled via table definitions/indexes on SQLite.

2. **Full Dual-Database Support (PostgreSQL and SQLite):**
   All migrations in `rustok-product-relations` are updated to provide matching, first-class SQLite implementations:
   - `m20260914_000001_create_product_relations`: creates `product_relations` table with identical types, default values, `CHECK (product_id != related_product_id)` self-relation check, and unique pair constraints on both engines.
   - `m20260921_000002_enforce_product_foreign_keys`: adds foreign key constraints on PostgreSQL referencing `products(tenant_id, id)` with `ON DELETE CASCADE`. On SQLite, handles foreign keys without illegal table mutations.
   - `m20261017_000003_enforce_relation_positions`: compacts historical positions using window functions (`ROW_NUMBER() OVER (PARTITION BY ...)`), and enforces non-negative positions and unique ordering position (`uq_product_relations_tenant_product_type_position`) across both PostgreSQL and SQLite.
   - `m20261017_000005_remove_redundant_product_tenant_key`: cleans up redundant constraints on PostgreSQL if present, and acts as a safe no-op on SQLite.

3. **Update Test Harness to Execute Canonical Migrations:**
   Remove manual raw SQL table creations from `tests/relation_service_tests.rs`. Test suites MUST apply `ProductRelationsModule::migrations()` directly against SQLite to ensure migrations are continuously verified in CI.

4. **Synchronize UI Parity and Physical Layout:**
   - Update `rustok-module.toml` to register `next_package = "@rustok/product-admin"`.
   - Reorganize `src/` to follow canonical slots: `src/dto/mod.rs`, `src/ports/mod.rs`, `src/tests/mod.rs` & `src/tests/contract.rs`.
   - Keep entities private (`pub(crate) mod entities;`).

## Sources of truth and ownership

- `rustok-product-relations` is the sole canonical owner of the `product_relations` table, its positions, relation types, and migrations.
- `rustok-product` is the sole owner of the `products` table, product lifecycle, and product identity.
- Relations reference `products` via product IDs scoped to `tenant_id`; relations never own product metadata.

## Invariants

- Self-relations are strictly forbidden: `product_id != related_product_id`.
- Tenant consistency: both `product_id` and `related_product_id` must belong to the same `tenant_id`.
- Unique relation pairs: `(tenant_id, product_id, related_product_id, relation_type)` must be unique.
- Non-negative, unique sequential ordering positions: `(tenant_id, product_id, relation_type, position)` must be unique and non-negative.
- Migrations must succeed cleanly on both PostgreSQL and SQLite without engine-exclusive failures.

## Non-goals

- `rustok-product-relations` does not manage product categories, collections, or bundles (owned by Product and Bundles modules).
- This decision does not introduce asynchronous graph databases or external recommendation engines.

## Data, transaction, and concurrency boundary

All relation position re-ordering and pair insertions execute in atomic database transactions (`DatabaseTransaction`). Unique constraint `uq_product_relations_tenant_product_type_position` prevents position collision races. Foreign key cascades cleanly delete relation edges upon product deletion.

## Context dimensions

- Tenant: Mandatory tenant isolation across all queries, indexes, and unique constraints.
- Locale: Relation types and names participate in localization, while relation edges and IDs are locale-invariant.
- Channel / Currency: Non-applicable to raw product relation edges.

## Events and projections

Mutations emit `ProductRelationCreated`, `ProductRelationUpdated`, `ProductRelationDeleted` domain events. Search indexers and storefront catalog projections consume these events to invalidate cached recommendation and related product carousels.

## Failure semantics

Violation of self-relation check, foreign key constraints, or position unique keys return typed domain errors (`InvalidRelationTarget`, `DuplicateRelationError`). Migrations fail immediately if dialect-specific statements fail.

## Migration and cutover

Historical migration files are updated to support SQLite dialect without altering existing PostgreSQL schema contracts. Redundant constraint creation on foreign `products` table is removed. Test suites cut over from manual raw SQL setup to executing canonical migrations.

## Alternatives considered

- Retaining raw SQL mocking in tests: rejected, creates divergence between test schema and production schema.
- Leaving PostgreSQL-only requirement: rejected, breaks local developer experience and platform SQLite compatibility contract.

## Verification

Run `cargo test -p rustok-product-relations` against SQLite. Verify migration execution, position compaction, self-relation rejection, and foreign key cascades. Run `npm run verify:module-source-layout` to verify canonical module physical structure.

## Consequences

- **Positive:**
  - `rustok-product-relations` is 100% compliant with PostgreSQL and SQLite.
  - Zero cross-module DDL mutations; table sovereignty of `products` is respected.
  - Test suites run real migrations instead of mocking schema.
  - Full Leptos and Next.js parity documented and verified.
- **Negative:**
  - Requires maintaining dialect compatibility in migration definitions across both PostgreSQL and SQLite.
