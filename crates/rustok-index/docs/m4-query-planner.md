# M4 query planner actualization

Date: 2026-07-29

This note actualizes the live `rustok-index` implementation plan after
rechecking the M3 storage and retained-evidence work already merged into
`main`.

## Recheck result

The M3 source implementation is complete through retained bundle review,
archive manifest generation, saved-manifest verification, and recursive
filesystem drift detection. The repository owner still needs to execute and
admit one fresh real PostgreSQL partition packet. That owner gate continues to
block production partition lifecycle design, but it does not require M4 query
source work to remain idle.

## Actualized status

- M3 real retained PostgreSQL packet execution: `open_owner_action`.
- M3 production partition lifecycle: `blocked_by_retained_packet`.
- M4 deterministic executable query planning: `source_complete_execution_pending`.
- M4 stable relation aliases for explicit link paths: `source_complete_execution_pending`.
- M4 typed referenced-field contracts: `source_complete_execution_pending`.
- M4 controlled PostgreSQL query compilation: `source_complete_execution_pending`.
- M4 root/one-link filter/order/count/keyset/offset semantics: `source_complete_execution_pending`.
- M4 query-scoped cursor envelopes: `source_complete_execution_pending`.
- M4 deterministic compiled-row decoding: `source_complete_execution_pending`.
- M4 one-row lookahead and next-cursor construction: `source_complete_execution_pending`.
- M4 many-link query semantics: `open`.
- M4 PostgreSQL/reference-engine equivalence: `open`.

## Completed M4 slice 1

`SchemaRegistry::plan_query`:

1. runs registry-backed query validation before planning;
2. collects every referenced link prefix from projection, filters, and ordering;
3. sorts link prefixes deterministically and assigns `t0`, `t1`, ... aliases;
4. resolves each join against the registered schema contract;
5. records every referenced field with type, cardinality, nullability, path, and
   relation alias;
6. binds projected and ordered fields to those canonical field contracts;
7. retains typed filters and pagination for the compiler;
8. publishes a versioned SHA-256 fingerprint over deterministic postcard bytes.

The fingerprint domain is now `rustok-index-query-plan-v2` because typed field
contracts are part of the executable plan identity.

## Completed M4 slices 2 and 3

The controlled PostgreSQL compiler supports the validated root and explicit
one-cardinality-link subset:

- tagged JSONB projection and hidden order-value columns;
- all current typed filter operators;
- deterministic ordering and explicit null placement;
- query-scoped, checksum/scope/schema/type validated keyset continuation;
- an ascending root `entity_id` tie-breaker;
- bounded offset compatibility;
- a separate exact-count statement without pagination leakage.

Tenant, schema, locale, link metadata, field names, filter values, cursor
values, limit, and offset remain bind parameters. Atomic predicates are total
booleans so missing optional links and tagged null values cannot corrupt
logical `Not`, `And`, or `Or` semantics.

`ExecutableQueryPlan::compile_postgres` only accepts plans without an opaque
continuation token. Continuation queries use
`SchemaRegistry::compile_postgres_query`, which requires a scoped v2 envelope.
The envelope carries a `rustok-index-cursor-query-v1` fingerprint over tenant,
schema, locale, filter, and ordered field/direction semantics. Changing filter
or order semantics produces `QueryFingerprintMismatch` before any keyset SQL is
emitted. Legacy raw v1 envelopes remain limited to codec round trips and the
test-only reference engine.

## Completed M4 slice 4

`SchemaRegistry::compile_postgres_page_query` wraps the controlled compiler and
changes only the validated page-limit bind from `N` to `N + 1`. The SQL string,
plan fingerprint, column metadata, filters, ordering, cursor predicate, offset,
and optional exact-count statement remain unchanged.

`decode_postgres_query_page` re-plans the query and verifies the plan fingerprint,
complete deterministic column contract, requested page size, maximum row count,
tagged `IndexValue` type/cardinality/nullability, relation identities, and exact
count. It removes the one-row lookahead, preserves projection order, reports
`has_more`, and creates a scoped next cursor from the last retained root and hidden
order values. Offset pages report `has_more` without synthesizing a cursor.

## Remaining bounded M4 work

Many-cardinality link paths remain fail-closed. The next source slice must add
an explicit semantic plan for:

- `EXISTS`-based many-link filtering;
- nested aggregation for many-link projection;
- duplicate-free exact count and root pagination;
- direct SeaORM bind/row adaptation and execution composition;
- SQL/parameter snapshots and PostgreSQL/reference-engine equivalence fixtures.

Production query-port composition and consumer cutover remain later slices.
The real retained PostgreSQL partition packet remains an independent owner gate
for production partition lifecycle work.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-index planner_tests -- --nocapture
cargo test -p rustok-index postgres_compiler_tests -- --nocapture
cargo test -p rustok-index postgres_query_result_tests -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-query-planner.mjs
node scripts/verify/verify-index-postgres-query-compiler.mjs
node scripts/verify/verify-index-query-result-decoder.mjs
cargo xtask module validate index
```
