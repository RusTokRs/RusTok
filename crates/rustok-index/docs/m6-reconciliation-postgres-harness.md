# M6 reconciliation PostgreSQL harness

Status: `executable_no_run`

## Scope

This slice adds one environment-gated PostgreSQL integration target for the existing
`PostgresIndexReconciliationRunner`. It does not change production reconciliation
code, migrations, job state transitions, source contracts, or runtime composition.

The target is
`crates/rustok-index/tests/source_reconciliation_postgres_test.rs` and uses
`RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a fallback.
When neither variable contains a PostgreSQL URL, the target reports a skip and
returns successfully.

## Database isolation

Each case:

1. creates a unique PostgreSQL schema;
2. opens one-connection SeaORM pools and sets `search_path` to that schema;
3. creates the owner `tenants` fixture table;
4. applies the real `IndexModule` migration sequence;
5. persists one active source-owned Index schema;
6. runs the canonical PostgreSQL reconciliation runner;
7. reads durable entity/job evidence;
8. drops the isolated schema.

A new scoped database connection and a newly constructed runner are used after a
yield. This is restart-compatible connection reconstruction evidence, not proof of a
process, container, host, or database restart.

## Retained cases

### Yield and resume

`reconciliation_yield_resumes_across_new_connection_and_preserves_job_identity`
uses a two-row bounded source with page size one.

The first runner processes one page and must yield with attempt one and zero completed
passes. A new connection and runner claim the same durable job, process the final
page, and must succeed with attempt two and the same job UUID.

The case then requires:

- two `index_entities` rows;
- exactly one reconciliation `index_jobs` row;
- terminal state `succeeded`;
- one completed pass and two durable processed pages;
- an additional invocation returning `AlreadyComplete` without another source scan.

### Pending cancellation

`pending_reconciliation_cancel_is_durable_and_tenant_scoped` first creates a yielded
pending reconciliation job. A new connection and runner must observe:

- a different tenant receiving `NotFound`;
- the exact tenant receiving `Cancelled` for the pending job;
- a repeated request receiving terminal `Cancelled`;
- durable job state `cancelled`;
- exactly one already-applied entity and one job row.

This case covers immediate pending cancellation. It does not claim a concurrent
running-worker cancellation race, lease-expiry recovery, or in-page interruption.

## Non-claims

This harness does not provide retained execution output and does not prove:

- compilation or PostgreSQL execution;
- process or database restart recovery;
- concurrent running-job cancellation;
- lease expiry, heartbeat races, retry/backoff, or dead-letter behavior;
- source/index digest comparison or orphan deletion;
- scheduler, server authorization, graceful shutdown, or transport behavior;
- locale/partition reconciliation checkpoint dimensions.

The canonical M6 reconciliation and drift-repair item remains open.

## Suggested maintainer commands

The implementation agent intentionally did not run these commands:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index --test source_reconciliation_postgres_test -- --nocapture

RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index --test source_reconciliation_postgres_test \
  reconciliation_yield_resumes_across_new_connection_and_preserves_job_identity -- --exact --nocapture

RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index --test source_reconciliation_postgres_test \
  pending_reconciliation_cancel_is_durable_and_tenant_scoped -- --exact --nocapture

node scripts/verify/verify-index-reconciliation-postgres-harness.mjs
```
