# M6 reconciliation source-failed progress recovery PostgreSQL harness

Status: executable target retained, not run.

This harness retains the current PostgreSQL reconciliation behavior when an owner source reports a retryable failure after one safe page boundary has already been persisted.

## Scenario

The source exposes two pages with a page size of one.

Page 1 returns one valid upsert and continuation cursor:

```json
{ "offset": 1 }
```

The runner applies the mutation, persists the cursor and cumulative page counters, then heartbeats before scanning page 2 because `heartbeat_every_pages` is one.

In failure mode, the second scan returns the bounded retryable owner-source code:

```text
owner_source_retryable_after_progress
```

The run returns `IndexReconciliationRunError::Source(IndexSourceError::SourceFailure { .. })` with `IndexSourceFailureKind::Retryable`.

## Durable failed-job evidence

The current runner terminalizes the job as `failed`; retryability is retained only in the diagnostic. PostgreSQL must contain:

- attempt count 1;
- `completed_passes = 0`;
- `pages_processed = 1`;
- source cursor `{ "offset": 1 }`;
- one entity row and one inbox row from page 1;
- cleared lease owner and expiry;
- non-null completion timestamp;
- `last_error_code = index.reconciliation_page_failed`.

The exact three-field diagnostic is:

```json
{
  "contract": "index_reconciliation_run_failure_v1",
  "dependency_code": "owner_source_retryable_after_progress",
  "retryable": true
}
```

No source payload, tenant, worker, job, event, entity, SQL, database message, transport detail or stack text is persisted in that object.

A later exact-tenant cancellation request observes `AlreadyTerminal(Failed)`.

## Recovery behavior

Failed reconciliation rows are terminal and are excluded from the current acquisition query. The harness therefore constructs a new runner/source mode and starts a new reconciliation job from the initial cursor.

The recovery source:

1. redelivers page 1 with the exact same event UUID, entity UUID and source version;
2. returns a valid page 2 mutation that never reached storage during the failed run.

The recovery job must:

- have a different job UUID;
- complete on attempt 1;
- process two pages and one pass;
- heartbeat once;
- report two mutations, one applied, one duplicate and zero stale;
- persist a terminal null source cursor;
- leave exactly two jobs, two entities and two inbox rows.

A subsequent invocation resolves the recovery job as `AlreadyComplete` without another mutation.

This is duplicate-safe operator recovery evidence. It is not automatic retry scheduling or failed-scope requeue.

## PostgreSQL isolation

The target reads `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a fallback. Without a PostgreSQL URL it reports a skip and succeeds.

Each invocation:

- creates a unique PostgreSQL schema;
- creates the tenant owner fixture;
- applies every real `IndexModule` migration;
- persists one active schema registration;
- materializes canonical source and schema registries;
- uses schema-local one-connection pools;
- reads durable evidence directly;
- drops the schema.

No sleep, polling delay, elapsed-time expiry or concurrent race is used.

## Scope boundaries

This harness does not change or claim:

- production code, migrations or reconciliation SQL/state-machine behavior;
- source, cursor, mutation identity, schema, diagnostic or public API contracts;
- automatic retry, backoff, jitter, scheduling or attempt exhaustion;
- failed-scope dead-letter admission or authorized operator requeue;
- cancellation, lease-loss, takeover, restart or heartbeat races;
- source-call timeout or pending-future preemption;
- digest comparison, orphan cleanup, targeted/full/shadow repair;
- locale or partition checkpoint dimensions;
- complete drift repair;
- server authorization, transport ownership or graceful shutdown.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Suggested validation

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_source_failed_progress_recovery_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-source-failed-progress-recovery-harness.mjs
```
