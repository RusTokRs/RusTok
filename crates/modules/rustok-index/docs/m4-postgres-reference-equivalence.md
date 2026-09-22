# M4 PostgreSQL/reference equivalence fixture, capture, and admission

Date: 2026-07-29

Status: `fixture_capture_and_admission_source_complete_owner_execution_pending`

This M4 evidence chain now has three bounded source components:

1. an owner-run PostgreSQL regression fixture that compares the production query path
   with an independent in-memory reference materialization;
2. a retained capture command that binds one successful fixture run to an exact clean
   Git commit, PostgreSQL identity, scenario contract, and exact stdout/stderr bytes;
3. a read-only admission command that reviews the immutable three-file bundle and emits
   a separate no-clobber receipt.

None of these commands were executed by the implementation agent. Live equivalence
execution and admission remain explicit repository-owner actions.

## Fixture execution boundary

The fixture runs only when `RUSTOK_INDEX_TEST_DATABASE_URL`, or the repository-wide
`DATABASE_URL` fallback, contains a PostgreSQL URL. Without one it prints an explicit
skip marker and succeeds without connecting to PostgreSQL. The retained capture rejects
that skipped-success path.

Each real fixture run:

1. creates a uniquely named PostgreSQL schema;
2. sets a single-connection search path to that schema;
3. creates only the minimal platform-owned `tenants` identity table required by Index
   foreign keys;
4. applies the canonical `IndexModule` migrations;
5. persists schemas through `PostgresSchemaRegistrationStore`;
6. writes records and relation ordinals through `PostgresMutationStore`;
7. executes structured queries through `PostgresIndexQueryPort`;
8. compares the complete `IndexQueryPage` with an independent reference page;
9. drops the isolated schema after successful comparison.

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

The source scenarios cover descending root ordering and cursor continuation, one-link
filtering/projection, many-link `Gte`/`Contains`/`Ne`/`IsNull`, nested relation alignment,
and bounded offset lookahead.

## Retained capture command

`index-query-equivalence-capture` lives under `ops/benches`; it is not linked into Index
production runtime. It requires explicit `INDEX_QUERY_EQUIVALENCE_ALLOW_CAPTURE=1` and
refuses to publish unless all of the following are true:

- the workspace is a clean Git checkout at the exact configured 40-character lowercase
  commit before and after the subprocess;
- PostgreSQL reports version 16, a numeric `system_identifier`, and a non-empty database
  name before and after execution;
- exactly `postgres_query_port_matches_reference_fixture` exits successfully through one
  test thread;
- output names the required fixture, reports one passed test, and does not contain the
  fixture skip marker;
- stdout and stderr are each at most 2 MiB.

The capture publishes a fresh descriptor-last no-clobber bundle:

- `stdout.log` — exact cargo/test stdout;
- `stderr.log` — exact cargo/test stderr;
- `equivalence.json` — contract version, completion time, repository/commit/run key,
  capture runner identity, PostgreSQL identity, exact command, fixed scenario names and
  digest, exit code, byte counts, and SHA-256 hashes of both logs.

The output root must not already exist. Files use create-new semantics and the descriptor
is published last. The descriptor does not retain the PostgreSQL URL, credentials,
workspace path, or arbitrary environment values.

## Read-only admission command

`index-query-equivalence-admission` also lives under `ops/benches` and requires explicit
`INDEX_QUERY_EQUIVALENCE_ALLOW_ADMISSION=1`. It does not connect to PostgreSQL and does
not execute Cargo or the fixture. Instead it requires expected repository, commit, and
run key values supplied independently by the reviewer.

Admission fails closed unless:

- the bundle root is an existing regular non-symlink directory;
- its exact inventory is `equivalence.json`, `stderr.log`, and `stdout.log`, with no
  aliases, subdirectories, symlinks, or extra files;
- the capture descriptor has no unknown fields and matches the capture v1 contract;
- repository, clean commit, run key, PostgreSQL 16 identity, runner identity, exact test
  command, six scenario names, and scenario digest all match the admitted contract;
- descriptor byte counts and SHA-256 hashes match the retained log bytes;
- retained UTF-8 output proves one successful fixture and contains no skip marker;
- inventory and all three files remain byte-identical across the full review.

The receipt parent must already exist and must be a regular non-symlink directory. The
receipt must be outside the immutable bundle and is created with no-clobber semantics.
It records `admitted: true` and `production_lifecycle_authorized: false`, source and
PostgreSQL identity, execution contract, exact inventory and artifact hashes, capture
runner provenance, and reviewer runner provenance.

A successful receipt admits only that exact equivalence bundle. It does not authorize
production partition lifecycle changes, consumer cutover, or any other deployment.

## Required environment

Capture:

- `INDEX_QUERY_EQUIVALENCE_ALLOW_CAPTURE=1`;
- `RUSTOK_INDEX_TEST_DATABASE_URL` or `DATABASE_URL`;
- `INDEX_QUERY_EQUIVALENCE_COMMIT`;
- `INDEX_QUERY_EQUIVALENCE_RUN_KEY`;
- optional output root, repository, workspace, cargo executable, and job metadata.

Admission:

- `INDEX_QUERY_EQUIVALENCE_ALLOW_ADMISSION=1`;
- `INDEX_QUERY_EQUIVALENCE_BUNDLE`;
- `INDEX_QUERY_EQUIVALENCE_EXPECTED_COMMIT`;
- `INDEX_QUERY_EQUIVALENCE_EXPECTED_RUN_KEY`;
- optional expected repository, receipt output path, and reviewer job metadata.

## Non-claims

This source does not:

- claim that PostgreSQL/reference equivalence has been executed or admitted;
- modify or archive the immutable capture bundle;
- add many-link aggregate ordering;
- compose the query port into server/storefront/admin/search consumers;
- publish a source-owned schema-registry composition contract;
- change production planner, compiler, decoder, query-port, migration, or mutation
  behavior;
- authorize production partition lifecycle work.

The plan/SQL snapshots, fixture, capture, and admission review are source complete. The
combined M4 roadmap item remains open until the repository owner runs the capture,
reviews it through admission, and retains the resulting bundle and receipt.

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

INDEX_QUERY_EQUIVALENCE_ALLOW_ADMISSION=1 \
INDEX_QUERY_EQUIVALENCE_BUNDLE=<fresh-capture-root> \
INDEX_QUERY_EQUIVALENCE_EXPECTED_COMMIT=<40-char-head-commit> \
INDEX_QUERY_EQUIVALENCE_EXPECTED_RUN_KEY=<stable-run-key> \
INDEX_QUERY_EQUIVALENCE_ADMISSION_OUTPUT=<existing-parent>/equivalence-admission.json \
  cargo run -p rustok-benchmarks --bin index-query-equivalence-admission

node scripts/verify/verify-index-postgres-reference-equivalence.mjs
node scripts/verify/verify-index-query-equivalence-capture.mjs
node scripts/verify/verify-index-query-equivalence-admission.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-benchmarks --bin index-query-equivalence-capture
cargo check -p rustok-benchmarks --bin index-query-equivalence-admission
cargo xtask module validate index
```
