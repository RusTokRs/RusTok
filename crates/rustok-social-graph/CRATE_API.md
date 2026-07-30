# rustok-social-graph / CRATE_API

## Public modules

- `entities`
- `error`
- `follow_read`
- `graphql` behind feature `graphql`
- `index`, `index_privacy`, and `index_privacy_shadow` behind feature `index`
- `index_consumer` and `index_dlq_receipt` behind feature `index-consumer`
- `maintenance`
- `migrations`
- `model`
- `observability`
- `ports`
- `service`

## Owner services and ports

SocialGraphService::new(DatabaseConnection) creates a read-only owner service; relation
writes fail closed until a transactional event bus is supplied.

SocialGraphService::with_event_bus(DatabaseConnection, TransactionalEventBus) creates the
write-capable owner service used by GraphQL and native storefront composition.

SocialGraphRelationEventMaintenanceService::with_outbox(DatabaseConnection) creates the
owner-local operational replay service with the canonical transactional outbox. CLI and host
adapters must use this constructor instead of constructing transport outside Social Graph.

- `SocialGraphCommandPort::set_relation` owns validated, idempotent relation commands.
- `SocialGraphPrivacyReadPort` owns block, mute, and bounded directional follow policy reads.
- `IndexSocialGraphPrivacyReadPort` is the typed Index projection reader.
- `IndexShadowSocialGraphPrivacyReadPort` compares Index with the authoritative owner port
  while always returning the owner result.
- `IndexPrivacyShadowObservation` is the telemetry-neutral bounded comparison record consumed
  by a host-owned Prometheus adapter.
- `SocialGraphFollowReadPort` exposes revision-bearing follow state and is not served from
  the current Index schema.
- `SocialGraphReceiptMaintenancePort` owns bounded completed-receipt cleanup.
- `SocialGraphRelationEventMaintenancePort` owns bounded replay of authoritative facts.
- `SocialRelationKind::{Block, Mute, Follow}` defines canonical persistence and event values.

## Command and receipt contract

- Commands require a valid tenant, deadline, non-empty idempotency key, actor/source
  ownership, distinct target, canonical kind, and optional expected revision.
- Receipt identity is tenant plus normalized idempotency key, and one key binds one complete
  command identity.
- Receipt reservation, mutation, optional event append, response snapshot, completion, and
  commit share one owner transaction.
- Exact receipt replay returns the committed response without publishing another event.
- Identity reuse fails as `social_graph.idempotency_conflict`.
- Corrupt, incomplete, unsupported, or revision-conflicting state fails closed.

## Events and authoritative replay

- The module publishes sealed `social_graph.relation.state_changed` v1 through
  `TransactionalEventBus::publish_contract_in_tx`.
- Payload contains relation ID, source/target user IDs, canonical kind, active state, and
  revision; request context and receipt internals are excluded.
- New relations and real active-state transitions publish before the shared commit.
- Receipt replay and persisted-state no-op publish no new live event.
- Missing transactional publication returns `social_graph.event_publication_unavailable`
  and rolls relation plus receipt back together.
- Bounded Social Graph replay republishes the same sealed facts. Consumers must handle
  duplicate/stale delivery through monotonic revision.
- The replay port accepts service/system actors only, selects one tenant-scoped UUID-cursor
  page, enforces the 1000-row maximum, and returns the last selected relation as the next
  explicit cursor.
- Dry-run selects and reports one page without publishing. A live page publishes every fact
  through the transactional outbox and rolls the whole page back if one append fails.
- `rustok-cli social_graph relation-event-replay` is the canonical operational adapter. It
  requires a tenant, supports an optional cursor and bounded limit, processes one page per
  invocation, and never reads owner-private tables or constructs outbox transport itself.
- Social Graph persistence remains authoritative for drift repair.

## Optional Index projection

Feature `index` enables generic Index conversion, typed privacy comparison, and neutral shadow
observations without moving source authority into Index.

