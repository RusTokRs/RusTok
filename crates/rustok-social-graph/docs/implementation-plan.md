# `rustok-social-graph` implementation gates

This is the live owner plan for tenant-scoped block, mute, directional follow,
durable commands, events, replay, maintenance, and approved projections.
Cross-module presentation policy remains coordinated by
`crates/rustok-profiles/docs/implementation-plan.md`.

## Current state

`rustok-social-graph` owns relation persistence and synchronous privacy/follower
ports. Social Graph persistence remains authoritative for drift repair. Profiles,
Notifications, Index, search, and other projections must not read owner tables or
authorize from replicated relation state.

The source-complete path includes durable command receipts, bounded cleanup,
transactional sealed relation events, bounded owner replay, the approved generic
Index relation projection, Index-owned tenant schema registration, result-first
persistent Iggy consumption, one shared EventRuntime Iggy transport, default-off
server lifecycle, bounded projection/DLQ/ack retry, durable exact-byte DLQ receipts,
deterministic UUIDv8 broker message IDs, graceful shutdown, enabled-worker readiness,
bounded per-consumer Prometheus telemetry, and a read-only partition-qualified
consumer-position observer with completeness-gated total/max lag.

Compilation, source-verifier execution, PostgreSQL receipt/concurrency evidence,
real-broker deterministic-ID/deduplication and publish-confirmation behavior,
observer reconnect/TLS/auth, multi-replica position/receipt semantics, and retained
runtime evidence remain maintainer-run or pending.

## Delivered owner relation contract

- PostgreSQL and SQLite migrations own one tenant/source/target/kind relation row.
- `SocialRelationKind::{Block, Mute, Follow}` defines canonical stored/event values.
- Source and target use tenant-composite integrity; self-relations are rejected.
- Relation revisions are positive and monotonic.
- `SocialGraphCommandPort` requires deadline, non-empty idempotency identity,
  actor/source ownership, target separation, canonical kind, and optional expected
  revision.
- `SocialGraphPrivacyReadPort` owns block/mute/follow policy reads.
- `SocialGraphFollowReadPort` exposes revision-bearing directional state.
- Follow batches are bounded to 100 unique targets and fail closed on owner errors.
- Profiles followers-only presentation always uses these owner ports.

## Delivered durable command receipts and maintenance

- Migration `m20260726_000003_create_command_receipts` owns versioned processing and
  completed receipts with tenant-scoped normalized idempotency identity.
- Receipt reservation, relation mutation, optional event append, response snapshot,
  completion, and commit share one transaction.
- Exact replay returns the committed response; identity reuse fails as
  `social_graph.idempotency_conflict`.
- Unsupported, incomplete, or corrupt receipts fail closed.
- `SocialGraphReceiptMaintenancePort` exposes service/system-only bounded dry-run/live
  cleanup ordered by `(completed_at, id)` with all candidates validated before delete.
- `rustok-social-graph-cli` exposes `social_graph receipt-cleanup`; tenant and positive
  retention days are mandatory, output is aggregate only, and no scheduler is enabled.

## Delivered transactional relation events and replay

- `rustok-events` owns sealed `social_graph.relation.state_changed` v1.
- Payload contains relation id, source/target user ids, canonical kind, active state,
  and revision only; tenant and actor remain envelope metadata.
- New relations and persisted active-state transitions publish through
  `TransactionalEventBus::publish_contract_in_tx` before shared commit.
- Receipt replay and exact persisted-state no-op publish no new live event.
- Event publication failure rolls relation and receipt back together.
- `SocialGraphRelationEventMaintenancePort` provides service/system-only tenant and
  exclusive-UUID-cursor replay with dry-run and page-atomic publication.
- Replay is at-least-once; consumers apply by relation id plus monotonic revision.
- Social Graph persistence remains authoritative for drift repair.

## Delivered approved Index contract and storage boundary

- `rustok-index` is the first named approved relation-event consumer.
- Feature `index` converts the sealed event to a non-localized generic schema and
  mutation without broker or database logic.
- Active state maps to upsert, inactive state to revisioned tombstone, relation id to
  entity identity, and relation revision to `source_version`.
- Feature `index-consumer` adds optional Iggy/Index runtime dependencies.
- `SocialGraphIndexProjector` persists or exactly recognizes the tenant schema through
  Index-owned `PostgresSchemaRegistrationStore` before mutation apply.
- Registration is tenant-scoped, exact-version idempotent, monotonic, and fail-closed
  for contract drift, retired state, invalid tenant, unsupported backend, or storage
  failure.
