# Product-SalesChannel Index relation owner ledger

Status: `owner_storage_source_complete_cross_owner_resolution_and_index_wiring_pending`.

## Purpose

The existing M7 admission contract requires a relation version that advances independently from both
`products.index_revision` and `channels.index_revision`. Product metadata still expresses desired
visibility with canonical Channel slugs, while the future Index relation must target stable Channel
UUIDs.

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
`allowed_channel_slugs` list. The existing Product visibility contract treats an empty slug allowlist
as unrestricted visibility. A future cross-owner resolver must explicitly translate that visibility
semantics into the current resolved Channel UUID set rather than copying an empty slug list into an
empty relation snapshot.

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

These readers expose Product-owned relation facts only. A future distribution adapter remains
responsible for locale fan-out and conversion to the admitted generic Index relation contract.

## Retained empty membership

A Product hard delete cannot erase the latest relation state. The Product migration installs an
`AFTER DELETE` trigger that takes the same relation advisory lock and appends exactly one empty
membership epoch when the retained current membership is non-empty.

If no relation state ever existed, or the latest membership is already empty, Product deletion does
not invent another epoch. Epoch exhaustion fails the owner delete closed rather than leaving a stale
non-empty relation snapshot.

The snapshot table has no foreign key to Product or Channel rows. This is intentional: Product and
Channel physical deletion must not erase the relation evidence needed for replay and reconciliation.

## Module boundary

`rustok-product` still has no dependency on `rustok-index` or `rustok-channel`. The owner store accepts
only UUIDs and Product identities. It does not query `channels`, resolve slugs, subscribe to Channel
events, or know the future Product schema version that will expose the Index link.

This separation avoids making Product installation depend on Channel migrations while preserving one
place that owns monotonic relation epochs.

## Still open

This slice does not yet:

- resolve `metadata.channel_visibility.allowed_channel_slugs` to Channel UUIDs;
- define the reviewed unrestricted-visibility-to-current-Channel resolution policy;
- react to Channel create/delete/slug movement or Product visibility changes;
- initialize relation state for existing Products;
- register a relation replay source or mutation-event route;
- add the Product-to-SalesChannel `IndexLink` (that requires a new Product schema version; v2 cannot
  be mutated in place);
- publish typed relation events or run a consumer;
- prove PostgreSQL concurrency, restart, delete/recreate, retry, out-of-order, or locale fan-out
  evidence;
- authorize Storefront or production Index cutover.

The next source slice should compose a cross-owner resolver in a layer that already sees both selected
Product and Channel modules. That resolver must re-read current Product visibility plus Channel
identity, preserve the existing unrestricted visibility semantics explicitly, then call this
Product-owned store; it must not move Channel SQL into `rustok-product`.

## Maintainer verification

```bash
cargo test -p rustok-product index_channel_relation --lib -- --nocapture
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-admission.mjs
cargo check -p rustok-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
