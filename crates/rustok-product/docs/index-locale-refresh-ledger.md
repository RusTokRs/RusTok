# Product Index locale refresh ledger

Status: `owner_source_complete_wire_and_consumer_pending`.

## Purpose

Product lifecycle events existed before the generic Index mutation route and carry only a
`product_id`. That payload cannot safely drive localized incremental ingestion because it does not
identify removed locales or expose the trigger-owned Product `index_revision`.

The Product module now retains one append-only owner row for every exact localized identity affected
by a Product lifecycle command. The ledger is written in the same database transaction as the
existing root event and Product mutation.

This is an owner boundary, not an Index table. `rustok-product` still has no dependency on
`rustok-index` and never creates `IndexMutation` values.

## Row contract

`product_index_locale_refresh_ledger` stores:

- monotonic `sequence_no` for bounded tenant-local paging;
- unique `refresh_id`, reserved as the future typed event and Index inbox identity;
- `root_event_id`, the exact durable Product lifecycle envelope that caused the row;
- exact `tenant_id`, `product_id`, and `locale` identity;
- positive `source_version` read from Product-owned live or tombstone storage;
- owner timestamp for diagnostics only.

The database rejects updates and deletes. A root event may produce several rows, but one exact
`(root_event_id, product_id, locale)` may be recorded only once.

## Transaction order

`ProductWriteTransaction::publish` performs the following steps:

1. classify whether the root event is `ProductCreated`, `ProductUpdated`, `ProductPublished`, or
   `ProductDeleted`;
2. write the existing root event through the transactional outbox and retain its envelope UUID;
3. query exact Product locale state after all owner writes and trigger execution;
4. insert bounded append-only refresh rows;
5. allow the caller to commit the Product transaction.

A source query or ledger insert failure rolls back the Product mutation and root event. No refresh
row can commit independently from its owner command.

## Live and delete versions

For a live translation, the row records the final positive `products.index_revision` after all
Product and translation triggers have completed.

For a removed locale or hard-deleted Product, the row records the exact positive
`product_index_tombstones.source_version`. Live identities suppress an older retained tombstone for
the same locale.

A later Product command may emit an older retained tombstone again. This is deliberate and safe:
the future generic Index consumer will use a new delivery identity while source-version monotonicity
classifies the older state as stale.

## Bounded owner API

`ProductIndexLocaleRefreshSource::list` exposes at most 256 rows for one exact tenant after one
non-negative sequence cursor. It returns only identities, versions, causation, and sequence data.
It does not return Product payload, execute an Index write, acknowledge a broker delivery, or mark
ledger rows mutable/consumed.

PostgreSQL is the authoritative production backend. Non-PostgreSQL Product test profiles keep their
existing root-event behavior but do not claim an Index refresh ledger.

## Deliberate limits

This slice does not:

- add or change a `rustok-events` wire family or committed digest;
- publish typed Product locale refresh events;
- register the Product v2 mutation route;
- start a broker relay or consumer;
- acknowledge deliveries or implement retry/DLQ policy;
- cover ProductVariant refresh identities;
- expose public HTTP, GraphQL, or admin transport;
- alter the separate concrete-repair evidence gate.

ProductVariant requires a separate owner slice because the current retained variant tombstone does
not preserve its parent `product_id`, which is needed for product-scoped change enumeration.

## Maintainer verification

```bash
node scripts/verify/verify-index-product-locale-refresh-ledger.mjs
cargo check -p rustok-product --all-targets
git diff --check
```

No validation command or database scenario was executed by the implementation agent.
