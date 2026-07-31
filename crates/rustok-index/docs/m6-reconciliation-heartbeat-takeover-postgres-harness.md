# M6 reconciliation heartbeat/takeover PostgreSQL harness

Status: executable target retained, not run.

This target retains a deterministic PostgreSQL boundary for the existing
`PostgresIndexReconciliationRunner` heartbeat and expired-running-job claim logic.
It does not modify the runner, migrations, state machine, source contract, cursor,
mutation model, or public API.

## Deterministic sequence

The owner source exposes two pages and two barrier pairs.

1. Worker A acquires one reconciliation job as attempt 1 and blocks inside the
   first source scan. Crossing the first barrier proves the running row and its
   lease are already durable.
2. A separate connection shortens only the exact attempt-1 lease owned by
   `heartbeat-worker-a` to 30 minutes.
3. The first page is released. Worker A applies and persists page one.
4. Before scanning page two, the configured `heartbeat_every_pages = 1` boundary
   executes with a one-hour lease duration.
5. The second source barrier is reached only after that heartbeat. PostgreSQL
   must then report `lease_expires_at > CURRENT_TIMESTAMP + INTERVAL '50 minutes'`.
6. A second runner invocation for the same tenant/schema must return `Busy` and
   must not claim or scan the source while the refreshed lease is active.
7. The harness explicitly expires only attempt 1. Worker B then claims the same
   job UUID as attempt 2, resumes from the durable page-one cursor, and succeeds.
8. Worker A is released only after worker B has published success. Its page-two
   mutation is duplicate-safe, but its attempt-1 terminal write must fail with
   `IndexReconciliationRunError::LeaseLost`.

No sleep, polling delay, or wall-clock race is used. The 30-minute and 50-minute
SQL thresholds leave a large deterministic margin around the one-hour heartbeat.

## Required durable evidence

Before heartbeat:

- one `running` reconciliation job;
- attempt count 1;
- owner `heartbeat-worker-a`;
- zero processed pages and zero completed passes;
- the manually shortened lease does not exceed the 50-minute threshold.

After heartbeat and before forced expiry:

- the same job UUID and attempt 1;
- one processed page and zero completed passes;
- the same lease owner;
- lease expiry beyond the 50-minute threshold;
- a competing invocation returns `Busy`.

After explicit expiry and takeover:

- worker B returns `Complete` under the same job UUID and attempt 2;
- worker A returns `LeaseLost` for attempt 1;
- the durable job is `succeeded` with two processed pages and one completed pass;
- lease ownership and expiry are cleared;
- exactly one reconciliation job, two entities, and two inbox events remain.

## Scope boundaries

This harness proves a written heartbeat/claim/fencing boundary only. It does not
prove:

- automatic scheduler polling or takeover discovery;
- elapsed-time lease expiry without the explicit SQL transition;
- heartbeat while a single source or mutation future is pending;
- source-call timeout or future preemption;
- cancellation racing with heartbeat;
- process, host, container, or PostgreSQL restart;
- replay retry/backoff, dead-letter admission, or operator requeue;
- source/index digest comparison, orphan cleanup, targeted repair, or complete
  drift repair;
- locale/partition reconciliation checkpoint dimensions;
- server authorization, transport, background task ownership, or graceful
  shutdown.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Maintainer validation

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_heartbeat_takeover_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-heartbeat-takeover-harness.mjs
```
