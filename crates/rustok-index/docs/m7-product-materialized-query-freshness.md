# M7 Product materialized/query freshness fence

Status: `source_complete_linked_target_fence_execution_pending`.

## Problem closed at source level

Product source admission and automatic Product-to-SalesChannel convergence protect source reads, but an
already-produced Index mutation can still arrive after owner state changes. A post-result freshness
check is too late because stale root or linked rows may already have affected filtering, ordering,
cursor pagination, many projections, aggregate ordering, and exact count.

The query boundary therefore treats owner freshness as an admission property of every materialized
`index_entities` relation used by the compiler, not only the root row.

## Generic entity query admission

`rustok-index` owns `PostgresQueryEntityAdmission`, a trusted schema-scoped PostgreSQL entity-admission
contract.

A rule:

- must reference the compiler-controlled `{{entity}}` alias;
- cannot contain bind placeholders, SQL statement boundaries, or comments;
- is bounded to 32 KiB;
- changes no user bind, selected column, plan fingerprint, or cursor contract;
- is applied to every compiler-owned materialized entity alias in page and exact-count SQL;
- fails closed when the compiler emits an unknown entity alias family or omits the canonical
  `is_deleted = FALSE` entity anchor.

Current compiler alias families covered by the admission boundary are:

- `tN` for root and ordinary outer-join entity relations;
- `mpN_tN` for many-cardinality projections;
- `mx_tN` for many-cardinality filter `EXISTS` relations;
- `mo_tN` for many-cardinality aggregate-order relations.

`PostgresIndexQueryAdmissionCatalog` still owns at most one owner rule per exact `SchemaRef`. At runtime
those owner rules are compiled into one schema-dispatch predicate. If at least one owner rule exists,
the immutable runtime adds local pass-through descriptors for otherwise ungoverned registered root
schemas. Those pass-through descriptors are not published owner rules; they only ensure that a query
rooted elsewhere still applies governed freshness rules to linked targets.

## Product graph owner rules

The selected Product distribution registers current entity admission for Product and ProductVariant.
When Channel is selected it also registers SalesChannel admission. No new Index schema or freshness
clock is introduced.

### Product

A Product materialized row is admitted only when, in the same PostgreSQL `REPEATABLE READ, READ ONLY`
query snapshot:

- the owner Product and exact locale translation still exist;
- materialized `source_version` equals current Product `projection_epoch`;
- the projection Product component equals current `products.index_revision`;
- an exact relation freshness witness exists;
- witness Channel generation equals current tenant Channel generation;
- no visibility convergence request is newer than the witness Product revision.

This is the existing Product source-read -> mutation-apply fence, now reusable when Product appears as
any governed materialized entity relation.

### ProductVariant

A ProductVariant row is admitted only when a live owner `product_variants` row exists for the same
tenant/UUID and:

`product_variants.index_revision = index_entities.source_version`.

A delayed stale ProductVariant row therefore cannot contribute its old payload to Product `variants`
projection/filter/order semantics after the owner revision has advanced or the owner row has been
deleted.

### SalesChannel

A SalesChannel row is admitted only when a live owner `channels` row exists for the same tenant/UUID
and:

`channels.index_revision = index_entities.source_version`.

A stale/deleted SalesChannel row therefore cannot contribute its old payload to Product
`sales_channels` projection/filter/order semantics while the Product root itself remains current.

## Query surfaces fenced against stale target payloads

The same composite admission is applied before or inside every materialized target relation used by:

- root predicates and exact count;
- ordinary linked projection/filter/order joins;
- many-cardinality nested projections;
- many-cardinality `EXISTS` filters;
- many-cardinality `MIN`/`MAX` aggregate ordering;
- exact-count recompilation of the same plan.

This closes the stale-materialized-target participation path without making generic Index code
understand Product, ProductVariant, or SalesChannel owner storage.

It does **not** yet prove complete linked-target availability semantics. In particular, a source link
may exist while its target has not yet been materialized, and existing left-join / many-subquery
semantics can represent an unavailable target as a missing/null relation. Complete Product graph parity
still requires retained evidence and an explicit fail-closed policy for that availability window so a
missing target cannot be mistaken for authoritative owner null/absence semantics.

## Retained PostgreSQL packets

The existing retained packets remain execution-pending:

1. `product_materialized_query_freshness_postgres.rs` — delayed Product scalar mutation and locale
   deletion;
2. `product_channel_convergence_postgres.rs` — Product visibility / Channel identity convergence,
   lease reclaim, rejected Product isolation, changed and unchanged membership;
3. `product_channel_identity_transitions_postgres.rs` — Channel create/delete/tenant-move and
   delete+recreate Product relation/freshness behavior.

They have not been executed or admitted by the implementation agent.

## Explicit remaining recreate boundary

ProductVariant and SalesChannel currently use their live owner `index_revision` as source version. A
hard delete followed by recreation of the same UUID can reset that owner-local revision to an earlier
numeric value. Equality-based entity admission cannot distinguish a new incarnation when the recreated
row happens to reuse the same revision as an old materialized row.

Therefore this slice does **not** claim delete+recreate identity safety for ProductVariant or
SalesChannel target materialization. The next source slice must make those two owner source clocks
monotonic across retained tombstone/recreate history, while preserving the current ProductVariant and
SalesChannel SchemaRefs. Do not add another Product schema or compatibility version.

## Remaining M7 admission

Still required before Storefront cutover:

- implement recreate-safe monotonic ProductVariant and SalesChannel source clocks;
- define and retain fail-closed linked-target availability semantics for link-present / target-missing
  windows;
- retain PostgreSQL linked-target stale/delete/recreate/query evidence after those source changes;
- execute/admit the retained Product freshness/convergence/identity packets;
- complete Product/ProductVariant/SalesChannel query equivalence and linked-target availability proof;
- admit canonical Product typed events only after event-contract digest verification;
- pass schema readiness, equivalence, convergence, freshness, and restart evidence.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-index-product-materialized-query-freshness.mjs
node scripts/verify/verify-index-linked-target-query-freshness.mjs
node scripts/verify/verify-index-query-runtime-composition.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.