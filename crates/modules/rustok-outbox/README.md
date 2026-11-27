# rustok-outbox

## Purpose

`rustok-outbox` owns the canonical outbox transport and relay pipeline for reliable event delivery in RusToK, the bounded retention of delivered events, plus the generic durable receipt primitive used by owner write ports.

## Responsibilities

`rustok-outbox-storage` implements the shared SQL tables, transactional append
and operation receipts. This module re-exports those APIs and owns relay,
retries, retention and operator access. An independently deployed owner can
use the storage library with its own database and compose delivery separately.

- Persist outbound events through the shared outbox transport.
- Relay pending events with claim, dispatch, retry, and DLQ semantics.
- Prune delivered events after a configured retention window, in bounded batches.
- Own the `sys_events` schema (table, claim index, retention index) and related migrations.
- Own the generic `owner_operation_receipts` schema and fenced idempotency
  admission/replay primitive; owner services retain their domain mutations and
  semantic side effects. The owner may persist terminal failures with `fail`.
  For retryable failures it may retain the processing receipt and fingerprint,
  or use `release_retryable` only when deleting that fingerprint is safe after
  rollback (or a commit error). Release requires both a matching lease token and
  `processing` status, so it cannot delete a completed receipt or a reclaimed
  operation. Owners coordinating another aggregate may use `lock_current` at
  the start of their owner transaction before acquiring aggregate locks; this
  verifies the receipt token and holds its row lock, but does not renew the
  lease or fence external side effects. An operation that may run longer
  than the five-minute processing lease must call `idempotency::renew_lease`
  periodically; renewal is fenced by the lease token, so a worker that lost
  its lease fails fast instead of after its side effects escaped. The
  generic receipt primitive acknowledges a consumer event only for a
  registered consumer: `acknowledge_existing` errors on an unregistered
  consumer key instead of silently reporting success.
  Consumer registration and retirement serialize registry mutations before drain
  reads. PostgreSQL retirement takes a non-blocking event-table writer fence;
  SQLite reserves the writer before scanning. Drain requires matching event-type
  receipts. ACK holds registry and source event authority through the owner
  transaction; contention must roll back owner effects and be retried.
  Unfenced transport ACK cannot complete a relay claim.
- Expose the runtime services used by `apps/server` event bootstrap and background delivery.
- Ship the module-owned Leptos admin UI package for relay visibility with a `core/transport/ui` FFA split.

## Entry points

- `OutboxModule`
- `OutboxTransport`
- `TransactionalEventWriter`
- `OutboxRelay`
- `OutboxRetention`
- `migration`
- `idempotency`

## Standalone relay

Run `cargo run -p rustok-outbox --bin rustok-outbox-service --features service`.
Set `RUSTOK_OUTBOX_DATABASE_URL` to the migrated PostgreSQL database where the
owner writes `sys_events`. Set `RUSTOK_OUTBOX_IGGY_CONFIG_PATH` to a mounted JSON
file with `mode` set to `external`, an explicit `external` object, and the
broker topology shared by producers and consumers. See the
[Iggy configuration reference](../../../docs/references/iggy/README.md).

The relay checks storage and the broker before claiming events. It uses the same
claim, retry, and dead-letter rules as the embedded relay. Shutdown lets the
current batch finish. Interrupted claims become eligible again after the claim
lease expires. Delivery is at least once and retains the durable envelope ID.
Relay tuning mirrors the embedded `rustok.events.*` policy through optional
environment variables over the `RelayConfig` defaults:
`RUSTOK_OUTBOX_RELAY_BATCH_SIZE`, `RUSTOK_OUTBOX_RELAY_MAX_ATTEMPTS`,
`RUSTOK_OUTBOX_RELAY_BACKOFF_BASE_MS`, `RUSTOK_OUTBOX_RELAY_BACKOFF_MAX_MS`,
`RUSTOK_OUTBOX_RELAY_MAX_CONCURRENCY`, `RUSTOK_OUTBOX_RELAY_CLAIM_TTL_MS`,
`RUSTOK_OUTBOX_RELAY_PUBLISH_TIMEOUT_MS`, and
`RUSTOK_OUTBOX_RELAY_WORKER_ID`.

## Interactions

