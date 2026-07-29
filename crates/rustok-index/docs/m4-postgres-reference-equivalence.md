# M4 PostgreSQL/reference equivalence fixture

Date: 2026-07-29

Status: `source_complete_owner_execution_pending`

This slice adds an owner-run PostgreSQL regression fixture for the M4 structured query
engine. It compares the production PostgreSQL query path with an independent in-memory
reference materialization built from the same validated `IndexRecord` contracts. It does
not claim that the fixture was executed or that live equivalence evidence has been
retained.

## Execution boundary

The test runs only when `RUSTOK_INDEX_TEST_DATABASE_URL`, or the repository-wide
`DATABASE_URL` fallback, contains a PostgreSQL URL. Without one, it prints an explicit
skip message and succeeds without connecting to a database.

Each run:

1. creates a uniquely named PostgreSQL schema;
2. sets a single-connection search path to that schema;
3. creates only the minimal platform-owned `tenants` identity table required by Index
   foreign keys;
4. applies the canonical `IndexModule` migrations;
5. persists schemas through `PostgresSchemaRegistrationStore`;
6. writes records and relation ordinals through `PostgresMutationStore`;
7. executes structured queries through `PostgresIndexQueryPort`;
8. drops the isolated schema after successful comparison.

The fixture does not write `index_schemas`, `index_entities`, or `index_links` directly,
and it does not introduce Testcontainers or a second database stack.

## Independent reference page

`ReferenceFixture` owns only validated in-memory records and registry contracts. It does
not inspect compiler SQL, bind positions, driver rows, or PostgreSQL tables. For each
query it independently evaluates:

- tenant, schema, and locale scope;
- every current logical and atomic filter operator;
- reference null and many-link `Ne` semantics;
- explicit ordering plus the ascending root identity tie-breaker;
- scoped cursor continuation and bounded offset pagination;
- one-row lookahead, `has_more`, and exact count before cursor/offset leakage;
- root and one-link projected values and relation identities;
- grouped many-link projection items with complete identity chains and aligned values;
- deterministic next-cursor construction.

The assertion compares the complete `IndexQueryPage`, not only root IDs or row counts.

## Retained scenarios

The source fixture covers:

- root filtering, descending typed ordering, exact count, first-page lookahead, and a
  second scoped-cursor page;
- one-link filtering and projection;
- many-link `Gte`, `Contains`, and reference-compatible `Ne` combined with nested
  projection aggregation;
- many-link `IsNull` over empty and mixed reachable values;
- bounded offset ordering and lookahead.

The records intentionally include a many-link path with both a non-null and a tagged-null
child value, a blocked child, an empty relation, ordered relation targets, and two
one-link targets. This makes null totality, independent atomic `EXISTS` branches,
relation ordering, duplicate-free count, and nested alignment observable.

## Non-claims

This slice does not:

- run the PostgreSQL fixture;
- retain database identity, timing, EXPLAIN, or result-digest evidence;
- add many-link aggregate ordering;
- compose the query port into server/storefront/admin/search consumers;
- authorize callers;
- change production planner, compiler, decoder, query-port, migration, or mutation
  behavior;
- authorize production partition lifecycle work.

The plan/SQL snapshots are source complete. This fixture makes the live equivalence test
source complete, while owner execution and retained live evidence remain open.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-index postgres_query_port_matches_reference_fixture -- --nocapture
node scripts/verify/verify-index-postgres-reference-equivalence.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo xtask module validate index
```