- `social_graph_relation_index_schema()` declares non-localized relation records keyed by
  tenant and relation ID.
- `social_graph_relation_index_mutation(tenant_id, event_id, event)` accepts only a
  validated sealed `SocialGraphRelationEvent`.
- Active revisions become `IndexMutation::Upsert`; inactive revisions become revisioned
  `IndexMutation::Delete` tombstones.
- Relation revision becomes Index `source_version`.
- Projection conversion contains no database, broker, or privacy authorization logic.

## Typed Index privacy reader

`IndexSocialGraphPrivacyReadPort::new(SharedIndexQueryRuntime)` stores only the neutral
`Arc<dyn IndexQueryPort>` capability. It receives no database connection, never constructs
`PostgresIndexQueryPort`, and never reads `social_graph_relations`.

- Block checks preserve either-direction semantics through typed `And` and `Or` filters.
- Mute and follow checks remain directional.
- Follow reads preserve source-actor validation.
- Follow batches retain the 100-target bound, deduplicate input, use typed `In`, validate
  projected UUIDs, and return deterministic sorted IDs.
- All operations preserve `PortCallPolicy::read`, tenant parsing, and self-relation rejection.
- Missing tenant schema or storage availability maps to retryable
  `social_graph.index_privacy_unavailable`.
- Plan/compiler/decoder/backend/result-contract drift maps to non-retryable
  `social_graph.index_privacy_contract_invalid`.

The typed reader is not independently authorized to decide privacy. A successful empty result
may be stale and therefore cannot prove that no current block or mute exists.

## Non-authoritative Index privacy shadow

`IndexShadowSocialGraphPrivacyReadPort` owns an authoritative
`Arc<dyn SocialGraphPrivacyReadPort>`, the typed Index reader, and an
`Arc<dyn IndexPrivacyShadowObserver>`.

- Every method executes the owner read first.
- Owner error is returned unchanged.
- After owner success, the same request is issued to Index for comparison.
- The projected comparison receives only the remaining caller deadline budget measured from
  entry to the shadow wrapper.
- Budget exhaustion or projected timeout records retryable
  `social_graph.index_privacy_unavailable` without changing the owner result.
- All four methods return the owner result; the Index shadow never authorizes or suppresses.
- Boolean comparisons classify `match_positive`, `match_negative`, `false_negative`, or
  `false_positive`.
- Follow batches compare sets and classify empty/non-empty matches, `batch_missing`,
  `batch_extra`, or `batch_mixed`.
- Projection failure records `error`, a bounded `IndexPrivacyShadowFailureCode`, and
  retryability.
- `IndexPrivacyShadowObservation` contains only operation, outcome, comparison duration,
  optional bounded failure code, and optional retryability.
- Logs contain operation, fixed outcome, booleans/counts, retryability, and comparison
  duration only.
- Tenant, user, relation, entity, payload, SQL, and raw storage details are absent from the
  neutral observation contract and logs.

The Social Graph crate has no telemetry dependency. `rustok-server` owns the host-owned
Prometheus adapter from `IndexPrivacyShadowObservation` to the single process metrics
registry.

The final notification block/mute policy uses the wrapper only when
`RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED=true`. The shadow is default-off. When
disabled, ordinary owner policy remains unchanged. When enabled, successful collector
registration and `SharedIndexQueryRuntime` are required, but the owner result remains
authoritative. Custom notification block/mute runtimes retain priority.

Metrics-disabled or uninitialized telemetry fails explicit shadow activation rather than
silently running an unmeasured evidence mode. Authoritative cutover remains blocked until
retained per-tenant freshness/watermark, lag, positive/negative parity, outage/recovery,
repair, latency, and negative-result fail-closed evidence exists.

## Privacy shadow metric contract

The host-owned Prometheus adapter publishes:

- `rustok_social_graph_index_privacy_shadow_observations_total{operation,outcome}`;
- `rustok_social_graph_index_privacy_shadow_failures_total{operation,error_code,retryable}`;
- `rustok_social_graph_index_privacy_shadow_comparison_duration_seconds{operation,outcome}`;
- `rustok_social_graph_index_privacy_shadow_last_observation_timestamp_seconds{operation,outcome}`.

