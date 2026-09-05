# ProductVariant Index refresh ledger

Status: `owner_source_complete_wire_and_consumer_pending`.

## Purpose

ProductVariant replay already uses the trigger-owned `product_variants.index_revision`, but retained
variant tombstones previously stored only `(tenant_id, variant_id, source_version)`. After physical
deletion that was insufficient to enumerate the exact variants affected by one Product owner event.

This slice adds forward-only parent identity to ProductVariant tombstones and one append-only owner
refresh ledger. It remains a Product boundary: `rustok-product` does not depend on `rustok-index`,
does not create `IndexMutation` values, and does not acknowledge broker deliveries.

## Parent-aware tombstones

New or subsequently rewritten rows in `product_variant_index_tombstones` retain `product_id` together
with the existing tenant, variant and source version. Delete and identity-move triggers capture the
old parent before the live row disappears.

Historical tombstones created before this migration keep `product_id = NULL`. They remain valid for
bounded replay by exact variant identity, but they are excluded from parent-scoped incremental
publication because their Product causation cannot be reconstructed safely. Reconciliation remains
the recovery path for those rows.

The tombstone table intentionally has no live Product foreign key so a hard-deleted Product and its
variants remain replayable.

## Ledger row contract

`product_variant_index_refresh_ledger` stores:

- tenant-local monotonic `sequence_no`;
- deterministic `refresh_id` derived from tenant, Product, root event and variant identity;
- `root_event_id`, the exact durable Product lifecycle envelope;
- exact `tenant_id`, parent `product_id`, and `variant_id`;
- positive trigger-owned `source_version`;
- owner timestamp for diagnostics only.

The database rejects nil identities, non-positive versions, updates and deletes. One root Product
event may record a variant at most once.

## Transaction order

For `ProductCreated`, `ProductUpdated`, `ProductPublished`, and `ProductDeleted`,
`ProductWriteTransaction::publish` now:

1. writes the existing root event through the transactional outbox;
2. retains its exact envelope UUID;
3. records Product locale refresh rows;
4. records all live and parent-aware retained ProductVariant identities for that Product;
5. allows the owner transaction to commit.

Variant collection is set-based and scoped to one Product. It does not impose a new maximum variant count on existing catalog commands. Any query or ledger-write failure rolls back the Product mutation,
root event, locale ledger and variant ledger together.

## Live and delete versions

Live variants use the final positive `product_variants.index_revision`. Removed variants use the
positive `product_variant_index_tombstones.source_version` captured by the Product-owned trigger.
A live variant suppresses a retained tombstone with the same tenant and variant identity.

## Bounded owner API

`ProductIndexVariantRefreshSource::list` returns at most 256 rows for one tenant after one
non-negative sequence cursor. Every row is validated fail-closed before it leaves Product ownership.
The API exposes only identity, revision, causation and paging state; ProductVariant payload remains
owned by the registered replay/load source.

## Deliberate limits

This slice does not add or change a `rustok-events` wire family or committed digest. It does not:

- publish typed ProductVariant refresh events;
- register a ProductVariant mutation route;
- start a relay or broker consumer;
- add retry, DLQ or acknowledgement policy;
- claim incremental coverage for historical parentless tombstones;
- expose HTTP, GraphQL or admin transport;
- alter or bypass the concrete-repair evidence gate.

## Maintainer verification

```bash
node scripts/verify/verify-index-product-variant-refresh-ledger.mjs
cargo check -p rustok-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, database scenarios, workflows or CI
were executed by the implementation agent.
