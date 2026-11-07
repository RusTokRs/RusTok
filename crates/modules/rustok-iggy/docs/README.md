# Documentation `rustok-iggy`

`rustok-iggy` is a transport crate for streaming event delivery based on Iggy. It
implements `EventTransport` and holds transport-level abstractions over
`rustok-iggy-connector`, without owning the connection/mode lifecycle itself.

## Purpose

- publish the canonical Iggy-based `EventTransport` surface for the platform;
- hold serialization, topology, DLQ and consumer-group abstractions inside the transport crate;
- separate transport behavior from connector-level connection management.

## Responsibilities

- `IggyTransport` and transport-facing configuration;
- JSON/MessagePack serialization and deserialization for publish and read paths,
  including root-envelope revalidation after decode;
- management of topology, persistent receive/ack cursors with connector metadata (`offset`/opaque `ack_token`), DLQ and health abstractions;
- observability hooks for the transport layer;
- no ownership over bundled/external connection lifecycle.

## Integration

- depends on `rustok-iggy-connector` for bundled/external mode abstraction and low-level message I/O;
- implements `EventTransport` for the platform event system;
- routes `module.build.queued` to the dedicated `module-build` topic so the
  build dispatcher does not consume unrelated domain events;
- exposes `PersistentConsumerGroup`, which retains the same remote cursor for
  receive and offset acknowledgement; callers must acknowledge a delivery
  before receiving another one;
- does not expose an in-memory consumer-group registry or a per-partition
  re-subscribe acknowledgement path, because either could acknowledge on a
  different broker cursor than the one that received the delivery;
- must remain a transport crate, not a connector/runtime configuration bucket;
- any changes to transport contracts must be synchronized with outbox/event docs and connector docs.

## Verification

- targeted compile/tests for transport configuration, serialization/deserialization, consumer consume path, topology and DLQ abstractions;
- integration tests are needed when changing the real Iggy SDK path;
- structural verification for local docs and connector/transport boundary.

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Documentation of `rustok-iggy-connector`](../../rustok-iggy-connector/docs/README.md)
- [Deduplication recovery window policy](./dedup-recovery-window-policy.md)
- [DLQ duplicate alert policy](./dlq-duplicate-alert-policy.md)
- [DLQ duplicate alert observability](./dlq-duplicate-alert-observability.md)
- [DLQ duplicate alert runtime](./dlq-duplicate-alert-runtime.md)
- [DLQ duplicate alert server observer](./dlq-duplicate-alert-server-observer.md)
- [DLQ duplicate inspection](./dlq-duplicate-inspection.md)
- [DLQ duplicate rolling window](./dlq-duplicate-rolling-window.md)
- [DLQ duplicate moving window scan](./dlq-duplicate-moving-window-scan.md)
- [DLQ duplicate external scan](./dlq-duplicate-external-scan.md)
- [DLQ duplicate external scan runtime evidence](./dlq-duplicate-external-scan-runtime-evidence.md)
- [DLQ duplicate fair window external scan runtime evidence](./dlq-duplicate-fair-window-external-scan-runtime-evidence.md)
- [DLQ duplicate moving window external observer runtime evidence](./dlq-duplicate-moving-window-external-observer-runtime-evidence.md)
