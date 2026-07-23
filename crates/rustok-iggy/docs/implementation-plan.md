# rustok-iggy implementation plan

## Current state

`rustok-iggy` implements the platform `EventTransport` over
`rustok-iggy-connector`. It owns serialization, topology, consumer groups,
transport-level consume/ack coordination, DLQ, replay, and health abstractions.
The connector owns connection lifecycle and low-level I/O. The transport has a
fake-connector consume path and metadata scope guard, but real Iggy SDK receive
and offset-commit evidence is not yet complete.

## Boundary and dependencies

- Owner: event transport platform.
- Dependency: `rustok-iggy-connector` must expose a real
  `ConnectorAckToken::iggy_sdk` receive/commit path before transport can claim
  production acknowledgement semantics.
- `rustok-iggy` owns DLQ/replay policy; the connector must not absorb it.
- Consumers such as outbox use the public `EventTransport` contract, not
  connector-specific I/O.

## Next results

1. **Complete real consumption and acknowledgement.** Wire connector SDK
   metadata into `consume_next_as_group` and commit the exact scoped cursor in
   `ack_consumed`. Done when embedded and remote Iggy tests prove a message is
   received once and its committed offset survives reconnect.
2. **Execute DLQ and replay against Iggy.** Replace planned offsets and
   metadata-only movement with bounded backend reads, republishes, retry
   limits, and idempotency evidence. Done when a real backend test covers a
   failure through DLQ, retry, and replay without cross-topic acknowledgements.
3. **Harden production operation.** Add reconnect, backpressure, topology,
   TLS/auth failure, health, metrics, and recovery evidence with an operator
   runbook. Done when degraded behavior is observable and operationally
   actionable for both embedded and remote modes.

## Verification

- Contract tests cover every public use case.
- `cargo test -p rustok-iggy --lib`
- `cargo test -p rustok-iggy --test integration`
- `node scripts/verify/verify-iggy-connector-source.mjs`
- Real embedded and remote Iggy integration tests for consume, commit, DLQ,
  replay, and reconnect.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Connector plan](../../rustok-iggy-connector/docs/implementation-plan.md)
- [Iggy integration reference](../../../docs/references/iggy/README.md)
