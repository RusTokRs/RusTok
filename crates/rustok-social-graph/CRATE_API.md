# rustok-social-graph / CRATE_API

## Public modules

- `entities`
- `error`
- `follow_read`
- `graphql` behind feature `graphql`
- `index` behind feature `index`
- `index_consumer` behind feature `index-consumer`
- `index_dlq_receipt` behind feature `index-consumer`
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
- Corrupt, incomplete, or unsupported receipt state fails closed.

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
- `receive_delivery()` is the primary typed receive boundary and returns either a
  validated `PersistentContractDelivery::Event` or an exact-byte
  `PersistentContractDelivery::DecodeFailure`. It never acknowledges either result.
- `receive_next()` remains the validated-event compatibility path.
- `project_consumed` and `acknowledge_consumed` retain one outstanding decoded delivery
  across bounded retries.
- `acknowledge_decode_failure` commits only the exact retained raw-delivery cursor after
  the server worker establishes a durable neutral poison result. It performs no
  projection, publication, or identity inference.
- The result-first decoded contract checks any durable DLQ receipt before projection and
  commits the source cursor only after a terminal owner result exists.
- `process_next()` is the direct serialized validated-event receive/register/apply/ack
  compatibility path. It acknowledges only after schema persistence and the Index inbox
  result are durable; that result is committed before broker acknowledgement.
- `Applied`, `Duplicate`, and `StaleIgnored` are terminal durable outcomes.
- Unrelated validated sealed families are acknowledged without schema registration or
  mutation.
- `SocialGraphIndexConsumerError::{stable_code,is_retryable}` exposes bounded host
  classification without schema JSON, payload, identity, or storage causes.

## Decoded-event DLQ receipt and broker identity

- Migration `m20260727_000004_create_index_dlq_receipts` owns one immutable poison
  identity per `(tenant_id, consumer_group, event_id)` with exact source
  stream/topic/partition/offset, original broker bytes, stable error code, and
  projection attempt count.
- Receipt states are `reserved`, leased `publishing`, terminal `published`, and
  bookkeeping-complete `acknowledged`. Claim leases are bounded and reclaimable after a
  crashed publisher.
- `project_consumed` checks the receipt before Index projection. `published` or
  `acknowledged` returns the bounded recovered dead-letter outcome; `reserved` or
  `publishing` remains retryable DLQ work and never re-enters mutation apply.
- Internal `index_dlq_message_id` derives a deterministic RFC 9562 UUIDv8 from a
  versioned domain plus tenant, consumer group, event ID, source stream/topic/partition/
  offset, and exact payload. Length framing prevents concatenation ambiguity.
- Retry count, wall-clock time, publisher instance, lease owner, and random values are
  excluded, so every retry of one immutable receipt produces the same broker ID.
- `publish_consumed_to_dlq` durably reserves/claims the identity, attaches the UUIDv8 to
  `DlqEntry`, publishes exact retained bytes, and returns success only after the receipt
  reaches `published`.
- `IggyTransport` maps the UUIDv8 to Iggy's `u128` message header through a lazy SDK
  publisher connection to the same configured endpoint and existing `dlq` topic.
- Repeated calls recognize `published` or `acknowledged` and skip broker publication.
- `acknowledge_consumed` commits the source cursor after a terminal Index or decoded DLQ
  result. Receipt transition to `acknowledged` is best-effort bookkeeping after broker
  commit and cannot turn a committed source offset back into a failure.
- `move_to_dlq_and_acknowledge` remains a convenience method and preserves durable
  receipt/broker publication before source acknowledgement.
- Ack failure after `publish_consumed_to_dlq` succeeds leaves a durable `published`
  receipt; redelivery skips both projection and DLQ publication and retries source ack.
- Broker success followed by process/DB failure before the `published` transition has
  confirmation ambiguity. Republication carries the same UUIDv8; physical duplicate
  suppression occurs only when Iggy deduplication is enabled and its per-partition
  cache/expiry still covers the recovery interval.
- Durable receipt state, not broker deduplication, remains the terminal owner contract;
  physical broker exactly-once is not claimed without retained configuration/runtime
  evidence or a stronger transaction/outbox mechanism.

