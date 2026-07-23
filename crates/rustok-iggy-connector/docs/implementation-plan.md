# rustok-iggy-connector implementation plan

## Current state

`rustok-iggy-connector` owns low-level bundled/external connection lifecycle,
publish/subscribe I/O, and connector metadata. `ConnectorAckToken` provides
scoped simulated and Iggy SDK token shapes. The external connector opens one SDK
consumer-group cursor that receives and commits the exact pending message
offset. Bundled mode starts the module-packaged Iggy process and delegates all
I/O to the same real SDK path as external mode; no in-memory broker is
implemented. The public mode contract is now exactly `bundled | external`.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: owner-owned Leptos admin package mounted through
  `rustok-module.toml`, with a sibling module-owned Next admin package.
- Leptos uses native `#[server]` functions as its primary path and keeps the
  GraphQL contract in parallel. The Next surface consumes the same GraphQL
  query and mutation.
- The connector owns singleton persistence, bundled artifact availability,
  readiness validation, and secret-safe external credential references.
- Runtime mode changes remain restart-boundary operations; no hot swap or
  implicit fallback is implemented.

## Boundary and dependencies

- Owner: event transport platform.
- Consumers receive opaque `ConnectorAckToken` values and must not add retry,
  DLQ, replay, serialization, or topology policy here.
- `rustok-iggy` owns transport policy and depends on this crate for lifecycle
  and scoped acknowledgement facts.
- Existing source guard: `node scripts/verify/verify-iggy-connector-source.mjs`.

## Next results

1. **Verify real Iggy SDK receive and commit.** Prove that the external SDK
   cursor receives, acknowledges, reconnects, and preserves its committed
   offset in both bundled and external integration environments.
2. **Harden lifecycle failure behavior.** Define and test reconnect/backoff,
   authentication, TLS, topology, batching, and shutdown semantics for both
   modes. Done when a connector failure has typed behavior and no implicit
   fallback to a simulated connection.
3. **Publish operational guarantees.** Add health/metrics signals and a
   lifecycle runbook covering mode selection, credentials, TLS, connection
   loss, and recovery. Done when operators can diagnose a disconnected or
   stalled subscriber without inspecting transport policy.
4. **Complete packaging evidence.** Prove that bundled distributions install
   the pinned `iggy-server` artifact and external-only distributions omit it.

## Verification

- Contract tests cover every public use case.
- `node scripts/verify/verify-iggy-connector-source.mjs`
- `cargo test -p rustok-iggy-connector --lib`
- Bundled and external Iggy SDK integration tests for receive, scoped ack,
  reconnect, TLS/auth failure, and shutdown.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Iggy transport plan](../../rustok-iggy/docs/implementation-plan.md)
- [Iggy integration reference](../../../docs/references/iggy/README.md)
