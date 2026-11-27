# Documentation `rustok-outbox`

`rustok-outbox` is a core module for transactional event persistence and relay
infrastructure for the platform event runtime.

## Purpose

- publish the canonical runtime entry type `OutboxModule`;
- keep the write-side outbox contract and relay semantics outside the host layer;
- provide the platform with a unified transactional publishing contract for events.

## Scope

- `TransactionalEventBus` and atomic publish-with-transaction semantics;
- object-safe `TransactionalEventWriter` injection for owner services that
  already own a `DatabaseTransaction`;
- persistence into `sys_events` through transactional transport; unconditional root/typed
  inserts require exactly one affected row, while the separate write-once path correlates
  exact saved-envelope identity/payload on conflict;
- `idempotency` admission, fenced lease reclaim, terminal replay, and typed
  failure persistence in `owner_operation_receipts`; owner services still own
  business validation, mutation, and side-effect evidence;
- rejection of root and typed-family envelopes whose metadata, registered schema,
  or semantic payload is invalid before persistence and relay;
- relay, retry and DLQ semantics for the event runtime;
- bounded retention of delivered events (`OutboxRetention`), configured at the
  server boundary (`events.outbox_retention.*`) and supervised as the
  `outbox_retention` runtime worker;
- generic durable-consumer registrations and per-event acknowledgements; retention
  holds a dispatched event until every registered consumer for its event type has
  acknowledged the original event ID, without querying consumer-owned receipt tables;
- module-owned Leptos admin package `rustok-outbox-admin` with FFA split `core/transport/ui` for read-only relay visibility.

## Relay, retry and DLQ policy

- Claim ownership is stored in `sys_events.claimed_by` and `sys_events.claimed_at`.
- PostgreSQL claim uses `FOR UPDATE SKIP LOCKED`; SQLite/test environment uses guarded update fallback.
- `claim_ttl` determines when a stuck claim can be reclaimed. Before dispatch,
  refresh the exact pending claim and check that the publish timeout fits
  the remaining lease budget. Reclaimed rows cannot start publication from
  a stale snapshot.
- `RelayConfig.max_concurrency` limits parallel dispatch.
- Retry uses capped exponential backoff with equal jitter.
- Runtime `apps/server` selects `max_attempts` from `rustok.events.dlq.max_attempts` if DLQ is enabled, otherwise from `rustok.events.relay_retry_policy.max_attempts`.
- A transient error leaves the event in `pending`, increments `retry_count`, writes `last_error`, clears the claim and sets `next_attempt_at`.
- A terminal error moves the event to `failed`/DLQ, saves `last_error`, clears the claim and is reflected in metrics/admin DLQ surface.

## Durable consumer retention contract

Consumers register their required event types through `OutboxConsumer` before
the retention worker starts. A dispatched event is retained while any registered
consumer lacks an acknowledgement for that event ID. Multiple consumers are
independent obligations; all must acknowledge before age-based pruning applies.
The optional failed-event retention window obeys the same holds. PostgreSQL
takes a SHARE lock on the registry before the final DELETE at READ COMMITTED,
so concurrent registration either precedes that snapshot or follows deletion.
SQLite serializes writes. Registration cannot recover already-pruned events.

Consumers write acknowledgements only after their own terminal receipt commits.
When both records share a database, the neutral owner port writes the Outbox
acknowledgement in the same transaction. Retryable failures do not acknowledge.
Registration persists across worker shutdown and feature-flag disablement so an
outage cannot silently release retained events. A consumer must be drained before
its registration is explicitly retired. `OutboxConsumer::retire_if_drained` refuses
retirement while any existing event remains unacknowledged; callers must stop the
worker and quiesce producers of the registered event types first.

Consumer registration and retirement serialize registry mutations before drain
reads. PostgreSQL retirement takes a non-blocking event-table writer fence;
SQLite reserves the writer before scanning. Drain requires matching event-type
receipts. ACK holds registry and source event authority through the owner
transaction: PostgreSQL uses NOWAIT exclusive source-event and NOWAIT shared
subscription locks, serializing concurrent ACKs for one event. Contention
must roll back owner effects and be retried. Missing-event startup backfill
requires an actively registered consumer. Event and receipt deletion batches
are capped at 10000 candidates, and direct receipt pruning observes the same
3650-day retention limit.
Unfenced transport ACK cannot complete a relay claim.

## Tenant and authorization boundary

