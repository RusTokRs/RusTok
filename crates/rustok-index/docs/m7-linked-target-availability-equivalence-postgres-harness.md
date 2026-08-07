# M7 linked-target availability equivalence PostgreSQL harness

Status: `source_ready_execution_pending`.

## Purpose

`product_linked_target_availability_equivalence_postgres.rs` retains the missing query-equivalence
packet for the Product graph availability policy introduced by #3186.

The packet does not change production code. It proves, on real PostgreSQL migrations and the canonical
query runtime, that a stale linked target excludes only the Product root that depends on that target and
that the same boundary applies before linked filtering, many aggregate ordering, pagination, and exact
count.

It also recomposes a fresh query runtime on a new PostgreSQL session while a target is stale and proves
that recovery requires only the current target mutation, not process-local availability state.

## Fixture topology

One tenant owns two independent graph roots:

- Product A -> Variant A -> SalesChannel `alpha`;
- Product B -> Variant B -> SalesChannel `beta`.

Product channel visibility is restricted through canonical owner metadata so Product A links only to
`alpha` and Product B only to `beta`. The initial generic Product/Channel convergence scheduler is run to
idle before Index materialization.

The packet then materializes both current Products, both current ProductVariants, and both current
SalesChannels through the real source registry and generic `PostgresMutationStore`.

The two-root topology is intentional: when Product A has an unavailable target, Product B must remain in
the result. A whole-query failure or whole-query empty result would not satisfy the contract.

## ProductVariant availability scenario

Variant A starts with SKU `middle-stale-sku`; Variant B uses `zulu-stable-sku`.

The packet first proves baseline filter and `MIN(variants.sku)` ordering return both Products with exact
count 2. Then it updates only Variant A SKU to `alpha-current-sku`.

Existing owner migrations provide the isolation required by the scenario:

- `m20260730_000002_add_product_variant_index_revision` increments Variant A `index_revision` on every
  update;
- `m20260731_000003_bump_product_index_revision_for_variant_membership` bumps Product revision only for
  Variant identity/membership changes (`id`, `tenant_id`, `product_id`), not SKU changes.

Therefore Product A remains owner-current while its materialized Variant target becomes stale.

Before applying the current Variant mutation, the packet requires:

- the old Variant source version is still physically present in `index_entities`;
- linked `IN` filtering that would otherwise match the stale old SKU returns only Product B;
- `MIN(variants.sku)` ordering returns only Product B;
- exact count is 1 on both query shapes.

A fresh `SharedIndexQueryRuntime` is then recomposed on a new PostgreSQL session while the same stale
row remains materialized. It must produce the same Product-B-only filter/order results and exact count.
This proves the authority boundary is reconstructed from durable Index/owner state rather than an
in-memory latch.

After only the current Variant mutation is applied, both the original and restarted query runtimes see
Product A again. The current-SKU linked filter returns both Products with exact count 2, and aggregate
ordering is `Product A, Product B` because `alpha-current-sku < zulu-stable-sku`.

## SalesChannel availability scenario

Channel A starts with name `Middle stale channel`; Channel B uses `Zulu stable channel`.

The packet updates only Channel A name to `Alpha current channel`.

Two existing Channel clocks are deliberately separated:

- `m20260730_000010_add_channel_index_revision` increments Channel A `index_revision` on the name update;
- `m20260807_000012_add_channel_index_identity_generation` observes only insert/delete or updates of
  `id`, `tenant_id`, or `slug`, so a name-only update must leave tenant Channel identity generation
  unchanged.

The packet explicitly checks that identity generation does not change. Product relation/projection
freshness therefore remains authoritative; only the linked SalesChannel target is stale.

Before applying the current Channel mutation, linked filtering on the stale old Channel name and
`MIN(sales_channels.name)` ordering must return only Product B with exact count 1. After applying only
the current SalesChannel mutation, the current-name filter and aggregate ordering return both Products
with exact count 2 and order `Product A, Product B`.

## Contracts exercised

The packet uses:

- real Channel, Product, and Index migrations;
- canonical selected Index + Channel + Product distribution composition;
- persisted tenant schema registration;
- generic Product/Channel ModuleWork convergence for initial relation state;
- canonical Product, ProductVariant, and SalesChannel source adapters;
- generic `PostgresMutationStore`;
- canonical `SharedIndexQueryRuntime`;
- linked `FilterExpr::In` many traversal;
- `OrderDirection::MinAsc` many aggregate ordering;
- page filtering and exact-count recompilation under the same availability predicate;
- fresh query-runtime composition on a new PostgreSQL connection while target materialization is stale.

The harness does not call private resolver/query implementations directly and does not create or alter
Index storage tables itself.

## Remaining evidence boundary

This packet is source-ready but unexecuted. It covers target-only update lag, filter/order/count parity,
and runtime recomposition/recovery. Remaining M7 evidence is primarily:

- execute/admit this packet and the other retained Product packets;
- retain any explicit replay-worker/crash-redelivery ordering evidence still required by M5/M6;
- finish canonical Product typed event admission after event-contract digest verification;
- complete Storefront query parity before cutover.

## Maintainer verification

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-distribution --features mod-product \
  --test product_linked_target_availability_equivalence_postgres -- --nocapture
node scripts/verify/verify-index-linked-target-availability-equivalence-postgres-harness.mjs
node scripts/verify/verify-index-link-target-availability.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