- Social Graph imports no Index entities and never writes `index_schemas` directly.
- `PostgresMutationStore` atomically records inbox terminal state with projection state.
- `Applied`, `Duplicate`, and `StaleIgnored` are terminal durable outcomes.

## Delivered durable DLQ receipt and broker-identity boundary

- Migration `m20260727_000004_create_index_dlq_receipts` owns immutable source identity
  and exact payload bytes for poison deliveries. The key is tenant, consumer group, and
  event ID; stored source coordinates must match on every retry.
- States are `reserved`, leased `publishing`, terminal `published`, and post-source-ack
  `acknowledged`. Publisher leases are bounded and reclaimable after process loss.
- `project_consumed` checks receipts before Index projection. Published receipts enter
  acknowledgement-only recovery; unfinished receipts remain retryable DLQ work and do
  not cross back into mutation apply.
- A versioned SHA-256 construction derives one RFC 9562 UUIDv8 from tenant, consumer
  group, event ID, source stream/topic/partition/offset, and exact retained payload.
  Every variable-length field is length-framed.
- Retry count, time, publisher/lease identity, and random state are excluded, so retries
  for one immutable receipt retain the same broker message ID.
- `publish_consumed_to_dlq` returns success only after exact-byte broker publication and
  the durable `published` transition. Repeated calls recognize a terminal receipt and
  skip broker publication.
- The UUIDv8 is attached separately from the source event ID. `IggyTransport` lazily
  opens one SDK publisher connection to the same configured endpoint and existing
  `dlq` topic, then maps the UUID to Iggy's `u128` message header.
- Publisher connection/configuration failures leave the source unacknowledged, clear the
  cached client, and are retried through the existing bounded DLQ retry path.
- Ack failure after successful publication leaves the terminal receipt intact. On
  redelivery the consumer skips projection and DLQ publication and retries source ack.
- A previously created unfinished receipt continues to completion even if policy for
  creating new DLQ decisions is later disabled.
- Source-ack success is the transport boundary. Updating the receipt to `acknowledged`
  afterward is best-effort bookkeeping and cannot convert a committed source offset
  back into failure.
- Broker success followed by process/DB failure before the `published` transition is a
  separate explicit confirmation ambiguity. A retry carries the same UUIDv8, but Iggy
  duplicate suppression applies only when the deployment enables it and the relevant
  per-partition cache/expiry still covers the recovery interval.
- Durable receipt state remains authoritative. Physical broker exactly-once is not
  claimed from a deterministic ID or bounded optional deduplication; production must
  retain evidence for its configured window or select a stronger transaction/outbox
  mechanism.
- Historical rows are intentionally not fabricated; the migration backfill contract is
  `none`.

## Delivered persistent consumer, host lifecycle, and readiness

- Persistent group `rustok-social-graph-index` consumes the shared `domain` topic.
- Only `ContractEventPayload::SocialGraphRelation` reaches schema registration and
  mutation apply; unrelated sealed families are acknowledged without projection.
- `receive_next`, `project_consumed`, and `acknowledge_consumed` retain one outstanding
  cursor delivery and expose a result-first flow.
- `process_next(&mut self)` remains the direct serialized register/apply/ack path.
- Execution is default-off until
  `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED=true` and then requires a worker host
  plus effective `outbox_iggy` delivery.
- `EventRuntime` publishes the exact configured `Arc<IggyTransport>` into shared
  context; relay and consumer reuse it, and the worker never starts or stops a second
  bundled broker process.
- Identified DLQ publication may open one additional lazy SDK client owned by that same
  transport. It connects to the existing endpoint and does not create another transport
  or bundled process.
- `SocialGraphIndexWorkerHandle` exposes task state and observes shared `StopHandle`.
- Projection, DLQ claim/publication, and source acknowledgement use bounded exponential
  backoff from reviewed event settings.
- New permanent/exhausted projection failures may choose DLQ only while policy is
  enabled. Existing durable receipts continue regardless of later policy changes.
- After any durable Index or DLQ result, only acknowledgement is retried; projection is
  not repeated in-process.
- Missing/stopped/invalid enabled worker state is critical in `runtime_guardrails`,
  reaches `/health/ready`, and changes aggregate guardrail metrics. Disabled execution
  is not degraded.

## Delivered durable-consumer observability

- `rustok-telemetry::runtime_consumer_metrics` registers one bounded shared collector
  in the process Prometheus registry rendered by `/metrics`.
- Delivery metrics cover received/terminal outcomes, projection/DLQ/ack retries,
  bounded stage/error failures, DLQ `published`, `already_published`, and `failure`
  results, receive-to-ack duration, worker lifecycle, in-flight state/timestamp, and
  last success.
