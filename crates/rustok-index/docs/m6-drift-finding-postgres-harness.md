# M6 drift-finding PostgreSQL lifecycle harness

## Purpose

This slice retains an environment-gated PostgreSQL integration target for the bounded
`PostgresIndexDriftFindingWriter` added in PR #2959. It verifies the production migration
shape and transaction semantics without claiming that the source/index digest producer,
repair admission, or an operator transport exists.

## Executable boundary

The target lives at:

```text
crates/rustok-index/tests/drift_finding_writer_postgres_test.rs
```

It reads `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a fallback.
When no PostgreSQL URL is available, the target reports a skip and succeeds.

For every invocation the harness:

1. creates an isolated PostgreSQL schema;
2. creates the tenant-owner fixture table;
3. applies every real `IndexModule` migration;
4. uses two independent connections to race the same deterministic finding key;
5. requires one `Created` and one `Refreshed` result with one retained row;
6. verifies exact tenant, key, check, severity, entity scope, digest, and bounded-details fields;
7. refreshes an open finding without changing its identity;
8. reopens a resolved finding and clears `closed_at`;
9. refreshes an ignored finding while preserving suppression;
10. proves the same logical scope under another tenant derives another key and row;
11. drops the isolated schema after successful assertions.

The concurrent first write retains advisory-lock serialization on PostgreSQL rather than the
SQLite contract fixture. The lifecycle transitions read production columns directly and
require deterministic identity preservation across refresh, reopen, and suppression.

## Deliberate limits

This harness does not add or claim:

- a source/index digest producer or comparator;
- owner snapshot or high-watermark semantics;
- orphan-link or missing-entity diagnosis;
- automatic finding resolution when digests converge;
- ignore/resolve commands, actor/reason audit, or repair authorization;
- targeted, full, or shadow repair execution;
- GraphQL, HTTP, CLI, admin, or scheduler invocation;
- retained execution evidence until the repository owner runs and admits the target.

## Owner verification

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test drift_finding_writer_postgres_test \
  -- --nocapture --test-threads=1

node scripts/verify/verify-index-drift-finding-postgres-harness.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

Tests, verifiers, formatting, Cargo checks, PostgreSQL execution, workflows, and CI were not
run by the implementation agent, per maintainer instruction.
