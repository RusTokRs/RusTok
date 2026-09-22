# Product materialized/query freshness continuation — 2026-08-07

Status overlay: `source_complete_runtime_evidence_pending`.

This continuation advances the current M7 Product/ProductVariant/SalesChannel plan after automatic
Product-to-SalesChannel convergence. The canonical current plan remains the broader M5/M6/M7 execution
ledger; this document records the source slice that closes its previously open Product root
materialized/query freshness item.

## Source-complete in this continuation

- generic trusted PostgreSQL root-query admission contract keyed by exact `SchemaRef`;
- one admission owner per schema and immutable query-runtime snapshot composition;
- fail-closed compiler root-anchor matching;
- admission before user filter/cursor/order/limit;
- the same admission predicate in page and exact-count SQL;
- canonical Product materialized freshness predicate using existing projection/freshness/convergence
  evidence;
- immediate Product/locale delete exclusion from Product root queries;
- visibility-change exclusion through retained Product convergence request watermarks;
- Channel identity exclusion through current tenant generation comparison;
- Product scalar/Variant exclusion through current projection epoch/Product component comparison;
- no new Product schema, relation membership copy, Index storage column, or typed event family.

## M7 Product root query freshness status

- [x] Source replay fails closed on stale Product relation freshness.
- [x] Automatic bounded Product visibility / Channel identity relation convergence.
- [x] Rejected Product poison isolation without making rejected Products source-admissible.
- [x] Root Product materialized/query freshness fence for source-read -> mutation-apply races.
- [ ] Execute PostgreSQL page/filter/order/cursor/exact-count race evidence.
- [ ] Execute automatic-convergence multi-host/restart/rejected-Product evidence.
- [ ] Prove linked SalesChannel/ProductVariant target availability and complete query equivalence.
- [ ] Admit canonical Product typed events/routes only after event-contract digest verification.
- [ ] Move Storefront traffic only after readiness, equivalence, convergence, and query-freshness
      evidence passes.

## Admission boundary after this slice

A Product root row that is stale relative to current owner Product/locale/projection/visibility/Channel
identity facts is removed from the root SQL relation before user filtering, ordering, cursor pagination,
limit, and exact count. A previously valid but later-applied stale Product mutation therefore cannot
become query-authoritative merely because it reached `index_entities` after the owner changed.

The fence does not recursively prove freshness of every linked target row. Product root relation
membership is current when admitted, but a missing/stale materialized SalesChannel or ProductVariant
target can still affect joins/projections. That remains part of Product/Variant/Channel query-equivalence
and linked-target evidence rather than being conflated with Product root freshness.

## Current execution cursor

Primary M6 owner cursor remains unchanged: execute and admit the locked concrete repair PostgreSQL
packet.

For M7, the next useful source/evidence slice is a retained PostgreSQL packet that proves the new
Product root admission across:

- a source-read -> Channel-change -> stale-mutation-apply race;
- same-membership Channel generation refresh;
- changed-membership projection advancement;
- Product visibility change and rejected/corrected visibility;
- Product scalar/Variant projection advancement;
- Product and locale deletion/recreation;
- filter/order/cursor/limit/exact-count exclusion;
- convergence lease expiry/restart/multi-host contention.

Product typed event work remains separately blocked on canonical event-contract digest admission.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-index-product-materialized-query-freshness.mjs
node scripts/verify/verify-index-query-runtime-composition.mjs
node scripts/verify/verify-index-product-channel-relation-convergence.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, PostgreSQL scenarios, workflows, or CI were executed
by the implementation agent.