- Depends on `rustok-core` for module/event runtime contracts and transport abstractions.
- Depends on default `rustok-api` for neutral port context/error/write-policy primitives.
- Exposes only host-neutral transactional outbox and relay contracts; the
  object-safe `TransactionalEventWriter` lets an owner append an `EventEnvelope`
  through its live SeaORM transaction without depending on the concrete
  transport, while host composition remains outside the crate.
- The append-only platform migration invokes the Outbox-owned receipt schema
  helper. A receipt is scoped by tenant, owner slug, and idempotency key, so
  unrelated owner operations cannot collide.
- The `sys_events` table shape is defined once, by the owner helper
  `create_sys_events_schema`. Module and test schemas reach it through
  `SysEventsMigration`, and the platform wrapper `m20260211_000002_create_sys_events`
  orders it on the platform timeline instead of keeping a second copy of the DDL, so a
  column change lands in both migrators with one edit.
- Used by `apps/server` for runtime relay wiring, background processing, and migrations.
- Integrates with target transports such as `rustok-iggy` instead of owning transport-specific adapters inline.
- The Leptos admin UI lives in `crates/modules/rustok-outbox/admin`, keeps framework-agnostic DTO/view-model helpers in `admin/src/core.rs`, and is mounted through manifest-driven host wiring.

## Tenant projection and integrity

- `sys_events.tenant_id` is a denormalised projection of the envelope
  tenant, populated by every canonical writer and backfilled by the
  platform migration `m20261121_000001_add_sys_events_tenant_projection`.
  Tenant-scoped operator reads (module admin counters, server DLQ API)
  filter on the column through `idx_sys_events_tenant_status_created_at`
  and only fall back to payload extraction for rows whose column is still
  `NULL`. The payload remains the canonical envelope.
- PostgreSQL deployments additionally carry CHECK constraints (added
  `NOT VALID`, so historical drift cannot break the upgrade): a closed
  status set, a non-negative retry counter, `claimed_by`/`claimed_at`
  always set as a pair, and a dispatched row always carrying
  `dispatched_at`. Operators can `VALIDATE CONSTRAINT` after cleaning
  any pre-existing drift.

## Relay policy

- Claims are owned by a relay worker through `claimed_by` / `claimed_at`; PostgreSQL uses `FOR UPDATE SKIP LOCKED`, while SQLite/test runs use the guarded update fallback.
- The claim batch is read through `idx_sys_events_pending_created_at (status, created_at)`, the
  outbox-owned index `create_sys_events_claim_index` creates: the pending queue is read in created-at
  order and the claim-liveness / retry-schedule predicates are evaluated on that same ordered range.
  `idx_sys_events_pending_next_attempt` and `idx_sys_events_claimed_at` predate that index, serve no
  query on their own, and are dropped by the append-only migration
  `m20261007_000016_drop_sys_events_superseded_indexes`.
- `claim_ttl` defines when a stuck claim can be reclaimed by a later iteration.
  Each claim pass fences its rows with a unique `{worker_id}:{uuid}` owner
  value, so two relays sharing a configured `worker_id` cannot complete each
  other's claims.
- Dispatch concurrency is bounded by `RelayConfig.max_concurrency`. A queued
  claim is refreshed by a conditional pending/token/timestamp update before
  dispatch. A reclaimed or terminal row cannot reach the target through the
  stale worker. Renewal and decoding consume the lease budget; publication
  starts only if the timeout fits. At-least-once delivery remains unchanged.
- A single dispatch is bounded by `RelayConfig.publish_timeout`, which must
  stay below `claim_ttl` (validated): a dispatch that outlives its lease can
  be reclaimed and delivered twice while still running.
- Delivery is at-least-once and unordered across concurrent dispatches;
  consumers must be idempotent and tolerant of reordering.
- Permanent dispatch failures (undecodable payload, envelope/row metadata
  mismatch (ID, populated tenant projection, event type or schema version),
  unregistered schema) detected before transport invocation move the event
  straight to `failed` (DLQ) without burning the retry budget. Transport
  failures follow the retry policy below.
- Retry uses capped exponential backoff with equal jitter.
- `max_attempts` is resolved by the server runtime from `rustok.events.dlq.max_attempts` when DLQ is enabled, otherwise from `rustok.events.relay_retry_policy.max_attempts`.
- A retryable failure keeps the event in `pending`, increments `retry_count`, stores `last_error`, clears the claim and sets `next_attempt_at`.
- A terminal failure moves the event to `failed` (DLQ), preserves `last_error`, clears the claim and increments DLQ metrics.

