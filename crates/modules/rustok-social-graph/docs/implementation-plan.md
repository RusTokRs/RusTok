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
persistent Iggy consumption, typed exact-byte decode failures, connector-owned neutral
poison receipts, one shared EventRuntime Iggy transport, default-off server lifecycle,
bounded projection/DLQ/receipt/ack retry, decoded-event and raw-delivery durable DLQ
recovery, deterministic UUIDv8 broker message IDs, graceful shutdown, enabled-worker
readiness, bounded per-consumer Prometheus telemetry, a read-only partition-qualified
consumer-position observer with completeness-gated total/max lag, and a count-only
neutral poison-receipt observer with stale-snapshot clearing.

The append-only migration tail now contains both decoded and neutral receipt
migrations. An opt-in PostgreSQL harness defines isolated-schema evidence for
concurrent neutral claim ownership, lease reclaim/fencing, collision rollback,
first-diagnostic retention, empty payloads, terminal recognition, and aggregate
inspection.

Compilation, source-verifier execution, PostgreSQL runtime execution, real-broker
deterministic-ID/deduplication and publish-confirmation behavior, observer reconnect/
TLS/auth, multi-replica position/receipt semantics, retention policy, and retained
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

## Delivered decoded-event DLQ receipt and broker-identity boundary

- Migration `m20260727_000004_create_index_dlq_receipts` owns immutable source identity
  and exact payload bytes for poison deliveries that already have trusted tenant and
  event identity.
- States are `reserved`, leased `publishing`, terminal `published`, and post-source-ack
  `acknowledged`. Publisher leases are bounded and reclaimable after process loss.
- `project_consumed` checks receipts before Index projection. Published receipts enter
  acknowledgement-only recovery; unfinished receipts remain retryable DLQ work.
- A versioned length-framed SHA-256 construction derives one RFC 9562 UUIDv8 from the
  immutable trusted receipt identity and exact payload.
- Retry count, time, publisher/lease identity, and random state are excluded.
- Exact-byte publication and durable `published` precede source acknowledgement.
- Ack failure after publication leaves a terminal receipt; redelivery retries ack only.
- Broker success followed by process/DB failure before `published` remains explicit
  confirmation ambiguity. A deterministic ID does not establish physical exactly-once.
- Historical rows are not fabricated; the migration backfill contract is `none`.

## Delivered typed raw poison terminalization boundary

- `PersistentContractConsumerGroup::receive_delivery` returns either a validated
  `ConsumedContractEvent` or `ConsumedContractDecodeFailure`; neither branch commits.
- `SocialGraphIndexConsumer::receive_delivery` exposes that typed result to the owner
  worker; raw acknowledgement is an isolated exact-cursor adapter.
- Decode/schema failures retain exact bytes, stream, topic, partition, offset, opaque
  acknowledgement metadata, bounded classification, and a deterministic connector
  delivery UUID without inventing tenant, actor, relation, or event identity.
- Connector migration `m20260728_000001_create_consumer_poison_receipts` owns the neutral
  result store. Delivery UUID/source-coordinate/payload collisions fail closed.
- Empty payload is valid. Error classification and observed attempt are retained as one
  first-observed diagnostic pair but are not immutable identity.
- Receipt states are `reserved`, leased `publishing`, `published`, and `acknowledged`.
- The worker checks an existing receipt before current DLQ policy. Existing selected
  work remains recoverable if creation of new DLQ decisions is later disabled.
- New undecodable deliveries remain uncommitted while no terminal policy is enabled.
- Exact-byte DLQ publication and durable `mark_published` precede source ack.
- Published redelivery is acknowledgement-only; post-ack `mark_acknowledged` is
  best-effort bookkeeping.
- Profiles privacy never reads or authorizes from this neutral receipt or broker state.

## Delivered persistent consumer, host lifecycle, and readiness

- Persistent group `rustok-social-graph-index` consumes the shared `domain` topic.
- Only `ContractEventPayload::SocialGraphRelation` reaches schema registration and
  mutation apply; unrelated validated sealed families are acknowledged without
  projection.
- `receive_delivery`, `project_consumed`, `acknowledge_consumed`, and
  `acknowledge_decode_failure` retain one outstanding cursor delivery and expose
  result-first decoded and raw paths.
- Execution is default-off until
  `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED=true` and then requires a worker host
  plus effective `outbox_iggy` delivery.
- `EventRuntime` publishes the configured shared `Arc<IggyTransport>`; relay and
  consumer reuse it and never start a second bundled broker process.
- Projection, decoded DLQ, neutral receipt, raw DLQ, and source acknowledgement use
  bounded exponential backoff from reviewed event settings.
- After a durable Index or DLQ result, only acknowledgement is retried.
- Missing/stopped/invalid enabled worker state is critical in `runtime_guardrails` and
  reaches `/health/ready`. Disabled execution is not degraded.
- A missing/stopped count-only poison observer is degraded, not critical, and does not
  stop projection or source acknowledgement.

