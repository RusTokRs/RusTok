# M7 Product source adapter

Status: `source_complete_owner_execution_pending`

## Scope

This slice adds the first selected production storage bridge for the generic Index Engine. When
`mod-product` is compiled and `ProductModule` participates in the runtime registry,
`rustok-distribution` publishes one locale-required `rustok-product::product@1` schema and one
PostgreSQL-backed bounded source. The schema contains Product-owned scalar state only:

- status;
- title, handle, and description;
- vendor and product type;
- primary category identity as a scalar UUID.

ProductVariant, SalesChannel, pricing, inventory, taxonomy links, timestamps, relevance, and
external search semantics are not part of this first schema.

## Ownership and composition

`rustok-product` owns the `products` and `product_translations` tables, the monotonic
`index_revision` migration, and a neutral `ProductRuntimeSelected` marker. The Product crate does
not depend on `rustok-index` and does not construct generic Index mutations.

`rustok-distribution` is the selected cross-module bridge. It detects the Product marker, registers
the generic schema, and publishes a `PostgresIndexSourceFactory`. The executable server invokes
`materialize_postgres_index_sources` before `materialize_index_source_registry`; all factories
write to one staged extension clone and the source catalog is committed only when every selected
factory succeeds.

Factory materialization performs no SQL and starts no task. Product-specific types never appear in
server replay composition, and Index core never imports Product or reads Product tables.

## Source contract

The selected Product PostgreSQL source supports:

- cursor scans ordered by the stable `(product_id, locale)` identity;
- one-row lookahead over the caller's bounded page limit;
- targeted loads over exact `(product_id, locale)` pairs;
- exact tenant and schema scope;
- canonical stored-locale and cursor validation;
- stable replay event UUIDs derived through the Index-owned event-identity helper from tenant,
  product, locale, and source revision;
- retryable storage failures and permanent contract/backend failures without exposing raw database
  errors.

The source reads only Product-owned `products` and `product_translations` tables. It emits generic
`IndexMutation::Upsert` values and never writes Index storage directly. Stable enumeration uses the
existing tenant/product identity constraint and unique Product-translation `(product_id, locale)`
index. The mutable source revision is deliberately excluded from the cursor so an update cannot
reorder an already visited row relative to the durable enumeration position.

## Monotonic source version

`products.index_revision` is a positive `BIGINT` owned by Product storage. Every Product row update
sets it to exactly `OLD.index_revision + 1` through a PostgreSQL trigger and refuses signed-range
exhaustion. Insert, update, delete, or reassignment of a Product translation updates the affected
Product row and therefore advances its revision.

The revision is storage-internal and is not added to Product API DTOs or the SeaORM write model.
Normal Product inserts use the database default. The value is used only as the generic mutation
`source_version` and as part of stable replay event identity; it is not the scan cursor.

## Explicitly open

- Product hard deletes do not yet emit durable Index tombstones. A stale indexed Product therefore
  requires the later incremental event path or reconciliation/repair slice.
- Translation deletion removes a locale row from current-state replay and likewise requires a
  later tombstone or reconciliation contract to remove an already indexed locale.
- Per-tenant schema application into `index_schemas` remains an owner operation. Runtime capability
  presence does not establish persisted schema readiness.
- Product domain events do not yet carry the source revision and are not connected to incremental
  Index mutation acknowledgement.
- A concurrent insert whose stable identity sorts behind the active cursor can require a later
  replay or reconciliation pass; no repeatable-read tenant snapshot is claimed by this adapter.
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
cargo check -p rustok-distribution --all-targets --features mod-product
cargo check -p rustok-server --all-targets --features mod-product
cargo test -p rustok-index source_event_id --lib -- --nocapture
cargo test -p rustok-index source_factory --lib -- --nocapture
cargo test -p rustok-distribution --all-targets --features mod-product -- --nocapture
```

Validation and live PostgreSQL execution are `maintainer-run`.