Operations are fixed to `blocks_between`, `source_mutes_target`, `source_follows_target`, and
`source_follows_targets`. Error-code labels are limited to
`social_graph.index_privacy_unavailable`, `social_graph.index_privacy_contract_invalid`, or
`other`. The adapter never emits tenant and user identifiers, relation/entity IDs, payload,
SQL, or raw storage error labels.

These metrics are measurement source only. They do not define sample thresholds, freshness
watermarks, scrape retention, admission, alerts, or authorization.

## Durable Index consumer

Feature `index-consumer` adds optional persistent Iggy consumption on top of `index`.

- `SocialGraphIndexProjector` registers the owner schema, persists or exactly recognizes the
  tenant contract through `PostgresSchemaRegistrationStore`, and applies through
  `PostgresMutationStore`.
- Schema persistence is tenant-scoped, exact-version idempotent, monotonic, and fail-closed
  for drift, retired state, invalid tenant, unsupported backend, or storage failure.
- `SocialGraphIndexConsumer` opens persistent group `rustok-social-graph-index` on topic
  `domain`.
- `receive_delivery()` returns either a validated event or an exact-byte decode failure and
  never acknowledges either result.
- `project_consumed` and `acknowledge_consumed` retain one outstanding decoded delivery
  across bounded retries.
- The decoded path is result-first: it checks durable DLQ state before projection and
  acknowledges only after a terminal Index or DLQ result exists and that result is committed.
- `acknowledge_decode_failure` commits only the exact raw-delivery cursor after the server
  establishes durable neutral poison state. It performs no projection or identity inference.
- `process_next()` remains the direct validated-event compatibility path and durably projects
  before source acknowledgement.
- `Applied`, `Duplicate`, and `StaleIgnored` are terminal outcomes.
- Unrelated validated sealed families are acknowledged without schema registration or
  mutation.
- Stable error classification exposes no schema JSON, payload, identity, or private storage
  cause.

The persistent consumer acknowledges only after durable projection or a durable terminal DLQ
result exists and that result is committed. It must not authorize privacy from projection,
receipt, broker, or lag state.

## Decoded and raw poison recovery

- The Social Graph DLQ receipt binds tenant, consumer group, event ID, exact broker
  coordinates, original bytes, stable error code, and projection attempt count.
- Receipt states are `reserved`, leased `publishing`, terminal `published`, and
  bookkeeping-complete `acknowledged`.
- A deterministic RFC 9562 UUIDv8 binds immutable receipt identity and is reused across
  publication retries.
- Published or acknowledged receipts skip projection and republication and enter
  acknowledgement-only recovery.
- Source acknowledgement precedes best-effort acknowledged bookkeeping; bookkeeping failure
  cannot reverse a committed cursor.
- Broker success before durable `published` remains an explicit duplicate ambiguity; no
  exactly-once claim is made without retained broker deduplication evidence.
- Raw decode failures retain exact bytes and coordinates without inventing tenant or event
  facts.
- Neutral raw poison receipts do not publish, acknowledge, authorize, or choose policy.
- Decoded or raw poison state must not authorize Profiles privacy or presentation.

## Server-owned optional lifecycle

- `mod-social_graph` composes `index-consumer`, but execution remains default-off until
  `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED=true`.
- Explicit worker enablement requires a worker-capable host and effective `outbox_iggy`.
- The shared configured `IggyTransport` is used by outbound relay, inbound consumer, and
  identified DLQ publication; the worker never starts or stops another broker process.
- Projection, DLQ publication, receipt transitions, and acknowledgement use bounded retry.
- Existing durable poison work continues toward its selected terminal result if policy for
  new DLQ decisions changes.
- Enabled worker absence, termination, or invalid readiness is critical; disabled execution
  contributes no readiness failure.
