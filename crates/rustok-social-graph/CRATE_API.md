# rustok-social-graph / CRATE_API

## Public Modules
- `entities`
- `error`
- `follow_read`
- `graphql` behind the `graphql` feature
- `index` behind the `index` feature
- `index_consumer` behind the `index-consumer` feature
- `maintenance`
- `migrations`
- `model`
- `observability`
- `ports`
- `service`

## Primary Public Types and Signatures
- `SocialGraphService::new(DatabaseConnection)` creates a read-only owner service.
  Relation writes fail closed until a transactional event bus is supplied.
- `SocialGraphService::with_event_bus(DatabaseConnection, TransactionalEventBus)`
  creates the write-capable owner service used by GraphQL and native storefront composition.
- `SocialGraphCommandPort::set_relation(PortContext, SetSocialRelationCommand)`
  persists relation state, a durable command receipt, and any real state-change event
  in one owner transaction.
- `SocialGraphPrivacyReadPort` exposes block/mute/follow policy reads.
- `SocialGraphFollowReadPort` exposes revision-bearing directional follow state.
- `SocialGraphReceiptMaintenancePort::cleanup_completed_receipts(...)` exposes
  explicit service/system-only dry-run and bounded completed-receipt cleanup.
- `SocialGraphReceiptMaintenanceService::new(DatabaseConnection)` provides the
  owner receipt-maintenance implementation without adding a user transport.
- `SocialGraphRelationEventMaintenancePort::replay_relation_state_events(...)`
  exposes service/system-only bounded replay of authoritative persisted relation facts.
- `SocialGraphRelationEventMaintenanceService::new(DatabaseConnection, TransactionalEventBus)`
  provides the owner replay implementation over the same typed Outbox contract as live writes.
- `SocialGraphRelationEventReplayCommand` carries an optional UUID cursor, limit,
  and dry-run mode; `SocialGraphRelationEventReplayResult` returns selected/published
  counts and the next UUID cursor.
- `SocialRelationKind::{Block, Mute, Follow}` and `SocialRelationKind::as_str()`
  define canonical persistence and event kind values.

## Optional Index projection and durable consumer
- Feature `index` enables the owner-published generic Index contract without making
  Index a runtime dependency of the default Social Graph build.
- `social_graph_relation_index_schema()` declares non-localized active relation
  records keyed by tenant and relation id with source user, target user, and kind fields.
- `social_graph_relation_index_mutation(tenant_id, event_id, event)` accepts only the
  sealed validated `SocialGraphRelationEvent` and maps its positive relation revision
  to the Index `source_version`.
- Active revisions become `IndexMutation::Upsert`; inactive revisions become
  revisioned `IndexMutation::Delete` tombstones.
- Feature `index-consumer` adds optional Iggy runtime composition on top of `index`.
- `SocialGraphIndexProjector::new(DatabaseConnection)` is transport-neutral. For a
  relevant sealed envelope it persists or exactly recognizes the tenant schema through
  Index-owned `PostgresSchemaRegistrationStore`, then applies or terminally recognizes
  the mutation through `PostgresMutationStore`.
- Schema registration is tenant-scoped, exact-version idempotent, monotonic, and
  fail-closed for contract drift or retired state. Social Graph imports no Index
  entities and writes no `index_schemas` rows directly.
- `SocialGraphIndexConsumer::open(Arc<IggyTransport>, DatabaseConnection)` opens the
  dedicated `rustok-social-graph-index` persistent contract consumer group on the
  shared `domain` topic and owns one projector.
- Staged methods `receive_next`, `project_consumed`, and `acknowledge_consumed` let a
  host retain one unacknowledged delivery across bounded retries without allowing a
  later cursor item to overtake it.
- `process_next(&mut self)` remains the direct serialized receive/project/ack path.
  It validates the sealed family, persists the tenant schema, durably applies or
  terminally recognizes duplicate/stale delivery in the Index inbox, and acknowledges
  only after that result is committed.
- `SocialGraphIndexConsumerError::{stable_code,is_retryable}` exposes bounded host
  classification without publishing schema JSON, payloads, or database causes.
- `move_to_dlq_and_acknowledge` publishes the exact retained broker bytes with connector
  metadata and only then commits the source offset. It is valid only before a durable
  Index result exists.
- Other sealed event families on the shared domain topic are acknowledged as unrelated
  by this dedicated consumer group without schema registration or Index mutation.
- Bounded Social Graph replay republishes the same sealed relation facts and therefore
  uses the same result-first schema/inbox path for repair.
- Neither adapter, projector, consumer, nor host worker reads Social Graph tables or
  makes the Index projection authoritative. Profiles privacy must not authorize from
  projection state.

## Server-owned optional lifecycle
- The server `mod-social_graph` feature composes `index-consumer`, but execution is
  default-off until `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED=true`.
