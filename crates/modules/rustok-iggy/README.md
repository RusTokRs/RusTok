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
