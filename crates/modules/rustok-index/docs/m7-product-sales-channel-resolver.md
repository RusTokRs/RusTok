# M7 Product-to-SalesChannel cross-owner resolver

Status: `automatic_convergence_and_query_fence_source_complete_runtime_evidence_pending`.

## Current contract

`rustok-distribution::product_index::channel_relation_resolver` is the selected cross-owner boundary
for Product visibility to SalesChannel UUID membership. The resolver itself does not write Index
tables, publish broker events, mutate Product metadata, or own a background loop. Automatic invocation
is composed separately through the generic ModuleWork runtime.

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

Resolution has two Product-owned durable outputs with different semantics:

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

## Automatic convergence composition

`rustok-distribution::product_index::channel_relation_convergence` wraps this resolver in one generic
`ModuleWorkRegistration` when both Product and Channel are selected.

Product visibility changes append exact Product-owned requests. Channel identity changes are detected
by comparing current Channel generation with a Product-owned tenant checkpoint. A Channel-generation
pass walks the tenant with the resolver's existing 64-Product keyset page. Claims, lease expiry, retry
availability, visibility cursor, sweep generation, and sweep cursor are durable Product-owned state.

The resolver remains a bounded primitive: it does not gain `tokio::spawn`, sleep/retry loops, broker
state, or cross-owner storage writes beyond the existing Product-owned relation/freshness stores.

Detailed convergence contract:
[Product-to-SalesChannel automatic convergence](./m7-product-sales-channel-convergence.md).

## Replay and query boundary

The canonical Product Index source materializes the `sales_channels` link. Live replay is fail-closed
unless the latest freshness witness for the projection's exact relation epoch matches current canonical
Product visibility and current tenant Channel identity generation. Product locale absence uses the same
freshness gate. Product hard-delete replay remains independent of a live witness because it removes the
graph.

Automatic convergence now re-establishes stale/missing relation freshness without a manual caller.
The materialized/query freshness fence separately closes the in-flight source-read -> mutation-apply
authority gap: a Product mutation produced before a later owner change may still be physically applied,
but the Product root row is query-inadmissible until materialized `projection_epoch`, current Product
revision, live locale identity, current freshness witness, Channel generation, and visibility-request
watermark agree.

The first retained PostgreSQL packet for delayed Product scalar mutation and locale deletion is
source-ready and execution-pending. Visibility/Channel-generation races plus multi-host/restart
convergence evidence remain the next packet.

## Still open

- execute/admit retained PostgreSQL delayed-mutation query-freshness evidence;
- Product visibility + Channel-generation convergence evidence for unchanged/changed membership;
- multi-host lease/restart/delete-recreate/rejected-Product convergence evidence;
- Product typed event routes after event-contract digest admission;
- complete Product/Variant/Channel query equivalence and Storefront cutover evidence.

## Maintainer verification

```bash
cargo test -p rustok-distribution --features mod-product --test product_materialized_query_freshness_postgres -- --nocapture
node scripts/verify/verify-index-product-materialized-query-freshness-postgres-harness.mjs
node scripts/verify/verify-index-product-materialized-query-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-convergence.mjs
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-channel --all-targets
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
