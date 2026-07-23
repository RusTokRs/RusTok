# rustok-iggy implementation plan

## Current state

`rustok-iggy` implements the platform `EventTransport` over
`rustok-iggy-connector`. It owns serialization, topology, consumer groups,
transport-level consume/ack coordination, DLQ, replay, and health abstractions.
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
Real-broker integration evidence for reconnect and persisted offsets is still
required.

## Boundary and dependencies

- Owner: event transport platform.
- Dependency: `rustok-iggy-connector` must expose a real
  `ConnectorAckToken::iggy_sdk` receive/commit path before transport can claim
  production acknowledgement semantics.
- `rustok-iggy` owns DLQ/replay policy; the connector must not absorb it.
- Consumers such as outbox use the public `EventTransport` contract, not
  connector-specific I/O.

## Next results

1. **Verify real consumption and acknowledgement.** Prove the remote connector
   SDK cursor receives a message and commits the exact scoped offset after a
   reconnect in both local and remote deployment paths.
2. **Execute DLQ and replay against Iggy.** Replace planned offsets and
   metadata-only movement with bounded backend reads, republishes, retry
   limits, and idempotency evidence. Done when a real backend test covers a
   failure through DLQ, retry, and replay without cross-topic acknowledgements.
3. **Harden production operation.** Add reconnect, backpressure, topology,
   TLS/auth failure, health, metrics, and recovery evidence with an operator
   runbook. Done when degraded behavior is observable and operationally
   actionable for both local and remote modes.

## Verification

- Contract tests cover every public use case.
- `cargo test -p rustok-iggy --lib`
- `cargo test -p rustok-iggy --test integration`
- `node scripts/verify/verify-iggy-connector-source.mjs`
- Real local and remote Iggy integration tests for consume, commit, DLQ,
  replay, and reconnect.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Connector plan](../../rustok-iggy-connector/docs/implementation-plan.md)
- [Iggy integration reference](../../../docs/references/iggy/README.md)
