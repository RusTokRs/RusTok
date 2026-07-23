# Documentation `rustok-iggy-connector`

`rustok-iggy-connector` is the connection abstraction layer for the Iggy transport
stack. It owns bundled/external mode switching, connection lifecycle and
low-level message I/O, without taking away transport-level semantics from `rustok-iggy`.

## Purpose

- publish the canonical connector contract for the Iggy-based transport stack;
- keep bundled/external mode switching and lifecycle management in a separate crate;
- provide `rustok-iggy` and other potential consumers with a unified low-level connector surface.

## Responsibilities

- `IggyConnector`, `ExternalConnector`, `BundledConnector`, `ConsumerCursor`;
- `ConnectorConfig`, `PublishRequest`, `MessageSubscriber`, `SubscriberMessage`, `SubscriberMessageMetadata`, `ConnectorAckToken`, `ConnectorError`;
- `IggyConnectorControl`, its secret-safe settings DTOs, and readiness snapshot;
- connection lifecycle, mode abstraction and low-level publish/subscribe contracts;
- external startup tries each configured TCP address in order and fails only
  after every candidate fails; it does not silently discard configured peers;
- creates the configured stream/topics on the broker when transport topology
  setup requests them, using the supplied partition and replication settings;
- real external consumer-group cursors via the Iggy SDK; a cursor keeps receive
  and offset acknowledgement on the same backend consumer and permits only one
  outstanding delivery;
- no ownership over transport-level serialization, DLQ, replay and topology policy;
- connector metadata includes only low-level facts (`stream`, `topic`, `partition`, optional `offset`, `message_id`, `delivery_attempt`, opaque `ack_token`) and does not define retry/DLQ/replay rules;
- `ConnectorAckToken` centralizes the Iggy SDK cursor token seam, and subscribers check scope tokens before acknowledgement without adding transport policy.

## Integration

- used by `rustok-iggy` as a low-level connection layer;
- must remain a separate connector crate without transport/business semantics;
- any changes to connector contracts must be synchronized with `rustok-iggy` docs and runtime expectations;
- disabled SDK support fails configuration explicitly and cannot supply a
  persistent consumer group.
- the module-owned migration persists only connection metadata and secret
  references; the host adapter resolves values through `rustok-secrets`.

## Verification

- targeted compile/tests for connector configuration, mode switching, request building and error handling;
- integration tests are needed when changing the real SDK/lifecycle path;
- structural verification for the boundary between connector and transport crate;
- no-compile guardrail for the current lifecycle seam: `node scripts/verify/verify-iggy-connector-source.mjs`.

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Documentation of `rustok-iggy`](../../rustok-iggy/docs/README.md)
