# M7 linked-target recreate PostgreSQL harness

Status: `source_ready_execution_pending`.

## Purpose

The canonical Product graph already has retained tombstone protocols that keep ProductVariant and
SalesChannel `index_revision` monotonic across hard delete followed by recreation of the same UUID.
This packet retains a real PostgreSQL proof that those owner clocks compose with the generic entity
query admission introduced for linked targets.

It adds no owner clock, no Index schema, and no compatibility version.

## Existing owner monotonicity

### ProductVariant

`m20260731_000004_add_product_index_tombstones` owns `product_variant_index_tombstones`.

On hard delete it retains `OLD.index_revision + 1`. Before a later insert of the same tenant/variant
UUID, `rustok_product_variant_seed_index_revision_from_tombstone` raises the new live
`index_revision` to at least `retained_source_version + 1`. The retained tombstone is cleared only
after the inserted live revision strictly supersedes it.

Therefore a recreated ProductVariant cannot reuse the source version of an older materialized target.

### SalesChannel

`m20260731_000011_add_channel_index_tombstones` provides the same invariant for
`channel_index_tombstones` and `channels.index_revision`.

On hard delete it retains `OLD.index_revision + 1`; before recreation of the same tenant/Channel UUID,
`rustok_channel_seed_index_revision_from_tombstone` seeds the live revision above the retained delete
version. The tombstone is cleared only after the live row supersedes it.

Channel identity generation remains a separate tenant freshness clock for Product-to-SalesChannel
membership resolution. It is not the SalesChannel Index mutation source version.

## Harness path

`crates/rustok-distribution/tests/product_linked_target_recreate_postgres.rs` uses real Channel,
Product, and Index migrations; selected Index + Channel + Product distribution composition; persisted
tenant schema registration; the real Product, ProductVariant, and SalesChannel source adapters; generic
`PostgresMutationStore`; the canonical shared query runtime; and the registered generic Product/Channel
convergence `ModuleWorkScheduler` worker.

No private resolver or query implementation is called directly.

## ProductVariant scenario

The packet first materializes one current Product, ProductVariant, and SalesChannel and requires the
Product nested projection to expose the initial Variant SKU and Channel name.

Then it:

1. hard-deletes the owner ProductVariant without delivering the target delete mutation to Index;
2. requires a retained ProductVariant tombstone source version newer than the old materialized Variant;
3. recreates the same Variant UUID with a different SKU;
4. requires the new live `index_revision` to be newer than both the tombstone and old materialized
   target source version;
5. requires the retained Variant tombstone to clear only after that superseding live revision;
6. materializes the current Product projection because Variant membership delete/insert advanced the
   Product owner clock;
7. proves the old ProductVariant target source version is still physically present in `index_entities`;
8. requires the current Product root to remain visible while its `variants` nested payload is empty —
   the stale old SKU must not leak through entity admission;
9. applies only the current ProductVariant mutation and requires the recreated SKU to appear.

The empty nested payload in step 8 is stale-target exclusion evidence only. It is not a claim that
link-present/target-unavailable should ultimately be authoritative empty semantics.

## SalesChannel scenario

The packet then keeps the same Product-to-SalesChannel UUID membership while deleting and recreating
the Channel before convergence:

1. record current Product `relation_epoch`, `projection_epoch`, materialized Product source version,
   and Channel generation;
2. hard-delete the Channel without delivering the target delete to Index;
3. require the retained Channel tombstone to be newer than the old materialized SalesChannel source
   version and Channel generation to advance;
4. recreate the same Channel UUID/canonical slug with a different name;
5. require the new live `channels.index_revision` to exceed the tombstone and old materialized source
   version, require the tombstone to clear, and require Channel generation to advance again;
6. require Product root query admission to fail before convergence because the Product relation witness
   still carries the old Channel generation;
7. run only the registered generic convergence scheduler;
8. require final membership to be unchanged, so Product `relation_epoch` and `projection_epoch` remain
   unchanged and the existing Product materialized source version is not replaced;
9. require freshness to reach the recreated Channel generation;
10. prove the old SalesChannel source version is still physically materialized while the Product root
    is query-admissible again;
11. require the Product `sales_channels` nested payload to be empty, proving the stale old Channel name
    cannot leak through the linked target row;
12. apply only the current SalesChannel mutation and require the recreated Channel name to appear.

This isolates Product relation freshness from target entity freshness: the Product root may be current
while a linked SalesChannel target still needs its own current materialization.

## Deliberate boundary

This packet does **not** define the final authoritative semantics for a link that exists while its target
is absent or not yet materialized. Current SQL represents a filtered/unavailable many target as an empty
nested projection. Before Storefront cutover, complete query parity still needs an explicit fail-closed
policy deciding whether that window should fail the root query rather than appear as empty/null.

The packet also does not claim execution success until the maintainer runs it.

## Maintainer verification

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-distribution --features mod-product \
  --test product_linked_target_recreate_postgres -- --nocapture
node scripts/verify/verify-index-linked-target-recreate-postgres-harness.mjs
node scripts/verify/verify-index-linked-target-query-freshness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
