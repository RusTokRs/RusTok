# Product Index locale refresh ledger

Product publishes the ledger through its transactional outbox. The Index refresh consumer
loads the current replay record through the Product owner port before applying a mutation.

## Purpose

Product owner events existed before the generic Index mutation route and carry only a `product_id`.
That payload cannot safely drive localized incremental ingestion because it does not identify removed
locales or expose the trigger-owned Product `index_revision`.

The Product module retains one append-only owner row for every exact localized identity affected by a
Product lifecycle command. Product EAV value commands now use the same Product locale-refresh boundary:
the existing `ProductAttributeValuesChanged` event first advances the Product Index clock in the same
transaction, then captures every current Product locale at that new owner version.

This is an owner boundary, not an Index table. `rustok-product` still has no dependency on
`rustok-index` and never creates `IndexMutation` values.

## Row contract

`product_index_locale_refresh_ledger` stores:

- monotonic `sequence_no` for bounded tenant-local paging;
- unique `refresh_id`, used as the typed refresh event identity;
- `root_event_id`, the exact durable Product owner envelope that caused the row;
- exact `tenant_id`, `product_id`, and `locale` identity;
- positive `source_version` read from Product-owned live or tombstone storage;
- owner timestamp for diagnostics only.

The database rejects updates and deletes. A root event may produce several rows, but one exact
`(root_event_id, product_id, locale)` may be recorded only once.

## Product owner clock

`products.index_revision` remains the single Product owner input watermark. No EAV-specific Index clock
is added.

Normal Product and translation writes already advance that revision through Product-owned triggers.
Standalone EAV value commands previously changed `product_attribute_values`, localized value rows, or
option memberships without updating `products`, so their final state could not be represented by a new
Product source version.

`ProductWriteTransaction::publish` now treats the existing `ProductAttributeValuesChanged` event as one
Product-only clock boundary on PostgreSQL:

1. before publishing the event, execute an exact tenant/Product `UPDATE products SET index_revision =
   index_revision`;
2. `trg_products_bump_index_revision` owns the actual `+1` and exhaustion guard;
3. the canonical Product graph projection trigger observes that new Product source version;
4. publish the existing ProductAttributeValuesChanged envelope in the same transaction;
5. capture Product locale refresh rows at the final revision;
6. do **not** fan the EAV command out to unchanged ProductVariant refresh rows.

The touch statement changes only `index_revision`. Product-SalesChannel convergence listens to Product
`metadata`, tenant, or identity changes, so an EAV-only command does not fabricate relation-convergence work.

Non-PostgreSQL Product profiles keep their existing event behavior and do not claim the PostgreSQL Index
clock/refresh guarantee.

## Lifecycle transaction order

For `ProductCreated`, `ProductUpdated`, `ProductPublished`, and `ProductDeleted`, the established
transaction order remains:

1. write the existing root event through the transactional outbox and retain its envelope UUID;
2. query exact Product locale state after all owner writes and trigger execution;
3. insert bounded append-only Product locale refresh rows;
4. capture ProductVariant refresh rows for the same lifecycle/Product command;
5. allow the caller to commit the Product transaction.

A clock update, source query, ledger insert, or outbox failure rolls back the entire owner transaction.
No refresh row can commit independently from its owner command.

## Live and delete versions

For a live translation, the row records the final positive `products.index_revision` after all Product,
translation, or EAV-command clock updates have completed.

For a removed locale or hard-deleted Product, the row records the exact positive
`product_index_tombstones.source_version`. Live identities suppress an older retained tombstone for the
same locale.

A later Product command may emit an older retained tombstone again. The Product replay response
contains the retained locale revision and the graph projection epoch from the same snapshot.
Index checks the event against the locale revision. It uses the graph epoch to order materialized
mutations and deduplicates them by the canonical source mutation ID. This also lets the first graph
snapshot, whose epoch starts at one, satisfy events from Product revisions committed before
Channel relation materialization.

## Bounded owner API

`ProductIndexLocaleRefreshSource::list` exposes at most 256 rows for one exact tenant after one
non-negative sequence cursor. It returns identities, versions, causation, and sequence data.
The relay tracks progress separately from the append-only ledger. The
[Product replay port](../ports/README.md#localized-product-replay) supplies the current payload.

PostgreSQL is the authoritative production backend. Non-PostgreSQL Product test profiles keep their
existing root-event behavior but do not claim an Index refresh ledger.

## Verification

```bash
node scripts/verify/verify-index-product-eav-owner-clock.mjs
node scripts/verify/verify-index-product-locale-refresh-ledger.mjs
node scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs
```

The [Index redelivery fixture](../../rustok-index/docs/m5-product-refresh-postgres-iggy-redelivery-evidence.md)
covers the owner revision fence and persistence before commit of the broker offset.