## Delivered-event retention

- The outbox is both the delivery queue and the durable record of what was delivered, so retention
  only removes rows that already reached the transport: `status = 'dispatched'` with a non-null
  `dispatched_at` older than the window. `pending` rows still owe a delivery and `failed` rows are
  the DLQ an operator works through; neither is ever pruned.
- `OutboxRetention::prune_once` selects the oldest `batch_size` delivered rows through
  `idx_sys_events_dispatched_at (status, dispatched_at)`, deletes exactly those ids with the same
  status/age predicates, and reports `OutboxPruneReport { pruned, batch_exhausted }` — a full batch
  means another run has work to do, so the table is trimmed over several bounded runs instead of one
  long delete.
- Dead-lettered rows are kept forever by default. An operator may opt into
  `failed_retention` (server: `events.outbox_retention.failed_retention_days`,
  0 = keep forever): rows whose `created_at` is older than the window are
  then eligible for pruning in the same bounded batches, but only after
  all registered durable consumers have acknowledged them. Enabling it
  accepts discarding an undelivered event with no remaining consumer hold.
- Both delivered and optional failed-event deletion recheck all registered
  consumer obligations. PostgreSQL serializes the final DELETE with registry
  writes by locking `sys_event_consumers` in SHARE mode in a short transaction
  before taking a fresh READ COMMITTED snapshot; SQLite serializes writers.
- Terminal owner-operation receipts are pruned under `receipt_retention`
  (server: `events.outbox_retention.receipt_retention_days`, default 90
  days, 0 = keep forever). A retry that arrives after its receipt was
  pruned runs the operation again; processing receipts are never pruned.
- Runtime wiring: `events.outbox_retention.{enabled,retention_days,batch_size,interval_seconds}`
  (defaults `true`, 30 days, 500, hourly), resolved by `apps/server` into the `outbox_retention`
  worker, guarded by the `worker:outbox_retention` readiness check, and reported through
  `rustok_outbox_pruned_total`, `rustok_outbox_retention_last_run_timestamp_seconds`,
  `rustok_runtime_worker_state{worker="outbox_retention"}` and
  `rustok_runtime_worker_failures_total{worker="outbox_retention"}`.
- `OutboxRetentionConfig::validate` rejects a non-positive window or a zero batch size, and the
  server rejects a window above ten years at boot instead of keeping every delivered row forever.
- The retention index is append-only for deployed schemas
  (`m20261007_000015_add_sys_events_retention_index`) and created by `SysEventsMigration` for
  module/test schemas.

## Docs

- [Public Contract Reference (CRATE_API.md)](./CRATE_API.md)
- [Module docs](./docs/README.md)
- [Platform docs index](../../../docs/index.md)

## Unconditional publication admission

