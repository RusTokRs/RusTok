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
The first binding verifies both persisted TTL and cap before commit. Claim
and refresh verify the complete event transition under the admission fence;
refresh locks and compares the exact source snapshot without synthetic event
updates. A rewritten transition rolls back before dispatch. SQLite regression
sources cover policy and claim rewriting; compilation and runtime checks are pending.
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

A changed bound route is a terminal pre-publication validation failure:
move directly to DLQ and release the exact claim without creating a new
dispatch intent. Retrying cannot change the immutable route; a charged
pending retry without an open intent violates the event-state guard.
Preserve the original checkpoint and attempt history. Reconcile the
deployment route before explicit DLQ replay. Regression source is unrun.

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
a privileged direct terminal DELETE can still bypass the age policy. At this historical migration boundary event IDs
could be reused after lawful pruning. The subsequent permanent event identity
authority below closes that gap for extant and newly admitted UUIDs. No claim
of protection from arbitrary DDL or restored backups.
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

## Exact temporal observation and transport ACK admission (source only)

Independent clock observations reserve the singleton using PostgreSQL row
ownership or a SQLite zero-row writer reservation, without firing a synthetic
row UPDATE. The observed millisecond must be read back exactly before commit;
a suppressed or rewritten advance is an error, not durable clock evidence.
The unclaimed transport ACK now acquires the event row with PostgreSQL
FOR UPDATE NOWAIT (SQLite writer reservation) before sampling SQL time and
compares the full expected event model before commit. Contention is retryable
at the caller; an existing claim or open intent still blocks the ACK. This
prevents an ACK waiting on source ownership from backdating retention age.
Neither path acquires a global clock writer lock while holding event or owner
rows. SQLite fault-injection and PostgreSQL source-contention regressions are
authored, not executed. Independent clock jumps and privileged DDL/restore
remain outside this guarantee.

## Durable shared retry policy (source only)

Forward migration `m20261128_000212_bind_outbox_retry_policy` and standalone
schema setup extend the existing execution-policy singleton with the complete
retry tuple (maximum attempts and base/capped backoff in whole milliseconds).
No second policy lock is introduced. The first updated relay binds the tuple
alongside the TTL; all later startup, claim and refresh admissions require
exact equality. The binding is read back before commit. A mismatch fails
before transport I/O or a new claim. Local polling, batch size, concurrency
and publication timeout remain process controls within the shared admission
bounds; publication timeout parity is not asserted by this tuple.
Ordinary SQL cannot partially bind, reset or change retry settings while any
claim remains, even expired. Drain all old relays and reconcile outstanding
claims before migrating or reconfiguring; old binaries do not enforce the new
tuple. Align every relay's settings after coordinated SQL reconfiguration.
Existing pending retry counts and deadlines are preserved; the new budget
applies to subsequent charged failures. This is a canonical relay contract,
not proof of broker effects or validation of arbitrary direct SQL outcomes.
SQLite/PostgreSQL regression sources cover mismatch and reconfiguration;
compilation, migration and execution remain pending.

## Retention event-row admission (source only)

Delivered and explicitly enabled failed-event pruning select the bounded batch
under the durable retention policy and consumer registry fences, using PostgreSQL
`FOR UPDATE SKIP LOCKED` (SQLite holds its writer reservation). Both paths
share the selection/delete protocol, retain the initial SQL-clock horizon, and
recheck SQL time and eligibility after event-row admission. Busy rows are left
for a later sweep; a short batch does not prove the eligible queue is empty.
This prevents waiting for a busy event row while holding the earlier shared
fences; it does not bound DDL or arbitrary external locks. A PostgreSQL
multi-connection regression covers locked oldest rows in both histories;
compilation and runtime verification remain pending.

## Permanent event identity authority (source only)

Forward migration `m20261128_000213_reserve_outbox_event_identities` and
standalone setup add `outbox_event_identities`, a permanent UUID-only
reservation table without a cascading foreign key. All extant event UUIDs
are backfilled under the event writer fence; previously pruned IDs cannot
be reconstructed. An AFTER INSERT trigger reserves the winning physical
source UUID in the same transaction, with unique-key enforcement and exact
readback. Rollback releases the reservation together with the source write.
There is no global singleton or broker I/O in this path. SQLite normalizes
UUID representations, uses a WITHOUT ROWID primary key and explicitly
rejects duplicate reservations even through INSERT OR REPLACE/IGNORE.
PostgreSQL uniqueness also fences stale-snapshot reinsertions.
Ordinary UPDATE, DELETE and PostgreSQL TRUNCATE of reservations are rejected;
new reservations require their live source, and source deletion requires
its reservation. Retention removes payload, attempts, checkpoint and ACK
history as before but not the UUID. Live exact replay retains its current
contract. After pruning, every attempted publication of that UUID fails
closed (contract write-once returns Unavailable, not successful replay):
there is no payload evidence to classify equality or a different request.
A stale ACK cannot acknowledge a resurrected source. Permanent UUID storage
grows with publication volume; it retains no payload, tenant or actor fields.
Capacity monitoring, legal retention review and independent backup/restore
continuity remain operational requirements. Stop/drain writers for migration;
privileged DDL, prior pruned IDs and restored snapshots are not fenced.
SQLite/PostgreSQL regression sources are authored; compilation and runtime
verification remain pending.