- Explicit enablement requires a worker-capable host and effective event delivery
  profile `outbox_iggy`; startup fails rather than silently consuming another profile.
- `EventRuntime` creates one configured `Arc<IggyTransport>` for `outbox_iggy`, stores
  that exact connector in `ServerRuntimeContext`, and shares it between outbound relay
  and inbound Social Graph consumption. The worker never starts or stops a second
  connector, which preserves single bundled-Iggy process ownership.
- `SocialGraphIndexWorkerHandle` exposes task completion/readiness source state and the
  worker subscribes to the shared `StopHandle` for graceful shutdown.
- Projection failures use bounded exponential retry from the reviewed event relay
  retry settings. Permanent or exhausted failures are moved to DLQ only when the
  reviewed event DLQ setting is enabled.
- DLQ publication uses exact original JSON/MessagePack bytes and precedes source ack.
  When DLQ is disabled or DLQ publication fails, the worker exits with the offset
  uncommitted.
- Once a durable Index result exists, only acknowledgement is retried. Ack failure is
  never converted into a poison delivery because redelivery is safe through Index
  duplicate/stale recognition.
- Malformed bytes that fail before `ConsumedContractEvent` construction remain
  unacknowledged; a lower-level connector poison-delivery contract is still pending.
- When explicit enablement is true, `runtime_guardrails` requires the worker handle to
  remain ready. Missing, stopped, or invalidly configured state reaches `/health/ready`
  and the aggregate runtime-guardrail Prometheus status under the existing rollout
  observe/enforce policy. A disabled worker contributes no readiness failure.
- Dedicated per-consumer throughput, retry, DLQ, lag, and last-success metrics remain
  pending retained observability work.

## Owner-local CLI adapter
- `rustok-social-graph-cli::command_provider(RuntimeComposition)` exposes selected
  distribution command `social_graph receipt-cleanup`.
- The command requires explicit `--tenant-id` and positive `--retention-days`;
  there is no default retention window.
- `--limit` defaults only the bounded page size to `100` and cannot exceed `1000`.
- `--dry-run` reaches `SocialGraphReceiptMaintenancePort` without deleting rows.
- The adapter derives the cutoff, builds a system-actor port context with deadline
  and idempotency semantics, and delegates to the owner maintenance service.
- It never imports receipt entities or reads/deletes owner tables directly.
- Output is aggregate only and no scheduler or automatic cleanup is enabled.

## Events
- Publishes sealed `rustok_events::SocialGraphRelationEvent` contract
  `social_graph.relation.state_changed` v1 through
  `TransactionalEventBus::publish_contract_in_tx`.
- Live publication occurs only for a new relation or persisted active-state transition.
- Exact persisted-state no-op and durable receipt replay publish no new live event.
- The event is an authoritative fact for one persisted revision. Its payload contains
  relation id, source/target user ids, canonical kind, active state, and revision;
  tenant and actor stay in envelope metadata.
- Event publication failure rolls relation state and command receipt back together
  and maps to `social_graph.event_publication_unavailable`.
- Bounded maintenance replay may repeat the same relation revision. Consumers must
  apply by relation id plus monotonic revision and ignore duplicate or lower revisions.
- Replay never makes a consumer projection authoritative; Social Graph persistence
  remains the drift-repair source.

## Dependencies on Other RusToK Crates
- `rustok-api` for neutral port context, actor, deadline, idempotency, event-replay policy, and typed errors.
- `rustok-core` for module and migration contracts.
- `rustok-events` for the sealed relation event family.
- `rustok-index` is optional and used by the feature-gated owner conversion,
  Index-owned tenant schema registration, and durable projection consumer.
- `rustok-iggy` is optional and used only by feature `index-consumer` for a persistent
  typed-event cursor, exact payload retention, DLQ publication, and result-first ack.
- `rustok-outbox` for the transactional event bus.
- `rustok-cli-core` and `rustok-runtime` are used only by sibling
  `rustok-social-graph-cli`, not by the owner domain crate.
- `outbox` is declared in both root `modules.toml` and local
  `rustok-module.toml` topology.

## Common AI Mistakes
- Uses `SocialGraphService::new(db)` for a write path and silently loses relation events.
- Publishes an arbitrary string event instead of the sealed `rustok-events` contract.
- Publishes after relation/receipt commit rather than in the shared transaction.
- Emits a live event for receipt replay or an exact persisted-state no-op.
- Treats replay as exactly-once or applies a lower revision over a newer consumer result.
- Starts historical replay before every active writer uses the atomic event path.
- Logs the raw replay UUID cursor or per-relation identifiers in aggregate maintenance telemetry.
- Adds idempotency keys, expected revision, request context, claims, roles,
  locale, channel, or receipt snapshots to the external event.
