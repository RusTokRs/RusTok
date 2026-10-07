# rustok-outbox

## Purpose

`rustok-outbox` owns the canonical outbox transport and relay pipeline for reliable event delivery in RusToK, the bounded retention of delivered events, plus the generic durable receipt primitive used by owner write ports.

## Responsibilities

- Persist outbound events through the shared outbox transport.
- Relay pending events with claim, dispatch, retry, and DLQ semantics.
- Prune delivered events after a configured retention window, in bounded batches.
- Own the `sys_events` schema (table, claim index, retention index) and related migrations.
- Own the generic `owner_operation_receipts` schema and fenced idempotency
  admission/replay primitive; owner services retain their domain mutations and
  semantic side effects.
- Expose the runtime services used by `apps/server` event bootstrap and background delivery.
- Ship the module-owned Leptos admin UI package for relay visibility with a `core/transport/ui` FFA split.

## Entry points

- `OutboxModule`
- `OutboxTransport`
- `TransactionalEventWriter`
- `OutboxRelay`
- `OutboxRetention`
- `migration`
- `idempotency`

## Interactions

- Depends on `rustok-core` for module/event runtime contracts and transport abstractions.
- Depends on default `rustok-api` for neutral port context/error/write-policy primitives.
- Exposes only host-neutral transactional outbox and relay contracts; the
  object-safe `TransactionalEventWriter` lets an owner append an `EventEnvelope`
  through its live SeaORM transaction without depending on the concrete
  transport, while host composition remains outside the crate.
- The append-only platform migration invokes the Outbox-owned receipt schema
  helper. A receipt is scoped by tenant, owner slug, and idempotency key, so
  unrelated owner operations cannot collide.
- The `sys_events` table shape is defined once, by the owner helper
  `create_sys_events_schema`. Module and test schemas reach it through
  `SysEventsMigration`, and the platform wrapper `m20260211_000002_create_sys_events`
  orders it on the platform timeline instead of keeping a second copy of the DDL, so a
  column change lands in both migrators with one edit.
- Used by `apps/server` for runtime relay wiring, background processing, and migrations.
- Integrates with target transports such as `rustok-iggy` instead of owning transport-specific adapters inline.
- The Leptos admin UI lives in `crates/modules/rustok-outbox/admin`, keeps framework-agnostic DTO/view-model helpers in `admin/src/core.rs`, and is mounted through manifest-driven host wiring.

## Relay policy

- Claims are owned by a relay worker through `claimed_by` / `claimed_at`; PostgreSQL uses `FOR UPDATE SKIP LOCKED`, while SQLite/test runs use the guarded update fallback.
- The claim batch is read through `idx_sys_events_pending_created_at (status, created_at)`, the
  outbox-owned index `create_sys_events_claim_index` creates: the pending queue is read in created-at
  order and the claim-liveness / retry-schedule predicates are evaluated on that same ordered range.
  `idx_sys_events_pending_next_attempt` and `idx_sys_events_claimed_at` predate that index, serve no
  query on their own, and are dropped by the append-only migration
  `m20261007_000016_drop_sys_events_superseded_indexes`.
- `claim_ttl` defines when a stuck claim can be reclaimed by a later iteration.
- Dispatch concurrency is bounded by `RelayConfig.max_concurrency`.
- Retry uses exponential backoff from `backoff_base` up to `backoff_max`.
- `max_attempts` is resolved by the server runtime from `rustok.events.dlq.max_attempts` when DLQ is enabled, otherwise from `rustok.events.relay_retry_policy.max_attempts`.
- A retryable failure keeps the event in `pending`, increments `retry_count`, stores `last_error`, clears the claim and sets `next_attempt_at`.
- A terminal failure moves the event to `failed` (DLQ), preserves `last_error`, clears the claim and increments DLQ metrics.

## Delivered-event retention

- The outbox is both the delivery queue and the durable record of what was delivered, so retention
  only removes rows that already reached the transport: `status = 'dispatched'` with a non-null
  `dispatched_at` older than the window. `pending` rows still owe a delivery and `failed` rows are
  the DLQ an operator works through; neither is ever pruned.
- `OutboxRetention::prune_once` selects the oldest `batch_size` delivered rows through
  `idx_sys_events_dispatched_at (status, dispatched_at)`, deletes exactly those ids with the same
  status/age predicates, and reports `OutboxPruneReport { pruned, batch_exhausted }` — a full batch
  means another run has work to do, so the table is trimmed over several bounded runs instead of one
  long delete.
- Runtime wiring: `events.outbox_retention.{enabled,retention_days,batch_size,interval_seconds}`
  (defaults `true`, 30 days, 500, hourly), resolved by `apps/server` into the `outbox_retention`
  worker, guarded by the `worker:outbox_retention` readiness check, and reported through
  `rustok_outbox_pruned_total`, `rustok_outbox_retention_last_run_timestamp_seconds`,
  `rustok_runtime_worker_state{worker="outbox_retention"}` and
  `rustok_runtime_worker_failures_total{worker="outbox_retention"}`.
- `OutboxRetentionConfig::validate` rejects a non-positive window or a zero batch size, and the
  server rejects a window above ten years at boot instead of keeping every delivered row forever.
- The retention index is append-only for deployed schemas
  (`m20261007_000015_add_sys_events_retention_index`) and created by `SysEventsMigration` for
  module/test schemas.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../../docs/index.md)
