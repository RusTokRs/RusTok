# M6 reconciliation PostgreSQL lease-fencing harness

Status: executable target retained, not run.

## Purpose

This harness retains deterministic PostgreSQL evidence for an expired reconciliation lease being
claimed by a new worker while the original worker is still inside a source scan. It exercises the
existing `PostgresIndexReconciliationRunner`; no production code or SQL is changed.

## Scenario

The target creates a unique PostgreSQL schema, applies the real `IndexModule` migrations, persists
one active schema, and builds two independent runners over the same database scope.

Worker A acquires a new reconciliation job as attempt 1 and blocks inside the source adapter behind
an async barrier. Crossing that barrier proves acquisition committed before the test inspects the
job row.

A separate connection then changes only that running job's `lease_expires_at` to a timestamp in the
past. No sleep, polling delay, or wall-clock race is used.

Worker B uses another connection, source registry, schema registry, and runner. It claims the same
job UUID as attempt 2, applies the stable source mutation, and completes the job.

Only after worker B has returned `Complete` does the test release worker A. Worker A receives the
same stable event UUID and source version. The inbox makes the mutation redelivery duplicate-safe,
but worker A still owns the stale attempt-1 lease token. Its terminal publication must therefore
return:

- `IndexReconciliationRunError::LeaseLost`;
- the same durable job UUID;
- attempt count 1.

## Durable assertions

Before takeover:

- state is `running`;
- attempt count is 1;
- cursor `completed_passes` is 0;
- cursor `pages_processed` is 0;
- the lease owner is present.

After takeover and stale-worker release:

- the same job UUID is `succeeded`;
- durable attempt count is 2;
- cursor `completed_passes` is 1;
- cursor `pages_processed` is 1;
- the lease owner is cleared;
- exactly one reconciliation job exists;
- exactly one entity exists;
- exactly one inbox event exists.

The final cardinalities prove that stale mutation redelivery is harmless while lease-owner and
attempt fencing prevent stale cursor or terminal-state publication.

## Isolation

The target reads `RUSTOK_INDEX_TEST_DATABASE_URL`, falling back to PostgreSQL `DATABASE_URL`. It
skips successfully when neither variable contains a PostgreSQL URL. Each invocation creates and
drops one unique schema and uses one-connection pools with a schema-local `search_path`.

## Non-claims

This harness does not prove automatic lease scheduling, heartbeat extension races, source timeouts,
operator cancellation, retries, dead-letter handling, process restart, PostgreSQL restart, server
host restart, graceful shutdown, digest comparison, orphan cleanup, targeted repair, or complete
drift repair. It does not close the combined M6 reconciliation item.

## Suggested validation

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_lease_fencing_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-lease-fencing-harness.mjs
```