## Raw decode-failure poison contract

- `ConsumedContractDecodeFailure` retains exact bytes, stream, topic, partition, offset,
  opaque acknowledgement metadata, bounded decode/schema classification, and a
  deterministic connector delivery UUID without inventing tenant or domain event facts.
- Connector migration `m20260728_000001_create_consumer_poison_receipts` owns neutral
  durable result storage for those raw deliveries.
- `ConsumerPoisonIdentity` binds one deterministic delivery UUID and one
  consumer-group/stream/topic/partition/offset coordinate set to the exact payload.
  Empty payload is valid input; UUID/source reuse with different coordinates or bytes
  fails closed.
- Error classification and observed attempt count are retained as first diagnostics but
  are excluded from immutable identity.
- Neutral receipt states are `reserved`, leased `publishing`, `published`, and
  `acknowledged`. The connector store does not publish, acknowledge, authorize, or choose
  policy.
- The server worker looks up the neutral receipt before applying current DLQ policy.
  Existing raw poison work continues recovery even if creation of new DLQ decisions is
  later disabled. A new undecodable delivery remains unacknowledged when DLQ is disabled.
- For a claimed receipt, the worker publishes
  `ConsumedContractDecodeFailure::to_dlq_entry` with exact bytes and its deterministic
  broker message ID, then persists `published` before entering source acknowledgement.
- A `published` or `acknowledged` receipt skips republication and enters
  acknowledgement-only recovery.
- Source acknowledgement precedes best-effort `mark_acknowledged` bookkeeping. A
  bookkeeping failure cannot reverse a committed source offset.
- Broker success followed by loss before durable `published` remains an explicit
  duplicate ambiguity. Recovery reuses the same deterministic message ID but does not
  claim exactly-once without retained broker deduplication evidence.
- Raw poison state has no projection or presentation authority and must not authorize
  Profiles privacy.

## Server-owned optional lifecycle

- The server `mod-social_graph` feature composes `index-consumer`, but execution is
  default-off until `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED=true`.
- Explicit enablement requires a worker-capable host and effective `outbox_iggy`.
- `EventRuntime` creates one configured `Arc<IggyTransport>` and shares that exact
  transport between outbound relay and inbound consumer. The worker never creates or
  stops a second bundled broker process.
- The server enables the connector `migrations` feature explicitly for neutral receipt
  storage; it does not rely on incidental transitive feature unification.
- Identified decoded and raw DLQ publication may lazily open one SDK client owned by that
  shared `IggyTransport`; it connects to the same configured broker and is dropped on
  failure for bounded retry-time reconnect.
- `SocialGraphIndexWorkerHandle` exposes task state and observes shared `StopHandle`.
- Projection, decoded/raw DLQ publication, neutral receipt transitions, and source
  acknowledgement use bounded exponential retry from reviewed event settings.
- Existing decoded or raw `reserved`/`publishing` receipts continue toward their
  previously chosen terminal result even if policy for new DLQ decisions is later
  disabled.
- After a durable terminal result exists, retries are acknowledgement-only; projection
  and terminal-result selection do not repeat.
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
- Delivery metrics cover validated and raw received deliveries, terminal outcome
  throughput, projection, DLQ, poison-receipt and ack retries, bounded stage/stable-code
  failures, DLQ `published`, `already_published`, and `failure` results,
  receive-to-ack duration, starts/terminations, in-flight state/timestamp, and last
  success.
- Raw terminal outcomes are bounded to `decode_dead_lettered` and
  `decode_dead_letter_recovered`.
- The position observer reads every topic partition plus the persistent group checkpoint
  and records snapshot timestamp, partition count, and completeness.
- `rustok_runtime_consumer_lag{aggregation="total|max"}` is published only from a
  complete coherent every-partition snapshot. Empty partitions contribute zero;
  missing or checkpoint-ahead-of-high-watermark state makes the snapshot incomplete.
- Incomplete snapshots set completeness to zero and clear lag gauges, preventing stale
  values from masquerading as current lag.