Unconditional root and typed envelope inserts must persist exactly one row; zero-row
suppression is a persistence error, not a successful owner publication. The separate
write-once path still proves replay from the exact stored envelope. This has no schema or
broker protocol change. The [source continuation](docs/implementation-plan.md#unconditional-publication-row-admission-2026-11-01-source-only)
records unrun pure and native stock rollback definitions; consumer delivery remains a
separate, unverified boundary.

## Known Limitations / Pending Implementation

The fresh sealed transaction-only helper and server acknowledged-local target are source-wired
for Inventory reservation invalidations. They reuse canonical `sys_events` and the existing
relay retry/DLQ policy, not a new bus/queue. Under Iggy-first delivery a local failure may resend
the same identity remotely; publication is not persistent remote consumption/ACK. Current scoped
Rust/backend/real relay/Iggy/restart/DLQ evidence remains open. See the
[API](CRATE_API.md#fresh-sealed-facts-and-local-acknowledgement-2026-11-01-source-only) and
[Inventory source packet](../rustok-inventory/contracts/evidence/reservation-events-source.json).

## Database clock authority

Durable relay claims, retry deadlines, storage timestamps and retention cutoffs
use SQL database time. PostgreSQL uses clock_timestamp(), not transaction-start
time; SQLite uses its SQL clock. Receipt renewals and reclaims use a PostgreSQL
write-side clock expression; reclaim compares the previous token and timestamp.
Relay renewal rotates its token even within the same clock tick and counts
database wait time in its monotonic dispatch budget. Clock errors fail closed.
Relay and retention intervals are bounded to 3650 days. This does not fence
primary clock discontinuities or guarantee exactly-once broker effects.

## Relay execution admission

Relay passes reserve at most max_concurrency rows across their sliding window and
advance a stable (created_at, id) cursor within the invocation. Due/reclaim
eligibility uses one initial SQL-clock horizon. A shared pass guard serializes
clones of a relay instance, including aborted tasks until they actually exit.
Independent updated relay instances/processes share a durable schema-local
execution quota via SQL claims. Reports count the joined
durable outcomes of the specific pass, not differences of lifetime counters.
A failed or cancelled pass can have committed earlier outcomes.
Retry uses equal jitter between half the capped exponential delay and the cap
(with a one millisecond floor). Startup validates relay configuration before
connecting broker transports. The sliding window replenishes available slots without waiting for a full wave.

## Distributed execution admission (2026-11-27)

The outbox_execution_policy singleton owns a per-database-schema budget
(max_in_flight, default 8, range 1..1024) and the shared claim_ttl_ms.
The first updated relay binds the TTL; subsequent embedded and standalone
relays must match it before starting their brokers or claiming work.
The local max_concurrency remains an upper bound, independent of the cluster
budget. Existing sys_events claimed_by/claimed_at pairs are the only durable
execution permits. Claim and refresh lock the policy before event rows;
PostgreSQL uses READ COMMITTED, SKIP LOCKED for candidates and NOWAIT for
refresh. The policy lock is released before any broker effect. Completed
claims release capacity through existing conditional state transitions.
Cancelled or crashed publications may retain reservations until expiry;
transport effects remain at-least-once and may outlive cancellation.
A sliding window replenishes a slot after a joined outcome rather than waiting
for a whole wave. Claim failures stop new admission and drain active tasks.
The pass cursor and initial eligibility horizon are preserved.

Deployment: stop and drain all old relay processes before applying
m20261127_000201_create_outbox_execution_policy. Old binaries bypass the
budget. Ensure no old in-flight publications remain before starting new
relays. Apply the migration before starting new workers; the schema helper
does not reset an existing policy. For a deliberate policy change, stop and
drain every relay, verify there are no live or ambiguous claims, then change
the singleton max_in_flight and claim_ttl_ms through controlled database
maintenance and align all runtime TTL settings before restart. No live
policy editing API is provided; online cap reduction or TTL change is not
supported. Multiple databases/schemas have independent budgets. Direct SQL
claim mutations outside the protocol, primary clock anomalies, uncooperative
transport cancellation and broker-side fencing remain outside this guarantee.

## Publication identity and acknowledgement boundary (2026-11-27)

Canonical root and sealed event UUIDs now populate the Iggy message header
ID without translation. Non-UUID publication keys, including the existing
DLQ replay attempt keys, map to a fixed namespaced UUIDv5; nil UUIDs and
empty keys are rejected. The namespace and the existing partition routing
must remain stable across retries. Explicit direct-mode publication awaits
the broker reply rather than accepting background enqueue as delivery.
No-SDK external connections and publications fail closed. The dedicated
poison-DLQ path retains its existing identity and partition mapping.
The strength of the broker response still depends on topic/server durability
and the reviewed broker deduplication policy. Successful direct send is not
a physical persistence proof or consumer cursor acknowledgement. An empty
confirmation list can be legitimate (including a duplicate response); no
broker offsets or persistence guarantees are inferred. Older publications
without the header identity cannot be retroactively deduplicated. Changing
topology, partition count or broker destination requires coordinated drain
and recovery. Do not treat SDK producer idempotence as cross-process Outbox
exactly-once delivery.

The relay sends invalid stored envelopes directly to its DLQ before invoking
the target. Every target error, including validation/serialization after a
remote send or a local projection, consumes the bounded ordinary retry
budget instead of assuming there was no side effect. Timeout outcomes remain
unconfirmed and may redeliver the same identity. A successful transport
reply followed by SQL failure or process death may also redeliver; the
existing fenced SQL acknowledgement and consumer idempotency still apply.
No transaction spans the database and the broker.
