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

`rustok-iggy-connector` provides the neutral durable result store for this identity.
It persists source-coordinate uniqueness, deterministic delivery UUID, exact payload,
first-observed bounded error code/attempt, leased publication state, terminal
`published`, and post-source-commit `acknowledged`. Later error classification or retry
count does not redefine identity. Empty payload is retained exactly. The store performs
no publication, acknowledgement, authorization, or policy choice.

The first approved owner worker is wired. `SocialGraphIndexConsumer::receive_delivery`
passes both typed variants to the server worker. For an undecodable delivery the worker
recognizes an existing neutral receipt before applying current DLQ policy, reserves a
new receipt only when policy permits, publishes `to_dlq_entry` exact bytes, persists
`published`, then acknowledges the retained cursor and records `acknowledged` as
best-effort bookkeeping. Existing durable work continues recovery if new DLQ decisions
are later disabled. The raw path never enters Index projection or creates tenant/event
identity.

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

The External raw-poison lifecycle harness uses one unique stream and one partition to
exercise the production cursor path. It injects two malformed fixtures, receives the
first through `PersistentContractConsumerGroup`, publishes exact bytes through
`IggyTransport::move_to_dlq`, shuts down the transport without source acknowledgement,
requires same-offset/bytes/UUID redelivery through a new transport and the same group,
acknowledges it explicitly, and then requires the second offset to become visible. An
independent real DLQ cursor verifies both payloads byte-for-byte. The harness is
source-complete and has not been executed.

A separate physical-header harness now covers the publisher wire contract without
repeating the lifecycle scenario. It creates a production `ConsumedContractDecodeFailure`,
derives one `DlqEntry`, publishes exactly once through `IggyTransport::move_to_dlq`, and
uses a probe-only SDK consumer on `dlq` to require:

- physical `message.header.id == broker_message_id.as_u128()`;
- physical partition equals `(uuid_as_u128 mod partitions) + 1` and remains in the
  one-based range;
- physical payload is exact;
- only the probe message's physical header offset is committed.

The SDK probe cannot publish, create source fixtures, acknowledge source cursors,
modify receipts, delete streams, or change deduplication. The physical-header harness
is source-complete and runtime-pending.

Neither external harness composes PostgreSQL receipt ordering, exercises broker
suppression, or claims bundled/TLS/auth/multi-replica proof. Source cursor lifecycle,
physical header observation, database ordering, and deduplication remain distinct
evidence boundaries.

Replay remains intentionally unavailable. A production replay API requires bounded
broker reads, republish, durable progress/idempotency evidence, and a real broker
integration test. DLQ retry requires a complete `DlqEntry`; there is no ID-only API
that can claim success without the original payload.

Compilation, source-verifier execution, external-Iggy cursor/header execution,
deduplication, receipt-plus-broker ordering, position/reconnect, persisted-offset,
TLS/auth, bundled-mode, and multi-replica evidence remain maintainer-run or pending.

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
- Consumer workers compose typed receive, connector receipt recognition/claim,
  exact-byte DLQ publication, durable `published`, source acknowledgement, and
  best-effort `acknowledged` in that order.
- The lifecycle fixture connector may publish arbitrary malformed bytes only.
  Production receive, classification, DLQ publication, reconnect, and source ack remain
  on `IggyTransport`/`PersistentContractConsumerGroup` APIs.
- The physical-header SDK client is observation-only: connect, open a unique DLQ group,
  receive one message, inspect header/partition/payload, and commit that probe offset.
- External evidence creates unique streams but does not use an unreviewed deletion API.
  It requires a disposable broker or operator-approved cleanup.
- Consumers such as Outbox and Social Graph use public transport contracts rather than
  connector cursor internals.
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
   storage retains private immutable identity, exact bytes including empty payload,
   first diagnostics, leased publication claims, terminal recognition, and post-commit
   acknowledgement state.
6. **First approved raw-poison worker.** The Social Graph Index worker adopts typed
   receive, recognizes existing durable choices before current policy, publishes exact
   bytes before `mark_published`, acknowledges only afterward, and keeps raw state out
   of Index projection and Profiles authorization.
7. **Deterministic identified DLQ publication.** `DlqEntry` can retain a stable owner or
   connector UUID separately from decoded event semantics; the lazy SDK publisher maps
   it to Iggy's `u128` message header, preserves one-based partition routing, and
   reconnects after failures without acknowledging the source.
8. **Partition-qualified position observation.** `ConsumerPositionSnapshot` contains
   every topic partition, committed offset, high-watermark, message count, capture
   timestamp, and checked per-partition lag.
