# M4 query planner actualization

Date: 2026-07-29

This note actualizes the live `rustok-index` implementation plan after rechecking the
M3 storage boundary and the M4 query slices merged into `main`.

## Recheck result

M3 source implementation remains complete through retained bundle review, archive
manifest generation, saved-manifest verification, and recursive filesystem drift
detection. The repository owner still needs to execute and admit one fresh real
PostgreSQL partition packet. That owner gate blocks production partition lifecycle
design but does not block database-independent M4 query source work.

## Actualized status

- M3 real retained PostgreSQL packet execution: `open_owner_action`.
- M3 production partition lifecycle: `blocked_by_retained_packet`.
- M4 deterministic executable query planning: `source_complete_execution_pending`.
- M4 stable relation aliases and typed referenced fields: `source_complete_execution_pending`.
- M4 root/one-link query compilation and result decoding: `source_complete_execution_pending`.
- M4 query-scoped cursors and lookahead pagination: `source_complete_execution_pending`.
- M4 many-link `EXISTS` filtering: `source_complete_execution_pending`.
- M4 nested many-link projection aggregation: `source_complete_execution_pending`.
- M4 PostgreSQL query port and strict row adapter: `source_complete_execution_pending`.
- M4 retained plan/SQL snapshots: `source_complete_owner_execution_pending`.
- M4 PostgreSQL/reference-engine equivalence: `open`.

## Executable plan v4

`SchemaRegistry::plan_query`:

1. validates the registry-backed query before planning;
2. collects and sorts every referenced link prefix;
3. assigns deterministic `t0`, `t1`, ... aliases;
4. resolves joins against registered schema contracts;
5. propagates `traverses_many` from the first many-cardinality link;
6. captures every referenced field with path, alias, type, cardinality, and nullability;
7. preserves public projection order;
8. groups projected many-traversing fields by terminal relation path into
   `PlannedManyProjection` contracts;
9. records every identity prefix required to reconstruct a complete nested relation
   chain;
10. retains filters, ordering, pagination, and exact-count intent.

The deterministic fingerprint domain is `rustok-index-query-plan-v4` because grouped
many-projection metadata is part of executable plan identity and compiler safety.

## Controlled PostgreSQL boundary

Root and explicit one-link projection, all validated filters, typed ordering, exact
count, keyset continuation, and bounded offset remain controlled SQL with ordered bind
DTOs. Many-link filters compile through independent correlated `EXISTS` chains.

Each `PlannedManyProjection` compiles as one correlated JSONB aggregate outside the
outer root rowset. Aggregate items preserve the complete linked entity identity chain
and aligned tagged field values. Stored link ordinal, target entity identity, and locale
produce deterministic item ordering. Missing reachable rows yield an empty array.

Because many projection does not enter the outer rowset, root pagination, one-row
lookahead, and exact count remain duplicate free.

## Result and execution handoff

`SchemaRegistry::compile_postgres_page_query` changes only the validated page-limit bind
from `N` to `N + 1`. `decode_postgres_query_page` re-plans and verifies:

- the v4 plan fingerprint;
- unique scalar and many-relation output aliases;
- exact scalar column and `CompiledManyRelationColumn` metadata;
- requested page size and maximum `N + 1` rows;
- tagged field type/cardinality/nullability;
- nested identity/value arity;
- non-nil and non-duplicate complete nested identity chains;
- optional exact count.

The lookahead row is removed. Cursor pages derive the next scoped cursor from the last
retained root/order tuple; offset pages report `has_more` without creating a cursor.

`PostgresIndexQueryPort` is now the Index-owned execution boundary. It verifies exact
active persisted schema contracts for the query tenant, converts every
`PostgresBindValue` variant, executes page and optional count in one read-only
repeatable-read PostgreSQL transaction, maps only compiler-declared aliases, and then
delegates semantic validation and cursor creation to the strict decoder.

## Retained snapshots

`query_snapshot_tests::retained_v4_plan_and_sql_snapshots_are_stable` compares a fixed
canonical query against three retained files:

- readable executable-plan metadata;
- the complete exact PostgreSQL SQL string;
- ordered bind, scalar-column, and many-relation metadata.

The fixture uses fixed identifiers and forbids automatic snapshot rewriting. SQL keeps
all contract values in `$N` binds.

## Remaining bounded M4 work

The combined roadmap item remains open until PostgreSQL/reference-engine equivalence is
implemented and executed. Additional boundaries remain:

- aggregate ordering semantics for paths traversing `many`;
- server/storefront/admin/search query-port composition and consumer cutover;
- live PostgreSQL/reference-engine fixtures and retained execution evidence.

The real retained PostgreSQL partition packet remains an independent owner gate for
production partition lifecycle work.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-index planner_tests -- --nocapture
cargo test -p rustok-index postgres_compiler_tests -- --nocapture
cargo test -p rustok-index postgres_many_projection_tests -- --nocapture
cargo test -p rustok-index postgres_query_result_tests -- --nocapture
cargo test -p rustok-index query_snapshot_tests -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-query-contract.mjs
node scripts/verify/verify-index-query-planner.mjs
node scripts/verify/verify-index-postgres-query-compiler.mjs
node scripts/verify/verify-index-query-result-decoder.mjs
node scripts/verify/verify-index-many-link-filtering.mjs
node scripts/verify/verify-index-query-snapshots.mjs
cargo xtask module validate index
```
