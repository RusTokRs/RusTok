# M7 Product-to-SalesChannel cross-owner resolver

Status: `source_complete_durable_triggering_and_runtime_evidence_pending`.

## Purpose

The Product owner persists a dedicated monotonic Product-to-SalesChannel relation epoch but
intentionally does not read Channel storage. `rustok-distribution` is the selected composition boundary
that can resolve Product visibility metadata against current tenant SalesChannel identities.

`ProductSalesChannelRelationResolver` reads Product visibility and current Channel identities, then
submits only the complete resolved UUID membership to
`ProductSalesChannelIndexRelationStore::replace`. It does not write Index rows, mutate Product
metadata, publish events, or own a background loop.

The canonical Product Index source already consumes the resulting relation through
`product_index_graph_projection_snapshots` and materializes the `sales_channels` link. The resolver is
therefore no longer waiting on a future Product schema; its remaining gap is durable convergence.

## Visibility policy

- missing `metadata.channel_visibility` means unrestricted visibility;
- an empty canonical `allowed_channel_slugs` array also means unrestricted visibility;
- unrestricted visibility resolves to every current tenant Channel identity;
- a non-empty allowlist resolves Channel UUIDs by canonical `lower(btrim(slug))` membership;
- malformed, non-canonical, duplicate, or non-string visibility fails closed;
- deleted Channel identities disappear from the next resolved set;
- an unresolved restricted slug contributes no target until a matching Channel exists.

The resolver deliberately does **not** filter `channels.is_active`. Relation membership represents
identity resolution; Channel runtime availability remains Channel-owned.

## Bounded contract

- at most 1024 canonical visibility slugs;
- at most 1024 resolved Channel UUID targets;
- at most 64 Products in one tenant convergence page;
- at most three exact Product stabilization attempts.

Tenant sweep enumeration uses stable Product UUID keyset ordering with one-row lookahead.

## Cross-owner consistency

For one Product, resolver stabilization is:

1. read Product visibility plus resolved Channel IDs in PostgreSQL `REPEATABLE READ`, `READ ONLY`;
2. commit the observation;
3. call the Product-owned relation writer;
4. observe the same inputs again in a fresh read-only repeatable-read transaction;
5. accept only if UUID membership is unchanged;
6. retry at most three times, then return `ConcurrentChange`.

Product hard deletion remains fenced by the owner writer's live-row lock. A Product that disappears
during reconciliation returns `ProductNotFound` rather than recreating relation state.

This is not an atomic cross-owner snapshot, durable watermark, checkpoint, or event acknowledgement.

## Tenant convergence page

`reconcile_tenant_page` reconciles at most 64 current Products in UUID order. A partial page can be
retried from the same input cursor because unchanged owner membership is idempotent. Products created
behind an already-consumed cursor still require durable owner triggering or a later sweep.

## Module boundary

Channel SQL exists only in `rustok-distribution`. `rustok-product` stays independent from
`rustok-channel` and `rustok-index` and accepts only resolved UUID membership.

The canonical Product Index source does not query `channels`; it reads only Product-owned relation and
projection state. This keeps replay deterministic over retained owner facts.

## Still open

- durable Product-visibility and Channel-identity triggers or an admitted relation freshness
  watermark/checkpoint;
- host retry/lease/checkpoint composition for convergence if event-driven triggering is not used;
- retained PostgreSQL concurrency/restart/delete-recreate/out-of-order evidence;
- Storefront or production Index cutover.

Incremental typed Product event wiring remains separately gated on event-contract digest admission.

## Maintainer verification

```bash
cargo test -p rustok-distribution product_sales_channel -- --nocapture
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-admission.mjs
node scripts/verify/verify-index-product-source.mjs
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
