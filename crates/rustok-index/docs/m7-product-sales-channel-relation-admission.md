# Product to SalesChannel relation admission

Status: `canonical_source_and_freshness_watermark_complete_runtime_evidence_pending`.

The current Product Index graph contains the Product-to-SalesChannel link. There is one canonical
Product contract; no compatibility schema is waiting to add relation support.

## Durable membership

`rustok-product` owns `product_sales_channel_index_relation_snapshots` and
`ProductSalesChannelIndexRelationStore`.

For each tenant/Product identity the append-only ledger stores a positive contiguous `relation_epoch`
and the complete canonical SalesChannel UUID membership. Equal membership is idempotent and does not
advance the epoch. Product hard delete retains final empty membership when needed.

The Product crate does not read Channel tables and has no `rustok-channel` or `rustok-index`
dependency.

## Durable freshness witness

Membership and freshness are deliberately separate.

`product_sales_channel_index_relation_freshness_snapshots` records that one retained relation epoch
was verified against:

- an observed Product `index_revision`;
- the canonical Product channel-visibility key;
- the tenant Channel identity generation.

The witness ledger is Product-owned, append-only, and prevents numeric watermark regression. A
freshness-only owner change can advance the witness without pretending that relation membership
changed.

Detailed owner contract:
[Product-SalesChannel freshness witness](../../rustok-product/docs/index-sales-channel-relation-freshness.md).

## Channel identity watermark

`rustok-channel` owns `channel_index_identity_generations`. The tenant generation advances in the same
transaction as Channel identity changes that can alter Product resolution: insert, delete, id change,
tenant move, and canonical slug change. It does not advance for `is_active` or unrelated Channel
configuration.

This makes Channel-side relation freshness observable without coupling Product storage to Channel
tables.

## Cross-owner resolution

The distribution resolver reads Product visibility, Product revision, Channel identity generation,
and current Channel UUID membership in a read-only repeatable-read snapshot. It writes membership
through the Product relation owner, re-observes current owner state, requires the second UUID set to
equal retained membership, and records the second observation as the freshness witness.

Resolver bounds remain 1024 visibility slugs, 1024 resolved Channel UUIDs, 64 Products per tenant page,
and three stabilization attempts. `channels.is_active` is not relation identity membership.

## Complete Product graph clock

`product_index_graph_projection_snapshots.projection_epoch` remains the only full Product mutation
`source_version`. Product revision and relation epoch are independent component clocks.

The canonical Product source joins the exact relation row referenced by projection state and emits:

- `sales_channel_ids`;
- the many-cardinality `sales_channels` `IndexLink`.

For live Product replay it additionally requires a current freshness witness for that exact relation
epoch. The current canonical Product visibility key and tenant Channel identity generation must equal
the witness. Product locale absence uses the same gate.

Hard-delete replay does not require a live freshness witness because it removes the Product graph.

## Production admission status

1. Durable relation membership storage: source complete.
2. Bounded cross-owner membership resolution: source complete.
3. Product-owned freshness witness ledger: source complete.
4. Tenant-scoped Channel identity generation: source complete.
5. Canonical Product replay/absence freshness gate: source complete.
6. Complete Product graph projection clock and SalesChannel link: source complete.
7. PostgreSQL concurrency/restart/delete-recreate/freshness evidence: pending.
8. Automatic owner-change scheduling/triggering policy, if required for convergence latency: pending.
9. Product typed event route/consumer after event-contract digest admission: pending.
10. Storefront/production cutover and query equivalence: pending.

## Maintainer validation

```bash
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-graph-projection-ledger.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-channel --all-targets
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
