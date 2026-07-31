# M6 reconciliation schema admission PostgreSQL harness

This harness retains the persisted-schema admission boundary already implemented by
`PostgresIndexReconciliationRunner`.

It is evidence for current behavior only. It does not change production code, SQL,
migrations, schema registration, source contracts, reconciliation cursors, mutation
identity, scheduling, or public APIs.

## Admission order

Reconciliation acquisition takes the schema-scope advisory transaction lock and then
verifies the exact tenant/module/entity/version row in `index_schemas` before it selects,
claims, completes, or inserts an `index_jobs` row.

The harness requires:

1. an absent persisted schema returns `SchemaNotRegistered`;
2. no reconciliation job is created;
3. the source is not scanned;
4. no entity or inbox write occurs.

The in-memory schema registry and source registry are deliberately complete in this case.
The denial therefore belongs to persisted tenant admission rather than source ownership or
schema compilation.

## Retired pending scope

After the schema is persisted as active, the first invocation processes one of two pages
and yields. PostgreSQL must retain:

- one `pending` reconciliation job;
- attempt count 1;
- `completed_passes = 0`;
- `pages_processed = 1`;
- source cursor `{ "offset": 1 }`;
- cleared lease ownership;
- one entity and one inbox row.

The harness then changes only the persisted schema status to `retired`.

A new invocation must return `SchemaRetired` before claiming the pending job. It must not:

- increment the attempt count;
- invoke the source;
- create another job;
- advance or clear the cursor;
- modify entity or inbox counts.

## Reactivation and same-job resume

After the exact persisted schema is returned to `active`, a newly constructed runner must
claim the existing pending job instead of inserting another row. It resumes from the
stored `{ "offset": 1 }` cursor and completes with:

- the same job UUID;
- attempt count 2;
- one page processed in the resumed invocation;
- one completed pass;
- one newly applied mutation;
- durable `pages_processed = 2`;
- a null terminal source cursor;
- exactly one job, two entities, and two inbox rows.

## Retired completed scope

The schema is retired again after success. Another invocation must return
`SchemaRetired` before the runner can resolve the retained succeeded job as
`AlreadyComplete`. The source and all durable counts remain unchanged.

Once the schema is active again, the same request returns `AlreadyComplete` for the
existing succeeded job without another source scan.

## PostgreSQL isolation

The target reads `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a
fallback. Without a PostgreSQL URL it reports a skip and succeeds.

Each invocation creates one unique PostgreSQL schema, creates the tenant owner fixture,
applies every real `IndexModule` migration, materializes canonical in-memory source/schema
registries, reads durable evidence, and drops the isolated schema.

No sleep, polling, wall-clock expiry, or concurrent race is used.

## Scope boundaries

This harness does not claim:

- authorized schema retirement or reactivation transport;
- schema migration or fingerprint replacement behavior;
- automatic scheduler handling for retired scopes;
- retry/backoff, dead-letter admission, or operator requeue;
- cancellation, lease-loss, heartbeat, takeover, or restart races;
- source-call timeout or pending-future preemption;
- source/index digest comparison, orphan cleanup, targeted/full/shadow repair,
  locale/partition dimensions, or complete drift repair.

The canonical M6 reconciliation and drift-repair item remains open.

## Suggested maintainer validation

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_schema_admission_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-schema-admission-harness.mjs
```
