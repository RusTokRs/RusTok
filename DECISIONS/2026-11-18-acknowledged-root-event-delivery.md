# Acknowledged local root-event delivery

- Date: 2026-11-18
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-core, apps/server, rustok-outbox
- Extends: [Sealed typed event families](./2026-07-17-sealed-typed-event-families.md)
- Supersedes: None
- Superseded by: None

## Context

The sealed-family delivery path already waits for matching local module
handlers and returns failures to the durable Outbox relay. Established root
`DomainEvent` records followed a different path: the relay published them to
an in-memory broadcast channel and could mark the outbox row dispatched before
the matching local module projections completed. The dispatcher retried a
failing handler locally, but after its retry budget the error did not reach the
Outbox relay. In the Iggy profile, remote acceptance could therefore be
mistaken for successful local projection.

The root event schema, wire representation, and publisher API do not need to
change to make the local acknowledgement boundary reliable.

## Decision

The Outbox relay's root-event delivery path uses the existing explicit
`EventHandler` / module listener registry and awaits every matching root
handler before returning success to the relay.

- The listener registry must be bound and active. An absent or inactive
  registry fails closed.
- An explicitly initialized empty registry is valid for root events with no
  matching local module handler.
- Handler retries use the existing dispatcher configuration and bounded
  handler capacity. Handler, capacity, task, and shutdown failures propagate
  to the Outbox relay.
- The single-node `outbox` profile acknowledges a root event only after local
  module handlers complete.
- The `outbox_iggy` profile sends the unchanged envelope to the configured
  primary first, then awaits local module handlers. If local delivery fails,
  the Outbox row remains retryable and the same event identity may be sent to
  Iggy again.
- The generic `EventBus.publish_envelope` broadcast API remains best-effort.
  The stronger guarantee applies only to the Outbox relay's
  `publish_root_envelope_and_wait` path and registered local module handlers.

## Sources of truth and ownership

- `rustok-events` owns the root event schema and envelope validation.
- The publishing domain remains the source of truth for business state and
  writes transactional events through `TransactionalEventBus`.
- `rustok-outbox` owns durable event persistence, retry, and DLQ policy.
- `rustok-core` owns the shared local `EventHandler` registry and
  acknowledged handler invocation.
- `apps/server` composes the selected relay target and lifecycle. It does not
  reconstruct domain facts or add a second event queue.

## Invariants

- A root outbox row is not acknowledged while a matching registered local
  module handler has failed.
- The original envelope identity is retained across relay retries.
- The active listener registry is required even when it contains no handlers.
- A local handler must be idempotent because the relay may redeliver after a
  handler succeeds but before the outbox acknowledgement commits.
- Remote consumers must be idempotent because Iggy may receive the same
  envelope more than once after partial local success.
- Root event wire formats and schema digests remain unchanged.

### Allowed states

- A live empty registry acknowledges a root event with no matching handler.
- A live registry acknowledges after every matching handler succeeds.
- A local failure after remote Iggy acceptance leaves the Outbox row retryable.

### Forbidden states

- An unbound or stopped registry is interpreted as an empty registry.
- A handler failure is logged and swallowed while the Outbox row is marked
  dispatched.
- A local broadcast send alone is treated as proof that module projections
  completed.

## Non-goals

- This does not add a remote persistent receive/cursor acknowledgement.
- This does not broadcast root Outbox events to arbitrary `EventBus` subscribers; only registered module handlers participate in the acknowledged Outbox path.
- This does not make the generic `EventBus.publish_envelope` broadcast API
  durable or acknowledged.
- This does not make an in-process EventBus publication crash-safe before the
  forwarder inserts it into the outbox.
- This does not introduce a second bus, in-memory fallback queue, or
  domain-specific host publisher.

## Data, transaction, and concurrency boundary

The business mutation and canonical event row remain atomic under the owner's
transaction through `TransactionalEventBus`. The relay owns the later
delivery transaction and only marks the row dispatched after the selected
transport returns success.

Local root handlers execute under the existing bounded listener capacity.
The same envelope ID is used for every retry. If a remote primary accepts the
event but local projection fails, the remote send can repeat; durable
consumer-side idempotency is required.

The legacy in-process EventBus forwarder is a compatibility bridge, not a
substitute for a transactional publisher. It retries transient persistence
failures and drains on graceful shutdown, but process failure before the first
outbox insert can still lose an in-memory publication.

## Context dimensions

Tenant identity and trace metadata remain part of the canonical root envelope
and are validated by the existing event contract. The delivery guarantee is
process-wide and does not vary by tenant, actor, locale, or module settings.

## Events and projections

Root `DomainEvent` and sealed contract families share the same local module
listener registry. Root events retain their existing schema and serialization.
A matching handler completes its projection before the relay acknowledgement;
replays use the original event ID and consumers recompute from authoritative
owner state.

## Failure semantics

- Invalid envelopes fail before transport delivery.
- Missing/inactive listener registry fails before irreversible remote
  publication.
- Handler/capacity/task failures return to Outbox retry/DLQ.
- A failure after Iggy acceptance may cause at-least-once remote delivery.
- A forced process abort before an in-memory bridge publication is persisted
  remains outside the durable Outbox guarantee.

## Migration and cutover

Not applicable. The implementation is additive to the local delivery API and
does not change the event schema, wire format, or persisted outbox row shape.

## Alternatives considered

- Keep root broadcast best-effort: rejected because the relay could acknowledge
  a row before local module projections completed.
- Add a second durable queue for root events: rejected because the existing
  Outbox and listener registry already own persistence and local dispatch.
- Change root event schemas to carry acknowledgement metadata: rejected because
  acknowledgement is a transport/runtime concern and does not change the
  canonical domain fact.

## Verification

Before merge, run the scoped Rust formatting, core/server checks, root and
sealed delivery tests, server event-transport tests, and Outbox transport
regressions listed in `apps/server/docs/implementation-plan.md`. Real Iggy
restart/cursor behavior remains a separate deployment verification gate.

## Consequences

Root and sealed local module projections now share one acknowledgement
boundary. A local projection failure can retain and retry the original outbox
row, including after a remote Iggy send. This strengthens correctness at the
cost of additional local handler latency and possible duplicate remote sends.
The generic broadcast API and pre-outbox in-memory bridge retain their weaker
semantics and must not be used for transactional business events.
