# rustok-social-graph / CRATE_API

## Public modules

- `entities`
- `error`
- `follow_read`
- `graphql` behind feature `graphql`
- `index` behind feature `index`
- `index_consumer` behind feature `index-consumer`
- `maintenance`
- `migrations`
- `model`
- `observability`
- `ports`
- `service`

## Owner services and ports

- `SocialGraphService::new(DatabaseConnection)` creates a read-only owner service;
  relation writes fail closed until a transactional event bus is supplied.
- `SocialGraphService::with_event_bus(DatabaseConnection, TransactionalEventBus)`
  creates the write-capable owner service used by GraphQL and native storefront
  composition.
- `SocialGraphCommandPort::set_relation(PortContext, SetSocialRelationCommand)`
  persists relation state, a durable command receipt, and any real state-change event
  in one owner transaction.
- `SocialGraphPrivacyReadPort` owns block/mute/follow policy reads.
- `SocialGraphFollowReadPort` exposes revision-bearing directional follow state.
- `SocialGraphReceiptMaintenancePort::cleanup_completed_receipts(...)` exposes
  service/system-only dry-run and bounded completed-receipt cleanup.
- `SocialGraphRelationEventMaintenancePort::replay_relation_state_events(...)`
  exposes service/system-only bounded replay of authoritative relation facts.
- `SocialGraphRelationEventReplayCommand` carries optional exclusive UUID cursor,
  limit, and dry-run mode; the result returns selected/published counts and next cursor.
- `SocialRelationKind::{Block, Mute, Follow}` defines canonical persistence and event
  kind values.

## Durable command contract

- Commands require valid tenant, deadline, non-empty idempotency key, actor/source
  ownership, distinct target, canonical kind, and optional expected revision.
- Receipt identity is tenant plus normalized idempotency key.
- One key binds one complete command identity.
- Receipt reservation, mutation, optional event append, response snapshot, completion,
  and commit share one database transaction.
- Exact receipt replay returns the committed response without publishing a new event.
- Identity reuse fails as `social_graph.idempotency_conflict`.
- Corrupt/incomplete/unsupported receipt state fails closed.

## Receipt cleanup and replay

- Cleanup accepts service/system actors only, uses an explicit past cutoff, supports
  dry-run, and limits batches to 1..1000.
- Candidates are tenant-scoped, completed, schema-v1, ordered by `(completed_at, id)`,
  and all validated before deletion.
- The CLI requires explicit tenant and positive retention days; no default retention
  policy or automatic scheduler exists.
- Relation-event replay is service/system-only, tenant-scoped, UUID-cursor bounded,
  dry-run capable, and page-atomic.
- Bounded Social Graph replay republishes the same sealed relation facts used by live
  writes. Consumers must handle duplicate/stale delivery through monotonic revision.

## Events

- Publishes sealed `rustok_events::SocialGraphRelationEvent`
  `social_graph.relation.state_changed` v1 through
  `TransactionalEventBus::publish_contract_in_tx`.
- Payload contains relation id, source/target user ids, canonical kind, active state,
  and revision only; tenant and actor remain envelope metadata.
- New relations and persisted active-state transitions publish before shared commit.
- Receipt replay and exact persisted-state no-op publish no new live event.
- Event publication failure rolls relation and receipt back together.
- Social Graph persistence remains authoritative for repair.

## Optional Index projection

- Feature `index` enables generic Index conversion without making Index a default
  runtime dependency.
- `social_graph_relation_index_schema()` declares non-localized relation records keyed
  by tenant and relation id.
- `social_graph_relation_index_mutation(tenant_id, event_id, event)` accepts only a
  validated sealed `SocialGraphRelationEvent`.
- Active revisions become `IndexMutation::Upsert`; inactive revisions become
  revisioned `IndexMutation::Delete` tombstones.
- Relation revision becomes Index `source_version`.
- The adapter contains no database, broker, or privacy authorization logic.

## Durable Index consumer

- Feature `index-consumer` adds optional Iggy runtime composition on top of `index`.
- `SocialGraphIndexProjector::new(DatabaseConnection)` registers an in-memory schema,
  persists or exactly recognizes the tenant schema through Index-owned
  `PostgresSchemaRegistrationStore`, and applies through `PostgresMutationStore`.
- Schema persistence is tenant-scoped, exact-version idempotent, monotonic, and
  fail-closed for drift, retired state, invalid tenant, unsupported backend, or storage
  failure. Social Graph imports no Index entities and writes no `index_schemas` rows.
- `SocialGraphIndexConsumer::open(Arc<IggyTransport>, DatabaseConnection)` opens
  persistent group `rustok-social-graph-index` on the shared `domain` topic.
- `receive_next`, `project_consumed`, and `acknowledge_consumed` retain one outstanding
  delivery across bounded retries.
- `process_next(&mut self)` is the direct serialized receive/register/apply/ack path.
  It acknowledges only after schema persistence and the Index inbox result are durable;
  that result is committed before broker acknowledgement.
- `Applied`, `Duplicate`, and `StaleIgnored` are terminal durable outcomes.
- Unrelated sealed families are acknowledged without schema registration or mutation.
- `SocialGraphIndexConsumerError::{stable_code,is_retryable}` exposes bounded host
  classification without schema JSON, payload, identity, or storage causes.

## Staged DLQ contract

- `publish_consumed_to_dlq(consumed, stable_error_code, retry_count)` publishes exact
  retained broker bytes with connector metadata and does not commit the source offset.
- `acknowledge_consumed` is called separately so the host can retry acknowledgement
  without republishing DLQ in-process.
