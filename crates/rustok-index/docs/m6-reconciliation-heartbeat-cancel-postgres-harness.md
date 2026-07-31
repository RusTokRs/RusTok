# Reconciliation heartbeat cancellation PostgreSQL harness

Status: executable target retained, not run.

## Purpose

This environment-gated PostgreSQL target retains the cancellation boundary after an active reconciliation worker has already refreshed its lease at a page boundary.

It complements the separate running-cancellation and heartbeat/takeover harnesses. The retained case proves that a recent heartbeat does not let a worker publish later progress or success after an exact-tenant cancellation request becomes durable.

## Deterministic sequence

The fixture source exposes two pages and two pairs of async barriers.

Worker A acquires attempt 1 and blocks in the first source scan. The test reads the durable running row and then shortens only the exact `(tenant, job, worker, attempt)` lease to 30 minutes.

After the first barrier is released, worker A:

1. applies the first mutation;
2. persists cursor progress for one page;
3. enters page index 1;
4. refreshes the lease using the configured one-hour duration;
5. enters the blocked second source scan.

The second source barrier is therefore reachable only after the page-boundary heartbeat has completed.

At that point PostgreSQL must contain:

- the original job UUID;
- state `running`;
- attempt count 1;
- durable `completed_passes = 0`;
- durable `pages_processed = 1`;
- lease owner `heartbeat-cancel-worker-a`;
- `lease_expires_at > CURRENT_TIMESTAMP + INTERVAL '50 minutes'`;
- `cancel_requested = false`.

The one-hour lease and 50-minute evidence threshold leave a deterministic ten-minute margin. No sleep, polling delay, or elapsed-time race is used.

## Cancellation after heartbeat

A separately reconstructed runner first proves that another tenant receives `NotFound`. The exact tenant then requests cancellation and must receive `Requested`.

Before worker A is released, the durable row must retain:

- the refreshed lease;
- page-one cursor progress;
- the same owner and attempt;
- `cancel_requested = true`.

Worker A then receives and applies the second mutation. Its in-memory outcome records two processed pages, one completed pass, one heartbeat, and two applied mutations. Before terminal success or second-page cursor publication, the existing cancellation check must transition the job to `cancelled`.

The durable cancelled job must retain the previous safe cursor boundary:

- `completed_passes = 0`;
- `pages_processed = 1`;
- cleared lease owner and expiry;
- attempt count 1;
- `cancel_requested = true`.

Both mutations are already durable, so the database contains exactly two entities and two inbox rows.

## Duplicate-safe recovery

Cancelled reconciliation jobs are terminal and excluded from active acquisition. A new invocation therefore creates a new job and scans from the initial source cursor.

The source emits the same two stable event UUIDs and source versions. Recovery must:

- use a different job UUID;
- complete as attempt 1;
- process two pages and one pass;
- execute one heartbeat;
- report zero newly applied mutations and two duplicates;
- preserve exactly two entities and two inbox rows.

A later invocation must return `AlreadyComplete` for the recovery job without another source scan.

## PostgreSQL isolation

The target reads `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a fallback. Without a PostgreSQL URL it reports a skip and succeeds.

Each invocation creates a unique schema, creates the tenant owner fixture, applies every real `IndexModule` migration, persists one active schema, uses one-connection pools with schema-local `search_path`, reads durable evidence, and drops the schema.

## Scope boundaries

This slice adds only test, documentation, and verifier files. It does not change production code, migrations, SQL, leases, cursors, source contracts, mutation semantics, event identity, diagnostics, public APIs, or the reconciliation state machine.

It does not claim:

- cancellation while the heartbeat SQL statement itself is concurrently executing;
- preemption of a currently pending source future;
- source-call timeout behavior;
- automatic scheduling or takeover discovery;
- retry/backoff or dead-letter requeue;
- process, host, container, or PostgreSQL restart;
- graceful task shutdown;
- source/index digest comparison;
- orphan cleanup;
- targeted, full, or shadow repair modes;
- locale or partition checkpoint dimensions;
- complete drift repair.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Suggested maintainer validation

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_heartbeat_cancel_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-heartbeat-cancel-harness.mjs
```

These commands were not run by the implementation agent.