- A separate `SocialGraphIndexPositionObserver` starts only when the durable consumer
  is explicitly enabled. It uses the shared transport configuration to open a read-only
  SDK client to the already-running endpoint; it never starts/stops a broker or mutates
  offsets.
- Every poll reads all `domain` topic partitions and persistent group checkpoints.
  Empty partitions contribute zero; missing or incoherent checkpoints make the
  snapshot incomplete.
- Metrics expose snapshot timestamp, partition count, completeness, and exact
  `rustok_runtime_consumer_lag{aggregation="total|max"}` only from complete snapshots.
  Incomplete snapshots clear lag gauges and set completeness to zero.
- Labels remain bounded to consumer, stage, outcome, result, reason, aggregation, and
  stable error code. Tenant, event, relation, partition, offset, payload, broker ID,
  ack token, credentials, and raw error text are not labels.
- Observer configuration/connection/snapshot failures are recorded by stable code,
  retried independently, and do not stop projection or enter readiness guardrails.
- Lag is never inferred from event age, processing duration, a delivered offset, or a
  local cursor counter.
- Malformed bytes before `ConsumedContractEvent` construction remain unacknowledged
  pending a connector-level poison-delivery contract.

## Remaining Social Graph scope

1. Execute PostgreSQL concurrent schema-registration, mutation, and DLQ receipt
   claim/publish/ack evidence.
2. Prove real-Iggy deterministic UUID header publication, same-partition retry,
   connection loss/reconnect, ack failure, restart, graceful shutdown, observer
   snapshot/reconnect, and multi-replica cursor/receipt ownership.
3. Exercise broker success followed by receipt-mark loss with deduplication disabled,
   enabled, capacity-evicted, and expired. Verify the configured window covers the
   maximum lease/restart/recovery horizon before relying on duplicate suppression.
4. Decide whether production requires an enforced and monitored Iggy deduplication
   contract, a broker transaction, or a DB-owned DLQ/outbox relay before claiming any
   stronger physical duplicate guarantee.
5. Validate lag under concurrent publication, empty/missing checkpoints, TLS/auth
   failures, rebalancing, and multiple worker replicas before defining alerts.
6. Define a connector-level poison contract for undecodable broker bytes.
7. Corrupt/delete projection state and prove bounded owner replay/rescan repair while
   Profiles privacy remains on authoritative owner ports.
8. Define DLQ receipt retention/reconciliation before permitting deletion; configure
   command-receipt retention cadence and retain cleanup CLI dry-run/live evidence.
9. Retain receipt/event concurrency, replay-window, rollback, telemetry, storefront,
   privacy, and operational packets.
10. Continue friendship lifecycle, broader directory/follow UX, lists, block/mute
    management, and moderation/admin repair.

## Verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-telemetry --all-targets
cargo test -p rustok-telemetry -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-events --all-targets
cargo test -p rustok-events --test social_graph_contracts -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-index --all-targets
cargo test -p rustok-index schema_registration --lib -- --nocapture
node scripts/verify/verify-index-schema-registration.mjs
RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets
node scripts/verify/verify-iggy-consumer-position.mjs
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index-consumer --all-targets
cargo test -p rustok-social-graph --features index-consumer index_consumer::tests -- --nocapture
cargo test -p rustok-social-graph --features index-consumer index_dlq_receipt::tests -- --nocapture
cargo test -p rustok-social-graph --features index-consumer index_dlq_message_id::tests -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --features mod-social_graph --all-targets
cargo test -p rustok-server social_graph_index_worker --lib -- --nocapture
cargo test -p rustok-server runtime_guardrails --lib -- --nocapture
node scripts/verify/verify-social-graph-index-consumer.mjs
node scripts/verify/verify-social-graph-index-runtime-consumer.mjs
node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs
node scripts/verify/verify-social-graph-index-dlq-receipts.mjs
node scripts/verify/verify-runtime-consumer-metrics.mjs
node scripts/verify/verify-social-graph-command-receipts.mjs
node scripts/verify/verify-social-graph-receipt-cleanup.mjs
node scripts/verify/verify-social-graph-receipt-cleanup-cli.mjs
node scripts/verify/verify-social-graph-relation-outbox.mjs
node scripts/verify/verify-social-graph-relation-event-replay.mjs
node scripts/verify/verify-profiles-storefront-boundary.mjs
rustok-cli social_graph receipt-cleanup --tenant-id <uuid> --retention-days 30 --limit 100 --dry-run
```

These commands remain maintainer-run and were not executed manually while publishing
this slice. `Cargo.lock` must be refreshed after synchronization with `main`.
