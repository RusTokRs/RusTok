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
  or use `release_retryable` after rollback (or a commit error). Release revokes
  the live lease without deleting the fingerprint, permitting only an exact
  request to re-admit the operation. Completed or reclaimed receipts cannot
  be released. Owners coordinating another aggregate may use `lock_current` at
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

## Consumer ACK admission and retention bounds (source only)

PostgreSQL consumer ACK reserves the source event row exclusively with NOWAIT
and reads its subscription with NOWAIT under the registry SHARE fence. This
serializes canonical ACK writers for one event without waiting while holding
owner locks. Contention requires rollback and retry of the owner transaction.
Trusted backfill may skip a missing event only for a currently registered
consumer. Live ACK still requires an existing event and exact event type.
Event and terminal-receipt pruning accept batches of 1..=10000; direct receipt
pruning also enforces the positive, at-most-3650-day window before SQL.
Source regressions are unrun; PostgreSQL multi-connection contention,
compilation and runtime verification remain open.

## Durable execution policy integrity (source only)

The append-only migration `m20261127_000202_fence_outbox_execution_policy`
and standalone owner schema install the same database guards. The singleton
cannot be inserted, deleted, replaced, truncated (PostgreSQL), change identity
or reset a bound TTL to NULL through ordinary SQL. Exact no-op reservation
updates remain valid. First TTL binding and later cap/TTL changes require
**all** claims to be absent, including expired or ambiguous claims.
PostgreSQL event claim writes take the existing policy row NOWAIT and write
it before checking live capacity; canonical relay admissions already hold
that row. The write fences stale repeatable-read admissions. SQLite serializes
writers and validates integer policy values and claim timestamp shape. Both
backends reject unbound policies, malformed claim pairs and acquisitions that
would exceed the live claim cap. Expired claims may exist for normal reclaim.
There is no second reservation table.

Stop and drain all relays, reconcile ambiguous external publications and
clear any remaining claims through reviewed recovery before this migration or
a policy change. The migration rejects missing/corrupt singleton state and
any outstanding claims; it never silently resets a policy. Apply it before
restarting workers. Offline downgrade also requires drained workers.
The guards enforce ordinary DML quota and policy integrity, not database
superuser/DDL authority, the identity of an arbitrary SQL writer that can
clear or replace a token, clock discontinuities, or fencing at a remote broker.
The PostgreSQL per-claim policy write and SQLite timestamp conversion in the
quota guard need workload measurement. Compilation, SQL migration lifecycle,
multi-connection contention and broker behavior remain unverified.

## Durable temporal authority integrity (source only)

Append-only migration `m20261127_000203_fence_outbox_temporal_clock`
and standalone schema guard the permanent clock singleton. Ordinary INSERT,
DELETE, replacement, PostgreSQL TRUNCATE, identity changes, fractional
SQLite values, decreasing floors and advances past SQL time are rejected.
Exact no-op reservations remain valid. Existing missing singletons are
not reseeded; migration validates the baseline under a writer fence.
A preexisting future floor is preserved and observations fail closed.

The guards protect the durable floor, not independent distributed time,
privileged DDL/restore or simultaneous primary/host clock jumps. Owner
transactions still do not take a shared clock writer lock. Drain old
binaries and use reviewed offline recovery; never reset at startup.
Both integrity installers now open a transaction/savepoint even for
direct standalone setup. PostgreSQL duplicate DDL errors are handled
within SQL, not by continuing an aborted transaction. SQLite and
PostgreSQL regression sources are authored; compilation, migration
lifecycle and contention evidence remain pending.

## Crash-safe dispatch attempt ledger (source only)

Before invoking a transport, the relay persists an `outbox_dispatch_attempts`
intent in a short transaction under the exact pending event claim. The attempt
UUID is local diagnostic identity; the original event UUID remains the stable
broker publication identity. At most one open attempt exists per event.
The ledger records `open`, `confirmed`, `uncertain` and `not_started`.
A transport error or timeout is uncertain, not proof of non-delivery.
Completion and event ACK/retry/DLQ are atomic. A lost claim cannot finalize
its successor. If the process dies after the intent commit, the next relay
claim closes that open attempt as uncertain, charges one retry failure and
schedules ordinary backoff or DLQ before another publish. Recovery itself is
not another external effect. A proven pre-call lease-budget failure closes the
intent as not_started and releases the claim without charging retry budget.
Invalid envelopes still go directly to DLQ without a transport intent.