- `move_to_dlq_and_acknowledge` remains a convenience method and preserves publish
  before acknowledgement order.
- DLQ is valid only before a durable Index result. Once Index or DLQ has a terminal
  result, recovery is acknowledgement-only.
- Publish failure leaves the source replayable. Ack failure after successful DLQ publish
  also leaves the source offset uncommitted; redelivery may republish until a durable
  DLQ identity/receipt contract is approved.
- Malformed broker bytes that fail before `ConsumedContractEvent` construction remain
  unacknowledged pending a connector-level poison-message contract.

## Server-owned optional lifecycle

- The server `mod-social_graph` feature composes `index-consumer`, but execution is
  default-off until `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED=true`.
- Explicit enablement requires a worker-capable host and effective `outbox_iggy`.
- `EventRuntime` creates one configured `Arc<IggyTransport>` and shares that exact
  connector between outbound relay and inbound consumer. The worker never creates or
  stops a second bundled broker process.
- `SocialGraphIndexWorkerHandle` exposes task state and observes shared `StopHandle`.
- Projection retry uses bounded exponential backoff from reviewed event settings.
- When enabled, missing/stopped/invalid worker state is critical in
  `runtime_guardrails`, `/health/ready`, and aggregate guardrail metrics. Disabled
  execution contributes no failure.
- `SocialGraphIndexPositionObserver` also starts only under explicit enablement. It
  opens a separate read-only SDK connection using shared transport configuration and
  observes the same `StopHandle`.
- Position observation never starts/stops a broker, consumes deliveries, stores offsets,
  or participates in projection readiness. Configuration/connection/snapshot failures
  are telemetry-only and independently retried.

## Runtime consumer metrics

- `rustok_telemetry::runtime_consumer_metrics` registers a bounded shared Prometheus
  collector in the existing process registry rendered by `/metrics`.
- Delivery metrics cover received deliveries, terminal outcome throughput, projection
  and ack retries, bounded stage/stable-code failures, DLQ publication results,
  receive-to-ack duration, starts/terminations, in-flight state/timestamp, and last
  success.
- The position observer reads every topic partition plus the persistent group checkpoint
  and records snapshot timestamp, partition count, and completeness.
- `rustok_runtime_consumer_lag{aggregation="total|max"}` is published only from a
  complete coherent every-partition snapshot. Empty partitions contribute zero;
  missing or checkpoint-ahead-of-high-watermark state makes the snapshot incomplete.
- Incomplete snapshots set completeness to zero and clear lag gauges, preventing stale
  values from masquerading as current lag.
- Labels are bounded to consumer, stage, outcome, result, reason, aggregation, and stable
  error code.
- Tenant, event, relation, partition, offset, payload, ack token, credentials, and raw
  error text are not labels.
- Event age, processing duration, one delivered offset, and local cursor counters are
  not valid lag inputs.

## Authority boundary

- Social Graph owner ports and storage remain authoritative for block/mute/follow.
- Profiles privacy must not authorize from Index state or consumer lag.
- The adapter, projector, consumer, worker, position observer, and telemetry never read
  Social Graph owner tables for projection work.
- Index is optional discovery/query infrastructure and must not authorize presentation.

## Dependencies

- `rustok-api`: port context, actor, deadlines, idempotency, replay policy, typed errors.
- `rustok-core`: module and migration contracts.
- `rustok-events`: sealed relation event family.
- optional `rustok-index`: schema, conversion, registration, mutation persistence.
- optional `rustok-iggy`: persistent typed cursor, exact payload retention, DLQ, ack,
  and read-only broker position observation.
- `rustok-outbox`: transactional event bus.
- sibling CLI uses `rustok-cli-core` and `rustok-runtime`.

## Common mistakes

- Writing through `SocialGraphService::new(db)` and silently losing events.
- Publishing arbitrary string events or publishing after relation commit.
- Emitting an event for receipt replay or persisted-state no-op.
- Treating replay as exactly-once or applying lower revision over newer state.
- Putting idempotency keys, request context, claims, roles, locale, channel, or receipt
  snapshots into the external event.
- Reading relation tables from Profiles, Index, or another consumer.
- Registering only in memory and assuming the persisted schema foreign key exists.
- Writing `index_schemas` directly from Social Graph.
- Acknowledging before schema and Index result are durable.
- Creating or shutting down a second Iggy transport in the worker or position observer.
- DLQing after a durable Index result instead of retrying ack only.
- Re-serializing a decoded envelope instead of using exact broker bytes.
- Republish-to-DLQ on every in-process ack retry instead of staged acknowledgement-only
  recovery.
- Using tenant/event/relation/partition/offset/payload/error text as metric labels.
- Publishing lag from an incomplete snapshot, event age, or one global offset.
- Making observer failure projection-critical or readiness-critical.
- Authorizing Profiles visibility from projection state or consumer lag.

## Errors / stable failure families

- `social_graph.idempotency_key_invalid`
- `social_graph.idempotency_conflict`
- `social_graph.command_receipt_corrupt`
- `social_graph.event_publication_unavailable`
- `social_graph.receipt_cleanup_forbidden`
- `social_graph.receipt_cleanup_limit_invalid`
- `social_graph.receipt_cleanup_cutoff_invalid`
- `social_graph.receipt_cleanup_cutoff_future`
- `social_graph.relation_event_replay_forbidden`
- `social_graph.relation_event_replay_limit_invalid`
- `social_graph.storage_unavailable`
- Index consumption maps typed schema/registry/mutation failures to bounded
  `social_graph.index.*` host codes without publishing private storage causes.
- Position observation uses bounded `iggy.consumer_position.*` stable codes.
