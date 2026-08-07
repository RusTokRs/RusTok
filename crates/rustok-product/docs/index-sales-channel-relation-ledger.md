# Product-SalesChannel Index relation owner ledger

Status: `canonical_graph_and_freshness_source_complete_runtime_evidence_pending`.

## Membership authority

`product_sales_channel_index_relation_snapshots` is the Product-owned durable authority for resolved
Product-to-SalesChannel UUID membership.

Each append-only row contains a positive tenant-local sequence, exact tenant/Product identity,
contiguous positive `relation_epoch`, and the complete canonical Channel UUID set. Membership is
bounded to 1024 non-nil sorted unique UUIDs. Empty membership is valid.

Equal membership is idempotent and does not advance `relation_epoch`. That remains true even when
freshness evidence changes.

`ProductSalesChannelIndexRelationStore::replace` requires the exact live Product under `FOR KEY SHARE`,
serializes the relation identity with an advisory transaction lock, and appends only when membership
changes. Product hard delete follows the same lock order and retains final empty membership when
needed.

The Product owner also exposes bounded relation change/current/targeted reads.

## Freshness is separate

`product_sales_channel_index_relation_freshness_snapshots` is a separate Product-owned append-only
witness ledger. It records that one retained `relation_epoch` was verified against an observed Product
source version, canonical Product visibility key, and tenant Channel identity generation.

This separation prevents a technical freshness refresh from fabricating a graph membership change.
When current owner inputs change but the resolved UUID set stays identical, relation epoch stays fixed
and only the freshness witness advances.

Detailed freshness contract:
`crates/rustok-product/docs/index-sales-channel-relation-freshness.md`.

## Cross-owner composition

`rustok-distribution::product_index::channel_relation_resolver` owns Product visibility to current
Channel UUID resolution. It writes membership through `ProductSalesChannelIndexRelationStore`, then
after a fresh repeatable-read observation writes the freshness witness through
`ProductSalesChannelIndexRelationFreshnessStore`.

Unrestricted visibility resolves against the whole current tenant Channel identity set. Restricted
visibility matches canonical `lower(btrim(slug))`; `is_active` does not alter identity membership.

The Product crate does not read Channel tables and has no `rustok-channel` or `rustok-index`
dependency.

## Canonical Product graph

`product_index_graph_projection_snapshots.projection_epoch` remains the complete Product record clock.
The canonical Product source uses the projection's exact relation epoch to materialize
`sales_channel_ids` and the `sales_channels` link.

Live replay additionally requires a current relation freshness witness. Product hard-delete replay
does not, because it removes graph membership.

## Still open

- retained PostgreSQL concurrency/restart/delete-recreate/freshness evidence;
- automatic owner-change convergence scheduling if required;
- Product typed event delivery after event-contract digest admission;
- query equivalence and Storefront/production Index cutover.

## Maintainer verification

```bash
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
