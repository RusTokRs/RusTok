# rustok-iggy

## Purpose

`rustok-iggy` owns the Iggy-based event streaming transport for RusToK.

## Responsibilities

- Implement the RusToK event transport contract on top of Iggy.
- Own transport-level topology, serialization, replay, and DLQ helpers.
- Keep high-level event-streaming behavior separate from connector lifecycle concerns.
- Delegate embedded-vs-remote connection management to `rustok-iggy-connector`.

## Entry points

- `IggyTransport`
- `IggyConfig`
- `TopologyManager`
- `ConsumerGroupManager`
- `DlqManager`
- `ReplayManager`

## Interactions

- Depends on `rustok-core` and shared event contracts for the transport abstraction.
- Uses `rustok-iggy-connector` for connection lifecycle and low-level message I/O.
- Can be used by `rustok-outbox` or other event-runtime layers that need streaming and replay semantics.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
