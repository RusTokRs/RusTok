# M6 reconciliation stale-version guard PostgreSQL harness

Status: executable target retained, not run.

## Purpose

This harness retains the canonical reconciliation runner's durable monotonic source-version boundary on PostgreSQL. It proves that a stale mutation is terminally acknowledged in `index_inbox`, counted as `StaleIgnored`, and allowed to advance reconciliation progress without changing the current entity projection.

The source owns one entity and emits four single-mutation pages with distinct stable event identities:

1. upsert source version `3`;
2. stale delete source version `2`;
3. fresh delete source version `4`;
4. stale upsert source version `3`.

The sequence covers both stale deletion of a live entity and stale resurrection after a newer tombstone.

## Attempt one: stale delete

The first invocation has a two-page budget.

Page one applies the source-version-3 upsert. Page two uses a new event UUID but an older source version and must return `StaleIgnored` from the mutation store.

The invocation must yield with:

- the original reconciliation job UUID;
- attempt count `1`;
- two processed pages;
- zero completed passes;
- one applied mutation;
- one stale mutation;
- one heartbeat;
- source cursor `{ "offset": 2 }`.

PostgreSQL must retain one live entity at source version `3` with the original payload. Both event identities, including the stale delete, must have terminal `applied` inbox rows with non-null completion timestamps. The pending job must persist `applied_count = 1`, `stale_count = 1`, and no failure diagnostic.

This boundary demonstrates that acknowledging a stale event does not mean applying its state transition.

## Attempt two: tombstone and stale resurrection

A newly constructed runner claims the same pending job as attempt `2` and resumes from `{ "offset": 2 }`.

Page three applies the source-version-4 delete. Page four uses another new event UUID for an upsert at source version `3`; it must be terminally acknowledged as stale and must not resurrect the entity.

The invocation must complete with two pages, one completed pass, one applied mutation, one stale mutation, and one heartbeat.

Final durable evidence requires:

- the same job UUID in `succeeded` state;
- attempt count `2`;
- four total processed pages;
- four total mutations;
- two applied mutations;
- zero duplicates;
- two stale mutations;
- a null terminal source cursor;
- one entity row at source version `4`;
- `is_deleted = true` and a null payload;
- four distinct terminal `applied` inbox rows;
- zero links.

A later invocation must return `AlreadyComplete` for the same succeeded job without another source scan.

## PostgreSQL isolation

The target reads `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as fallback. Without a PostgreSQL URL it reports a skip and succeeds.

Each invocation creates a unique schema, creates the tenant fixture, applies every real `IndexModule` migration, persists one active schema, materializes the canonical source/schema registries, reads durable evidence, and drops the schema.

No sleep, polling delay, elapsed-time expiry, concurrent worker race, direct `index_jobs` mutation, or production code change is used.

## Scope boundaries

This harness does not change production code, migrations, reconciliation SQL/state-machine, mutation identity, source/cursor contracts, schema contracts, diagnostics, or public APIs.

It does not add automatic retry/backoff, failed-scope dead-letter admission, authorized requeue, cancellation, lease takeover, source timeout, scheduler ownership, digest comparison, orphan cleanup, targeted/full/shadow repair, locale/partition checkpoint dimensions, or complete drift repair.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Suggested maintainer validation

These commands were intentionally not run by the implementation agent:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_stale_version_guard_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-stale-version-guard-harness.mjs
```
