# M6 reconciliation running-cancel PostgreSQL harness

Status: **source-ready / unvalidated**

## Scope

This slice adds one environment-gated PostgreSQL integration target for the existing
`PostgresIndexReconciliationRunner`. Production code, migrations, source contracts,
job state transitions, cursors, event identities, and public APIs are unchanged.

The harness covers cancellation requested while a reconciliation worker is blocked
inside an owner source scan. It deliberately releases the source only after a second
connection has persisted `cancel_requested = true` for the exact running job.

## Retained race

The target requires this sequence:

1. a runner acquires one `running` reconciliation job;
2. the owner source blocks before returning its first page;
3. a wrong tenant receives `IndexReconciliationCancelOutcome::NotFound`;
4. the exact tenant receives `Requested`;
5. PostgreSQL stores `cancel_requested = true` while the job is still running;
6. the source returns one stable upsert mutation;
7. the mutation is durably applied;
8. the runner observes cancellation before progress publication and terminalizes the
   job as `cancelled`;
9. the cancelled job retains `completed_passes = 0` and `pages_processed = 0` even
   though one entity and one inbox delivery exist.

This distinction is intentional. Page counters in the returned in-memory outcome
show work performed by the cancelled attempt, while the durable cursor remains at
the last safely committed reconciliation boundary.

## Duplicate-safe recovery

A new runner and database connection then execute the same source scope. Because the
cancelled row is terminal, the runner creates a new reconciliation job. The source
redelivers the same stable event UUID and source version from the unadvanced cursor.

The recovery attempt must:

- finish as `Complete`;
- use a new job UUID with attempt count one;
- report one duplicate and zero newly applied mutations;
- retain exactly one entity and one inbox row;
- leave exactly one cancelled job and one succeeded job.

The inbox identity and monotonic source-version guard, not cancellation timing, own
redelivery safety.

## PostgreSQL isolation

The test uses `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a
fallback. Without a PostgreSQL URL it reports a skip and succeeds.

It creates one unique schema, uses one-connection SeaORM pools with schema-local
`search_path`, creates the tenant owner fixture, applies all real Index migrations,
persists one active schema, reads durable evidence, and drops the schema afterward.

## Non-claims

This is executable source evidence only. It does not prove:

- that the target has been compiled or executed;
- process, host, container, or database restart recovery;
- cancellation before an owner source future becomes cooperative;
- replay-worker in-page interruption from PR #2649;
- source-call timeout behavior from PR #2639;
- retry/backoff, dead-letter scheduling, or operator requeue;
- orphan detection, source/index digests, or complete drift repair;
- scheduler ownership, server authorization, transport, or graceful shutdown.

The canonical M6 reconciliation and drift-repair item remains open.

## Maintainer execution

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_running_cancel_postgres_test \
  -- --nocapture

node scripts/verify/verify-index-reconciliation-running-cancel-harness.mjs
```

No test, Cargo command, verifier, PostgreSQL target, workflow, or CI job was executed
while preparing this source slice.
