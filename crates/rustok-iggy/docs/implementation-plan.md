# rustok-iggy implementation plan

## Current state

`rustok-iggy` implements platform `EventTransport` over `rustok-iggy-connector`.
It owns serialization, topology, persistent root/contract consumer groups, typed raw
contract decode-failure delivery, DLQ, health abstractions, deterministic identified-DLQ
publication, and the read-only broker-backed consumer-position observer.

JSON uses RFC 3339 timestamps and MessagePack uses UTC microseconds. Root and typed
consumer APIs retain one cursor across receive and acknowledgement; the removed
per-partition re-subscribe path could not prove exact cursor commit semantics.
Both deployment modes use the real SDK: `Bundled` manages the installed native
server on loopback and `External` connects to an independently managed broker.

`PersistentContractConsumerGroup::receive_delivery` validates source stream/topic
metadata before decoding and returns either a validated `ConsumedContractEvent` or an
exact-byte `ConsumedContractDecodeFailure`. Deserialization and registered-schema
failures retain partition, offset, opaque acknowledgement token, and exact payload but
do not invent a tenant or domain event id. The compatibility `receive` API maps that
typed result to a bounded validation error and leaves the offset uncommitted.

A decode failure derives one versioned RFC 9562 UUIDv8 from length-framed stream,
topic, partition, offset, and exact payload. Error kind, retry count, time, process
identity, random values, credentials, connector message identity, and acknowledgement
token are excluded. The UUID can populate the transport-required event-shaped DLQ
field and explicit broker message header, but it is a connector delivery identity only.

`rustok-iggy-connector` now provides the neutral durable result store for this identity.
It persists source-coordinate uniqueness, deterministic delivery UUID, exact payload,
first-observed bounded error code/attempt, leased publication state, terminal
`published`, and post-source-commit `acknowledged`. Later error classification or retry
count does not redefine identity. The store performs no publication or acknowledgement.
No Social Graph or other runtime worker is wired to acknowledge this path yet.

An owner may attach a stable non-nil UUID to a `DlqEntry`. `IggyTransport` then lazily
opens one SDK publisher connection to the same configured endpoint and sends the exact
payload to the existing `dlq` topic with that UUID as Iggy's `u128` message header ID.
The connection is cached, dropped on publish failure for retry-time reconnect, and
cleared before transport shutdown. Entries without an explicit broker ID preserve the
existing generic connector path.

The deterministic header is an additional duplicate-suppression input, not durable
exactly-once. Iggy server deduplication is deployment-owned, optional, per-partition,
bounded by cache capacity/expiry, and may be disabled. The owner receipt remains the
durable decision record for decoded deliveries; the connector receipt is the durable
result record for undecodable deliveries. Broker success followed by loss before a
durable result is recorded remains a confirmation ambiguity whenever the dedup window
is absent or expired.

`IggyConsumerPositionObserver` opens a separate read-only SDK client to the already
running configured endpoint. It reads every topic partition plus the persistent
consumer-group checkpoint for that partition. Aggregate lag is published only when
every partition is empty or has a coherent stored offset not ahead of the observed
high-watermark. Missing checkpoints and inconsistent observations fail closed.

Replay remains intentionally unavailable. A production replay API requires bounded
broker reads, republish, durable progress/idempotency evidence, and a real broker
integration test. DLQ retry requires a complete `DlqEntry`; there is no ID-only API
that can claim success without the original payload.

Compilation, source-verifier execution, real-Iggy deterministic-ID/deduplication,
raw decode-failure publication/acknowledgement, snapshot/reconnect, persisted-offset,
TLS/auth, and multi-replica evidence remain maintainer-run or pending.

## Boundary and dependencies

- Owner: event transport platform.
- `rustok-iggy-connector` owns the primary broker/process lifecycle, exact
  `ConnectorAckToken::iggy_sdk` receive/commit path, and neutral raw-poison result
  persistence.
- `rustok-iggy` owns serialization, typed raw decode-failure retention, owner-directed
  DLQ composition, deterministic message-header publication, and neutral
  consumer-position semantics.
- Identified DLQ publication and position observation may create additional SDK client
  connections to the same endpoint but never another bundled process.
- The identified publisher mutates only the existing `dlq` topic and never commits a
  source offset; source acknowledgement remains an owner decision after durable result.
- `acknowledge_decode_failure` only commits the exact retained cursor token. It never
  publishes, retries, persists, or selects poison policy.
- Consumer workers compose the typed decode failure, connector receipt, exact-byte DLQ
  publication, and source acknowledgement in that order.
- Consumers such as Outbox and Social Graph use public transport contracts rather than
  connector internals.
- Transport positions, tenant/event identities, broker IDs, credentials, raw payloads,
  acknowledgement tokens, and raw errors are not Prometheus labels.

