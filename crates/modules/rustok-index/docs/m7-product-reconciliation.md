# M7 bounded Product source reconciliation

Status: `source_complete_owner_execution_pending`

This capability replays the current Product and ProductVariant sources through more than one complete
cursor pass. It does not own Product schema compatibility, source mutation, or owner writes.

`PostgresIndexReconciliationRunner` resolves the source from `SharedIndexSourceRegistry`; callers
provide tenant, exact current schema reference, worker identity, page/pass bounds, heartbeat cadence,
and lease duration.

## Why a second pass exists

A cursor-ordered rebuild can miss an identity inserted behind the current cursor while the first pass
is running. Product uses `(product_id, locale)` and ProductVariant uses `variant_id`, so a new lower key
cannot be discovered by continuing the same pass.

A reconciliation job therefore performs a bounded number of complete source passes. For the
first Product admission workflow, two passes are the recommended minimum:

1. replay the current owner projection;
2. restart at the beginning and catch identities that appeared behind the first cursor, including
   retained Product or ProductVariant tombstones.

Repeated mutations are safe because the current sources derive deterministic event UUIDs from exact
tenant/entity/locale, the canonical source event domain, and mutation source version. The mutation
store recognizes duplicate and stale delivery.

## Durable job boundary

Reconciliation uses `index_jobs` with:

- `kind = 'reconcile'`;
- `scope_kind = 'schema'`;
- request contract `index_reconciliation_job_v1`;
- cursor contract `index_reconciliation_cursor_v1`.

Those `*_v1` names are generic durable Index job-format contracts, not Product schema compatibility
versions.

The request stores registry-resolved source name and pass count. Active/succeeded stored requests that
differ from the current source/pass contract fail closed.

`index_jobs.cursor` stores completed pass count, optional opaque current source cursor, and cumulative
page/mutation/outcome counters. Index does not interpret the source-owned cursor.

## Bounded execution and recovery

Each invocation validates bounded page size, page budget, heartbeat cadence, one through eight complete
passes, and a one-through-86400-second lease.

Every page is applied through `PostgresMutationStore`. Fenced progress is persisted only after mutation
results are durable. A crash before cursor update repeats the page with the same deterministic delivery
identities. Expired attempts are reclaimed under a new attempt fence; old workers cannot commit
progress or terminal state.

When page budget is exhausted, the job yields to claimable pending state. A later invocation resumes
the exact pass and source cursor. Cancellation is observed around page work and wins when committed
first.

The runner starts no task, sleeps nowhere, owns no retry schedule, and imports no Product,
ProductVariant, SalesChannel, Search, or Storefront implementation.

## Retained deletes and canonical Product graph

Product replay combines live localized owner state with `product_index_tombstones`, requires current
Product graph projection state, and uses `projection_epoch` as the complete Product mutation clock.
ProductVariant replay combines live owner state with `product_variant_index_tombstones` and uses its
owner revision.

Reconciliation calls these registered sources and does not read Product tables directly.

Live/tombstone coexistence for one exact key remains a permanent source-record failure.

## Concurrency limit

Multi-pass reconciliation narrows but does not eliminate the live-write window. This capability
therefore does **not** claim a repeatable-read owner snapshot, source watermark, quiescence barrier,
or authoritative consumer readiness.

For canonical Product graph use, relation freshness is an additional independent gate: Product
visibility or Channel identity may change before the bounded relation resolver converges. Durable
Product/Channel triggering or an admitted freshness watermark is required separately.

## Still open

- snapshot/watermark admission that closes final-pass concurrent writes;
- durable Product/Channel relation convergence/freshness triggering;
- typed Product/ProductVariant event delivery after event-contract digest admission;
- tombstone retention/purge admission against consumer checkpoints;
- persisted readiness evidence for the one current Product/ProductVariant/SalesChannel schema set;
- retained PostgreSQL insert-behind-cursor, delete/recreate, restart, lease-reclaim, drift, and
  convergence evidence;
- authoritative Storefront query cutover.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, PostgreSQL execution, migrations, workflows, and CI
are maintainer-run. The implementation agent did not execute them.
