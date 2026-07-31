# M7 bounded Product source reconciliation

Status: `source_complete_owner_execution_pending`

This slice adds a generic, explicit, bounded reconciliation capability that can replay the
already-published Product and ProductVariant sources through more than one complete cursor pass.
It does not change either source, any Product schema fingerprint, source name, replay event
identity, or owner revision contract.

The capability is implemented by `PostgresIndexReconciliationRunner`. It resolves the source
from `SharedIndexSourceRegistry`; callers provide only the tenant, exact schema, worker identity,
page/pass bounds, heartbeat cadence, and lease duration.

## Why a second pass exists

A cursor-ordered rebuild can miss an identity inserted behind the current cursor while the first
pass is running. Product uses `(product_id, locale)` and ProductVariant uses `variant_id`, so a
new lower key cannot be discovered by continuing the same pass.

A reconciliation job therefore performs a validated number of complete source passes. For the
first Product admission workflow, two passes are the recommended minimum:

1. the first pass replays the current owner projection;
2. the second pass restarts at the beginning and catches identities that appeared behind the
   first pass cursor, including retained Product or ProductVariant tombstones.

Repeated mutations remain safe because owner sources derive deterministic event UUIDs from the
exact tenant, entity, locale, schema-version domain, and source version. The Index mutation store
terminally recognizes duplicate or stale delivery.

## Durable job boundary

Reconciliation uses the existing `index_jobs` table with:

- `kind = 'reconcile'`;
- `scope_kind = 'schema'`;
- request contract `index_reconciliation_job_v1`;
- cursor contract `index_reconciliation_cursor_v1`.

The request stores the registry-resolved source name and requested pass count. A stored active or
succeeded request that differs from the current source/pass contract fails closed.

`index_jobs.cursor` stores only runner-owned state:

- completed pass count;
- the optional opaque source cursor for the current pass;
- cumulative page, mutation, applied, duplicate, and stale counters.

The source cursor remains an `IndexSourceCursor` and is never interpreted by Index. The existing
`index_checkpoints` rebuild cursor is not reused or reset, so reconciliation cannot corrupt the
M6 rebuild checkpoint or its succeeded job.

## Bounded execution and recovery

Each invocation validates:

- page limit through `IndexSourceScanRequest`;
- one through 1024 pages;
- heartbeat cadence within the invocation page budget;
- one through eight complete passes;
- a whole-second lease from one through 86400 seconds.

Every page is applied through `PostgresMutationStore`. The fenced job cursor is persisted only
after every mutation result is durable. A crash before that cursor update repeats the page with
the same deterministic delivery IDs. An expired attempt is reclaimed with an incremented
attempt fence; the old worker cannot persist progress or terminal state.

When the page budget is exhausted, the job yields to immediately claimable `pending` state. A
later invocation resumes the exact pass and source cursor. The final pass and terminal job state
are committed together. Cancellation is observed before and after page work and wins over
progress, success, failure, or yield when committed first.

The runner starts no task, sleeps nowhere, owns no retry schedule, and imports no Product,
ProductVariant, SalesChannel, Search, or Storefront implementation.

## Retained tombstones

Product replay already exposes a tenant-scoped union of live rows and
`product_index_tombstones`. ProductVariant replay does the same with
`product_variant_index_tombstones`. Reconciliation calls those unchanged registered sources, so
physical deletes and recreated identities participate in every pass without Index reading owner
tables directly.

Live/tombstone coexistence for one exact owner key remains a permanent source-record failure; the
runner does not choose one side.

## Concurrency limit

Multi-pass reconciliation narrows but does not eliminate the live-write window. An identity can
still be inserted behind the cursor during the final pass. This capability therefore does **not**
claim a repeatable-read owner snapshot, source watermark, quiescence barrier, or authoritative
consumer readiness.

Admission still requires one of the following to be proven separately:

- a repeatable-read/exported owner snapshot covering the whole pass;
- an owner high-watermark plus a complete incremental catch-up;
- an explicit write barrier/quiescence interval;
- or retained evidence that another reconciliation after the write window converged.

Until that proof and persisted tenant schema activation exist, Storefront must not cut over
authoritatively to Product v2.

## Explicitly open

- automatic scheduling, retry/backoff, dead-letter policy, and graceful task shutdown;
- a snapshot/watermark protocol that closes the final-pass concurrent-write window;
- incremental Product/translation/ProductVariant event delivery and acknowledgement;
- tombstone retention/purge admission against all relevant consumer checkpoints;
- persisted per-tenant Product v2 and ProductVariant v2 schema application;
- retained PostgreSQL insert-behind-cursor, delete/recreate, restart, lease-reclaim, drift, and
  convergence evidence;
- authoritative Storefront query cutover;
- durable Product/ProductVariant-to-SalesChannel UUID relations and cross-owner revisions.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, PostgreSQL execution, and CI are
maintainer-run. The implementation agent did not execute them.
