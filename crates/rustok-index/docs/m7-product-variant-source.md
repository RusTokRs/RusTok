# M7 ProductVariant source adapter

Status: `source_complete_owner_execution_pending`

## Scope

When `mod-product` is selected, `rustok-distribution` publishes non-localized
`rustok-product::product_variant@1` and `@2` schemas through one stable PostgreSQL source,
`product-variant-postgres-primary`.

ProductVariant v1 retains the original scalar contract. ProductVariant v2 adds the stable variant
identity field used by the Product v2 many-link contract.

The schemas contain ProductVariant-owned state only:

- owning `product_id`;
- stable variant identity in v2;
- SKU, barcode, EAN, UPC, and shipping-profile slug;
- inventory policy, management mode, and quantity;
- weight unit, option1/2/3, and display position.

Pricing, inventory ledger state, translations, and SalesChannel relations remain outside this source.

## Ownership and composition

`rustok-product` owns `product_variants`, positive monotonic `index_revision`, and retained
`product_variant_index_tombstones`. The Product crate does not depend on `rustok-index` and
publishes only `ProductRuntimeSelected`.

`rustok-distribution` registers Product and ProductVariant schemas plus database-aware source
factories. Host factory staging remains atomic across both replay sources and the separate Product
locale-absence provider. Factory materialization performs no SQL and starts no task.

## Source contract

The ProductVariant source supports:

- cursor scans ordered by stable `variant_id`;
- one-row lookahead over the bounded page limit;
- targeted loads over exact non-localized variant keys;
- exact tenant and schema scope for v1 and v2;
- stable replay event UUIDs derived from tenant, variant, event domain, and source revision;
- retryable storage failures and permanent contract/backend failures without raw database details.

The source reads one union of live `product_variants` rows and retained tombstones. It emits generic
`IndexMutation::Upsert` or `IndexMutation::Delete` values and never writes Index storage directly.
Live/tombstone coexistence for one exact identity fails closed.

## Monotonic source version and retained deletes

`product_variants.index_revision` is a positive `BIGINT`. A PostgreSQL trigger advances it for
updates and refuses signed-range exhaustion.

Hard delete stores a tombstone at `OLD.index_revision + 1`. Identity movement stores a tombstone for
the old identity. Identity reuse seeds the inserted live revision above the retained tombstone and
clears only a strictly superseded tombstone.

The revision remains storage-internal and is used only as generic mutation `source_version` and
replay event identity, not as the scan cursor.

## Product graph boundary

Product v2 declares a many-cardinality `variants` link to ProductVariant v2. ProductVariant remains a
standalone replay source; Product membership changes advance Product revision through the dedicated
membership trigger so Product link state cannot change under a stale Product version.

No ProductVariant locale-absence provider is required because the schema is non-localized and
retained hard deletes are emitted as ordinary `Delete` mutations.

## Explicitly open

- production mutation-event routes and concrete broker consumer wiring;
- richer ProductVariant relations and SalesChannel semantics;
- persisted per-tenant schema readiness;
- tombstone purge admission and retention evidence;
- retained PostgreSQL replay, delete/recreate, restart, and equivalence evidence;
- authoritative consumer cutover.

No repeatable-read full-tenant owner snapshot is claimed by the replay source. Reconciliation remains
required for concurrent inserts that sort behind an active cursor.

## Owner verification

The implementation agent did not run commands. The repository owner should run:

```bash
node scripts/verify/verify-index-product-variant-source.mjs
node scripts/verify/verify-index-product-graph-source.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-distribution --all-targets --features mod-product
cargo check -p rustok-server --all-targets --features mod-product
cargo test -p rustok-distribution product_variant_index --features mod-product -- --nocapture
```

Validation and live PostgreSQL execution are `maintainer-run`.
