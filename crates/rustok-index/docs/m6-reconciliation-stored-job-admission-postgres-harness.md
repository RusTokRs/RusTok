# M6 reconciliation stored-job admission PostgreSQL harness

Status: executable target retained, not run.

This harness retains the current fail-closed admission boundary for durable reconciliation jobs whose persisted request or cursor JSON no longer matches the runner contract.

## Pending-job fixture

Each case creates a unique PostgreSQL schema, creates the tenant owner fixture, applies every real `IndexModule` migration, persists one active schema and materializes the canonical source/schema registries.

A counted two-page source is run with a one-page budget. Attempt 1 must:

- create one reconciliation job;
- apply page one;
- persist one entity and one inbox row;
- persist `completed_passes = 0`;
- persist `pages_processed = 1`;
- persist source cursor `{ "offset": 1 }`;
- yield the job as `pending` with its lease released;
- call the source exactly once.

The fixture then changes one JSONB contract field on that exact pending job.

## Stored request mismatch

The first case changes only `request.pass_count` from `1` to `2` while the new invocation requests the original pass count `1`.

Admission must return:

- `IndexReconciliationRunError::InvalidStoredJob`;
- exact reason `stored reconciliation request does not match the source/pass contract`.

The error occurs after the schema-scope advisory lock is acquired but before the pending row is claimed. PostgreSQL and the counted source must retain:

- the same pending job UUID;
- attempt count `1`;
- corrupted stored pass count `2`;
- the original cursor and page-one progress;
- released lease ownership;
- one source scan total;
- one job, one entity and one inbox row;
- no failure diagnostic or completion timestamp.

## Stored cursor contract mismatch

The second case changes only `cursor.contract` from `index_reconciliation_cursor_v1` to `index_reconciliation_cursor_corrupt`.

Admission must return:

- `IndexReconciliationRunError::InvalidStoredJob`;
- exact reason `cursor contract is invalid`.

It must again fail before claim, attempt increment, source scan or any new entity/inbox write. The same pending row retains attempt `1`, page-one cursor progress and released lease ownership.

## Repair and resume

Each case restores only the corrupted JSONB field. A newly constructed runner must then:

- claim the same pending job UUID;
- increment the attempt count to `2`;
- resume from `{ "offset": 1 }`;
- scan only page two;
- apply one mutation;
- complete one pass;
- persist `pages_processed = 2` and a null terminal source cursor;
- leave exactly one reconciliation job, two entities and two inbox rows.

A later invocation must return `AlreadyComplete` for the same succeeded job without another source scan.

## Safety boundary

The harness changes no production code, migration, reconciliation SQL/state machine, source/cursor wire contract, mutation identity, schema, diagnostic or public API.

It does not claim:

- automatic repair of corrupted durable JSON;
- operator inspect or repair authorization;
- audit records for manual repair;
- retry, backoff, dead-letter admission or scheduling;
- cancellation, lease takeover, restart or source timeout behavior;
- source/index digest comparison, orphan cleanup, targeted/full/shadow repair, locale/partition dimensions or complete drift repair.

No sleep, polling delay, elapsed-time expiry or concurrent worker race is used.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_stored_job_admission_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-stored-job-admission-harness.mjs
```
