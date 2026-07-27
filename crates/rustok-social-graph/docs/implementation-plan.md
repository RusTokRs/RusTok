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

The source-complete path now includes durable command receipts, bounded cleanup,
transactional sealed relation events, bounded owner replay, the approved generic
Index relation projection, Index-owned tenant schema registration, result-first
persistent Iggy consumption, one shared EventRuntime Iggy connector, default-off
server lifecycle, bounded retry, staged exact-byte DLQ-before-ack, graceful shutdown,
enabled-worker readiness, and bounded per-consumer Prometheus telemetry.

Compilation, source-verifier execution, PostgreSQL concurrency, real-broker restart,
multi-replica recovery, partition-qualified high-watermark/lag telemetry, and retained
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
- `SocialGraphIndexWorkerHandle` exposes task state and observes shared `StopHandle`.
- Projection retry is bounded exponential backoff from reviewed event settings.
- Permanent or exhausted projection failures may publish exact original broker bytes
  to DLQ only when policy is enabled.
- `publish_consumed_to_dlq` and `acknowledge_consumed` are staged. DLQ publish completes
  before the worker enters source acknowledgement-only recovery.
- `move_to_dlq_and_acknowledge` remains a convenience method with the same order.
- After any durable Index or DLQ result, only acknowledgement is retried; projection is
  not repeated in-process.
- Missing/stopped/invalid enabled worker state is critical in `runtime_guardrails`,
  reaches `/health/ready`, and changes aggregate guardrail metrics. Disabled execution
  is not degraded.

## Delivered durable-consumer observability

- `rustok-telemetry::runtime_consumer_metrics` registers one bounded shared collector
  in the process Prometheus registry used by `/metrics`.
- Metrics cover received deliveries, terminal outcomes, projection/ack retries,
  bounded stage/error failures, DLQ publication success/failure, receive-to-ack
  duration, worker starts/terminations, in-flight state/timestamp, and last success.
- Consumer, stage, outcome, result, reason, and stable error-code labels are bounded.
  Tenant, event, relation, partition, offset, payload, ack-token, and raw error-message
  values are not labels.
- Source position and lag metrics are intentionally absent. A shared-topic consumer
  needs a partition-qualified position vector and broker high-watermarks; a single
  last offset or event age would be misleading.
- A process crash after successful DLQ publish but before source ack can still republish
  on redelivery; a durable DLQ identity/receipt decision remains open.
- Malformed bytes that fail before `ConsumedContractEvent` construction remain
  unacknowledged pending a connector-level poison-delivery contract.

## Remaining Social Graph scope

1. Add connector partition high-watermark observations and a partition-qualified
   acknowledged-position snapshot, then derive true bounded consumer lag.
2. Execute PostgreSQL concurrent schema-registration and mutation evidence.
3. Prove real-Iggy restart/redelivery, ack failure, DLQ failure, connector loss,
   graceful shutdown, and multi-replica cursor ownership.
4. Corrupt/delete projection state and prove bounded owner replay/rescan repair while
   Profiles privacy remains on authoritative owner ports.
5. Decide whether DLQ publish-success/source-ack-failure needs a durable owner receipt
   or another idempotent DLQ identity.
6. Configure receipt-retention cadence and retain cleanup CLI dry-run/live evidence.
7. Retain receipt/event concurrency, replay-window, rollback, telemetry, storefront,
   privacy, and operational packets.
8. Continue friendship lifecycle, broader directory/follow UX, lists, block/mute
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
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index-consumer --all-targets
cargo test -p rustok-social-graph --features index-consumer index_consumer::tests -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --features mod-social_graph --all-targets
cargo test -p rustok-server social_graph_index_worker --lib -- --nocapture
cargo test -p rustok-server runtime_guardrails --lib -- --nocapture
node scripts/verify/verify-social-graph-index-consumer.mjs
node scripts/verify/verify-social-graph-index-runtime-consumer.mjs
node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs
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