9. **Fail-closed aggregate lag.** `total_lag` and `max_lag` exist only for a complete
   coherent snapshot; empty partitions contribute zero and missing checkpoints do not.
10. **Shared-endpoint composition.** Bundled publisher/observation clients connect to the
    reviewed loopback endpoint already managed by `IggyTransport`; external clients reuse
    reviewed address/auth/TLS configuration.
11. **External raw-poison lifecycle harness.** A versioned source contract, opt-in real
    external broker test, bounded timeouts, exact-byte DLQ cursor, no-ack transport
    reconnect/redelivery, explicit source advancement, and static guard define the first
    broker-backed raw cursor evidence without claiming execution.
12. **Physical header/partition harness.** A separate source contract and static guard
    define one production DLQ publication plus a probe-only SDK read that checks exact
    UUID/u128 header mapping, one-based UUID partition routing, exact payload, and probe
    offset commit without introducing deduplication claims.

## Next results

1. **Execute external lifecycle and header evidence.** Run both harnesses on a disposable
   external Iggy server, retain broker/version/configuration metadata and source/output
   digests, and keep their runtime packets separate.
2. **Compose receipt-plus-broker ordering evidence.** Add PostgreSQL receipt storage to a
   broker-backed scenario and prove reserve/claim -> exact publish -> durable `published`
   -> source ack -> best-effort `acknowledged`, including acknowledgement-only recovery.
3. **Verify duplicate behavior separately.** Exercise publish failure reconnect and the
   same deterministic UUID with deduplication disabled, enabled, capacity-evicted, and
   expired. Do not infer these outcomes from the one-message header probe.
4. **Verify bundled raw lifecycle.** Repeat the external scenario through the packaged
   bundled server and prove start, readiness, restart, durable data reuse, and shutdown.
5. **Verify connector receipt concurrency.** Execute and retain the existing PostgreSQL
   claim ownership, lease expiry/reclaim, UUID/source collision, rollback,
   first-diagnostic, and aggregate inspection packet.
6. **Verify consumer-position observation.** Prove every-partition topic/offset reads,
   empty/missing checkpoint behavior, concurrent publication during a snapshot,
   reconnect, TLS/auth failure, and multi-replica consumer-group semantics.
7. **Choose production confirmation policy.** Require and verify a dedup window covering
   the maximum recovery horizon, or move DLQ publication behind a database-owned outbox
   relay/broker transaction before claiming stronger duplicate suppression.
8. **Execute broker-backed replay.** Prove real DLQ retry, then design bounded replay
   with durable progress and idempotency.
9. **Harden production operation.** Retain reconnect, backpressure, topology, health,
   lag alert, decode-failure, receipt, dedup configuration, cleanup, and recovery evidence
   with operator runbooks.

## Verification

- `cargo test -p rustok-iggy --lib`
- `cargo test -p rustok-iggy contract_decode_failure --lib -- --nocapture`
- `cargo test -p rustok-iggy --test integration`
- `RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS='host:8090' cargo test -p rustok-iggy --features iggy --test contract_poison_external_iggy -- --nocapture --test-threads=1`
- `RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS='host:8090' cargo test -p rustok-iggy --features iggy --test contract_poison_external_iggy_header -- --nocapture --test-threads=1`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy-connector --features iggy,migrations --all-targets`
- `cargo test -p rustok-iggy-connector --features migrations consumer_poison_receipt -- --nocapture`
- `node scripts/verify/verify-iggy-connector-source.mjs`
- `node scripts/verify/verify-iggy-contract-decode-failure.mjs`
- `node scripts/verify/verify-iggy-contract-poison-external-evidence.mjs`
- `node scripts/verify/verify-iggy-contract-poison-external-header-evidence.mjs`
- `node scripts/verify/verify-iggy-consumer-poison-receipts.mjs`
- `node scripts/verify/verify-iggy-consumer-position.mjs`
- `node scripts/verify/verify-social-graph-index-runtime-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
- `node scripts/verify/verify-social-graph-index-dlq-receipts.mjs`
- Real bundled/external Iggy evidence for topology, validated/decode-failure delivery,
  deterministic message headers, dedup disabled/enabled/expiry/capacity behavior,
  consume, commit, position snapshot, DLQ retry, reconnect, TLS/auth, and multi-replica
  behavior.

These commands and scenarios remain maintainer-run and were not executed manually in
this slice.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Connector plan](../../rustok-iggy-connector/docs/implementation-plan.md)
- [External raw poison evidence guide](./contract-poison-external-evidence.md)
- [External physical header evidence guide](./contract-poison-external-header-evidence.md)
- [Iggy integration reference](../../../docs/references/iggy/README.md)
