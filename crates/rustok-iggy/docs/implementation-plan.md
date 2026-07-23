# rustok-iggy implementation plan

## Current state

`rustok-iggy` implements the platform `EventTransport` over
`rustok-iggy-connector`. It owns serialization, topology, consumer groups,
transport-level consume/ack coordination, DLQ, and health abstractions.
It supports JSON (RFC 3339 timestamps) and MessagePack (`rmp-serde`, UTC
microsecond timestamps); Postcard is intentionally absent because it cannot
decode the internally tagged published event enums.
The connector owns connection lifecycle and low-level I/O. Root and typed
consumer APIs retain a single cursor across receive and acknowledgement; the
legacy per-partition re-subscribe path has been removed because it could not
prove that an acknowledgement committed the cursor that received the event.
Both modes use the real SDK cursor implementation: `Bundled` manages the
module-installed native server on loopback and `External` connects to
independently managed Iggy.
Topology creation is delegated to the connector and is only marked initialized
after the broker accepts the stream and all required topics. Real-broker
integration evidence for reconnect, persisted offsets, and topology is still
required.

Replay is intentionally not exposed. The removed in-memory replay planner could
return a successful replay ID without reading or republishing any broker data.
A production replay API will be introduced only with a bounded broker-read and
republish implementation, durable progress/idempotency evidence, and a real
broker integration test. Similarly, DLQ retry requires a complete `DlqEntry`;
there is no ID-only retry API that can claim success without a payload.

## Boundary and dependencies

- Owner: event transport platform.
- Dependency: `rustok-iggy-connector` must expose a real
  `ConnectorAckToken::iggy_sdk` receive/commit path before transport can claim
  production acknowledgement semantics.
- `rustok-iggy` owns DLQ policy; the connector must not absorb it.
- Consumers such as outbox use the public `EventTransport` contract, not
  connector-specific I/O.

## Next results

1. **Verify real consumption and acknowledgement.** Prove the remote connector
   SDK cursor receives a message and commits the exact scoped offset after a
   reconnect in both local and remote deployment paths.
2. **Execute DLQ and introduce broker-backed replay against Iggy.** Prove DLQ
   movement and retry with an actual connector. Design replay as a bounded
   backend read and republish operation with durable progress, idempotency, and
   a real broker test; do not reintroduce an in-memory planner or a no-op API.
3. **Harden production operation.** Add reconnect, backpressure, topology,
   TLS/auth failure, health, metrics, and recovery evidence with an operator
   runbook. Done when degraded behavior is observable and operationally
   actionable for both local and remote modes.

## Verification

- Contract tests cover every public use case.
- `cargo test -p rustok-iggy --lib`
- `cargo test -p rustok-iggy --test integration`
- `node scripts/verify/verify-iggy-connector-source.mjs`
- Real local and remote Iggy integration tests for topology, consume, commit,
  DLQ retry, and reconnect; add replay coverage with its future broker-backed
  implementation.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Connector plan](../../rustok-iggy-connector/docs/implementation-plan.md)
- [Iggy integration reference](../../../docs/references/iggy/README.md)
