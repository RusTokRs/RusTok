# Product to SalesChannel relation admission

Status: `source_contract_complete_persistence_and_wiring_pending`

Rechecked on 2026-08-03 against current `main` after the database-neutral mutation-event
acknowledgement contract and production source-call timeout landed. The canonical
implementation plan remains unchanged because it correctly keeps durable
Product-to-SalesChannel relations open; this admission-only slice does not complete that
roadmap item.

## Problem

The Product Index source currently owns Product scalar state, ProductVariant links, and
Product-owned channel visibility metadata. A real `IndexLink` from Product to the
SalesChannel schema is different: Channel create, delete, reassignment, or identity
changes can alter the resolved relation without changing `Product.index_revision`.
Re-emitting the same Product source version would be treated as duplicate or stale and
could retain obsolete links.

A relation version must therefore not be derived with `max(product_revision,
channel_revision)`, a hash, truncated pairing function, timestamp, row count, or the
current Product revision. Independent owner counters do not guarantee that any of
those encodings advances for every relation-only change.

## Compiled admission contract

`rustok-distribution::product_index::relation_admission` defines the bounded
pre-persistence contract:

- `ProductSalesChannelRelationEpoch` is a positive `u64` intended to be persisted by
  the eventual authoritative relation owner;
- `ProductSalesChannelRelationSnapshot` is scoped by exact non-nil tenant, non-nil
  Product, and canonical locale;
- channel UUIDs must be non-nil and unique, and are sorted canonically;
- an empty set is valid and represents removal of all Product-to-Channel links;
- event identity is derived with the versioned
  `rustok-distribution.product-sales-channel-relation-v1` domain and the exact
  relation epoch;
- an identical epoch is accepted only for an identical retry;
- changed membership under the same epoch fails closed;
- epoch regression and tenant/Product/locale scope changes fail closed;
- a strictly greater epoch is admitted as an advanced relation snapshot.

The locale dimension is retained because Product Index records are locale-required.
One durable relation epoch may fan out to each current Product locale, but every
locale-specific mutation gets its own stable Index event identity.

## Production admission requirements

The relation must remain unwired until one authoritative owner provides all of the
following atomically:

1. durable epoch storage for the exact tenant/Product relation identity;
2. a strictly monotonic increment on assignment create/delete, Channel deletion,
   Channel identity movement, and any change that alters the resolved target set;
3. one transaction boundary that commits membership and the new epoch together;
4. bounded current-state scan and targeted-load access ordered by relation identity
   and epoch;
5. retained delete/empty-membership state so removed links replay after restart;
6. stable event identity and source-version reuse for retries of the same epoch;
7. a registered relation event descriptor, owner delivery route, broker adapter, and
   commit-before-ack worker wiring for changes committed after a replay page was read;
8. PostgreSQL evidence for concurrent assignment, delete/recreate, restart, retry,
   out-of-order delivery, and locale fan-out.

The generic mutation-event acknowledgement and source timeout substrates now exist,
but no Product-to-SalesChannel descriptor, source route, owner storage, broker consumer,
or host worker is added by this slice.

## Explicit non-claims

This slice does not add a Product-to-SalesChannel `IndexLink`, schema version, source
query, migration, relation owner table, event consumer, checkpoint dimension, or
Storefront cutover. It does not select which source module owns the durable epoch.

The existing `allowed_channel_slugs` Product field remains Product-owned visibility
metadata and is not evidence that the cross-owner relation epoch exists.

The canonical M7 Product/Variant/Channel link item remains open until persistence,
source wiring, retained empty-membership behavior, incremental ingestion, and retained
PostgreSQL evidence satisfy the requirements above.

## Maintainer validation

Execution is maintainer-owned. Suggested commands:

```bash
cargo test -p rustok-distribution product_sales_channel_relation -- --nocapture
cargo check -p rustok-distribution --all-targets
node scripts/verify/verify-index-product-channel-relation-admission.mjs
```
