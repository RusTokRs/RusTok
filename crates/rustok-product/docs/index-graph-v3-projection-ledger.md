# Product Index graph v3 projection epoch ledger

Status: `projection_epoch_source_complete_v3_replay_pending`.

## Purpose

Product v3 must eventually materialize one complete localized Product record containing both
Product-owned scalar/Variant state and the resolved Product-to-SalesChannel relation. Those two input
families have independent monotonic counters:

- `products.index_revision` advances for Product/translation/Variant-membership state;
- `product_sales_channel_index_relation_snapshots.relation_epoch` advances for resolved Channel UUID
  membership.

The Index mutation store accepts only full `Upsert`/`Delete` mutations and ignores an incoming
mutation when its `source_version` is less than or equal to the already materialized version. Using
either input counter directly for Product v3 would therefore make changes from the other input family
eligible for stale-ignore. Deriving a source version with `max`, a hash, a timestamp, or a pairing
function would retain the same defect.

`product_index_graph_v3_projection_snapshots` introduces a separate Product-owned monotonic
`projection_epoch`. It is the future Product v3 source-version candidate; the two input counters are
retained as evidence, not encoded into that source version.

## Storage contract

Each immutable row contains:

- positive tenant-local `sequence_no`;
- exact non-nil `tenant_id` and `product_id`;
- positive contiguous `projection_epoch`, beginning at `1`;
- positive `product_source_version` watermark;
- positive Product-to-SalesChannel `relation_epoch` watermark.

For one exact tenant/Product identity:

- the first projection epoch is exactly `1`;
- every later row is exactly previous epoch plus one;
- neither input watermark may regress;
- at least one input watermark must advance;
- an identical input pair is idempotent and must not append another epoch;
- retained rows cannot be updated or deleted.

A PostgreSQL advisory transaction lock in the
`product-index-graph-v3-projection` domain serializes the exact tenant/Product projection identity.
The same lock is taken by the table guard and by the canonical reconciliation function.

## Canonical reconciliation

`rustok_product_reconcile_index_graph_v3_projection(tenant_id, product_id)` observes:

1. the live Product `index_revision`, or the maximum retained Product tombstone version after hard
   deletion;
2. the latest Product-owned Product-to-SalesChannel relation epoch.

If either input does not yet exist, no Product v3 projection snapshot is invented. This is deliberate:
a Product with no resolved relation state is not yet replay-ready for the future v3 graph schema.

For an existing projection the function merges each observed input watermark with the already retained
watermark using `GREATEST`, then appends one new `projection_epoch` only if at least one component
advanced. These `GREATEST` operations are **not** the Index source-version algorithm. They only prevent
a concurrent observer from regressing one already retained component. The independent
`projection_epoch` remains the sole future v3 source version.

The migration backfills epoch `1` for every live or retained Product identity that already has a
relation snapshot.

## Owner triggers

Projection reconciliation is invoked after:

- Product insert;
- Product `index_revision` update;
- Product hard delete;
- Product-to-SalesChannel relation snapshot insert.

The Product hard-delete trigger is deliberately named
`trg_products_zz_index_graph_v3_projection_delete`. PostgreSQL orders same-kind triggers by name, so
the existing `trg_products_retain_empty_channel_relation` trigger runs first. Its final empty relation
snapshot invokes projection reconciliation; the trailing Product delete trigger is then idempotent for
the same final input pair. No committed v3 projection state is left pointing at the pre-delete Channel
membership.

## What this closes

This ledger closes one prerequisite that the previous plan understated: Product v3 cannot safely use
`relation_epoch` directly as the full-record Index `source_version`. It now has a dedicated monotonic
owner arbitration boundary that advances when either Product graph input family advances.

The ledger does **not** prove that the relation membership is fresh relative to the newest Product
visibility metadata. The current cross-owner resolver is bounded and source-complete, but durable
Product/Channel triggering or an admitted convergence watermark is still pending. Product v3 replay
must therefore remain non-authoritative until that freshness boundary and retained PostgreSQL evidence
are complete.

## Still open

This slice does not:

- publish `rustok-product::product@3`;
- modify Product v1/v2 schema fingerprints or replay semantics;
- register a Product v3 replay/absence source;
- materialize a Product-to-SalesChannel `IndexLink`;
- add a typed event, broker route, worker, checkpoint, lease, retry loop, or acknowledgement;
- prove relation freshness after Product visibility or Channel identity changes;
- execute PostgreSQL concurrency/restart/delete-recreate evidence;
- authorize Storefront or production Index cutover.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-index-product-v3-projection-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
