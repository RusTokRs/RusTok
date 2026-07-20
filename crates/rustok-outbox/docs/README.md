# Documentation `rustok-outbox`

`rustok-outbox` is a core module for transactional event persistence and relay
infrastructure for the platform event runtime.

## Purpose

- publish the canonical runtime entry type `OutboxModule`;
- keep the write-side outbox contract and relay semantics outside the host layer;
- provide the platform with a unified transactional publishing contract for events.

## Scope

- `TransactionalEventBus` and atomic publish-with-transaction semantics;
- persistence into `sys_events` through transactional transport;
- relay, retry and DLQ semantics for the event runtime;
- module-owned Leptos admin package `rustok-outbox-admin` with FFA split `core/transport/ui` for read-only relay visibility.

## Relay, retry and DLQ policy

- Claim ownership is stored in `sys_events.claimed_by` and `sys_events.claimed_at`.
- PostgreSQL claim uses `FOR UPDATE SKIP LOCKED`; SQLite/test environment uses guarded update fallback.
- `claim_ttl` determines when a stuck claim can be reclaimed.
- `RelayConfig.max_concurrency` limits parallel dispatch.
- Retry uses exponential backoff from `backoff_base` to `backoff_max`.
- Runtime `apps/server` selects `max_attempts` from `rustok.events.dlq.max_attempts` if DLQ is enabled, otherwise from `rustok.events.relay_retry_policy.max_attempts`.
- A transient error leaves the event in `pending`, increments `retry_count`, writes `last_error`, clears the claim and sets `next_attempt_at`.
- A terminal error moves the event to `failed`/DLQ, saves `last_error`, clears the claim and is reflected in metrics/admin DLQ surface.

## Incident response

Primary owner for outbox/event delivery is the Platform foundation on-call. Escalation path: owner of `crates/rustok-outbox`, then owner of server runtime composition.

When backlog, retry or DLQ grows:

1. Check `/health/ready` and metrics `rustok_outbox_backlog_size`, `rustok_outbox_pending_lag_seconds`, `rustok_outbox_retries_total`, `rustok_outbox_dlq_total`.
2. Check worker state `worker:outbox_relay` and `rustok_runtime_worker_state{worker="outbox_relay"}`.
3. For stuck claims, verify `claim_ttl`, `claimed_by`, `claimed_at` and wait for reclaim or execute the standard operator procedure replay/requeue.
4. For DLQ, do not edit payload manually: first classify the error, confirm consumer idempotency and only then trigger redelivery.
5. After rollback or requeue, save evidence: affected event ids, retry counts, DLQ count, health snapshot and the final downstream consumer status.

## Integration

- used by `apps/server` for migrations, runtime relay bootstrap and event transport wiring;
- depends on `rustok-core` for module contracts and event transport abstractions, and on `rustok-api` for shared `PortContext`/`PortError` and write-policy primitives;
- exposes host-neutral relay and transactional event contracts; the host supplies runtime composition without an outbox framework adapter;
- can forward delivery to downstream transports like `rustok-iggy`, without owning provider-specific delivery semantics;
- remains a `Core` module regardless of the fact that part of the bootstrap wiring lives in the host runtime.
- module-level `health()` returns `Degraded` when host runtime evidence is unavailable; specific checks are at `/health/ready`.

## Verification

- `cargo xtask module validate outbox`
- `cargo xtask module test outbox`
- `node scripts/verify/verify-outbox-admin-boundary.mjs`
- `node scripts/verify/verify-outbox-admin-boundary.test.mjs`
- `npm run verify:outbox:fba`
- targeted event-runtime tests for transactional publish, relay and backlog semantics

### Reliability evidence

Transactional publish and relay failure modes are covered by targeted regression tests:

- `cargo test -p rustok-outbox --test transactional_events_integration_test`:
  - `test_transactional_event_publishing_rollback` confirms that a transaction rollback does not leave `sys_events`;
  - `test_transactional_event_publishing_commit` confirms that a commit creates one durable envelope in `Pending`;
  - `test_transactional_publish_rejects_non_outbox_transport` confirms fail-fast on incompatible transport.
- `cargo test -p rustok-outbox --test integration`:
  - `relay_retries_then_succeeds` covers transient error and redelivery;
  - `relay_moves_to_dlq_on_max_retry` covers terminal state/DLQ;
  - `relay_reclaims_stale_claims` covers reclaiming stuck claims;
  - `relay_bounds_parallel_dispatch` covers bounded concurrency;
  - `relay_processes_baseline_batch_with_bounded_latency` establishes a baseline for batch latency.

These tests cover transactional rollback/commit and relay retry/reclaim/DLQ semantics. Business idempotency of downstream consumers and restart E2E matrix should be confirmed by separate consumer-level scenarios, because outbox is responsible for durable delivery, not for the side effects of a specific recipient.

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