- Lets an Index or other consumer projection become authoritative for block/mute/follow state.
- Builds the Index mutation from owner tables or an unsealed transport-local payload.
- Registers only in the in-memory `SchemaRegistry` and then assumes the tenant schema
  foreign key exists in Index storage.
- Writes `index_schemas` directly from Social Graph instead of using the Index owner API.
- Acknowledges the broker message before tenant schema registration and the Index
  inbox/result are durable.
- Enables the host worker without `outbox_iggy`, or enables it implicitly by compiling
  the module feature.
- Creates another `IggyTransport` in the worker instead of reusing the connector owned
  by `EventRuntime`, especially in bundled mode where this starts a second process.
- Lets the consumer worker shut down the shared relay connector instead of observing
  the host-owned `StopHandle` lifecycle.
- DLQs a delivery after the Index result committed instead of retrying ack only.
- Re-serializes a decoded envelope for DLQ rather than using exact broker bytes.
- Runs concurrent receive/apply/ack operations on one persistent cursor instead of
  preserving one outstanding delivery.
- Treats an unrelated sealed event on the shared domain topic as a Social Graph relation.
- Runs receipt cleanup for a user actor, future cutoff, unbounded batch, or
  without validating all candidates before deletion.
- Gives the CLI an implicit retention default or lets it query receipt tables directly.
- Adds an automatic cleanup scheduler without reviewed deployment cadence and evidence.

## Minimum Contract Set

### Input DTOs/Commands
- Relation commands require a valid tenant, deadline, non-empty idempotency key,
  actor/source ownership, canonical relation kind, target distinct from source,
  and optional optimistic revision.
- Receipt cleanup requires service/system actor, write-port policy, cutoff strictly
  in the past, explicit dry-run mode, and limit from 1 to 1000.
- CLI receipt cleanup additionally requires explicit positive retention days and
  derives the cutoff before delegating to the owner port.
- Relation-event replay requires service/system actor, `event_replay` policy,
  optional exclusive UUID cursor, explicit dry-run mode, and limit from 1 to 1000.
- Index conversion requires non-nil tenant/event ids and one validated sealed relation event.
- Index runtime consumption requires the `index-consumer` feature, an initialized
  Iggy transport, an Index database connection, tenant rows, Index migrations, and
  successful Index-owned persisted schema registration.
- Server execution additionally requires explicit enablement, a worker host,
  `outbox_iggy`, the shared configured EventRuntime Iggy connector, positive bounded
  attempts/backoff, and reviewed DLQ configuration.

### Domain Invariants
- Relation identity is unique by tenant/source/target/kind.
- Revision is positive and monotonic.
- Durable receipt identity is unique by tenant and normalized idempotency key.
- One idempotency key binds one complete relation command identity.
- Receipt reservation, relation mutation, optional event append, response snapshot,
  receipt completion, and commit share one database transaction.
- Cleanup selects completed schema-v1 receipts only, tenant-scoped, strictly before
  cutoff, in `(completed_at, id)` order, and fails the whole batch on one corrupt candidate.
- Historical replay scans authoritative tenant rows in ascending relation UUID order;
  its cursor is exclusive and caller-owned.
- Replay dry-run writes nothing. A live replay page appends all selected events in one
  transaction and rolls the complete page back if one append fails.
- Historical replay starts only after event-aware writers are active, so concurrent new
  relation rows are covered by the live path rather than relying on UUID insertion order.
- Index relation records are non-localized, keyed by relation id, and use relation
  revision as source version. Inactive state is a tombstone, not a second truth source.
- Tenant schema persistence precedes mutation apply. Exact active registration is
  idempotent; conflict, retired, lower-version, or storage failure prevents acknowledgement.
- Index inbox outcomes `Applied`, `Duplicate`, and `StaleIgnored` are terminal durable
  results and may be acknowledged.
- Before a durable result, exhausted/permanent failures may be acknowledged only after
  successful exact-byte DLQ publication under enabled policy. After a durable result,
  only source acknowledgement may be retried.
- Explicitly enabled worker readiness is critical; disabled execution is an intentional
  non-required runtime state rather than degradation.

### Events / Outbox Side Effects
- Root and local manifests declare the Outbox dependency before write composition.
- GraphQL mutations require `TransactionalEventBus` in schema data.
- Profiles native storefront writes require the same bus in `HostRuntimeContext`.
- Read-only owner adapters do not require Outbox composition.
- Replay uses the same sealed event mapper and transactional bus as live relation changes.
- The `rustok-events` digest artifact must be regenerated and reviewed whenever
  the sealed relation event changes the registry or typed wire schemas.
- The Index adapter/projector/consumer change no event schema or digest; they consume
  the existing v1 family.

### Errors / Failure Codes
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
- Index projection additionally preserves typed `SchemaRegistrationError`,
  `SchemaRegistryError`, and `MutationStorageError` internally and maps them to stable
  `social_graph.index.*` host codes without publishing schema JSON or storage causes.
