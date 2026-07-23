# rustok-iggy-connector implementation plan

## Current state

`rustok-iggy-connector` owns low-level embedded/remote connection lifecycle,
publish/subscribe I/O, and connector metadata. `ConnectorAckToken` provides
scoped simulated and Iggy SDK token shapes. Current remote and embedded
subscribers emit canonical `sim:*` tokens; the real Iggy SDK receive/commit
path is not yet wired. Simulation remains an explicit supported connector mode,
not evidence of a real offset commit.

## Boundary and dependencies

- Owner: event transport platform.
- Consumers receive opaque `ConnectorAckToken` values and must not add retry,
  DLQ, replay, serialization, or topology policy here.
- `rustok-iggy` owns transport policy and depends on this crate for lifecycle
  and scoped acknowledgement facts.
- Existing source guard: `node scripts/verify/verify-iggy-connector-source.mjs`.

## Next results

1. **Wire real Iggy SDK receive and commit.** Build `ConnectorAckToken::iggy_sdk`
   from the SDK subscriber cursor, validate its scope, and commit precisely that
   cursor in the remote and embedded adapters. Done when SDK-backed tests prove
   receive, ack, reconnect, and persisted offset behavior.
2. **Harden lifecycle failure behavior.** Define and test reconnect/backoff,
   authentication, TLS, topology, batching, and shutdown semantics for both
   modes. Done when a connector failure has typed behavior and no implicit
   fallback to a simulated connection.
3. **Publish operational guarantees.** Add health/metrics signals and a
   lifecycle runbook covering mode selection, credentials, TLS, connection
   loss, and recovery. Done when operators can diagnose a disconnected or
   stalled subscriber without inspecting transport policy.

## Verification

- Contract tests cover every public use case.
- `node scripts/verify/verify-iggy-connector-source.mjs`
- `cargo test -p rustok-iggy-connector --lib`
- Embedded and remote Iggy SDK integration tests for receive, scoped ack,
  reconnect, TLS/auth failure, and shutdown.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Iggy transport plan](../../rustok-iggy/docs/implementation-plan.md)
- [Iggy integration reference](../../../docs/references/iggy/README.md)
