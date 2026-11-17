# rustok-iggy

## Purpose

`rustok-iggy` owns the Iggy-based event streaming transport for RusToK.

## Responsibilities

- Implement the RusToK event transport contract on top of Iggy.
- Own transport-level topology, serialization/deserialization, DLQ helpers, and connector metadata hand-off for ack/retry coordination.
- Keep high-level event-streaming behavior separate from connector lifecycle concerns.
- Delegate bundled versus external connection management to `rustok-iggy-connector`.

## Entry points

- `IggyTransport`
- `IggyConfig`
- `TopologyManager`
- `ConsumedEvent`, `PersistentConsumerGroup`, and
  `IggyTransport::open_persistent_consumer_group`
- `DlqManager`

## Interactions

- Depends on `rustok-core` and shared event contracts for the transport abstraction.
- Uses `rustok-iggy-connector` for connection lifecycle and low-level message I/O.
- Can be used by `rustok-outbox` or other event-runtime layers that need durable streaming semantics.
- Exposes only persistent consumer cursors for receive/acknowledgement; the
  former in-memory group manager and per-partition re-subscribe path are not
  safe for durable remote consumers and are intentionally absent.
- The Iggy SDK cursor uses explicit acknowledgement. Polling and shutdown do
  not commit an offset the owner has not acknowledged.
- Persistent contract readers accept the canonical legacy root format and the
  sealed typed format on shared topics. A legacy root is lifted to the typed
  reader with the same envelope identity and broker acknowledgement metadata.

## Docs

- [CRATE_API.md](CRATE_API.md)
- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../../docs/index.md)
