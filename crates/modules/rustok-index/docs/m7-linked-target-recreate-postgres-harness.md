# M7 linked-target recreate PostgreSQL harness

Status: `source_ready_execution_pending`.

## Purpose

The canonical Product graph has retained tombstone protocols that keep ProductVariant and SalesChannel
`index_revision` monotonic across hard delete followed by recreation of the same UUID. This packet now
retains both that owner-ordering proof and the query-path-scoped fail-closed availability semantics for
Product links.

It adds no owner clock, no Index schema, and no compatibility version.

## Existing owner monotonicity

### ProductVariant

`m20260731_000004_add_product_index_tombstones` owns `product_variant_index_tombstones`.

On hard delete it retains `OLD.index_revision + 1`. Before later insert of the same tenant/variant UUID,
`rustok_product_variant_seed_index_revision_from_tombstone` raises live `index_revision` to at least
`retained_source_version + 1`. The tombstone clears only after strict live supersession.

### SalesChannel

`m20260731_000011_add_channel_index_tombstones` provides the same invariant for
`channel_index_tombstones` and `channels.index_revision`.

Channel identity generation remains a separate Product relation freshness clock. It is not the
SalesChannel entity mutation source version.

## Harness path

`crates/rustok-distribution/tests/product_linked_target_recreate_postgres.rs` uses real Channel,
Product, and Index migrations; selected Index + Channel + Product distribution composition; persisted
tenant schema registration; real Product/ProductVariant/SalesChannel source adapters; generic
`PostgresMutationStore`; canonical shared query runtime; and the registered generic Product/Channel
`ModuleWorkScheduler` convergence worker.

No private resolver or query implementation is called directly.

## Baseline

The packet materializes one current Product, ProductVariant, and SalesChannel and requires:

- scalar Product query visible with exact count 1;
- Product graph query visible with exact count 1;
- `variants.sku` contains the original Variant SKU;
- `sales_channels.name` contains the original Channel name.

## ProductVariant recreate scenario

1. Hard-delete ProductVariant without delivering the target delete mutation to Index.
2. Require retained Variant tombstone source version newer than the old materialized target.
3. Recreate the same Variant UUID with a different SKU.
4. Require new live `index_revision` newer than both tombstone and old materialized version.
5. Require tombstone clear only after strict live supersession.
6. Materialize the current Product projection because Variant membership delete/insert advanced Product
   owner state.
7. Prove the old ProductVariant source version is still physically present in `index_entities`.
8. Require scalar-only Product query to remain visible: it does not reference `variants` and therefore
   does not depend on Variant target materialization.
9. Require Product graph query that references `variants` to return zero rows and exact count zero.
   Current Product link presence plus unavailable/stale target is fail-closed, not authoritative empty
   nested data.
10. Apply only the current ProductVariant mutation and require the Product graph to reappear with the
    recreated SKU.

## SalesChannel recreate scenario

The packet then keeps the exact same Product-to-SalesChannel UUID membership while deleting/recreating
Channel before convergence:

1. record Product `relation_epoch`, `projection_epoch`, materialized Product source version, and Channel
   generation;
2. hard-delete Channel without delivering the target delete mutation to Index;
3. require retained Channel tombstone newer than the old materialized target and generation advance;
4. recreate the same Channel UUID/canonical slug with different name;
5. require live `channels.index_revision` above tombstone and old target version, tombstone cleared, and
   generation advanced again;
6. before convergence require both scalar and graph Product queries fail owner freshness because the
   relation witness still has old Channel generation;
7. run only the registered generic convergence scheduler;
8. require unchanged final Product membership, therefore unchanged `relation_epoch`, `projection_epoch`,
   and materialized Product source version;
9. require relation freshness witness reaches the recreated Channel generation;
10. prove the old SalesChannel source version is still physically materialized;
11. require scalar-only Product query visible again — Product owner authority is current;
12. require Product graph query that references `sales_channels` still returns zero rows/exact count —
    target entity authority is not current yet;
13. apply only the current SalesChannel mutation and require the graph to reappear with recreated Channel
    name.

This cleanly separates Product owner freshness from target availability and prevents target lag from
masquerading as an authoritative empty relation.

## Remaining evidence boundary

This packet covers nested projection and exact-count behavior across delete/recreate. Additional retained
query-equivalence evidence is still required for linked filtering, many aggregate ordering, and restart /
replay ordering with link-present/target-unavailable states.

The packet does not claim execution success until the maintainer runs it.

## Maintainer verification

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-distribution --features mod-product \
  --test product_linked_target_recreate_postgres -- --nocapture
node scripts/verify/verify-index-link-target-availability.mjs
node scripts/verify/verify-index-linked-target-recreate-postgres-harness.mjs
node scripts/verify/verify-index-linked-target-query-freshness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
