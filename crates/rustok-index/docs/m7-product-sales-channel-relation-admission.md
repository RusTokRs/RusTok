# Product to SalesChannel relation admission

Status: `owner_storage_source_complete_cross_owner_resolution_and_index_wiring_pending`

Rechecked on 2026-08-07 against the source-complete Product/Variant/Channel graph and the admitted
pre-persistence relation contract. The Product-owned durable relation snapshot ledger now closes the
storage/epoch boundary, but cross-owner resolution, a new Product schema version, event wiring, and
retained PostgreSQL evidence remain open.

## Problem

The Product Index source owns Product scalar state, ProductVariant links, and Product-owned channel
visibility metadata. A real `IndexLink` from Product to the SalesChannel schema is different: Channel
create, delete, slug movement, or identity changes can alter the resolved target set without changing
`products.index_revision`.

A relation version must therefore not be derived with `max(product_revision, channel_revision)`, a
hash, truncated pairing function, timestamp, row count, or the current Product revision. Independent
owner counters do not guarantee that any of those encodings advances for every relation-only change.

## Compiled admission contract

`rustok-distribution::product_index::relation_admission` remains the database-neutral semantic gate:

- `ProductSalesChannelRelationEpoch` is a positive `u64` relation version;
- `ProductSalesChannelRelationSnapshot` is scoped by exact non-nil tenant, non-nil Product, and
  canonical locale;
- channel UUIDs must be non-nil and unique, and are sorted canonically;
- an empty set is valid and represents removal of all Product-to-Channel links;
- event identity is derived with the versioned
  `rustok-distribution.product-sales-channel-relation-v1` domain and the exact relation epoch;
- an identical epoch is accepted only for an identical retry;
- changed membership under the same epoch fails closed;
- epoch regression and tenant/Product/locale scope changes fail closed;
- a strictly greater epoch is admitted as an advanced relation snapshot.

The locale dimension remains in the Index-side snapshot because Product Index records are
locale-required. One Product-owned relation epoch may fan out to every current Product locale, with a
separate deterministic Index event identity for each locale.

## Product-owned durable owner storage

`rustok-product` now owns `product_sales_channel_index_relation_snapshots` and
`ProductSalesChannelIndexRelationStore`.

The owner storage is append-only and independent from both owner revision columns. For each exact
`(tenant_id, product_id)` identity it persists:

- a tenant-local positive sequence number for bounded change consumption;
- a positive contiguous relation epoch beginning at `1`;
- the complete resolved Channel UUID set as canonical bounded JSON.

The writer serializes one relation identity with a PostgreSQL advisory transaction lock. Equal
membership is returned as an idempotent `Unchanged` result without advancing the epoch. Changed
membership appends exactly one next epoch in the same transaction.

The owner API also provides bounded append-only change pages, current-state scans in Product UUID
order, and exact current targeted loads. It does not read Channel tables or import Channel types.

A Product hard delete appends an empty membership epoch when the retained current relation is
non-empty. The relation table intentionally has no Product or Channel foreign key, so physical owner
row deletion cannot erase replay/reconciliation evidence.

Detailed owner contract: `crates/rustok-product/docs/index-sales-channel-relation-ledger.md`.

## What the storage slice closes

The production admission list is now split more precisely:

1. **Durable epoch storage for exact tenant/Product identity:** source complete.
2. **Strict increment when resolved membership changes:** source complete inside the Product-owned
   relation writer; discovering Channel-side changes remains pending.
3. **Atomic membership + epoch commit:** source complete.
4. **Bounded current-state scan and targeted load:** source complete.
5. **Retained empty-membership state:** source complete for explicit replacement and Product hard
   delete; Channel-driven removal still requires the resolver.
6. **Stable event/source-version reuse:** the relation epoch contract is source complete; conversion
   to locale-specific Index mutations remains unwired.
7. **Relation event descriptor, route, broker adapter, commit-before-ack worker:** pending.
8. **PostgreSQL concurrency/restart/retry/delete-recreate/out-of-order/locale evidence:** pending.

## Cross-owner resolver requirement

The next source slice must live in a layer that can observe both selected Product and Channel modules.
It must:

1. read current canonical `metadata.channel_visibility.allowed_channel_slugs` from Product;
2. resolve only current matching Channel UUIDs for the same tenant;
3. submit the complete resolved UUID set to `ProductSalesChannelIndexRelationStore::replace`;
4. re-run on Product visibility changes and on Channel create/delete/slug movement that can change a
   Product's resolved set;
5. preserve bounded work and idempotent retry behavior;
6. never move Channel SQL or a `rustok-channel` dependency into `rustok-product`.

The resolver may be eventually triggered by owner events, but the relation owner commit itself must
remain atomic and monotonic.

## Index schema and wiring requirement

Product v2 cannot be modified in place because its schema fingerprint is already published. The real
Product-to-SalesChannel `IndexLink` therefore requires a new Product schema version after the owner
relation source is composed.

That future schema/source slice must fan one relation epoch out to the exact current Product locales,
use the admitted relation event identity, and materialize SalesChannel UUID targets without querying
Channel storage from the Product source.

The Product typed incremental event family remains separately blocked by canonical event-contract
digest admission. This relation owner storage does not bypass that gate.

## Explicit non-claims

This slice does not yet add:

- the cross-owner Product visibility to Channel UUID resolver;
- Channel create/delete/slug-change integration;
- initial backfill for existing Products;
- a new Product Index schema version or Product-to-SalesChannel `IndexLink`;
- a relation replay source in `rustok-distribution`;
- a relation mutation-event route, broker consumer, retry policy, or acknowledgement;
- retained PostgreSQL runtime evidence;
- Storefront or production partition cutover.

`allowed_channel_slugs` remains Product-owned desired visibility metadata. The new relation ledger is
resolved relation owner state only after a resolver writes it; neither one alone proves production
cutover readiness.

## Maintainer validation

```bash
cargo test -p rustok-product index_channel_relation --lib -- --nocapture
cargo test -p rustok-distribution product_sales_channel_relation -- --nocapture
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-admission.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
