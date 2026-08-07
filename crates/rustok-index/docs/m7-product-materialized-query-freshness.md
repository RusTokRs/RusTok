# M7 Product materialized/query freshness fence

Status: `source_complete_link_target_availability_execution_pending`.

## Query freshness boundary

Product source admission and automatic Product-to-SalesChannel convergence protect source reads, but an
already-produced Index mutation can still arrive after owner state changes. A post-result check is too
late because stale root or linked rows may already have affected filtering, ordering, cursor pagination,
nested projection, aggregate ordering, and exact count.

`rustok-index` therefore owns two module-neutral query admission layers:

1. `PostgresQueryEntityAdmission` fences every compiler-owned materialized `index_entities` relation
   against owner freshness;
2. schema-scoped linked-target availability requires a current materialized+owner-admitted target when
   a validated query actually traverses a link owned by a schema that selected the policy.

Neither layer adds a Product schema, Product relation copy, or new freshness clock.

## Entity freshness admission

Current compiler aliases covered by `PostgresQueryEntityAdmission` are:

- `tN` for root and ordinary joins;
- `mpN_tN` for many projections;
- `mx_tN` for many-filter `EXISTS` paths;
- `mo_tN` for many aggregate-order paths.

The admission changes no user binds, selected columns, plan fingerprint, or cursor contract. Unknown
compiler alias families and missing canonical `is_deleted = FALSE` anchors fail closed.

Current Product graph owner rules are:

- Product: current projection/revision/relation freshness/Channel generation/visibility request/locale;
- ProductVariant: live same tenant/UUID with `product_variants.index_revision == source_version`;
- SalesChannel: live same tenant/UUID with `channels.index_revision == source_version`.

## Query-path-scoped linked-target availability

Product registers exactly one generic link-target availability policy for its current Product
`SchemaRef`.

The Index runtime derives the first-hop link names actually referenced by the validated `IndexQuery`
across selected fields, filters, and ordering. When no Product link path is referenced, no availability
predicate is added, so scalar-only Product queries do not become dependent on unrelated Variant or
SalesChannel materialization.

When one or more Product links are referenced, the query port injects the same root predicate into page
SQL and exact-count SQL before entity admission:

- inspect only `index_links` rows whose source identity **and source_version** match the current
  materialized Product row;
- inspect only link names referenced by this query;
- require every such current link row to have a matching live `index_entities` target;
- require that target to pass the same owner freshness dispatcher used by ordinary materialized target
  aliases;
- if any current queried link lacks a live owner-admitted target, exclude the Product root.

The predicate is owned entirely by generic Index storage/runtime. Product owner admission still does
not read `index_links` or `index_entities`, and the generic compiler contains no Product/Variant/Channel
branch.

This establishes the semantic distinction required for Product graph authority:

- no current link row for the queried link = authoritative absent relation;
- current link row + current admitted target = authoritative linked relation;
- current link row + missing/stale/deleted target = query fails closed for that Product root, **not**
  authoritative null/empty linked data.

The policy is intentionally query-path scoped and one-hop. Current Product targets are ProductVariant
and SalesChannel, both link-free in the canonical graph, so recursive availability SQL is unnecessary.

## Recreate monotonicity remains source complete

ProductVariant and SalesChannel do not need another recreate clock.

Product migration `m20260731_000004_add_product_index_tombstones` and Channel migration
`m20260731_000011_add_channel_index_tombstones` already retain delete source versions at
`OLD.index_revision + 1`, seed same-UUID recreations above the retained tombstone, reject exhaustion,
and clear tombstones only after strict live supersession.

Therefore an old materialized ProductVariant/SalesChannel source version cannot collide with the
recreated incarnation's current owner revision.

## Updated linked-target PostgreSQL packet

`product_linked_target_recreate_postgres.rs` remains source-ready and execution-pending, now with the
canonical availability semantics.

For ProductVariant recreate, the old target row remains physically materialized while Product is
refreshed to its current projection. A scalar-only Product query remains visible, but a graph query that
requests `variants` fails closed with zero rows/exact count until only the current ProductVariant
mutation is applied; then the recreated SKU appears.

For SalesChannel recreate, same UUID/canonical slug is restored before relation convergence. After
freshness-only convergence Product `relation_epoch`, `projection_epoch`, and materialized Product source
version remain unchanged. Scalar-only Product query becomes visible again, but a graph query requesting
`sales_channels` stays fail closed while the old physical Channel target is stale. Applying only the
current SalesChannel mutation restores the recreated Channel name.

This proves Product owner authority and linked-target authority are separate without interpreting target
unavailability as empty relation state.

## Retained M7 PostgreSQL packets

Four packets are source-ready and execution/admission pending:

1. `product_materialized_query_freshness_postgres.rs`;
2. `product_channel_convergence_postgres.rs`;
3. `product_channel_identity_transitions_postgres.rs`;
4. `product_linked_target_recreate_postgres.rs`.

None has been executed or admitted by the implementation agent.

## Remaining M7 evidence

The source-level availability policy is complete. Before Storefront cutover the remaining linked-target
work is evidence/equivalence rather than another runtime clock:

- retain PostgreSQL cases for linked filtering and many aggregate ordering while target materialization
  is unavailable/current;
- retain restart/replay ordering around link-present/target-missing recovery;
- execute/admit the existing linked-target recreate packet;
- complete Product/ProductVariant/SalesChannel query equivalence across projection/filter/order/count;
- execute/admit the other Product freshness/convergence packets;
- admit canonical Product typed events only after event-contract digest verification.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-distribution --features mod-product \
  --test product_linked_target_recreate_postgres -- --nocapture
node scripts/verify/verify-index-link-target-availability.mjs
node scripts/verify/verify-index-linked-target-recreate-postgres-harness.mjs
node scripts/verify/verify-index-linked-target-query-freshness.mjs
node scripts/verify/verify-index-product-materialized-query-freshness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
