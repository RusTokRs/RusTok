# M4 query planner actualization

Date: 2026-07-29

This note actualizes the live `rustok-index` implementation plan after rechecking the M3 storage and retained-evidence work already merged into `main`.

## Recheck result

The M3 source implementation is complete through retained bundle review, archive manifest generation, saved-manifest verification, and recursive filesystem drift detection. The repository owner still needs to execute and admit one fresh real PostgreSQL partition packet. That owner gate continues to block production partition lifecycle design, but it does not require query-planning source work to remain idle.

No surviving remote branch whose name contains `index` was found before this slice. The new work therefore starts from current `main` rather than carrying an older branch forward.

## Actualized status

- M3 real retained PostgreSQL packet execution: `open_owner_action`.
- M3 production partition lifecycle: `blocked_by_retained_packet`.
- M4 deterministic executable query planning: `source_complete_execution_pending`.
- M4 stable relation aliases for explicit link paths: `source_complete_execution_pending`.
- M4 SQL compilation: `open`.
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

## Next bounded slice

Compile `ExecutableQueryPlan` through controlled SeaQuery/PostgreSQL SQL while preserving:

- tenant and locale predicates as non-optional scope constraints;
- deterministic aliases and parameter order;
- typed JSONB field extraction;
- nested link projection and filtering;
- exact count and bounded offset compatibility;
- keyset cursor predicates with an entity-id tie-breaker;
- SQL/parameter snapshots and reference-engine equivalence fixtures.

Production query-port composition and consumer cutover remain later slices.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-index planner_tests -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-query-planner.mjs
cargo xtask module validate index
```
