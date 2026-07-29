# M4 PostgreSQL/reference equivalence fixture and capture

Date: 2026-07-29

Status: `fixture_and_capture_source_complete_owner_execution_pending`

This slice provides both the owner-run PostgreSQL regression fixture for the M4
structured query engine and a separate retained capture command. The fixture compares
the production PostgreSQL query path with an independent in-memory reference
materialization built from the same validated `IndexRecord` contracts. The capture
command binds one successful fixture execution to an exact clean Git commit,
PostgreSQL identity, scenario contract, and retained stdout/stderr bytes.

Neither source path was executed by the implementation agent. Live equivalence evidence
remains an explicit repository-owner action.

## Fixture execution boundary

The test runs only when `RUSTOK_INDEX_TEST_DATABASE_URL`, or the repository-wide
`DATABASE_URL` fallback, contains a PostgreSQL URL. Without one, it prints an explicit
skip message and succeeds without connecting to a database.

Each fixture run:

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

## Retained capture command

`index-query-equivalence-capture` lives under `ops/benches`; it is not linked into Index
production runtime. It requires explicit `INDEX_QUERY_EQUIVALENCE_ALLOW_CAPTURE=1` and
refuses to publish unless all of the following are true:

- the configured workspace is a Git checkout with an exact 40-character lowercase
  `INDEX_QUERY_EQUIVALENCE_COMMIT` at `HEAD`;
- the worktree is clean before and after the cargo subprocess;
- PostgreSQL reports version 16, a numeric `system_identifier`, and a non-empty database
  name;
- the exact `postgres_query_port_matches_reference_fixture` cargo test exits successfully;
- output names the required fixture, reports one passed test, and does not contain the
  fixture's skip marker;
- stdout and stderr are each at most 2 MiB;
- PostgreSQL identity is unchanged after the fixture exits.

The capture retains a descriptor-last no-clobber bundle:

- `stdout.log` — exact cargo/test stdout;
- `stderr.log` — exact cargo/test stderr;
- `equivalence.json` — contract version, completion time, repository/commit/run key,
  runner identity, PostgreSQL identity, exact command, fixed scenario names and scenario
  digest, exit code, byte counts, and SHA-256 hashes of both logs.

The output root must not already exist. Files are created with create-new semantics and
the descriptor is published last, so a bundle without `equivalence.json` is incomplete
and must not be admitted. The descriptor does not retain the PostgreSQL URL, credentials,
or arbitrary environment values.

## Required environment

- `INDEX_QUERY_EQUIVALENCE_ALLOW_CAPTURE=1` — explicit execution and publication opt-in;
- `RUSTOK_INDEX_TEST_DATABASE_URL` or `DATABASE_URL` — PostgreSQL connection used by the
  capture identity query and fixture;
- `INDEX_QUERY_EQUIVALENCE_COMMIT` — exact clean checkout commit;
- `INDEX_QUERY_EQUIVALENCE_RUN_KEY` — stable 1–128 byte ASCII run identity;
- optional `INDEX_QUERY_EQUIVALENCE_OUTPUT_ROOT` — fresh bundle path, defaulting to
  `target/index-query-equivalence/<run-key>`;
- optional `INDEX_QUERY_EQUIVALENCE_REPOSITORY`, workspace root, cargo executable, and job
  metadata overrides.

## Non-claims

This source does not:

- claim that PostgreSQL/reference equivalence has been executed;
- admit or archive a retained bundle;
- add many-link aggregate ordering;
- compose the query port into server/storefront/admin/search consumers;
- authorize callers;
- change production planner, compiler, decoder, query-port, migration, or mutation
  behavior;
- authorize production partition lifecycle work.

The plan/SQL snapshots, fixture, and capture command are source complete. The combined M4
roadmap item remains open until the repository owner runs the capture, reviews the
retained bundle, and records admission/provenance.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-index postgres_query_port_matches_reference_fixture -- --nocapture

INDEX_QUERY_EQUIVALENCE_ALLOW_CAPTURE=1 \
RUSTOK_INDEX_TEST_DATABASE_URL=postgres://... \
INDEX_QUERY_EQUIVALENCE_COMMIT=<40-char-head-commit> \
INDEX_QUERY_EQUIVALENCE_RUN_KEY=<stable-run-key> \
  cargo run -p rustok-benchmarks --bin index-query-equivalence-capture

node scripts/verify/verify-index-postgres-reference-equivalence.mjs
node scripts/verify/verify-index-query-equivalence-capture.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-benchmarks --bin index-query-equivalence-capture
cargo xtask module validate index
```
