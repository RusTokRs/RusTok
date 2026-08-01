# M5 mutation event acknowledgement contract

Status: `registry_and_commit_before_ack_contract_source_complete_broker_wiring_pending`

## Purpose

Bounded replay sources already provide deterministic generic `IndexMutation` values, inbox
idempotency, and monotonic source-version persistence. Incremental ingestion needs a separate
contract because a broker delivery also owns an acknowledgement token and may be redelivered
when acknowledgement is lost after the database transaction commits.

This slice adds the database-neutral registration and orchestration boundary. It does not select
or start a broker consumer.

## Event registry

`IndexMutationEventCatalog` registers one immutable route containing:

- owner module;
- versioned event domain;
- existing replay source name;
- one exact `SchemaRef`.

Every exact schema may have only one incremental event domain. Materialization fails closed unless
the named replay source exists, has the same owner module, and declares the exact schema. This
prevents incremental and full replay paths from silently using different mutation ownership or
schema contracts.

Event domains and source names are bounded machine identifiers. A delivery is rejected before
persistence when its event UUID, tenant UUID, or entity UUID is nil, when its source version is
zero, when its event domain is unknown, or when the mutation schema does not match the registered
route.

## Commit-before-ack ordering

`IndexMutationEventWorker` executes one delivery in this order:

1. resolve the exact event route;
2. validate the mutation schema;
3. call the existing `IndexReplayMutationSink` with the registered replay source name;
4. wait for a durable `Applied`, `Duplicate`, or `StaleIgnored` outcome;
5. acknowledge the exact opaque broker delivery token.

A mutation failure suppresses acknowledgement. Applied, duplicate, and stale outcomes are all
terminal deliveries and are acknowledged only after the sink returns.

If acknowledgement fails after durable persistence, the worker returns
`IndexMutationEventProcessError::Acknowledge`. The broker may redeliver the same logical event.
The existing inbox UUID deduplication and monotonic source-version checks own safety for that
redelivery; the worker does not invent a second ordering clock.

The acknowledgement token is an associated broker-adapter type. Index does not parse, log,
persist, derive, or compare it with the logical event UUID.

## Failure boundaries

Mutation persistence and acknowledgement failures expose bounded machine-readable codes and a
retryable/permanent classification. Raw broker payloads, database errors, transport details,
request context, tenant data, and acknowledgement tokens are not accepted by these failure types.

There is deliberately no distributed transaction between PostgreSQL and the broker. The supported
boundary is durable database commit followed by acknowledgement, with duplicate-safe redelivery
when the second step fails or the process exits between the two steps.

## Explicit non-claims

This slice does not add:

- Product, ProductVariant, or SalesChannel event routes;
- an Iggy, Kafka, NATS, AMQP, or other broker adapter;
- payload decoding or schema evolution for owner event envelopes;
- consumer groups, partition assignment, polling, tasks, or graceful shutdown;
- batch transactions, retry scheduling, backoff, dead-letter persistence, or lag metrics;
- PostgreSQL/broker crash execution evidence;
- persisted per-tenant schema readiness or Storefront cutover.

The M5 implementation-plan item remains open until selected owner modules register real event
routes and the server composes a broker consumer with retained commit/ack/redelivery evidence.

## Maintainer validation

Execution is maintainer-owned. Suggested commands:

```bash
cargo test -p rustok-index mutation_event -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-mutation-event-ack-contract.mjs
```
