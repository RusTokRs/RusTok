# rustok-iggy-connector

## Purpose

`rustok-iggy-connector` owns the connection abstraction for Iggy transports in RusToK.

## Responsibilities

- Provide the connector trait used by transport layers.
- Support bundled and external Iggy connection modes.
- Own low-level connection lifecycle and publish/subscribe mechanics.
- Expose connector-owned subscriber metadata (`offset`, `message_id`, `delivery_attempt`, opaque `ack_token`) for offset/ack/retry coordination without defining transport retry, DLQ, or replay policy.
- Keep `ConnectorAckToken` as the connector-owned Iggy-SDK acknowledgement seam; subscribers validate token scope before ack and higher layers still treat tokens as opaque.
- Keep connector concerns separate from higher-level event transport behavior.

## Entry points

- `IggyConnector`
- `BundledConnector`
- `ExternalConnector`
- `IggyConnectorControl` / `SharedIggyConnectorControl`
- `ConnectorConfig`
- `PublishRequest`
- `SubscriberMessage` / `SubscriberMessageMetadata`
- `ConsumerCursor` for persistent external consumer-group receive/ack
- `ConnectorAckToken`

## Interactions

- Used by `rustok-iggy` as the low-level connection/backend layer.
- Bundled mode manages the Iggy artifact packaged by the module on a supported
  host; it never substitutes an in-memory broker. External mode connects to an
  operator-managed Iggy deployment.
- Can be reused by other transport experiments without pulling in `rustok-iggy` transport policy.
- Keeps Iggy SDK specifics and connector-mode switching out of higher-level runtime crates.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- No-compile source guardrail: `node scripts/verify/verify-iggy-connector-source.mjs` from the repository root
- [Platform docs index](../../docs/index.md)
