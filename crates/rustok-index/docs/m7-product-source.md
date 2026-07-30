# M7 Product source adapter

Status: `source_complete_owner_execution_pending`

## Scope

This slice adds the first production owner adapter for the generic Index Engine.
`rustok-product` publishes one locale-required `rustok-product::product@1` schema and one
PostgreSQL-backed bounded source. The schema contains Product-owned scalar fields only:

- status;
- title, handle, and description;
- vendor and product type;
- primary category identity as a scalar UUID;
- published and updated timestamps.

ProductVariant, SalesChannel, pricing, inventory, taxonomy links, relevance, and external search
semantics are not part of this schema.

## Composition

`ProductModule` publishes the schema during ordinary module registration. A
`PostgresIndexSourceFactory` is published separately because the source adapter needs the selected
host database connection. The server invokes `materialize_postgres_index_sources` before
`materialize_index_source_registry`; all factories write to one staged clone and the source catalog
is committed only when every selected factory succeeds.

Factory materialization performs no SQL and starts no task. Product-specific types never appear in
server composition. The selected distribution enables the Product `index` feature when
`mod-product` is compiled.

## Source contract

`ProductPostgresIndexSource` supports:

- cursor scans ordered by `(index_revision, product_id, locale)`;
- one-row lookahead over the caller's bounded page limit;
- targeted loads over exact `(product_id, locale)` pairs;
- exact tenant and schema scope;
- canonical locale validation;
- stable replay event UUIDs derived from tenant, product, locale, and source revision;
- retryable storage failures and permanent contract/backend failures without exposing raw database
  errors.

The source reads only Product-owned `products` and `product_translations` tables. It emits generic
`IndexMutation::Upsert` values and never writes Index storage directly.

## Monotonic source version

`products.index_revision` is a positive `BIGINT` owned by Product storage. A Product row update
advances it through a database trigger. Insert, update, delete, or reassignment of a Product
translation advances the affected Product revision. The bounded replay path is supported by
`idx_products_index_replay (tenant_id, index_revision, id)`.

The revision is storage-internal and is not added to Product API DTOs or the SeaORM write model.
Normal Product inserts use the database default; Product and translation updates advance the
revision inside PostgreSQL.

## Explicitly open

- Product hard deletes do not yet emit durable Index tombstones. A stale indexed Product therefore
  requires the later incremental event path or reconciliation/repair slice.
- Per-tenant schema application into `index_schemas` remains an owner operation. Runtime capability
  presence does not establish persisted schema readiness.
- Product domain events do not yet carry the source revision and are not connected to incremental
  Index mutation acknowledgement.
- ProductVariant and SalesChannel schemas, links, and sources remain open.
- Storefront/admin/search authoritative consumer cutover remains forbidden.
- Retained PostgreSQL replay, restart, cancellation, drift, freshness, and equivalence evidence has
  not been executed.

## Owner verification

The implementation agent did not run commands. The repository owner should run:

```bash
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-product --all-targets --features index
cargo check -p rustok-server --all-targets --features mod-product
cargo test -p rustok-index source_factory --lib -- --nocapture
cargo test -p rustok-product --all-targets --features index -- --nocapture
```

Validation and live PostgreSQL execution are `maintainer-run`.
