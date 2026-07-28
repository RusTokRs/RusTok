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

Feature `migrations` registers `m20260728_000001_create_consumer_poison_receipts` and
exposes `ConsumerPoisonReceiptStore`. The receipt is neutral because malformed bytes
have no trusted tenant or domain event identity. Its immutable identity binds one
deterministic connector delivery UUID to consumer group, stream, topic, partition,
offset, and exact payload, including an empty payload. Source coordinates are unique;
another UUID, coordinate set, or payload fails closed. The first bounded error code and
observed delivery attempt are retained as diagnostics, but later classification/retry
drift does not redefine connector identity.

Receipt states are `reserved`, leased `publishing`, terminal `published`, and
post-source-commit `acknowledged`. The store performs no broker publication, DLQ routing,
source acknowledgement, authorization, or policy selection.

`ConsumerPoisonReceiptInspector` now provides one read-only aggregate snapshot for a
validated consumer group. It exposes only total, reserved, publishing, expired-
publishing, published, and acknowledged counts. Unknown/corrupt states fail closed when
the known-state sum differs from total, and an expired-publishing count cannot exceed
publishing. The inspector never returns delivery identifiers, source coordinates,
payloads, classifications, publisher identities, or timestamps and performs no claim,
repair, retention, deletion, publication, or acknowledgement action.

The first approved consumer is now wired at the owner/server layer. The Social Graph
Index worker constructs `ConsumerPoisonIdentity`, recognizes an existing receipt before
applying current DLQ policy, reserves/claims new work, publishes exact bytes through
`rustok-iggy`, persists `published`, commits the source cursor, and then records
`acknowledged` as best-effort bookkeeping. This composition remains outside the
connector store. Existing durable work continues recovery if new DLQ decisions are
later disabled.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: owner-owned Leptos admin package mounted through
  `rustok-module.toml`, with a sibling module-owned Next admin package.
- Leptos uses native `#[server]` functions as its primary path and keeps GraphQL in
  parallel. The Next surface consumes the same GraphQL query and mutation.
- The connector owns singleton settings persistence, bundled artifact availability,
  readiness validation, secret-safe external credentials, cursor facts, neutral
  raw-poison result persistence, and bounded read-only receipt aggregates.
- Runtime mode changes remain restart-boundary operations; no hot swap or implicit
  fallback is implemented.

## Boundary and dependencies

- Owner: event transport connector/control-plane.
- `rustok-iggy` owns serialization and transport policy and supplies a deterministic
  connector delivery UUID plus exact bytes after decode/schema failure.
- Consumer workers own retry limits, DLQ publication choice, and source acknowledgement
  ordering; the connector receipt only records durable result progress.
- Error kind and delivery attempt are retained first-observation diagnostics, not
  connector identity fields.
- The receipt contains no tenant, decoded event, actor, claims, locale, credentials,
  acknowledgement token, or authorization state.
- Aggregate inspection is consumer-group scoped, count-only, and read-only. Alert
  thresholds, reclaim decisions, repair, and retention remain operator policy outside
  this crate.
- Profiles and Social Graph must never authorize from this receipt, its aggregate
  inspection, or any broker state.
- The server enables feature `migrations` explicitly when it composes the neutral store;
  runtime availability does not rely on transitive feature unification.
- Existing source guard: `node scripts/verify/verify-iggy-connector-source.mjs`.
- Receipt/first-consumer guard:
  `node scripts/verify/verify-iggy-consumer-poison-receipts.mjs`.
- Aggregate inspection guard:
  `node scripts/verify/verify-iggy-consumer-poison-inspection.mjs`.

## Delivered results

1. **Exact connector cursor ownership.** One cursor owns receive and exact scoped commit.
2. **Neutral durable poison result boundary.** PostgreSQL/SQLite DDL, private immutable
   source identity, empty/exact-byte retention, UUID/source collision validation,
   first-diagnostic retention, leased publication claims, terminal recognition, and
   bounded stable errors are source-complete.
3. **No invented domain ownership.** The receipt does not require or synthesize tenant
   or event identity and has no authorization side effect.
4. **First approved receipt consumer.** The Social Graph Index worker composes typed
   decode failure, receipt recovery/claim, exact-byte publication, durable
   published-before-ack ordering, and best-effort acknowledgement bookkeeping without
   adding broker or domain policy to this crate.
5. **Read-only operational inspection.** One bounded consumer-group query reports
   known-state and expired-lease counts, rejects corrupt aggregate state, and exposes no
   delivery-level facts or mutation side effects.

## Next results

1. **Reconcile the append-only migration tail.** Add the already-present Social Graph
   Index DLQ migration and this connector poison migration to the explicit platform
   release-order tail before migration compatibility validation. Do not rewrite the
   previously published prefix; reconcile current `main` and refresh `Cargo.lock`.
2. **Verify real Iggy SDK receive and commit.** Prove validated and malformed delivery,
   exact publish-before-ack ordering, acknowledgement-only redelivery, reconnect, exact
   commit, publication failure, restart, and multi-replica behavior in bundled and
   external environments.
3. **Verify PostgreSQL receipt concurrency and inspection.** Prove claim ownership,
   lease expiry and reclaim, UUID/source collisions, first-diagnostic retention,
   rollback, terminal recognition, aggregate consistency, and read-only inspection under
   multiple workers.
4. **Harden lifecycle failure behavior.** Define reconnect/backoff, authentication, TLS,
   existing-topology validation, batching, and shutdown semantics without simulated
   fallback.
5. **Publish operational guarantees.** Compose count-only health/metrics and an operator
   runbook for disconnected/stalled subscribers, poison receipt claims, publication
   ambiguity, and recovery. Keep alert thresholds, reclaim, repair, and retention as
   explicit reviewed policy rather than storage side effects.
6. **Complete packaging evidence.** Prove bundled distributions install the pinned
   server artifact and external-only distributions omit it.

## Verification

- `node scripts/verify/verify-iggy-connector-source.mjs`
- `node scripts/verify/verify-iggy-consumer-poison-receipts.mjs`
- `node scripts/verify/verify-iggy-consumer-poison-inspection.mjs`
- `node scripts/verify/verify-social-graph-index-runtime-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
- `cargo test -p rustok-iggy-connector --features migrations consumer_poison_receipt -- --nocapture`
- `cargo test -p rustok-iggy-connector --features migrations consumer_poison_inspection -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy-connector --features iggy,migrations --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --features mod-social_graph --all-targets`
- Bundled/external Iggy integration evidence for receive, scoped ack, reconnect,
  TLS/auth failure, poison publication/recovery, inspection, and shutdown.

Tests, Cargo commands, formatting, verifiers, database scenarios, and real-broker
scenarios remain maintainer-run and were not executed in this slice. `Cargo.lock` must
be refreshed after reconciliation with current `main`.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Iggy transport plan](../../rustok-iggy/docs/implementation-plan.md)
- [Iggy integration reference](../../../docs/references/iggy/README.md)
