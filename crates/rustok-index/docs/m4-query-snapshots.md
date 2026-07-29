# M4 retained query plan and SQL snapshots

Date: 2026-07-29

Status: `source_complete_owner_execution_pending`

This slice retains deterministic golden fixtures for the executable-plan v4 and
controlled PostgreSQL compiler contracts introduced by the nested many-link projection
work. It does not execute SQL and does not claim PostgreSQL/reference-engine
equivalence.

## Canonical fixture

The retained query uses fixed, source-independent contracts:

- tenant `00000000-0000-0000-0000-000000000001`;
- required locale `en-US`;
- root schema `rustok-product::product@1`;
- root scalar projection `id`;
- one many-cardinality link `variants` targeting
  `rustok-product::variant@1`;
- nested projection `variants.id`;
- cursor page size `2` without a continuation token;
- no filter, explicit ordering, or exact count.

The domain names exist only in the test fixture registry and ordered binds. The exact
SQL snapshot contains compiler-owned table/column/alias text plus `$N` placeholders;
it does not interpolate `variants`, module names, entity names, UUIDs, locales, field
names, or page size.

## Retained files

- `m4_many_projection.plan.snap` records aliases, joins, typed scalar projection,
  grouped `PlannedManyProjection` metadata, pagination, and exact-count intent.
- `m4_many_projection.sql` records the complete controlled PostgreSQL statement,
  including the correlated row-preserving JSONB aggregate and deterministic
  ordinal/entity/locale ordering.
- `m4_many_projection.compiled.snap` records the exact ordered bind DTOs, scalar
  column metadata, and `CompiledManyRelationColumn` contract.

`query_snapshot_tests::retained_v4_plan_and_sql_snapshots_are_stable` compares all
three files byte-for-byte. Contract changes therefore require an intentional fixture
update in the same pull request.

## Additional source scenarios

The compiler scenarios cover root and one-link projection, typed filters, exact count,
keyset and bounded offset, correlated many-link filtering, grouped many projection,
and fail-closed plan metadata tampering.

The nested result scenarios cover valid aligned identity/value arrays and reject:

- identity arity drift;
- selected-field arity drift;
- nil nested identities;
- duplicate complete identity chains.

These tests are source evidence only until the repository owner executes them.

## Boundary

This slice does not:

- connect to PostgreSQL, prepare statements, or execute SQL;
- adapt SeaORM bind values or rows;
- prove PostgreSQL output equals the test-only reference engine;
- add many-link ordering;
- publish `IndexQueryPort` or cut over a consumer;
- change migrations or partition lifecycle state.

The combined roadmap item for plan/SQL snapshots and PostgreSQL/reference-engine
equivalence remains open until live equivalence evidence is implemented and executed.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-index query_snapshot_tests -- --nocapture
cargo test -p rustok-index postgres_compiler_tests -- --nocapture
cargo test -p rustok-index postgres_many_projection_tests -- --nocapture
cargo test -p rustok-index postgres_query_result_tests -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-query-planner.mjs
node scripts/verify/verify-index-postgres-query-compiler.mjs
node scripts/verify/verify-index-query-result-decoder.mjs
node scripts/verify/verify-index-many-link-filtering.mjs
node scripts/verify/verify-index-query-snapshots.mjs
cargo xtask module validate index
```
