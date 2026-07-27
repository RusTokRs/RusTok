# rustok-iggy implementation plan

## Current state

`rustok-iggy` implements platform `EventTransport` over `rustok-iggy-connector`.
It owns serialization, topology, persistent root/contract consumer groups, DLQ,
health abstractions, and the read-only broker-backed consumer-position observer.

JSON uses RFC 3339 timestamps and MessagePack uses UTC microseconds. Root and typed
consumer APIs retain one cursor across receive and acknowledgement; the removed
per-partition re-subscribe path could not prove exact cursor commit semantics.
Both deployment modes use the real SDK: `Bundled` manages the installed native
server on loopback and `External` connects to an independently managed broker.

`IggyConsumerPositionObserver` opens a separate read-only SDK client to the already
running configured endpoint. It reads every topic partition plus the persistent
consumer-group checkpoint for that partition. Aggregate lag is published only when
every partition is empty or has a coherent stored offset not ahead of the observed
high-watermark. Missing checkpoints and inconsistent observations fail closed.

Replay remains intentionally unavailable. A production replay API requires bounded
broker reads, republish, durable progress/idempotency evidence, and a real broker
integration test. DLQ retry requires a complete `DlqEntry`; there is no ID-only API
that can claim success without the original payload.

Compilation, source-verifier execution, real-Iggy snapshot/reconnect, persisted-offset,
TLS/auth, and multi-replica evidence remain maintainer-run or pending.

## Boundary and dependencies

- Owner: event transport platform.
- `rustok-iggy-connector` owns the primary broker/process lifecycle and exact
  `ConnectorAckToken::iggy_sdk` receive/commit path.
- `rustok-iggy` owns serialization, DLQ policy, and neutral consumer-position semantics.
- The position observer may create another SDK client connection but never another
  bundled process and never mutates offsets.
- Consumers such as Outbox and Social Graph use public transport contracts rather than
  connector internals.
- Transport positions, tenant/event identities, credentials, and raw errors are not
  Prometheus labels.

## Delivered results

1. **Persistent result-first consumer groups.** Root and sealed-family cursors retain
   one remote cursor across receive and exact scoped acknowledgement.
2. **Exact-byte contract delivery.** Successfully decoded contract deliveries retain
   original broker bytes for lossless owner-directed DLQ publication.
3. **Partition-qualified position observation.** `ConsumerPositionSnapshot` contains
   every topic partition, committed offset, high-watermark, message count, capture
   timestamp, and checked per-partition lag.
4. **Fail-closed aggregate lag.** `total_lag` and `max_lag` exist only for a complete
   coherent snapshot; empty partitions contribute zero and missing checkpoints do not.
5. **Shared-endpoint composition.** Bundled observation connects to the reviewed
   loopback endpoint already managed by `IggyTransport`; external observation reuses
   reviewed address/auth/TLS configuration.

## Next results

1. **Verify real consumption and acknowledgement.** Prove receive and exact offset
   commit across reconnect in bundled and external modes.
2. **Verify consumer-position observation.** Prove every-partition topic/offset reads,
   empty/missing checkpoint behavior, concurrent publication during a snapshot,
   reconnect, TLS/auth failure, and multi-replica consumer-group semantics.
3. **Execute DLQ and broker-backed replay.** Prove real DLQ movement/retry, then design
   bounded replay with durable progress and idempotency.
4. **Harden production operation.** Retain reconnect, backpressure, topology, health,
   lag alert, and recovery evidence with operator runbooks.

## Verification

- `cargo test -p rustok-iggy --lib`
- `cargo test -p rustok-iggy --test integration`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets`
- `node scripts/verify/verify-iggy-connector-source.mjs`
- `node scripts/verify/verify-iggy-consumer-position.mjs`
- Real bundled/external Iggy evidence for topology, consume, commit, position snapshot,
  DLQ retry, reconnect, TLS/auth, and multi-replica behavior.

These commands and scenarios remain maintainer-run and were not executed manually in
this slice. `Cargo.lock` requires refresh after synchronization with `main`.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Connector plan](../../rustok-iggy-connector/docs/implementation-plan.md)
- [Iggy integration reference](../../../docs/references/iggy/README.md)