## Delivered results

1. **Persistent result-first consumer groups.** Root and sealed-family cursors retain
   one remote cursor across receive and exact scoped acknowledgement.
2. **Exact-byte contract delivery.** Successfully decoded contract deliveries retain
   original broker bytes for lossless owner-directed DLQ publication.
3. **Typed raw decode-failure delivery.** Deserialization and canonical-schema failures
   retain exact bytes and immutable source coordinates through `receive_delivery`
   without automatic acknowledgement or invented tenant/event identity.
4. **Stable connector delivery identity.** A versioned length-framed SHA-256 contract
   derives a UUIDv8 from immutable source coordinates and payload; classification and
   retry/process/time values cannot drift the identity.
5. **Neutral durable raw-poison result.** Connector-owned PostgreSQL/SQLite receipt
   storage retains private immutable identity, exact bytes, first diagnostics, leased
   publication claims, terminal recognition, and post-commit acknowledgement state.
6. **Deterministic identified DLQ publication.** `DlqEntry` can retain a stable owner or
   connector UUID separately from decoded event semantics; the lazy SDK publisher maps
   it to Iggy's `u128` message header, preserves one-based partition routing, and
   reconnects after failures without acknowledging the source.
7. **Partition-qualified position observation.** `ConsumerPositionSnapshot` contains
   every topic partition, committed offset, high-watermark, message count, capture
   timestamp, and checked per-partition lag.
8. **Fail-closed aggregate lag.** `total_lag` and `max_lag` exist only for a complete
   coherent snapshot; empty partitions contribute zero and missing checkpoints do not.
9. **Shared-endpoint composition.** Bundled publisher/observation clients connect to the
   reviewed loopback endpoint already managed by `IggyTransport`; external clients reuse
   reviewed address/auth/TLS configuration.

## Next results

1. **Wire raw decode failures into approved workers.** Adopt `receive_delivery` in the
   Social Graph Index consumer, reserve/recognize the connector receipt, publish exact
   bytes before `mark_published`, acknowledge only afterward, and record
   `acknowledged` as best-effort bookkeeping.
2. **Reconcile migration release order.** Append the existing decoded-event DLQ receipt
   migration and the connector raw-poison migration to the explicit platform tail
   without rewriting the published prefix.
3. **Verify real consumption and acknowledgement.** Prove validated and raw-failure
   receive, no implicit commit, explicit post-result commit, and reconnect in bundled and
   external modes.
4. **Verify deterministic DLQ behavior.** Prove the outgoing header ID, same-partition
   retry, publish failure reconnect, broker-success/result-mark crash, and duplicate
   behavior with deduplication disabled, enabled, capacity-evicted, and expired.
5. **Verify consumer-position observation.** Prove every-partition topic/offset reads,
   empty/missing checkpoint behavior, concurrent publication during a snapshot,
   reconnect, TLS/auth failure, and multi-replica consumer-group semantics.
6. **Choose production confirmation policy.** Require and verify a dedup window covering
   the maximum recovery horizon, or move DLQ publication behind a database-owned outbox
   relay/broker transaction before claiming stronger duplicate suppression.
7. **Execute broker-backed replay.** Prove real DLQ retry, then design bounded replay
   with durable progress and idempotency.
8. **Harden production operation.** Retain reconnect, backpressure, topology, health,
   lag alert, decode-failure, receipt, dedup configuration, and recovery evidence with
   operator runbooks.

## Verification

- `cargo test -p rustok-iggy --lib`
- `cargo test -p rustok-iggy contract_decode_failure --lib -- --nocapture`
- `cargo test -p rustok-iggy --test integration`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy-connector --features iggy,migrations --all-targets`
- `cargo test -p rustok-iggy-connector --features migrations consumer_poison_receipt -- --nocapture`
- `node scripts/verify/verify-iggy-connector-source.mjs`
- `node scripts/verify/verify-iggy-contract-decode-failure.mjs`
- `node scripts/verify/verify-iggy-consumer-poison-receipts.mjs`
- `node scripts/verify/verify-iggy-consumer-position.mjs`
- `node scripts/verify/verify-social-graph-index-dlq-receipts.mjs`
- Real bundled/external Iggy evidence for topology, validated/decode-failure delivery,
  deterministic message headers, dedup disabled/enabled/expiry/capacity behavior,
  consume, commit, position snapshot, DLQ retry, reconnect, TLS/auth, and multi-replica
  behavior.

These commands and scenarios remain maintainer-run and were not executed manually in
this slice. `Cargo.lock` requires refresh after synchronization with `main`.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Connector plan](../../rustok-iggy-connector/docs/implementation-plan.md)
- [Iggy integration reference](../../../docs/references/iggy/README.md)