Migration `m20261127_000204_create_outbox_dispatch_attempts` adds the owner
schema with event FK cascade and an indexed single-open-attempt invariant.
Retained event history includes its attempts; event retention cascades them.
DLQ replay resets the event retry budget, not the historical attempt rows.
This is conservative at-least-once: a crash after intent but before invocation
is indistinguishable from a crash after broker acceptance. The journal does
not establish exactly-once, broker acknowledgement durability, per-consumer
success, or protection against arbitrary privileged DML. Stop/drain old relays
before migrating and do not mix old writers with ledger-aware relays.
Regression sources are authored; compilation, SQL lifecycle and crash/remote
effect evidence remain pending.

An unclaimed transport ACK and both retention paths now also reject events with
an open dispatch intent, even if an external SQL mutation cleared the claim or
changed status. Closed attempt history still follows event retention. A known
pre-intent budget exhaustion releases its exact claim without a retry charge.
PostgreSQL/SQLite integration regression sources check outcome history,
rollback on a lost journal row, and retention of unresolved evidence; unrun.

## Durable fanout recovery (source only)

The relay pins a versioned delivery route per event before invoking any target
or wrapper. An incompatible route refuses delivery before external I/O; review
and drain pending/DLQ events before changing profiles or broker routing.
The server external-Iggy fanout records primary acceptance in a separate
short SQL transaction under the exact live claim and open dispatch intent.
On retry, a committed checkpoint skips the primary call and retries local
delivery. A crash between broker acceptance and checkpoint commit is still
ambiguous and can redeliver. The original event UUID remains the broker ID.
The same route also pins whether local delivery was required when fanout
first reached its validated registry. A mismatch fails closed.

Local handlers, cache rotations and other process-local effects are not
skipped on the basis of another host's success. Bundled Iggy is deliberately
not eligible for primary skipping because the configured local path is not
proof of shared durable broker identity. External configuration fingerprint
covers credential-free endpoint routing, protocol/TLS domain, stream and
serialization, not remote broker identity or retention. Replacing a broker
behind the same endpoints requires reviewed recovery; never reuse a route
checkpoint against a different broker. The standalone broker-only relay has
a distinct route from the server fanout; do not run different route owners
against the same pending queue. Generic library targets use an unclassified
route unless the host explicitly supplies a stable one.

The route and checkpoint share the event retention lifecycle, including DLQ
replay; replay does not silently discard confirmed evidence. Old relay
binaries must be stopped/drained before migration
`m20261127_000205_create_outbox_delivery_checkpoints`. No SQL transaction
spans broker I/O. This is not an exactly-once guarantee, a complete
per-handler receipt system, or a proof that the broker retained an accepted
message. Source regression tests are authored, not executed.

A changed bound route is a terminal pre-publication validation failure:
move directly to DLQ and release the exact claim without creating a new
dispatch intent. Retrying cannot change the immutable route; a charged
pending retry without an open intent violates the event-state guard.
Preserve the original checkpoint and attempt history. Reconcile the
deployment route before explicit DLQ replay. Regression source is unrun.
Checkpoint-enabled primary publication refuses without its relay task scope;
the test-only uncheckpointed transport retains ordinary direct behavior.
The server regression exercises local failure, relay reconstruction and
confirmed-primary skipping with a transport double (not a live broker).

Relay route configuration is validated before acquiring any event claim or
entering the polling loop. A wrapper cannot disable checkpointing after a
primary confirmation has been persisted for that event; the relay rejects
that invocation before primary I/O, preserving the existing checkpoint.
Focused source regressions cover both paths; execution remains unverified.

## Exact durable dispatch admission (2026-11-27; source only)

Claim batches must write every selected locked candidate and read back exactly
that many claimed rows before commit. A suppressed write aborts the whole
batch. Claim renewal returns the actual row read under the new claim instead
of patching a queued snapshot in memory. Before any external publication,
route checkpoint and open attempt creation must each insert exactly one row
and match the readback identity and initial state in the same transaction.
Suppressed or altered inserts abort admission and roll back any newly created
checkpoint/intent; the pending claim remains reclaimable under its lease.
No retry is charged for this storage-admission error. SQLite source regressions
cover suppressed claim, checkpoint and attempt writes and fresh retry metadata.
This does not make arbitrary later SQL mutation safe or guarantee exactly-once.
Compilation and regression execution remain pending.

## Database-enforced delivery history (2026-11-28; source only)