- Tenant-facing DLQ list and native dashboard reads always derive tenant scope from the trusted request context. They do not accept a client-selected tenant identifier.
- DLQ inspection requires `logs:read`; replay/requeue is a state-changing operator action and requires `logs:manage`.
- Replay looks up the event by both event id and trusted tenant scope. Missing and cross-tenant targets fail closed as not found.
- Native pending, dispatched, failed and retry counters require tenant context. They prefer the canonical top-level `tenant_id`; the nested legacy event tenant is considered only when that key is absent, never combined with it using `OR`.
- A platform-global operational view is not exposed through tenant admin transport. Platform on-call investigation uses owner metrics, health and controlled database/operator procedures.

## Incident response

Primary owner for outbox/event delivery is the Platform foundation on-call. Escalation path: owner of `crates/modules/rustok-outbox`, then owner of server runtime composition.

When backlog, retry or DLQ grows:

1. Check `/health/ready` and metrics `rustok_outbox_backlog_size`, `rustok_outbox_pending_lag_seconds`, `rustok_outbox_retries_total`, `rustok_outbox_dlq_total`.
2. Check worker state `worker:outbox_relay` and `rustok_runtime_worker_state{worker="outbox_relay"}`.
3. For stuck claims, verify `claim_ttl`, `claimed_by`, `claimed_at` and wait for reclaim or execute the standard operator procedure replay/requeue.
4. For DLQ, do not edit payload manually: first classify the error, confirm consumer idempotency and only then trigger redelivery.
5. After rollback or requeue, save evidence: affected event ids, retry counts, DLQ count, health snapshot and the final downstream consumer status.

## Integration

- used by `apps/server` for migrations, runtime relay bootstrap and event transport wiring;
- depends on `rustok-core` for module contracts and event transport abstractions, and on `rustok-api` for shared `PortContext`/`PortError` and write-policy primitives;
- exposes host-neutral relay and transactional event contracts; `OutboxTransport`
  implements `TransactionalEventWriter`, and the host or owner facade supplies
  that port without requiring domain services to construct the adapter;
- owns the generic owner-operation receipt schema used by write-port owners;
  its append-only production migration and standalone migration share one
  schema helper;
- can forward delivery to downstream transports like `rustok-iggy`, without owning provider-specific delivery semantics;
- remains a `Core` module regardless of the fact that part of the bootstrap wiring lives in the host runtime.
- module-level `health()` returns `Degraded` when host runtime evidence is unavailable; specific checks are at `/health/ready`.

## Verification

- `cargo xtask module validate outbox`
- `cargo xtask module test outbox`
- `node scripts/verify/verify-outbox-admin-boundary.mjs`
- `node scripts/verify/verify-outbox-admin-boundary.test.mjs`
- `node scripts/verify/verify-outbox-dlq-tenant-rbac.mjs`
- `npm run verify:outbox:fba`
- targeted event-runtime tests for transactional publish, relay and backlog semantics

### Reliability evidence

Transactional publish and relay failure modes are covered by targeted regression tests:

- `cargo test -p rustok-outbox --test transactional_events_integration_test`:
  - `test_transactional_event_publishing_rollback` confirms that a transaction rollback does not leave `sys_events`;
  - `test_transactional_event_publishing_commit` confirms that a commit creates one durable envelope in `Pending`;
  - `test_transactional_publish_rejects_non_transactional_transport` confirms fail-fast on incompatible transport in the default profile.
- `cargo test -p rustok-outbox --test integration`:
  - `relay_retries_then_succeeds` covers transient error and redelivery;
  - `relay_moves_to_dlq_on_max_retry` covers terminal state/DLQ;
  - `relay_reclaims_stale_claims` covers reclaiming stuck claims;
  - `relay_bounds_parallel_dispatch` covers bounded concurrency;
  - `relay_processes_baseline_batch_with_bounded_latency` establishes a baseline for batch latency.
- `cargo test -p rustok-outbox-storage --lib idempotency::tests::completed_receipts_replay_and_keys_remain_owner_scoped`:
  - proves terminal replay and tenant/owner receipt namespace isolation.

These tests cover transactional rollback/commit, relay retry/reclaim/DLQ, and
generic receipt semantics. Each owner must still prove that its business
mutation, owner evidence, and receipt completion commit atomically; Outbox is
not responsible for a recipient's domain side effects.

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Contract write-once and causation boundary](./contract-write-once-causation.md)
- [Manifest layer contract](../../../../docs/modules/manifest.md)

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
