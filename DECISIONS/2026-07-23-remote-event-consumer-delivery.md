# ADR: Remote event consumer delivery

- Status: accepted
- Date: 2026-07-23

## Context

An outbox establishes durable publication, but a remote consumer still needs a
durable broker cursor, exact acknowledgement, restart behavior, and owner-side
idempotency. An in-memory group registry that receives through one subscriber
and acknowledges through another cannot prove which broker offset it commits.

The module-build dispatcher already has an owner transaction that records a
terminal build result or recognizes an already-settled request. It is the first
appropriate consumer to establish the platform contract without creating a
host-owned generic business consumer.

## Decision

Remote consumers use `PersistentConsumerGroup` or
`PersistentContractConsumerGroup`. Each retains one connector cursor across
receive and acknowledgement and allows one unacknowledged delivery at a time.
The owner validates the delivery, persists a terminal result or durable
idempotent recognition, then acknowledges that exact delivery. A failure before
acknowledgement terminates the owner worker so its deployment supervisor
restarts it and the broker can redeliver.

The in-memory `ConsumerGroupManager` and the per-partition receive/re-subscribe
acknowledgement API are removed. The first concrete owner is
`rustok-module-build-dispatcher` on the dedicated `module-build` topic. Its
remote Iggy connection requires TLS and the dispatcher does not fall back to a
server-local polling or build path.

## Consequences

- Remote delivery is at-least-once; consumers must use durable idempotency.
- Receive and acknowledgement are traceable to the same connector cursor.
- Invalid or failed deliveries remain uncommitted for restart/redelivery rather
  than being silently skipped.
- A real-broker multi-replica restart/replay test remains required before the
  contract can be considered fully verified for more consumer owners.
