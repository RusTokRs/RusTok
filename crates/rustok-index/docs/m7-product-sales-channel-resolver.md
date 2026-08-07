# M7 Product-to-SalesChannel cross-owner resolver

Status: `freshness_watermark_source_complete_runtime_evidence_pending`.

## Current contract

`rustok-distribution::product_index::channel_relation_resolver` is the selected cross-owner boundary
for Product visibility to SalesChannel UUID membership. It does not write Index tables, publish broker
events, mutate Product metadata, or own a background loop.

For one exact Product it observes in PostgreSQL `REPEATABLE READ`, `READ ONLY`:

- canonical Product channel visibility;
- current Product `index_revision`;
- current tenant Channel identity generation;
- the complete resolved Channel UUID membership.

Visibility policy remains:

- missing visibility or an empty allowlist is unrestricted;
- unrestricted means every current tenant Channel identity;
- restricted visibility resolves canonical `lower(btrim(slug))` membership;
- malformed/non-canonical/duplicate visibility fails closed;
- `channels.is_active` is not relation identity state.

The resolver is bounded to at most 1024 visibility slugs, 1024 resolved Channel UUIDs, 64 Products per
tenant page, and three stabilization attempts.

## Membership and freshness

Resolution now has two Product-owned durable outputs with different semantics:

1. `ProductSalesChannelIndexRelationStore::replace` owns the complete UUID membership and advances
   `relation_epoch` only when that membership changes.
2. `ProductSalesChannelIndexRelationFreshnessStore::record` owns an append-only witness that the exact
   retained relation epoch was checked against current Product visibility and current tenant Channel
   identity generation.

After the relation write, the resolver opens a fresh repeatable-read observation. It accepts only when
the newly observed UUID set equals the membership retained by the relation owner, then records that
second observation as the freshness witness. If numeric freshness watermarks race backwards relative to
an already retained witness, the resolver retries within the same three-attempt bound.

A freshness-only owner change does not fabricate a new relation epoch. For example, a Channel slug
change can advance Channel identity generation while leaving an unrestricted Product's UUID membership
unchanged; only the freshness witness advances.

## Channel identity generation

`rustok-channel` owns `channel_index_identity_generations`, a durable tenant-scoped generation updated
transactionally by Channel insert/delete/id/tenant/canonical-slug changes. Unrelated Channel state such
as `is_active`, targets, OAuth configuration, or resolution policies does not invalidate Product graph
membership.

A tenant with no historical Channel identity has generation `0`. Once the first Channel identity
mutation occurs, the positive generation is retained even if all Channels are later deleted.

## Replay boundary

The canonical Product Index source already materializes the `sales_channels` link. Live replay is now
fail-closed unless the latest freshness witness for the projection's exact relation epoch matches:

- current canonical Product visibility; and
- current tenant Channel identity generation.

Product locale absence uses the same freshness gate. Product hard-delete replay remains independent of
a live witness because it removes the graph.

This is an admitted source-level freshness watermark, not an automatic convergence scheduler. Owner
changes intentionally make live Product replay unavailable until an exact reconciliation or bounded
tenant sweep records a current witness.

## Still open

- retained PostgreSQL concurrency/restart/delete-recreate/freshness evidence;
- host scheduling or owner-triggered reconciliation if automatic convergence latency is required;
- Product typed event routes after event-contract digest admission;
- complete Product/Variant/Channel query equivalence and Storefront cutover evidence.

## Maintainer verification

```bash
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-channel --all-targets
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
