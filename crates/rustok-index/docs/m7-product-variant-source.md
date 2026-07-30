# M7 ProductVariant source adapter

Status: `source_complete_owner_execution_pending`

## Scope

This slice adds a second selected Product-owned current-state replay source through
`rustok-distribution`. When `mod-product` is selected, the distribution publishes the
non-localized `rustok-product::product_variant@1` schema and a PostgreSQL-backed bounded source.

The schema contains ProductVariant-owned scalar state only:

- owning `product_id` as a UUID field;
- SKU, barcode, EAN, UPC, and shipping-profile slug;
- inventory policy, management mode, and quantity;
- weight unit;
- option1, option2, and option3;
- stable display position.

Weight magnitude, ProductVariant translations, option-value relations, pricing, inventory ledger
state, and SalesChannel visibility are not part of this schema.

## Ownership and composition

`rustok-product` owns `product_variants` and the positive monotonic `index_revision` migration.
The Product crate still does not depend on `rustok-index` and publishes only
`ProductRuntimeSelected` for selected distribution bridges.

The existing distribution `product_index` module now composes two byte-independent adapters:
Product and ProductVariant. The already merged Product adapter is moved without changing its blob
content. ProductVariant registers one additional schema and one additional
`PostgresIndexSourceFactory`; host factory staging remains atomic across both sources.

Index core and server remain source-domain agnostic. Factory materialization performs no SQL and
starts no task.

## Source contract

The ProductVariant PostgreSQL source supports:

- cursor scans ordered by stable `variant_id`;
- one-row lookahead over the caller's bounded page limit;
- targeted loads over exact non-localized variant keys;
- exact tenant and schema scope;
- stable replay event UUIDs derived from tenant, variant, and source revision;
- retryable storage failures and permanent contract/backend failures without raw database details.

The source reads only `product_variants`. It emits generic `IndexMutation::Upsert` records and never
writes Index storage directly. `product_id` is retained as a scalar field so a future versioned link
contract can use it without changing source storage semantics.

## Monotonic source version

`product_variants.index_revision` is a positive `BIGINT` with database default `1`. A PostgreSQL
`BEFORE UPDATE` trigger sets the next value to exactly `OLD.index_revision + 1` and refuses signed
range exhaustion. The column remains storage-internal and is not added to Product API DTOs or the
SeaORM write model.

The revision is used only as generic mutation `source_version` and replay event identity. It is not
the scan cursor.

## Product v1 compatibility boundary

The already published `rustok-product::product@1` schema is unchanged. ProductVariant does not add
a link to Product in this slice because Index links require schema-declared join fields on both
ends, while Product v1 intentionally has no identity field in its field set. Adding such a field
would change the Product v1 fingerprint.

Product-to-variant traversal therefore requires an explicit future Product schema version and a
versioned link contract. This slice does not silently mutate an existing schema fingerprint.

## Explicitly open

- ProductVariant hard deletes do not yet emit durable Index tombstones.
- ProductVariant domain events do not yet carry `index_revision` into incremental ingestion.
- ProductVariant translations and localized titles remain outside the non-localized v1 schema.
- Product/ProductVariant links require a versioned Product schema contract.
- SalesChannel schema/source and Product/Variant channel visibility links remain open.
- Persisted per-tenant schema application, consumer cutover, and retained PostgreSQL evidence remain
  owner operations.
- No repeatable-read full-tenant replay snapshot is claimed; reconciliation remains required for
  concurrent inserts that sort behind an active cursor.

## Owner verification

The implementation agent did not run commands. The repository owner should run:

```bash
node scripts/verify/verify-index-product-variant-source.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-distribution --all-targets --features mod-product
cargo check -p rustok-server --all-targets --features mod-product
cargo test -p rustok-distribution product_variant_index --features mod-product -- --nocapture
```

Validation and live PostgreSQL execution are `maintainer-run`.