- The position observer is read-only telemetry and never consumes, acknowledges, projects,
  or owns readiness.

## Metrics

- Runtime consumer metrics use bounded labels for consumer, stage, outcome, result, reason,
  aggregation, and stable code.
- Privacy shadow metrics use fixed operation/outcome domains and bounded error-code mapping.
- Tenant, event, relation, partition, offset, payload, broker ID, acknowledgement token,
  credentials, raw error text, and privacy request identities are forbidden labels.
- Lag is published only from a complete every-partition snapshot plus group checkpoint.
- Incomplete snapshots clear lag gauges rather than presenting stale values.
- Event age, one observed offset, and local cursor counters are not valid lag inputs.

## Authority boundary

- Social Graph owner storage, commands, events, replay, drift repair, and privacy decisions
  remain authoritative for block, mute, and follow.
- Notification block/mute policy may run the approved Index projection only through
  `IndexShadowSocialGraphPrivacyReadPort`, which always returns the owner result.
- Index shadow mismatch, timeout, or failure never authorizes, suppresses, widens, or changes
  policy.
- The projected comparison must stay within the remaining caller deadline budget.
- Privacy shadow observations and metrics observe parity but must not authorize or suppress.
- Profiles privacy, presentation visibility, and revision-bearing follow state must not
  authorize from Index state, DLQ receipts, broker IDs, deduplication state, consumer lag, or
  shadow metrics.
- Projection infrastructure must not independently authorize presentation.

## Dependencies

- `rustok-api`: port context, actor, deadlines, idempotency, replay policy, typed errors.
- `rustok-core`: module and migration contracts.
- `rustok-events`: sealed relation event family.
- optional `rustok-index`: schema conversion, registration, mutation persistence, and typed
  query runtime consumption.
- optional `rustok-iggy`: persistent typed cursor, exact payload retention, DLQ publication,
  acknowledgement, and read-only position observation.
- `rustok-outbox`: transactional event bus.
- `sha2`: deterministic receipt-bound UUIDv8 derivation.
- `tokio`: bounded async shadow comparison and test/runtime scheduling.

## Common mistakes

- Writing through `SocialGraphService::new(db)` and silently losing events.
- Publishing arbitrary strings or publishing after relation commit.
- Emitting an event for receipt replay or persisted-state no-op.
- Reading Social Graph tables from Profiles, Index, or another consumer.
- Constructing replay outbox transport in CLI or host code instead of using
  `SocialGraphRelationEventMaintenanceService::with_outbox`.
- Constructing `PostgresIndexQueryPort` in Social Graph or bypassing
  `SharedIndexQueryRuntime`.
- Returning the Index result from the privacy shadow instead of the owner result.
- Awaiting the projected privacy shadow beyond the remaining caller deadline budget.
- Treating shadow mismatch, error, empty projection, observation, or metric as a privacy
  decision.
- Treating schema readiness as a freshness watermark.
- Logging tenant/user/relation identities from shadow comparison.
- Adding tenant, user, relation, entity, payload, SQL, or raw error metric labels.
- Adding a telemetry dependency to the Social Graph owner instead of using the host-owned
  Prometheus adapter.
- Enabling the evidence shadow without the single process Prometheus registry.
- Using Index for revision-bearing follow state or Profiles presentation authorization.
- Acknowledging before durable Index or DLQ result completion.
- Inventing tenant/event identity for undecodable bytes.
- Treating a deterministic broker ID as an exactly-once guarantee.
- Republishing DLQ content during acknowledgement-only recovery.
- Publishing lag from an incomplete snapshot.
- Authorizing Profiles visibility from projection or poison state.

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
- `social_graph.index_privacy_unavailable`
- `social_graph.index_privacy_contract_invalid`
- bounded `social_graph.index.*` projection and DLQ families
- bounded `iggy.connector.poison_*` raw poison families
- bounded `iggy.contract.decode_invalid` and `iggy.contract.schema_invalid`
- bounded `iggy.consumer_position.*` observation families
