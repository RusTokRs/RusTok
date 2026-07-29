# M4 query planner actualization

Date: 2026-07-29

This note actualizes the live `rustok-index` implementation plan after rechecking the M3 storage and retained-evidence work already merged into `main`.

## Recheck result

The M3 source implementation is complete through retained bundle review, archive manifest generation, saved-manifest verification, and recursive filesystem drift detection. The repository owner still needs to execute and admit one fresh real PostgreSQL partition packet. That owner gate continues to block production partition lifecycle design, but it does not require query-planning or controlled SQL source work to remain idle.

No surviving remote branch whose name contains `index` was found before the planner slice. The M4 chain therefore started from current `main` rather than carrying an older branch forward.

## Actualized status

- M3 real retained PostgreSQL packet execution: `open_owner_action`.
- M3 production partition lifecycle: `blocked_by_retained_packet`.
- M4 deterministic executable query planning: `source_complete_execution_pending`.
- M4 stable relation aliases for explicit link paths: `source_complete_execution_pending`.
- M4 controlled PostgreSQL query compilation: `source_complete_execution_pending`.
- M4 typed filter/order/count/keyset semantics: `open`.
- M4 PostgreSQL/reference-engine equivalence: `open`.

## Completed M4 slice 1

`SchemaRegistry::plan_query` now:

1. runs the existing registry-backed query validation before planning;
2. collects every referenced link prefix from projection, filters, and ordering;
3. sorts link prefixes deterministically and assigns `t0`, `t1`, ... aliases;
4. resolves each join against the registered schema contract;
5. binds projected and ordered fields to the same relation aliases;
6. retains the typed filter and pagination contracts for the SQL compiler;
7. publishes a versioned SHA-256 plan fingerprint over deterministic postcard bytes.

The planner remains database independent and source-domain agnostic. It does not read source tables, execute SQL, bypass tenant/locale validation, decode cursors, or authorize callers.

## Completed M4 slice 2

`ExecutableQueryPlan::compile_postgres` now emits controlled SQL, ordered typed bind values, deterministic identity/projection columns, exact root scope, and one-cardinality projection joins. It binds tenant, schema, locale, link, target schema, projected field, and limit values rather than interpolating them. It also rechecks the planner path-to-alias mapping before constructing SQL.

The compiler deliberately returns typed pending errors for filters, explicit ordering, exact count, cursor continuation, offset pagination, and many-link aggregation. It does not connect to PostgreSQL or execute statements.

## Next bounded slice

Typed filter/order/count/keyset compilation remains the next bounded M4 slice. It must preserve:

- tenant and locale predicates as non-optional scope constraints;
- deterministic aliases and parameter order;
- typed JSONB scalar/list extraction aligned with secondary indexes;
- nested link filtering and projection semantics;
- exact count without pagination leakage;
- keyset cursor predicates with an entity-id tie-breaker;
- bounded offset compatibility as a separate explicit path;
- SQL/parameter snapshots and reference-engine equivalence fixtures.

Production query-port composition and consumer cutover remain later slices.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-index planner_tests -- --nocapture
cargo test -p rustok-index postgres_compiler_tests -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-query-planner.mjs
node scripts/verify/verify-index-postgres-query-compiler.mjs
cargo xtask module validate index
```
