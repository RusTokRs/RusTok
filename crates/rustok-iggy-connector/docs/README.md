# Documentation `rustok-iggy-connector`

`rustok-iggy-connector` is the connection abstraction layer for the Iggy transport
stack. It owns embedded/remote mode switching, connection lifecycle and
low-level message I/O, without taking away transport-level semantics from `rustok-iggy`.

## Purpose

- publish the canonical connector contract for the Iggy-based transport stack;
- keep embedded/remote mode switching and lifecycle management in a separate crate;
- provide `rustok-iggy` and other potential consumers with a unified low-level connector surface.

## Responsibilities

- `IggyConnector`, `RemoteConnector`, `EmbeddedConnector`, `ConsumerCursor`;
- `ConnectorConfig`, `PublishRequest`, `MessageSubscriber`, `SubscriberMessage`, `SubscriberMessageMetadata`, `ConnectorAckToken`, `ConnectorError`;
- connection lifecycle, mode abstraction and low-level publish/subscribe contracts;
- real remote consumer-group cursors via the Iggy SDK; a cursor keeps receive
  and offset acknowledgement on the same backend consumer and permits only one
  outstanding delivery;
- no ownership over transport-level serialization, DLQ, replay and topology policy;
- connector metadata includes only low-level facts (`stream`, `topic`, `partition`, optional `offset`, `message_id`, `delivery_attempt`, opaque `ack_token`) and does not define retry/DLQ/replay rules;
- `ConnectorAckToken` centralizes the simulated token and real Iggy SDK cursor token seam, and remote/embedded subscribers check scope token before ack without adding transport policy.

## Integration

- used by `rustok-iggy` as a low-level connection layer;
- must remain a separate connector crate without transport/business semantics;
- any changes to connector contracts must be synchronized with `rustok-iggy` docs and runtime expectations;
- SDK-disabled simulation remains explicit and cannot supply a persistent
  production consumer group.

## Verification

- targeted compile/tests for connector configuration, mode switching, request building and error handling;
- integration tests are needed when changing the real SDK/lifecycle path;
- structural verification for the boundary between connector and transport crate;
- no-compile guardrail for the current lifecycle seam: `node scripts/verify/verify-iggy-connector-source.mjs`.

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Documentation of `rustok-iggy`](../../rustok-iggy/docs/README.md)