## Delivered durable-consumer observability

- `rustok-telemetry::runtime_consumer_metrics` registers one bounded shared collector
  in the process Prometheus registry rendered by `/metrics`.
- Delivery metrics cover terminal outcomes, bounded retries/failures, DLQ results,
  receive-to-ack duration, lifecycle, in-flight state, last success, and complete lag.
- Raw outcomes are bounded to `decode_dead_lettered` and
  `decode_dead_letter_recovered`; delivery-level facts are not labels.
- `SocialGraphIndexPositionObserver` reads all topic partitions and persistent group
  checkpoints without mutating offsets. Incomplete snapshots clear lag gauges.
- `SocialGraphIndexPoisonObserver` reads only
  `ConsumerPoisonReceiptInspector::summarize` for the fixed consumer group.
- Poison metrics expose fixed `total`, `reserved`, `publishing`,
  `expired_publishing`, `published`, and `acknowledged` states plus availability and
  snapshot time. Unavailable inspection and shutdown clear stale values.
- Failure logs contain bounded stable codes and omit storage error text.
- Tenant, event, relation, partition, offset, payload, broker ID, publisher identity,
  acknowledgement token, credentials, and raw error text are not metric labels.
- Metrics and observers never authorize, acknowledge, reclaim, repair, retain, or
  delete receipt state.

## Migration-order reconciliation

The platform migrator discovers both receipt migrations, their truthful backfill
contracts remain `none`, and the explicit append-only release-order tail now ends with:

1. `m20260727_000004_create_index_dlq_receipts`
2. `m20260728_000001_create_consumer_poison_receipts`

This preserves the previously published migration prefix. The reconciliation was
merged separately before the later health/evidence slices.

## PostgreSQL poison receipt evidence

The opt-in `consumer_poison_receipt_postgres` target:

- selects `RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL` with `DATABASE_URL` fallback;
- skips without a PostgreSQL URL and contains no default credentials;
- creates and drops one unique schema per scenario;
- uses one connection per pool so session-local `search_path` is deterministic;
- uses independent pools for concurrent claim ownership;
- verifies one `Claimed` and one `Busy` result;
- verifies lease reclaim fences the previous publisher with `ClaimLost`;
- verifies UUID/source/payload conflicts roll back without rewriting the original row;
- verifies the winning reservation retains one atomic first-observed diagnostic pair;
- verifies reserved/published/acknowledged aggregate consistency and terminal redelivery.

Direct test SQL is limited to deterministic lease expiry and read-only diagnostics.
The harness and source guard are source-complete; no PostgreSQL run has been executed.

## Remaining Social Graph scope

1. Execute and retain the PostgreSQL neutral receipt harness, including server version,
   command/environment evidence, repeated concurrent ownership, and schema cleanup.
2. Execute decoded receipt and schema-registration concurrency/rollback/retention
   scenarios against PostgreSQL.
3. Prove real-Iggy validated and undecodable receive, deterministic UUID header
   publication, same-partition retry, connection loss/reconnect, ack failure, restart,
   graceful shutdown, observer snapshot/reconnect, and multi-replica ownership.
4. Exercise broker success followed by receipt-mark loss with deduplication disabled,
   enabled, capacity-evicted, and expired.
5. Decide whether production requires monitored Iggy deduplication, a broker
   transaction, or a DB-owned DLQ/outbox relay before stronger duplicate guarantees.
6. Validate lag and poison aggregates under concurrent publication, missing checkpoints,
   expired claims, TLS/auth failures, rebalancing, and multiple replicas before alerts.
7. Corrupt/delete projection state and prove bounded owner replay/rescan repair while
   Profiles privacy remains on authoritative owner ports.
8. Define decoded/raw receipt retention and reconciliation before deletion; retain
   cleanup CLI dry-run/live evidence.
9. Retain receipt/event concurrency, replay-window, rollback, telemetry, storefront,
   privacy, and operational packets.
10. Continue friendship lifecycle, directory/follow UX, lists, block/mute management,
    and moderation/admin repair.

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
cargo test -p rustok-iggy contract_decode_failure --lib -- --nocapture
node scripts/verify/verify-iggy-contract-decode-failure.mjs
RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy-connector --features iggy,migrations --all-targets
cargo test -p rustok-iggy-connector --features migrations consumer_poison_receipt -- --nocapture
cargo test -p rustok-iggy-connector --features migrations consumer_poison_inspection -- --nocapture
RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL='postgresql://…' cargo test -p rustok-iggy-connector --features migrations --test consumer_poison_receipt_postgres -- --nocapture
node scripts/verify/verify-iggy-consumer-poison-receipts.mjs
node scripts/verify/verify-iggy-consumer-poison-inspection.mjs
node scripts/verify/verify-iggy-consumer-poison-postgres-evidence.mjs
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
node scripts/verify/verify-social-graph-index-poison-observer.mjs
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
this slice.