Migration `m20261128_000206_fence_outbox_delivery_history` and standalone schema setup use the same
transactional installer. Attempt identity is immutable; inserts require a pending
matching claim and route, and an open attempt may close only once with an event
state consistent with its outcome. The event transition and closure still commit
in one owner transaction. A route starts unconfirmed, binds topology once and
confirms primary once under a matching open claim. PostgreSQL child writers
reserve the parent NOWAIT rather than wait in reverse lock order. Direct child
deletion while its event exists, closed-attempt reopening, checkpoint reset,
SQLite child REPLACE (including rowid collisions), and PostgreSQL TRUNCATE are
rejected. Open attempts also prevent direct parent deletion; ordinary terminal
event retention still cascades closed history. Existing history is preserved
without inventing checkpoint evidence for pre-checkpoint attempts.
These guards enforce structural transitions, not the truth of a broker ACK:
SQL with sufficient privileges can still manufacture a valid-looking transition.
Parent identity/SQLite parent REPLACE, arbitrary privileged DDL, restored backups,
and proof of retention eligibility against direct parent DELETE are separate
boundaries. Stop/drain older relay binaries before deployment. SQLite and
PostgreSQL regression sources are authored, not executed.

The idempotent transport ACK fallback now requires an unclaimed dispatched
row without any open attempt in one read snapshot. A contradictory external
status-only update cannot report successful ACK or permit retention while
unresolved evidence exists. A SQLite source regression covers this anomaly
and explicit closure/retention; execution remains pending.

## Exact dispatch outcome integrity (source only)

Relay writer fences verify the complete event snapshot. ACK, retry, DLQ and
pre-publication release read back the expected event transition before commit;
closing an attempt requires its exact claim token and verifies its terminal
record. Recovery explicitly uses the abandoned intent's token, not the new
claim's token. Fanout topology and primary confirmation also require exact
checkpoint readback. Rewritten outcomes abort the SQL transaction instead of
reporting success or silently losing retry state. No broker I/O is added to
these transactions; ambiguous broker effects still require recovery.
SQLite regression sources exercise rewritten event and attempt ACK outcomes
independently of the history guard; compilation and execution remain pending.

## Durable retryable receipt release (source only)

A retryable release changes `processing` to `released` under the exact live
lease, rotates its token and retains the original operation UUID and request
fingerprint. Only the identical request can immediately re-admit a released
receipt; a different operation or payload conflicts. Re-admission locks the
prior state and token, compares the stored snapshot, rotates the token again
and verifies the resulting row before commit. Release verifies its readback
as well. Released receipts are not terminal and are excluded from retention;
abandoned keys therefore remain reserved until the exact command completes
or an explicitly reviewed recovery is performed. This changes no schema
columns; the status is stored in the existing string column. Stop/drain old
receipt writers before rollout: older versions delete on release and do not
recognize `released`. Previously deleted fingerprints cannot be reconstructed.
SQLite source regression is extended; compilation and PostgreSQL concurrent
admission evidence remain pending.

## Immutable source event envelope (2026-11-28; source only)

Migration `m20261128_000207_fence_outbox_event_envelope` shares its installer with standalone schema setup.
The original event id, tenant projection, event type, schema version, payload
and creation timestamp cannot change through ordinary UPDATE. Retry, ACK and
DLQ replay retain that identity. SQLite forbids rowid mutation and different
physical UUID aliases; an exact-key duplicate INSERT (including REPLACE) is
suppressed without changing the original row or cascading its history. This
preserves the existing write-once readback/conflict protocol and unconditional
inserts still require one affected row. The unique normalized UUID index rejects
preexisting alias collisions at migration rather than discarding history.
PostgreSQL legacy tenant backfill considers nested tenant only if the top-level
key is absent, leaves nil tenant unprojected and does not rerun after the guard.
This is not retrospective repair of previously incorrect projections.

Parent deletion now requires a terminal, unclaimed event and all registered
consumer ACKs. PostgreSQL requires READ COMMITTED and takes the registry SHARE
fence NOWAIT in a direct delete, avoiding a blocking reverse lock order;
SQLite serializes writes. Existing open-attempt protection remains in force.
Retention queries exclude claimed rows. Configured retention age and failed
retention enablement remain enforced by the application, not by these SQL guards;
a privileged direct terminal DELETE can still bypass the age policy. Event IDs
can be reused after lawful pruning; permanent deduplication/tombstones are not
introduced. No claim of protection from arbitrary DDL or restored backups.
Drain writers before migration. PostgreSQL/SQLite regressions are authored;
compilation, migration and contention execution remain pending.

## Durable event retention policy (2026-11-28; source only)

Migration `m20261128_000208_create_outbox_event_retention_policy` and standalone schema share a singleton
`outbox_event_retention_policy`. A database starts unbound: event deletion
fails closed until the first valid pruner binds the delivered and optional DLQ
windows. A NULL failed window means keep DLQ indefinitely. Every later pruner
must match both windows (whole milliseconds, 1..=3650 days), including on an
empty queue. Local batch size and cadence remain independent. Before every
bounded event delete, the pruner rechecks the durable values under the policy
writer lock, then takes the consumer registry fence and event locks. The SQL
DELETE trigger independently enforces the enabled window, event age and SQL
clock floor. PostgreSQL direct DELETE takes the policy row SHARE NOWAIT and
requires READ COMMITTED; SQLite uses its serialized writer. Previous consumer,
claim and open-attempt holds remain mandatory. The singleton cannot be deleted,
replaced, truncated or reset to unbound, and schema setup never reseeds it.

