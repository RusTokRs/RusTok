# M5 mutation event acknowledgement contract

Status: `generic_contract_complete_social_graph_route_source_complete_runtime_execution_pending`.

## Purpose

Bounded replay sources already provide deterministic generic `IndexMutation` values, inbox
idempotency, and monotonic source-version persistence. Incremental ingestion needs a separate
contract because a broker delivery also owns an acknowledgement token and may be redelivered
when acknowledgement is lost after the database transaction commits.

The database-neutral registration and orchestration boundary is complete. Social Graph is now the
first owner to register one exact production route and matching bounded replay source. Its existing
Iggy consumer remains the concrete broker adapter and acknowledgement owner. Product-family routes
and new retained runtime evidence remain separate work.

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
persistence when its event UUID, tenant UUID, or entity UUID is nil, when its source version is zero,
when its event domain is unknown, or when the mutation schema does not match the registered route.

PostgreSQL source factories and mutation routes are now materialized atomically. Factories register
into a staged source catalog; route owner/source/schema validation runs against that same staged
catalog; and neither catalog is published when any factory or route fails.

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
`IndexMutationEventProcessError::Acknowledge`. The broker may redeliver the same logical event. The
existing inbox UUID deduplication and monotonic source-version checks own safety for that redelivery;
the worker does not invent a second ordering clock.

The acknowledgement token is an associated broker-adapter type. Index does not parse, log, persist,
derive, or compare it with the logical event UUID.

## First production route: Social Graph

Social Graph registers the exact route and source identity:

```text
event domain: social_graph.relation.state_changed.v1
source name:  social_graph.relation.state_changed.v1
schema:       rustok-social-graph/relation/v1
owner:        social_graph
```

The bounded source scans and loads authoritative `social_graph_relations` rows. The existing Iggy
worker already owns persistent receive/ack, retry/backoff, DLQ/poison receipts, graceful shutdown,
and runtime consumer metrics. The new route prevents that concrete consumer from remaining an
unregistered side path relative to generic replay ownership.

See [M5 Social Graph production mutation route](./m5-social-graph-mutation-route.md).

## Failure boundaries

Mutation persistence and acknowledgement failures expose bounded machine-readable codes and a
retryable/permanent classification. Raw broker payloads, database errors, transport details, request
context, tenant data, and acknowledgement tokens are not accepted by these failure types.

There is deliberately no distributed transaction between PostgreSQL and the broker. The supported
boundary is durable database commit followed by acknowledgement, with duplicate-safe redelivery when
the second step fails or the process exits between the two steps.

## Remaining non-claims

This boundary still does not add:

- Product, ProductVariant, or SalesChannel event routes;
- a second Iggy, Kafka, NATS, AMQP, or other generic broker adapter;
- generic payload decoding or schema evolution for arbitrary owner event envelopes;
- generic consumer groups, partition assignment, polling tasks, or graceful shutdown;
- generic batch transactions, retry scheduling, backoff, dead-letter persistence, or lag metrics;
- new PostgreSQL/broker crash execution evidence for the registered route;
- persisted per-tenant schema readiness or Storefront cutover.

The M5 implementation-plan item remains partially open until the remaining selected owners register
real event routes and retained commit/ack/redelivery evidence is admitted.

## Maintainer validation

Execution is maintainer-owned. Suggested commands:

```bash
cargo test -p rustok-index mutation_event -- --nocapture
cargo test -p rustok-index source_factory -- --nocapture
cargo test -p rustok-social-graph --features index-consumer index_source -- --nocapture
node scripts/verify/verify-index-mutation-event-ack-contract.mjs
node scripts/verify/verify-index-social-graph-mutation-route.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-social-graph --features index-consumer --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, PostgreSQL/Iggy scenarios, workflows, or CI were
run by the implementation agent.
