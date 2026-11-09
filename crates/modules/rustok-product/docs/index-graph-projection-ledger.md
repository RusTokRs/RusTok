# Product Index graph projection ledger

Status: `canonical_source_and_freshness_gate_complete_runtime_evidence_pending`.

## Purpose

The current Product Index record is one complete localized graph projection containing Product scalar
state, ProductVariant membership, and resolved SalesChannel UUID membership. Product state and relation
membership advance independently, so neither `products.index_revision` nor `relation_epoch` is a safe
complete record clock.

`product_index_graph_projection_snapshots` owns the monotonic `projection_epoch` used by the canonical
Product Index source as its full-record `source_version`.

There are no parallel Product Index compatibility implementations. The numeric `SchemaVersion` remains
a generic Index storage key only.

## Storage contract

Each append-only row contains:

- positive tenant-local `sequence_no`;
- exact non-nil tenant/Product identity;
- positive contiguous `projection_epoch` beginning at `1`;
- positive retained Product `product_source_version` watermark;
- positive Product-to-SalesChannel `relation_epoch` watermark.

Projection epochs are contiguous, component watermarks cannot regress, an unchanged component pair does
not append, and retained rows cannot be updated or deleted. The
`product-index-graph-projection` advisory lock serializes one exact Product projection identity.

## Reconciliation

`rustok_product_reconcile_index_graph_projection(tenant_id, product_id)` observes the live Product
`index_revision` (or maximum retained Product tombstone source version after deletion) and the latest
Product-owned relation epoch.

If either input is absent, no projection is invented. Otherwise `projection_epoch` advances exactly
once when at least one retained component watermark advances. `GREATEST` only merges already retained
component watermarks; it is not the Index source-version encoding.

Reconciliation runs after Product insert, Product `index_revision` changes, Product hard delete, and
relation snapshot inserts. Hard-delete ordering ensures final projection state observes the retained
empty SalesChannel membership.

The Product update trigger uses `AFTER UPDATE` and compares the final old and new revisions.
The Product `BEFORE UPDATE` trigger increments `index_revision` even when the statement only
sets a field such as `vendor` or `metadata`. The graph captures that revision in the same
transaction. Its relation watermark can advance independently when channel membership changes.

### Recovery after upgrading the update trigger

Apply `m20261109_000040_fix_index_graph_update_trigger` in a maintenance window. The trigger
replacement takes a short lock that excludes concurrent Product writers. Recovery of older
revisions is separate so that a catalog scan cannot extend that lock.

For each affected tenant, connect to the database with the Product tables on the session's
search path. From the repository root, load the [repair query](../scripts/repair_index_graph.sql)
in `psql` and execute it in autocommit mode, once per page:

```sql
\i crates/modules/rustok-product/scripts/repair_index_graph.sql
EXECUTE repair_product_index_graph('<tenant-uuid>', '00000000-0000-0000-0000-000000000000');
```

Pass each returned `next_cursor` as the second argument for the next call. Stop when
`next_cursor` is NULL, even if intermediate pages return zero `candidates`. Each call examines
up to 250 tenant-scoped Products and uses the same reconciler as the triggers for outdated or
missing graph rows that already have a relation snapshot. Retry a failed page with its original
cursor. The reconciler rechecks the current owner state and skips unchanged component versions.
It leaves source rows and retained history intact. The trigger rollback restores the old trigger
definition and keeps all appended projection history.

After the graph repair, use the [Index replay runtime](../../rustok-index/docs/m6-replay-runtime-composition.md)
for the affected Product scope to refresh previously materialized rows. Products whose relation
snapshot or witness is missing still go through normal Channel relation convergence.

## Canonical source

`rustok-product::index_projection` owns the PostgreSQL replay and absence read
contracts. The Distribution adapter combines the Product replay projection
with the Channel identity generation relation in one statement to emit one current Product
contract containing Product scalars, the `variants` link, and the `sales_channels` link. Live Product
rows require the projection Product watermark to equal current `products.index_revision`.

The projection ledger intentionally does not encode relation freshness. That is now enforced by the
separate Product-owned
`product_sales_channel_index_relation_freshness_snapshots` witness and the Channel-owned tenant
`channel_index_identity_generations` watermark.

For live replay, the canonical source requires a witness for the projection's exact `relation_epoch`
whose canonical visibility key and Channel identity generation match current owner facts. Product
locale absence uses the same gate. Product hard-delete replay does not require a live witness because
it removes the graph.

Detailed freshness contract:
`crates/modules/rustok-product/docs/index-sales-channel-relation-freshness.md`.

## Remaining admission

The source-level freshness watermark gap is closed. Still open are retained PostgreSQL
freshness/concurrency/restart/delete-recreate evidence, automatic reconciliation scheduling if required
by the production freshness SLO, typed Product event admission/routes, and Storefront/query-equivalence
cutover evidence.

## Maintainer verification

Scoped verification commands:

```bash
node scripts/verify/verify-index-product-graph-projection-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

The Distribution PostgreSQL freshness fixture exercises scalar updates, trigger upgrade and rollback,
repair of an older committed revision, and readmission after Index materializes the repaired source.
