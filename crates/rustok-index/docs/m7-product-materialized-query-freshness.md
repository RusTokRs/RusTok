# M7 Product materialized/query freshness fence

Status: `source_complete_runtime_evidence_pending`.

## Problem closed by this slice

Product source admission already rejects stale Product-to-SalesChannel relation state, and automatic
convergence re-establishes owner freshness after Product visibility or Channel identity changes. Those
contracts do not by themselves protect an Index mutation that was produced under a valid source
snapshot and then applied after owner state changed.

The remaining window is:

1. Product source reads a valid Product projection and freshness witness;
2. owner Product/Channel state changes;
3. the already-produced mutation is applied to `index_entities/index_links`;
4. an Index query executes before the corrective mutation arrives.

A post-result freshness check is insufficient because a stale row can already have changed filtering,
ordering, cursor pagination, and exact count before the check sees returned rows.

## Generic root query admission

`rustok-index` now owns a trusted schema-scoped PostgreSQL root-admission contract.

`PostgresQueryRootAdmission` accepts a source-code predicate template that:

- must reference the compiler-controlled `{{root}}` alias;
- cannot contain bind placeholders, SQL statement boundaries, or comments;
- is bounded to 32 KiB;
- is applied to the compiler's exact canonical root `index_entities` baseline;
- fails closed if page or exact-count SQL no longer contains exactly one expected root anchor.

The admission predicate changes no user binds, selected columns, query fingerprint, or cursor contract.
It is injected into root `WHERE` before user filter/cursor/order/limit semantics. The same predicate is
injected into exact-count SQL.

`PostgresIndexQueryAdmissionCatalog` owns at most one rule per exact `SchemaRef`. Query runtime
materialization snapshots that catalog into `PostgresIndexQueryPort`; schemas without a rule retain the
existing generic behavior.

## Product admission evidence

The selected distribution registers one rule for the canonical Product schema. A materialized Product
root row is query-admissible only when all of these facts are true in the same PostgreSQL
`REPEATABLE READ`, `READ ONLY` query snapshot:

- the owner Product still exists;
- the exact Product locale translation still exists;
- the materialized `index_entities.source_version` equals the latest Product
  `product_index_graph_projection_snapshots.projection_epoch`;
- that projection's Product component equals current `products.index_revision`;
- a freshness witness exists for the projection's exact `relation_epoch`;
- the witness Product revision is not ahead of current Product revision;
- witness Channel identity generation equals current tenant `channel_index_identity_generations`
  (or generation `0` when no Channel generation row exists);
- there is no retained Product visibility convergence request with
  `product_source_version > witness.product_source_version`.

The last condition is the current-visibility proof. Product convergence requests are appended only for
Product INSERT or canonical `channel_visibility` changes. Therefore an unrelated Product scalar update
does not falsely invalidate relation freshness, while any later visibility change makes an older
witness query-inadmissible without re-parsing Product visibility JSON in SQL.

## Why no extra materialized watermark is needed

The existing clocks already carry the required evidence:

- `index_entities.source_version` is Product `projection_epoch`;
- Product projection state identifies current Product and relation components;
- the freshness witness identifies the observed Product revision and Channel generation;
- the convergence request ledger identifies visibility-affecting Product revisions;
- live Product/translation rows prove the materialized identity/locale still exists.

A new Product schema, duplicate relation membership, query-time visibility parser, or new Index storage
column would add another authority without improving the proof.

## Important transitions

### Product scalar/Variant change

Product `index_revision` and projection advance. An older materialized row fails the projection epoch
comparison until the new Product mutation is applied.

### Product visibility change

The Product update advances projection and appends an exact visibility convergence request. An older
witness cannot satisfy the request-watermark check. The row remains query-inadmissible until resolver
freshness is current and the new Product projection is materialized.

### Channel identity change

Current tenant Channel generation immediately differs from the old witness, so Product rows are
query-inadmissible even if an old already-produced mutation applies afterward. Automatic convergence
then refreshes witness/membership.

If resolved UUID membership is unchanged, `relation_epoch`/`projection_epoch` need not move; updating
the freshness witness to the current Channel generation is enough to make the unchanged materialized
Product row query-admissible again.

If membership changes, Product `relation_epoch` and `projection_epoch` advance; the old materialized
row remains inadmissible until the new Product mutation is applied.

### Product/locale delete

A stale materialized Product row fails immediately because the live Product or exact translation no
longer exists, even before the retained delete mutation reaches Index storage.

### Rejected Product owner data

A rejected Product remains individually fail-closed. Its retained visibility request stays newer than
its last valid freshness witness, so query admission excludes it without blocking unrelated valid
Products in the same tenant.

## Scope and remaining admission

This closes the source-read -> mutation-apply **Product root materialized/query freshness** gap at query
admission source level. It does not claim successful PostgreSQL execution, latency, query parity, or
Storefront production readiness.

Still required before cutover:

- retained PostgreSQL evidence proving page/filter/order/cursor/exact-count exclusion across the owner
  race window;
- automatic-convergence multi-host/restart/rejected-Product evidence;
- complete Product/Variant/Channel query equivalence, including linked target availability behavior;
- canonical Product typed event admission after event-contract digest verification;
- Storefront cutover evidence after schema readiness, equivalence, convergence, and query-freshness
  packets pass.

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
