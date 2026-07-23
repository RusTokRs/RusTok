# rustok-iggy

## Purpose

`rustok-iggy` owns the Iggy-based event streaming transport for RusToK.

## Responsibilities

- Implement the RusToK event transport contract on top of Iggy.
- Own transport-level topology, serialization/deserialization, replay, DLQ helpers, and connector metadata hand-off for ack/retry coordination.
- Keep high-level event-streaming behavior separate from connector lifecycle concerns.
- Delegate bundled versus external connection management to `rustok-iggy-connector`.

## Entry points

- `IggyTransport`
- `IggyConfig`
- `TopologyManager`
- `ConsumedEvent`, `PersistentConsumerGroup`, and
  `IggyTransport::open_persistent_consumer_group`
- `DlqManager`
- `ReplayManager`

## Interactions

- Depends on `rustok-core` and shared event contracts for the transport abstraction.
- Uses `rustok-iggy-connector` for connection lifecycle and low-level message I/O.
- Can be used by `rustok-outbox` or other event-runtime layers that need streaming and replay semantics.
- Exposes only persistent consumer cursors for receive/acknowledgement; the
  former in-memory group manager and per-partition re-subscribe path are not
  safe for durable remote consumers and are intentionally absent.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
