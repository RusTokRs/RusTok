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
- An event is eligible for retention only after every registered consumer for
  its event type has acknowledged that event ID. This includes the optional
  age-based failed-event retention policy; pending events are never pruned.
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

## Sources of truth and ownership

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

## Non-goals

- This does not add remote broker cursor acknowledgements or prove Iggy
  consumer completion.
- This does not change event schemas, payloads, IDs, relay status, or retry/DLQ
  policy.
- This does not make a consumer's external side effects transactional; those
  remain protected by that owner's idempotency and receipt contract.
- This does not automatically unregister consumers when a feature flag is
  disabled.

## Data, transaction, and concurrency boundary

Registration is additive and durable. Disabling a worker does not silently
remove its retention obligation; this preserves the ability to resume after an
outage. A consumer must not retire its registration until all events for its
required event types have reached a terminal receipt and been acknowledged.
Retirement is an explicit owner/operator action, not an automatic side effect
of process shutdown or an environment flag. `OutboxConsumer::retire_if_drained`
refuses to remove a registration while any existing event of its registered
types is unacknowledged; the caller must first stop the consumer and quiesce
those event producers.

- Registration is idempotent and committed before the retention worker starts.
- Registration and retirement serialize through a PostgreSQL SHARE ROW
  EXCLUSIVE registry lock at READ COMMITTED. Retirement then acquires
  SHARE on sys_events with NOWAIT before drain reads. SQLite reserves its
  writer before the drain. Producers must still be quiesced operationally;
  events committed after retirement are not owed to the retired consumer.
- Drain checks require both the consumer key and event type on each receipt.
  Retirement preserves terminal receipts for the same consumer identity.
- ACK takes the registry SHARE lock with NOWAIT and locks the source and
  subscription rows through the owner transaction on PostgreSQL. SQLite
  reserves its writer. Contention requires owner rollback and retry. Only
  trusted startup backfill of terminal receipts may accept an absent event.
- Receipt and acknowledgement are committed in one owner transaction.
- Duplicate workers may race to acknowledge the same event; the unique key
  makes the operation idempotent.
- Retention rechecks the acknowledgement predicate in every event delete,
  not only in its candidate scan. PostgreSQL takes a SHARE lock on
  `sys_event_consumers` before the final READ COMMITTED delete statement,
  serializing against the ROW EXCLUSIVE lock acquired by registry writers.
  A newly committed registration therefore precedes the deletion snapshot or
  follows the deletion; it cannot retroactively recover already-pruned rows.
  SQLite serializes writes. The lock covers only the bounded event delete,
  not broker I/O or receipt pruning.
- If a consumer is unhealthy, retained rows may exceed the nominal retention
  window. This is intentional backpressure on storage, not permission to lose
  an unconsumed event.

## Context dimensions

The Outbox consumer retention guarantee operates across all tenants and channels.
Tenant identity is preserved inside the source event envelope. Consumer registration
and event acknowledgement keys are system-level and partitioned per database instance,
ensuring consistent multi-tenant retention guarantees without cross-tenant leakage.

## Events and projections

- Source events in `sys_events` remain the canonical outbox record.
- `sys_event_consumers` projects the active required consumer registry per event type.
- `sys_event_consumer_receipts` projects durable acknowledgements per consumer and event ID.
- Consumer tables (e.g. notifications receipts) remain autonomous projections that
  commit alongside the generic Outbox acknowledgement port.

## Failure semantics

- A consumer outage or retryable failure leaves the event unacknowledged, preserving it in `sys_events`.
- Permanent consumer rejections are recorded as terminal outcomes and acknowledged to Outbox.
- Storage failures during acknowledgement rollback the consumer transaction to prevent desynchronization.
- Retention queries skip unacknowledged events gracefully, applying backpressure rather than dropping data.

## Migration and cutover

Schema migration `m20261118_000001_add_sys_event_consumer_retention` creates
`sys_event_consumers` and `sys_event_consumer_receipts`. Deployments must apply
the schema before starting code that registers consumers. During a rolling upgrade,
old retention workers that do not understand consumer registrations must be drained
before relying on the new guarantee. New event types are registered before workers start,
and existing terminal receipts are backfilled prior to retention startup.

## Alternatives considered

- Make Outbox query consumer tables directly: rejected because it reverses dependency direction
  and tightly couples generic outbox infrastructure to individual module domains.
- Increase static retention windows: rejected because static timers merely shift the failure horizon
  without providing reliable durable delivery guarantees during extended outages.
- In-memory consumer tracking: rejected because process restarts, rolling upgrades, or crashes
  would lose unconsumed event records.

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

## Consumer ACK admission and retention bounds (source only)

PostgreSQL consumer ACK reserves the source event row exclusively with NOWAIT
and reads its subscription with NOWAIT under the registry SHARE fence. This
serializes canonical ACK writers for one event without waiting while holding
owner locks. Contention requires rollback and retry of the owner transaction.
Trusted backfill may skip a missing event only for a currently registered
consumer. Live ACK still requires an existing event and exact event type.
Source regressions are unrun; PostgreSQL multi-connection contention,
compilation and runtime verification remain open.

## Transaction-bound consumer effect admission (source only)

Notifications intake serializes terminal acceptance and rejection on the
canonical source row before reading either owner outcome. PostgreSQL uses
READ COMMITTED and FOR UPDATE NOWAIT; SQLite reserves its writer before
reading. Decode uses the locked source snapshot. Acceptance writes run in
a savepoint, rolled back before a permanent rejection is persisted; retryable
errors abort the whole transaction. An existing terminal outcome is replayed
without another owner write. A legacy contradictory accepted/rejected pair
fails closed and requires reviewed reconciliation. The terminal owner
outcome, source inbox and generic retention ACK commit together. No broker
or external I/O is allowed in this transaction.

OutboxConsumer::reserve_effect_in_transaction exposes the existing exact
consumer/event receipt insertion result as a transaction-local admission:
true means a new receipt, false means an exact prior committed receipt
(or one inserted earlier in the same transaction). Perform the SQL owner
effect only on true and commit receipt and effect together; roll back on
any error. It is not a reservation that can be committed separately and
does not cover external effects, broker replay after event retention, or
unregistered consumers. Existing late ACK/backfill semantics remain intact.
This does not automatically make arbitrary module handlers idempotent.
SQLite and PostgreSQL regression sources are authored, not executed.
