# Outbox retention honors registered durable consumers

- Date: 2026-11-18
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-outbox, rustok-notifications, apps/server
- Extends: [Outbox runtime](./2026-03-11-queue-runtime-source-of-truth-outbox.md)
- Supersedes: None
- Superseded by: None

## Context

The Notifications outbox-intake worker reads committed `sys_events` rows and
persists accepted receipts or permanent rejections in notification-owned
tables. The Outbox retention worker independently prunes dispatched rows by
age. If intake is disabled, unavailable, or backlogged beyond the retention
window, an event can be deleted before Notifications records a terminal
receipt. The notification owner then cannot recover the source envelope.

The notification receipt tables are private to Notifications. Making Outbox
query those tables would reverse ownership and couple generic retention to one
consumer. A fixed larger retention window only moves the loss boundary.

## Decision

Outbox owns a generic, durable consumer-registration and acknowledgement
contract:

- A consumer registers the exact event types it requires before its worker and
  Outbox retention are started.
- A dispatched event is eligible for retention only after every registered
  consumer for its event type has acknowledged that event ID.
- A consumer acknowledges only after its own durable terminal receipt has
  committed. Retryable failures do not acknowledge.
- Where a consumer's terminal receipt and Outbox acknowledgement share the
  same database, the owner invokes the Outbox acknowledgement port in the same
  transaction as its receipt. A failure rolls back both.
- Acknowledgements are idempotent and scoped by consumer key and event ID.
- Multiple consumers may require the same event type; every registered
  consumer must acknowledge before the event can be pruned.
- The generic Outbox schema and retention code do not read consumer-owned
  receipt tables.

Notifications registers its supported event types before
`connect_runtime_workers_with_runtime` starts the retention worker. The
server supplies a neutral receipt port to Notifications so accepted receipts
and permanent rejections atomically acknowledge the corresponding Outbox
event. Existing terminal intake receipts are backfilled before retention is
started.

## Ownership and schema

- `rustok-outbox` owns `sys_event_consumers` and
  `sys_event_consumer_receipts`, their migrations, and the retention predicate.
- A consumer key and event type form one required subscription.
- An acknowledgement is keyed by the original Outbox event ID and consumer
  key. The receipt references `sys_events` and is cascade-deleted when the
  canonical event is pruned.
- Notifications owns its source inbox, accepted intake receipt, and permanent
  rejection. Outbox owns only the generic delivery acknowledgement.
- `apps/server` composes the owner port and orders registration/backfill
  before retention startup. It does not query notification-owned tables.

## Invariants

- An event with at least one registered, unacknowledged consumer is never
  deleted by Outbox retention, regardless of its age or dispatched status.
- A consumer outage or retryable failure retains the original event.
- A permanent rejection is a terminal consumer outcome and may be acknowledged
  after that rejection is durable.
- A failure to persist the Outbox acknowledgement cannot leave a durable
  notification receipt without a matching acknowledgement: both commit in the
  same database transaction.
- Replaying an already acknowledged event does not create duplicate receipts
  or change the canonical event.
- No registered consumer means the existing age-based retention behavior is
  unchanged.

## Registration lifecycle

Registration is additive and durable. Disabling a worker does not silently
remove its retention obligation; this preserves the ability to resume after an
outage. A consumer must not retire its registration until all events for its
required event types have reached a terminal receipt and been acknowledged.
Retirement is an explicit owner/operator action, not an automatic side effect
of process shutdown or an environment flag. `OutboxConsumer::retire_if_drained`
refuses to remove a registration while any existing event of its registered
types is unacknowledged; the caller must first stop the consumer and quiesce
those event producers.

New event types are registered before the intake worker starts. Existing
terminal Notifications receipts are backfilled to Outbox acknowledgements
before retention starts; already-pruned event IDs are harmless during that
backfill because the owning terminal receipt is already durable.

## Failure and concurrency semantics

- Registration is idempotent and committed before the retention worker starts.
- Receipt and acknowledgement are committed in one owner transaction.
- Duplicate workers may race to acknowledge the same event; the unique key
  makes the operation idempotent.
- Retention rechecks the acknowledgement predicate in its delete statement,
  not only in its candidate scan.
- If a consumer is unhealthy, retained rows may exceed the nominal retention
  window. This is intentional backpressure on storage, not permission to lose
  an unconsumed event.
- Deployments must apply the new schema before starting code that registers
  consumers. During a rolling upgrade, old retention workers that do not
  understand consumer registrations must be drained before relying on the new
  guarantee.

## Non-goals

- This does not add remote broker cursor acknowledgements or prove Iggy
  consumer completion.
- This does not change event schemas, payloads, IDs, relay status, or retry/DLQ
  policy.
- This does not make a consumer's external side effects transactional; those
  remain protected by that owner's idempotency and receipt contract.
- This does not automatically unregister consumers when a feature flag is
  disabled.

## Verification

Before merge, run the focused Outbox storage/retention tests, Notifications
outbox-intake tests, server worker/bootstrap tests, migration composition
checks, and the PostgreSQL migration/retention contention gate. The maintainer
owns runtime verification in the target environment.

## Consequences

Notifications can be unavailable longer than the configured Outbox retention
window without losing events it has not yet accepted or permanently rejected.
The tradeoff is that unacknowledged rows remain stored until the consumer
recovers or its registration is explicitly retired. The same mechanism can
protect future durable consumers without teaching Outbox their domain schema.
