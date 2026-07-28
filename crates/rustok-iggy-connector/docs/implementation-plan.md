# rustok-iggy-connector implementation plan

## Current state

`rustok-iggy-connector` owns low-level bundled/external connection lifecycle,
publish/subscribe I/O, connector metadata, scoped acknowledgement tokens, connector
settings persistence, and the neutral durable result store for broker deliveries that
cannot be decoded into trusted event identity.

The external connector opens one SDK consumer-group cursor that receives and commits
the exact pending message offset. Bundled mode starts the module-packaged Iggy process
and delegates all I/O to the same real SDK path as external mode; no in-memory broker is
implemented. The public mode contract is exactly `bundled | external`. External startup
validates configured addresses in order, providing initial connection failover without
claiming runtime high availability. Topology setup uses the same SDK connection before
the transport becomes ready.

Feature `migrations` now registers `m20260728_000001_create_consumer_poison_receipts`
and exposes `ConsumerPoisonReceiptStore`. The receipt is neutral because malformed
bytes have no trusted tenant or domain event identity. It binds one deterministic
connector delivery UUID to consumer group, stream, topic, partition, offset, exact
payload, bounded error code, and observed attempt count. Source coordinates are unique;
conflicting identity or bytes fail closed.

Receipt states are `reserved`, leased `publishing`, terminal `published`, and
post-source-commit `acknowledged`. The store performs no broker publication, DLQ routing,
or acknowledgement. An approved worker must persist/recognize the terminal result and
complete any required publication before it calls the cursor acknowledgement API.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: owner-owned Leptos admin package mounted through
  `rustok-module.toml`, with a sibling module-owned Next admin package.
- Leptos uses native `#[server]` functions as its primary path and keeps GraphQL in
  parallel. The Next surface consumes the same GraphQL query and mutation.
- The connector owns singleton settings persistence, bundled artifact availability,
  readiness validation, secret-safe external credentials, cursor facts, and neutral
  raw-poison result persistence.
- Runtime mode changes remain restart-boundary operations; no hot swap or implicit
  fallback is implemented.

## Boundary and dependencies

- Owner: event transport connector/control-plane.
- `rustok-iggy` owns serialization and transport policy and supplies a deterministic
  connector delivery UUID plus exact bytes after decode/schema failure.
- Consumer workers own retry limits, DLQ publication choice, and source acknowledgement
  ordering; the connector receipt only records durable result progress.
- The receipt contains no tenant, decoded event, actor, claims, locale, credentials,
  acknowledgement token, or authorization state.
- Profiles and Social Graph must never authorize from this receipt or any broker state.
- Existing source guard: `node scripts/verify/verify-iggy-connector-source.mjs`.
- Receipt guard: `node scripts/verify/verify-iggy-consumer-poison-receipts.mjs`.

## Delivered results

1. **Exact connector cursor ownership.** One cursor owns receive and exact scoped commit.
2. **Neutral durable poison result boundary.** PostgreSQL/SQLite DDL, source-coordinate
   uniqueness, exact-byte conflict validation, leased publication claims, terminal
   recognition, and bounded stable errors are source-complete.
3. **No invented domain ownership.** The receipt does not require or synthesize tenant
   or event identity and has no authorization side effect.

## Next results

1. **Wire the first approved sealed-family worker.** Adapt the Social Graph Index worker
   to `receive_delivery`, construct `ConsumerPoisonIdentity` from the decode-failure
   contract, publish exact bytes before `mark_published`, acknowledge only afterward,
   and mark `acknowledged` as best-effort bookkeeping. Redelivery must skip duplicate
   publication after a terminal receipt.
2. **Reconcile the append-only migration tail.** Add the already-present Social Graph
   Index DLQ migration and this connector poison migration to the explicit platform
   release-order tail before migration compatibility validation. Do not rewrite the
   previously published prefix.
3. **Verify real Iggy SDK receive and commit.** Prove validated and malformed delivery,
   reconnect, exact commit, publication failure, restart, and multi-replica behavior in
   bundled and external environments.
4. **Harden lifecycle failure behavior.** Define reconnect/backoff, authentication, TLS,
   existing-topology validation, batching, and shutdown semantics without simulated
   fallback.
5. **Publish operational guarantees.** Add health/metrics and an operator runbook for
   disconnected/stalled subscribers, poison receipt claims, publication ambiguity, and
   recovery.
6. **Complete packaging evidence.** Prove bundled distributions install the pinned
   server artifact and external-only distributions omit it.

## Verification

- `node scripts/verify/verify-iggy-connector-source.mjs`
- `node scripts/verify/verify-iggy-consumer-poison-receipts.mjs`
- `cargo test -p rustok-iggy-connector --features migrations consumer_poison_receipt -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy-connector --features iggy,migrations --all-targets`
- Bundled/external Iggy integration evidence for receive, scoped ack, reconnect,
  TLS/auth failure, poison publication/recovery, and shutdown.

Tests, Cargo commands, formatting, verifiers, database scenarios, and real-broker
scenarios remain maintainer-run and were not executed in this slice. `Cargo.lock` must
be refreshed after reconciliation with current `main`.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Iggy transport plan](../../rustok-iggy/docs/implementation-plan.md)
- [Iggy integration reference](../../../docs/references/iggy/README.md)
