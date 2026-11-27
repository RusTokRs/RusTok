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
- Participate as a deployment-scoped runtime extension; the module trait uses
  the non-toggleable `Core` kind rather than a separate tenant-lifecycle kind.

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

- [CRATE_API.md](CRATE_API.md)
- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- No-compile source guardrail: `node scripts/verify/verify-iggy-connector-source.mjs` from the repository root
- [Platform docs index](../../../docs/index.md)

## Publication identity and acknowledgement boundary (2026-11-27)

Canonical root and sealed event UUIDs now populate the Iggy message header
ID without translation. Non-UUID publication keys, including the existing
DLQ replay attempt keys, map to a fixed namespaced UUIDv5; nil UUIDs and
empty keys are rejected. The namespace and the existing partition routing
must remain stable across retries. Explicit direct-mode publication awaits
the broker reply rather than accepting background enqueue as delivery.
No-SDK external connections and publications fail closed. The dedicated
poison-DLQ path retains its existing identity and partition mapping.
The strength of the broker response still depends on topic/server durability
and the reviewed broker deduplication policy. Successful direct send is not
a physical persistence proof or consumer cursor acknowledgement. An empty
confirmation list can be legitimate (including a duplicate response); no
broker offsets or persistence guarantees are inferred. Older publications
without the header identity cannot be retroactively deduplicated. Changing
topology, partition count or broker destination requires coordinated drain
and recovery. Do not treat SDK producer idempotence as cross-process Outbox
exactly-once delivery.

The relay sends invalid stored envelopes directly to its DLQ before invoking
the target. Every target error, including validation/serialization after a
remote send or a local projection, consumes the bounded ordinary retry
budget instead of assuming there was no side effect. Timeout outcomes remain
unconfirmed and may redeliver the same identity. A successful transport
reply followed by SQL failure or process death may also redeliver; the
existing fenced SQL acknowledgement and consumer idempotency still apply.
No transaction spans the database and the broker.