- Labels are bounded to consumer, stage, outcome, result, reason, aggregation, and stable
  error code.
- Tenant, event, relation, partition, offset, payload, delivery/broker ID, ack token,
  credentials, and raw error text are not labels.
- Event age, processing duration, one delivered offset, and local cursor counters are
  not valid lag inputs.

## Authority boundary

- Social Graph owner ports and storage remain authoritative for block/mute/follow.
- Profiles privacy must not authorize from Index state, decoded or raw DLQ receipts,
  broker IDs, deduplication state, or consumer lag.
- The adapter, projector, consumer, worker, position observer, and telemetry never read
  Social Graph relation tables for projection work.
- Index and DLQ/lag operations are optional infrastructure and must not authorize
  presentation.

## Dependencies

- `rustok-api`: port context, actor, deadlines, idempotency, replay policy, typed errors.
- `rustok-core`: module and migration contracts.
- `rustok-events`: sealed relation event family.
- optional `rustok-index`: schema, conversion, registration, mutation persistence.
- optional `rustok-iggy`: persistent typed cursor, exact payload retention, deterministic
  DLQ header publication, DLQ, ack, and read-only broker position observation.
- server composition uses `rustok-iggy-connector` feature `migrations` for neutral raw
  poison receipt storage.
- `sha2`: versioned deterministic UUIDv8 derivation from immutable receipt identity.
- `rustok-outbox`: transactional event bus.
- sibling CLI uses `rustok-cli-core` and `rustok-runtime`.

## Common mistakes

- Writing through `SocialGraphService::new(db)` and silently losing events.
- Publishing arbitrary string events or publishing after relation commit.
- Emitting an event for receipt replay or persisted-state no-op.
- Treating replay, a deterministic broker ID, or broker DLQ publication as exactly-once
  without retained deduplication configuration/runtime evidence.
- Putting idempotency keys, request context, claims, roles, locale, channel, or command
  receipt snapshots into the external event.
- Reading relation tables from Profiles, Index, or another consumer.
- Registering only in memory and assuming the persisted schema foreign key exists.
- Writing `index_schemas` directly from Social Graph.
- Calling the compatibility `receive_next` in a worker that must handle raw decode
  failures.
- Acknowledging before schema/Index result or durable decoded/raw DLQ publication is
  complete.
- Inventing tenant or event identity for undecodable bytes or placing raw receipt state
  into Profiles authorization.
- Creating or shutting down a second Iggy transport or bundled process in the worker,
  identified publisher, or position observer.
- Generating a random broker message ID per retry or including retry count/time in it.
- Assuming Iggy deduplication is enabled, durable, global, or unbounded.
- DLQing after a durable Index result instead of retrying ack only.
- Re-serializing a decoded envelope instead of using exact broker bytes.
- Ignoring an existing decoded or raw receipt and re-entering projection or selecting a
  new terminal result on redelivery.
- Treating `reserved` or `publishing` as safe source-ack states.
- Returning DLQ success before the `published` receipt transition.
- Republish-to-DLQ on every ack retry after a durable `published` receipt exists.
- Blocking recovery of an existing receipt merely because new DLQ decisions were later
  disabled.
- Deleting receipt rows without an approved retention/reconciliation contract.
- Using tenant/event/relation/partition/offset/payload/broker-ID/error text as metric
  labels.
- Publishing lag from an incomplete snapshot, event age, or one global offset.
- Making observer failure projection-critical or readiness-critical.
- Authorizing Profiles visibility from projection state, DLQ receipts, or consumer lag.

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
- Decoded DLQ receipts add bounded `social_graph.index.dlq_receipt_*` and
  `social_graph.index.dlq_publish_in_progress` codes.
- Neutral raw receipts add bounded `iggy.connector.poison_*` families. The worker also
  uses bounded `iggy.connector.poison_claim_busy` for live claim contention.
- Raw transport classification remains `iggy.contract.decode_invalid` or
  `iggy.contract.schema_invalid`.
- Identified Iggy publication logs bounded `iggy.dlq_publisher.*` codes.
- Position observation uses bounded `iggy.consumer_position.*` stable codes.
