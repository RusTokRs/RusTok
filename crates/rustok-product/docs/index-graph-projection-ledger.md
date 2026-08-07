# Product Index graph projection ledger

Status: `canonical_source_complete_runtime_evidence_pending`.

## Purpose

The current Product Index record is one complete localized graph projection. It contains Product
scalar state, ProductVariant membership, and resolved SalesChannel UUID membership. Product state and
resolved channel membership advance independently, so neither `products.index_revision` nor
`product_sales_channel_index_relation_snapshots.relation_epoch` can safely be used as the complete
record clock.

`product_index_graph_projection_snapshots` owns the one monotonic `projection_epoch` used by the
canonical Product Index source as its full-record `source_version`.

There are no parallel Product Index schema implementations. The numeric `SchemaVersion` field remains
part of the generic Index storage key, but the selected distribution registers exactly one current
Product contract and one current ProductVariant contract.

## Storage contract

Each immutable row contains:

- positive tenant-local `sequence_no`;
- exact non-nil `tenant_id` and `product_id`;
- positive contiguous `projection_epoch`, beginning at `1`;
- positive retained Product `product_source_version` watermark;
- positive Product-to-SalesChannel `relation_epoch` watermark.

For one tenant/Product identity, projection epochs are contiguous, component watermarks cannot regress,
an unchanged component pair does not append, and retained rows cannot be updated or deleted.

The `product-index-graph-projection` PostgreSQL advisory lock serializes this exact owner projection
identity.

## Reconciliation

`rustok_product_reconcile_index_graph_projection(tenant_id, product_id)` observes:

1. live Product `index_revision`, or the maximum retained Product tombstone version after deletion;
2. the latest Product-owned SalesChannel relation epoch.

If either input is absent, no projection is invented. Otherwise reconciliation advances
`projection_epoch` exactly once when at least one retained component watermark advances.
`GREATEST` is used only to merge already retained component watermarks; it is not the Index
source-version encoding.

Reconciliation runs after Product insert, Product `index_revision` changes, Product hard delete, and
relation snapshot inserts. On hard delete, the projection trigger sorts after the retained-empty
relation trigger, so final projection state observes the final empty SalesChannel membership.

## Canonical source contract

`rustok-distribution::product_index::product` reads this projection ledger and emits one current
Product Index contract with:

- Product identity and scalar fields;
- `variant_ids` plus a many `variants` link;
- `sales_channel_ids` plus a many `sales_channels` link;
- `projection_epoch` as the complete mutation `source_version`.

The Product owner crate still does not import `rustok-index` or `rustok-channel`. Cross-owner Channel
resolution remains in `rustok-distribution` and writes only resolved UUID membership through the
Product-owned relation store.

## Freshness boundary

Projection ordering and relation freshness are separate properties. A Product visibility change can
advance Product state before the bounded cross-owner resolver has recomputed SalesChannel membership.
Therefore the canonical source remains non-authoritative until durable Product/Channel convergence
triggering or an admitted freshness watermark and retained PostgreSQL evidence prove the relation is
current.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-index-product-graph-projection-ledger.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
