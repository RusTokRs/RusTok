# rustok-outbox

## Purpose

`rustok-outbox` owns the canonical outbox transport and relay pipeline for reliable event delivery in RusToK, plus the generic durable receipt primitive used by owner write ports.

## Responsibilities

- Persist outbound events through the shared outbox transport.
- Relay pending events with claim, dispatch, retry, and DLQ semantics.
- Own the `sys_events` schema and related migrations.
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
- Used by `apps/server` for runtime relay wiring, background processing, and migrations.
- Integrates with target transports such as `rustok-iggy` instead of owning transport-specific adapters inline.
- The Leptos admin UI lives in `crates/rustok-outbox/admin`, keeps framework-agnostic DTO/view-model helpers in `admin/src/core.rs`, and is mounted through manifest-driven host wiring.

## Relay policy

- Claims are owned by a relay worker through `claimed_by` / `claimed_at`; PostgreSQL uses `FOR UPDATE SKIP LOCKED`, while SQLite/test runs use the guarded update fallback.
- `claim_ttl` defines when a stuck claim can be reclaimed by a later iteration.
- Dispatch concurrency is bounded by `RelayConfig.max_concurrency`.
- Retry uses exponential backoff from `backoff_base` up to `backoff_max`.
- `max_attempts` is resolved by the server runtime from `rustok.events.dlq.max_attempts` when DLQ is enabled, otherwise from `rustok.events.relay_retry_policy.max_attempts`.
- A retryable failure keeps the event in `pending`, increments `retry_count`, stores `last_error`, clears the claim and sets `next_attempt_at`.
- A terminal failure moves the event to `failed` (DLQ), preserves `last_error`, clears the claim and increments DLQ metrics.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
