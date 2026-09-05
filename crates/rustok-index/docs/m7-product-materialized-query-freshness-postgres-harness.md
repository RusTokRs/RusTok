# M7 Product materialized/query freshness PostgreSQL harness

Status: `source_ready_execution_pending`.

## Purpose

This retained PostgreSQL packet exercises the source-read -> owner-change -> delayed-mutation-apply
window against the real Product replay source, Index mutation store, persisted schema readiness, and
canonical shared query runtime. It is intentionally separate from the Product-to-SalesChannel
convergence worker packet so a failure identifies either materialized query admission or convergence,
not a mixture of both mechanisms.

The packet is source-ready but has not been executed by the implementation agent.

## Production surfaces used

`crates/rustok-distribution/tests/product_materialized_query_freshness_postgres.rs` creates an isolated
PostgreSQL schema and uses:

- the real Product migration chain;
- the real Index migration chain;
- `rustok_distribution::build_runtime_extensions` with Index, Channel, and Product selected;
- the immutable source schema registry and `PostgresSchemaRegistrationStore` for tenant readiness;
- `materialize_postgres_index_sources` plus `SharedIndexSourceRegistry::load`;
- `PostgresMutationStore` to physically apply the delayed mutation;
- `materialize_postgres_index_query_runtime` and `SharedIndexQueryRuntime::execute_query`;
- the Product root query-admission rule introduced by the materialized freshness slice.

No fake Index source, fake query port, direct `index_entities` mutation, background task, or timing
sleep is used.

## Scenario 1: delayed scalar mutation

Three Product fixtures share tenant and locale scope. `A stale candidate` sorts before
`B fresh control`; a third Product is reserved for locale deletion.

1. The control Product is loaded from the real Product source and materialized normally.
2. The stale candidate is loaded from the real Product source, but its mutation is retained in memory.
3. The owner Product is updated through PostgreSQL (`vendor` change). The real Product revision and
   graph-projection triggers advance owner state.
4. The already-produced old mutation is then applied through `PostgresMutationStore`.
5. Direct read-only evidence confirms that the delayed old source version is physically present in `index_entities`.
6. A real Product Index query applies a title `IN` filter, title ascending order, cursor page size one,
   lookahead/limit, and exact count.

Expected retained assertion: only `B fresh control` is query-visible, exact count is one, and no
continuation cursor is emitted. Therefore the physically materialized stale A row cannot influence
filter/order/cursor/limit/exact-count semantics.

The packet then loads and applies the corrective current mutation. The same query must return A on
page one with exact count two and a continuation cursor; page two must return B with exact count two.
This proves normal cursor behavior is restored after current Product materialization is admitted.

## Scenario 2: locale deletion after source read

1. The locale-deletion Product is loaded from the real Product source while its `en` translation
   exists.
2. The owner translation is deleted before the retained upsert mutation is applied. The real Product
   translation trigger advances Product owner revision/projection state.
3. The old upsert is applied through `PostgresMutationStore` and is again confirmed physically present
   in `index_entities`.
4. A real Product query for that Product/locale must return zero rows and exact count zero.

This proves live locale ownership is part of admission: a stale upsert cannot become query-authoritative
while the retained Product delete mutation is still in transit.

## Deliberate scope boundary

This first packet proves the materialized stale-mutation mechanism for Product revision/projection
changes and live locale deletion. Channel-generation and visibility convergence races are deliberately
not folded into this file.

The next retained packet should cover:

- Product `channel_visibility` change after source read;
- Channel identity generation change with unchanged UUID membership;
- Channel identity change that produces new relation membership/projection epoch;
- multi-host lease competition and restart recovery for the convergence worker;
- rejected Product isolation while valid Products continue converging.

Keeping those cases together allows the evidence to prove the durable convergence state machine and
its query-admission interaction without weakening this packet into a broad scenario with ambiguous
failure ownership.

## Admission state

Source existence is not evidence admission. This packet remains `source_ready_execution_pending` until
a maintainer runs the PostgreSQL harness and retains the resulting execution evidence under the
repository's normal evidence/admission process.

Suggested maintainer commands, intentionally not run by the implementation agent:

```bash
cargo test -p rustok-distribution --features mod-product --test product_materialized_query_freshness_postgres -- --nocapture
node scripts/verify/verify-index-product-materialized-query-freshness-postgres-harness.mjs
node scripts/verify/verify-index-product-materialized-query-freshness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No test, Node verifier, Cargo check, formatting command, PostgreSQL scenario, workflow, or CI job was
run by the implementation agent.