For coordinated reconfiguration, stop/drain all retention workers, review the
new deletion horizon and update both windows in a controlled SQL transaction
on the existing singleton, align runtime settings and restart. An old worker
with mismatched settings fails closed rather than overwriting policy. Direct
SQL with policy-write authority can still change windows; this is not a
privilege boundary. Stop old binaries before migration. Owner-operation
receipt retention remains a separate, not yet durably bound contract. Event
status and dispatched_at mutation, privileged DDL/restore, and independent
clock discontinuities are separate remaining boundaries. SQLite/PostgreSQL
regression sources added; compilation and runtime evidence remain pending.

## Exact initial event persistence (source only)

Fresh root and sealed-contract inserts require one affected row and exact
readback of the complete initial event model, including pending status, zero
retry count, absent claim and canonical payload. Write-once contract inserts
check the fresh-write path separately from an existing identity replay.
Standalone root publication holds one SQL transaction through insert/readback
and commit. Owner transaction append errors require rollback of the owner
transaction; no external I/O occurs in these checks. A rewritten insert cannot
be acknowledged as a successful fresh event. Existing duplicate replay still
compares canonical facts rather than regenerated timestamps or trace metadata.
SQLite regression source covers rewritten standalone and owner-transaction
inserts; compilation and execution remain pending.
## Database-enforced event state transitions (2026-11-28; source only)

Forward migration `m20261128_000209_fence_outbox_event_state` installs the
same event-state guard in PostgreSQL and SQLite. New rows must enter as pristine
`pending` events. While pending, only a monotonic claim acquire/rotate/release,
a retry charged against one open dispatch intent, a terminal validation or
dispatch failure, or a valid delivery transition is accepted. Delivery requires
the exact current open attempt, or the legacy unfenced ACK path on an unclaimed
row with no open attempt. `dispatched_at` cannot be rewritten after delivery.
The only terminal-to-pending edge is DLQ replay with closed attempt history,
zero retry count, cleared retry metadata and no live claim.

Retry deadlines are bounded relative to the claim time; the retry counter fails
closed at its integer limit. The trigger reads attempt history without taking
child-row locks, preserving parent-event-before-attempt lock order. Claim
timestamps cannot be backdated or renewed with the same token. ACK and admin
DLQ replay now retain their SQL transaction through exact readback; concurrent
replay has a single conditional-update winner.

The guard enforces the legal state graph, not caller identity or broker truth:
a database principal able to issue a structurally valid transition can still
forge a plausible outcome. Arbitrary privileged DDL, restored backups, and
external broker ambiguity remain separate trust boundaries. Drain old writers
before migration. PostgreSQL/SQLite regression sources were updated or added;
compilation and execution remain pending.

## Durable consumer registry and ACK integrity (2026-11-28; source only)

Migration `m20261128_000210_fence_outbox_consumer_receipts` and standalone
schema setup share the same PostgreSQL and SQLite installer. Registry rows
are immutable; deletion requires a drained event type and matching receipts
for every extant source event. Consumer receipts require the exact source
event and active subscription on insertion, cannot be updated, and can be
deleted only after their parent is gone. PostgreSQL TRUNCATE is rejected.
SQLite exact-key duplicate INSERT/REPLACE preserves the original fact;
conflicting rowid replacement is rejected even with recursive triggers off.
Migration validates existing identity and source linkage and fails closed on
inconsistent history; previously retired subscriptions need not be recreated.

Canonical registration and fresh ACK require one affected row and exact
readback under the existing registry/source locks. An exact prior ACK is
recognized before INSERT; a suppressed new INSERT cannot masquerade as a
prior effect. Retirement also verifies the affected count. PostgreSQL direct
receipt insertion uses registry SHARE NOWAIT, source FOR UPDATE NOWAIT and
subscription FOR SHARE NOWAIT; registry retirement requires READ COMMITTED
and the event-table SHARE NOWAIT fence. SQLite serializes writers.
Stop/drain writers before migration and reconcile any rejected legacy data
offline; no automatic rewriting or deletion of ACK history.

A structurally correct ACK remains a trusted assertion by the owner. A role
with arbitrary INSERT access can fabricate a matching ACK without performing
the effect; generic Outbox cannot inspect another owner's private tables.
The caller must commit the owner effect and ACK together or roll back both.
External effects, credential isolation, event identity after retention and
consumer recovery remain separate boundaries. SQLite/PostgreSQL regression
sources are authored; compilation, migrations and contention tests are unrun.
