# M6 reconciliation mutation-storage failed-progress recovery PostgreSQL harness

Status: executable target retained, not run.

This harness retains the current reconciliation behavior when mutation storage becomes temporarily unavailable on page two after page one has already crossed a durable progress boundary.

## Failed run

The fixture source owns two pages with a scan limit of one.

Page one returns one valid upsert and continuation cursor `{ "offset": 1 }`. The canonical PostgreSQL reconciliation runner must:

- apply the mutation;
- persist one entity and one inbox row;
- persist `pages_processed = 1` and `completed_passes = 0`;
- persist source cursor `{ "offset": 1 }`;
- execute the configured page-boundary heartbeat.

The page-two source call blocks behind two async barriers. Once the test observes the running job at the page-one boundary, a separate schema-scoped connection temporarily renames only `index_entities`. The source is then released and returns a fully valid second-page mutation.

Mutation persistence inserts its inbox candidate inside a transaction and then fails while reading the temporarily unavailable entity table. The whole page-two mutation transaction must roll back. The runner must return:

- `IndexReconciliationRunError::MutationFailed`;
- mutation position `0`;
- dependency code `mutation_storage_retryable`;
- `IndexReplayFailureKind::Retryable`.

The table is restored before final evidence is read.

## Durable failure evidence

The failed job must retain the previous safe boundary:

- state `failed`;
- attempt count `1`;
- `completed_passes = 0`;
- `pages_processed = 1`;
- source cursor `{ "offset": 1 }`;
- cleared lease owner and expiry;
- non-null completion timestamp;
- exactly one entity and one inbox row.

The exact failure diagnostic is:

```json
{
  "contract": "index_reconciliation_run_failure_v1",
  "dependency_code": "mutation_storage_retryable",
  "retryable": true
}
```

The object contains exactly three fields. It contains no database error text, SQL, table name, tenant, worker, job, source payload, mutation payload, transport detail or stack text.

Retaining exactly one inbox row proves the page-two inbox candidate rolled back with the unavailable entity read. A later exact-tenant cancellation request must return `AlreadyTerminal(Failed)`.

## Duplicate-safe recovery

Failed reconciliation jobs are terminal and excluded from active acquisition under the current runner. Recovery therefore creates a new job from the initial source cursor.

The recovery source returns:

1. page one with the exact same event UUID, entity UUID and source version already committed by the failed job;
2. the same valid page-two mutation that failed before durable persistence.

The recovery job must:

- use a different job UUID;
- complete on attempt `1`;
- process two pages and one pass;
- execute one page-boundary heartbeat;
- report two mutations;
- report one duplicate, one applied and zero stale mutations;
- persist a terminal null source cursor.

Final PostgreSQL evidence requires one failed job plus one succeeded job, exactly two entities and exactly two inbox rows. A later invocation must return `AlreadyComplete` for the succeeded recovery job without another page scan.

## Isolation

The target reads `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a fallback. Without a PostgreSQL URL it reports a skip and succeeds.

The fixture creates a unique PostgreSQL schema, creates the tenant owner row, applies every real `IndexModule` migration, persists one active schema, materializes canonical source and schema registries, reads durable evidence and drops the isolated schema.

No sleep, polling delay, elapsed-time expiry or concurrent worker race is used. The only coordination is the deterministic page-two source barrier around the schema-local DDL transition.

## Scope boundaries

This harness does not change production code, migrations, reconciliation SQL or state machines, source or cursor contracts, mutation identity, schemas, diagnostics or public APIs.

It does not add automatic retry, backoff, scheduling, attempt exhaustion, failed-scope dead-letter admission, authorized requeue, lease takeover, restart handling, source timeout, pending-future preemption, digest comparison, orphan cleanup, targeted/full/shadow repair, locale or partition dimensions, or complete drift repair.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_mutation_storage_failed_progress_recovery_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-mutation-storage-failed-progress-recovery-harness.mjs
```
