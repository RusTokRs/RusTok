# Product-SalesChannel Index relation owner ledger

Status: `owner_storage_source_complete_resolver_composed_index_wiring_pending`.

## Purpose

The M7 admission contract requires a relation version that advances independently from both
`products.index_revision` and `channels.index_revision`. Product metadata expresses desired visibility
with canonical Channel slugs, while the future Index relation targets stable Channel UUIDs.

`product_sales_channel_index_relation_snapshots` is the Product-owned durable boundary for that
resolved UUID membership. It deliberately does not resolve slugs, read Channel tables, construct an
Index mutation, or publish a broker event.

## Storage contract

Each append-only snapshot contains:

- one positive `sequence_no`; change reads and cursors remain tenant-scoped;
- exact non-nil `tenant_id` and `product_id` identity;
- one positive contiguous `relation_epoch` for that tenant/Product relation;
- the complete resolved Channel UUID membership as canonical JSON.

The Channel UUID array is bounded to 1024 entries. Every value must be a canonical non-nil UUID
string, strictly sorted and unique. An empty array is valid and means that the resolved relation has
no Channel targets.

An empty resolved UUID membership is not the same thing as an empty Product
`allowed_channel_slugs` list. The Product visibility contract treats an empty slug allowlist as
unrestricted visibility. The distribution-owned resolver now preserves that distinction explicitly by
resolving unrestricted Products against the current tenant Channel identity universe.

The first persisted epoch is exactly `1`. A later snapshot must be exactly the previous epoch plus
one and must change membership. Equal membership is an idempotent retry and is not appended.
Updates and deletes of retained snapshots are rejected.

## Product-owned write API

`ProductSalesChannelIndexRelationStore::replace` accepts an already resolved Channel UUID set. It:

1. validates tenant/Product identity and the bounded unique Channel set;
2. requires and locks the exact live Product row with `FOR KEY SHARE`;
3. acquires one PostgreSQL advisory transaction lock for the exact tenant/Product relation;
4. loads the latest retained snapshot;
5. returns `Unchanged` without creating a new epoch when membership is identical;
6. otherwise appends epoch `1` or exactly `previous + 1`;
7. commits the complete membership and epoch in the same transaction.

The Product row lock is deliberate. A stale resolver cannot append a new non-empty relation after a
concurrent Product delete. Product deletion takes the row lock first and its `AFTER DELETE` trigger
then takes the same relation advisory lock, so the lock order stays consistent and the final retained
state converges to an empty relation epoch.

The store maps storage failures to one bounded `Unavailable` error rather than exposing raw database
diagnostics.

## Bounded owner reads

The same owner boundary exposes:

- `list_changes` — tenant-scoped append-only sequence pages of at most 256 snapshots;
- `scan_current` — current state for at most 256 Products in stable Product UUID order;
- `load_current` — exact current state for at most 64 Product UUIDs.

These readers expose Product-owned relation facts only. The distribution layer remains responsible
for cross-owner resolution, locale fan-out, and eventual conversion to the Index relation contract.

## Retained empty membership

A Product hard delete cannot erase the latest relation state. The Product migration installs an
`AFTER DELETE` trigger that takes the same relation advisory lock and appends exactly one empty
membership epoch when the retained current membership is non-empty.

If no relation state ever existed, or the latest membership is already empty, Product deletion does
not invent another epoch. Epoch exhaustion fails the owner delete closed rather than leaving a stale
non-empty relation snapshot.

The snapshot table has no foreign key to Product or Channel rows. This is intentional: Product and
Channel physical deletion must not erase the relation evidence needed for replay and reconciliation.

## Cross-owner composition

`rustok-distribution::product_index::channel_relation_resolver` now reads Product visibility plus
current tenant Channel identities and submits only the complete resolved UUID set to this owner API.
The Product crate remains unaware of Channel tables and Channel types.

The resolver uses bounded observe/write/re-observe stabilization. That composition does not change the
owner transaction contract and does not turn Product storage into an atomic cross-owner snapshot.
Durable event triggers, watermarks/checkpoints, and runtime evidence remain separate admission work.

Detailed resolver contract:
`crates/rustok-index/docs/m7-product-sales-channel-resolver.md`.

## Module boundary

`rustok-product` still has no dependency on `rustok-index` or `rustok-channel`. The owner store accepts
only UUIDs and Product identities. It does not query `channels`, resolve slugs, subscribe to Channel
events, or know the future Product schema version that will expose the Index link.

This separation avoids making Product installation depend on Channel migrations while preserving one
place that owns monotonic relation epochs.

## Still open

This owner slice plus resolver composition still do not:

- register durable Product/Channel relation triggers or checkpoints;
- register a relation replay source or mutation-event route;
- add the Product-to-SalesChannel `IndexLink` (that requires a new Product schema version; v2 cannot
  be mutated in place);
- publish typed relation events or run a consumer;
- prove PostgreSQL concurrency, restart, delete/recreate, retry, out-of-order, or locale fan-out
  evidence;
- authorize Storefront or production Index cutover.

The next unblocked source slice is a new Product Index schema version plus relation replay adapter in
the distribution/Index integration boundary. It must consume this owner epoch rather than deriving a
relation version from Product or Channel revisions.

## Maintainer verification

```bash
cargo test -p rustok-product index_channel_relation --lib -- --nocapture
cargo test -p rustok-distribution product_sales_channel -- --nocapture
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
