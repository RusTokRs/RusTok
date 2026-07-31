# M6 reconciliation process restart PostgreSQL harness

Status: executable, not run.

This harness retains an OS-process reconstruction contract for the existing
`PostgresIndexReconciliationRunner`. It is separate from the same-process new-connection
coverage in PR #2660 and from the running cancellation race in PR #2663.

## Parent process

`reconciliation_yield_resumes_across_two_test_processes` creates one unique PostgreSQL
schema, the tenant owner fixture, all real `IndexModule` migrations, and one active
source-owned schema. The parent never constructs a reconciliation runner.

It launches the integration-test executable twice in sequence. Each child runs only
`process_restart_worker_resumes_reconciliation_from_env` and receives the database URL,
isolated schema, tenant UUID, and expected phase through private test environment
variables.

## First child

The first process reconstructs the schema/source registries and runner from scratch. It
scans one of two source rows with a one-page budget and must return `Yielded`.

After the process exits, durable evidence must contain:

- one `pending` reconciliation job;
- attempt count 1;
- completed passes 0;
- processed pages 1;
- one entity and one inbox event.

## Second child

The second process creates another connection, registries, source adapter, and runner. It
claims the pending row, resumes from the stored source cursor, processes the second row,
and must return `Complete`.

Final durable evidence must contain:

- the same reconciliation job UUID;
- state `succeeded`;
- attempt count 2;
- completed passes 1;
- processed pages 2;
- two entities and two inbox events;
- exactly one reconciliation job row.

## Isolation and command

The target reads `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a
fallback. Without a PostgreSQL URL it reports a skip and succeeds. The isolated schema is
dropped after the parent assertions.

Suggested maintainer command:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_process_restart_postgres_test \
  reconciliation_yield_resumes_across_two_test_processes \
  -- --exact --nocapture
```

## Non-claims

This executable-no-run target does not prove a full server, container, host, or database
restart. It does not execute migrations in each child, restart PostgreSQL, exercise
running cancellation, retry/backoff/dead-letter behavior, locale/partition checkpoints,
or complete drift repair. No Cargo, verifier, PostgreSQL, or CI execution result is
recorded by this change.
