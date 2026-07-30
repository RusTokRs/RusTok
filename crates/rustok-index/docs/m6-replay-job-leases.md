# M6 fenced replay job leases

Status: `source_complete_owner_execution_pending`

This slice adds durable ownership for one schema-scoped rebuild job. It does not add a
scheduler, multi-page loop, cancellation command, backoff policy, or production source
adapter.

## Job identity

`PostgresIndexReplayJobStore` owns `index_jobs` rows with:

- `kind = 'rebuild'`;
- `scope_kind = 'schema'`;
- one tenant and exact `SchemaRef`;
- the exact `index_replay_job_v1` request contract;
- one bounded source name and worker identity.

The source name remains part of the request payload because the canonical `index_jobs`
scope columns describe the schema, while replay ownership is fixed by
`IndexSourceCatalog`. A stored row with another source or request shape fails closed.
The persisted schema must exist and remain active before a job can be acquired.

## Claim and fencing

Acquisition serializes the tenant/source/schema scope with a PostgreSQL transaction
advisory lock. A pending job becomes claimable after `available_at`; a running job
becomes reclaimable after `lease_expires_at`. Reclaim increments `attempt_count`.

Every heartbeat and terminal update matches all of:

- tenant and job UUID;
- `kind = 'rebuild'` and `state = 'running'`;
- lease owner;
- attempt count;
- an unexpired lease.

An expired attempt therefore cannot heartbeat, fail, or complete after a newer worker
has reclaimed the same job.

## Fenced checkpoint progression

`PostgresIndexReplayCheckpointStore` is constructed from an acquired
`IndexReplayJobLease`. It rejects another tenant, source, or schema before opening a
transaction. Both checkpoint reads and writes lock and validate the active replay job
attempt before accessing `index_checkpoints`.

Checkpoint writes use the order:

1. lock and validate the current `(job_id, worker_id, attempt_count)`;
2. upsert the exact rebuild checkpoint;
3. commit the transaction.

A stale worker may still finish an already-started idempotent mutation transaction, but
it cannot advance the durable cursor. The existing inbox delivery identity and
monotonic source-version rules make such mutation replay safe.

## Terminal success

A replay job can enter `succeeded` only while its lease is active and the durable exact
checkpoint exists with a JSON `null` cursor. Missing or continuing checkpoints are
rejected. Job completion and checkpoint validation use one transaction and lock job
then checkpoint in the same order as checkpoint progression.

A failed terminal job remains retained as evidence and permits a later acquisition to
create a new job UUID. Retry timing and dead-letter policy remain outside this slice.

## Still open

- binding job acquisition directly to the materialized source registry in server
  composition;
- a bounded multi-page runner with heartbeat cadence and graceful lease loss;
- cancellation observation and terminal cancellation;
- bounded retry/backoff and dead-letter policy;
- locale and partition checkpoint dimensions;
- Product and later source adapters;
- retained PostgreSQL crash/reclaim/restart evidence and multi-instance tests.

## Owner validation

```bash
node scripts/verify/verify-index-replay-job-leases.mjs
node scripts/verify/verify-index-source-replay-contract.mjs
cargo check -p rustok-index --all-targets
cargo test -p rustok-index source_replay_job --lib -- --nocapture
```

These commands are maintainer-run for this slice.
