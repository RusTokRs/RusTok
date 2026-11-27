# rustok-outbox-storage

Shared SQL storage for Outbox events and owner-operation receipts. Apply
`SysEventsMigration` before accepting writes in an independently provisioned
database. The platform migrator uses the same schema helpers.

Use `TransactionalEventBus` and `idempotency::complete` inside the transaction
that persists the owner mutation. The transport-backed `publish_*_in_tx` methods
require the caller's `DatabaseTransaction`; the bus delegates through the
transport's transactional capability, including when a wrapper forwards it.
Where a receipt coordinates an independent
aggregate claim, `idempotency::renew_lease` postpones reclamation for
operations that outlive the five-minute processing lease, and
`idempotency::lock_current` verifies and locks the receipt in
that transaction before taking the aggregate lock. This is not a lease renewal
or a fence for external side effects. The caller controls commit and rollback.
Configure Outbox to deliver committed events from that database. See the
[Outbox runtime contract](../../modules/rustok-outbox/README.md).

Replay verifies the stored row ID and any populated tenant projection against
the decoded envelope. Historical null projections remain permitted.

Consumer registration and retirement serialize registry mutations before drain
reads. PostgreSQL retirement takes a non-blocking event-table writer fence;
SQLite reserves the writer before scanning. Drain requires matching event-type
receipts. ACK holds registry and source event authority through the owner
transaction; contention must roll back owner effects and be retried.
Unfenced transport ACK cannot complete a relay claim.

## Database clock authority

Durable relay claims, retry deadlines, storage timestamps and retention cutoffs
use SQL database time. PostgreSQL uses clock_timestamp(), not transaction-start
time; SQLite uses its SQL clock. Receipt renewals and reclaims use a PostgreSQL
write-side clock expression; reclaim compares the previous token and timestamp.
Relay renewal rotates its token even within the same clock tick and counts
database wait time in its monotonic dispatch budget. Clock errors fail closed.
Relay and retention intervals are bounded to 3650 days. This does not fence
primary clock discontinuities or guarantee exactly-once broker effects.

## Strict owner-operation lease expiry

Owner-operation receipts now enforce a strict five-minute half-open live
interval. A matching token cannot renew, lock, finalize or release an expired
receipt. Sample SQL time after acquiring the receipt write lock, recheck
expiry on reclaim and restamp fresh admission after unique-index contention.
Completion and failure use a transaction or an owner-transaction savepoint;
owner errors require rollback of the entire owner transaction. Expired release
preserves the request fingerprint for exact-request reclamation. Owners must
renew before expiry and keep owner transactions bounded; a row lock alone
does not extend the lease. External effects need owner-specific idempotency.

## Durable temporal clock authority

The outbox_temporal_clock singleton stores the maximum committed SQL-clock
observation across process restarts. Independent relay-pass, receipt admission,
renewal/reclaim and retention boundaries advance it in a short transaction.
Owner transactions read the floor and SQL time without taking the global writer
lock. Missing schema, backward observations below the committed floor, slow
observations (>2s), host-clock steps (>100ms during observation) or SQL/host
skew outside a 2s sanity bound fail closed; host time is not a timestamp
fallback. Relay claim refresh checks its live interval after acquiring row
ownership. Event retention checks time again after the registry fence.

This is bounded discontinuity detection, not a trusted independent distributed
time service: simultaneous host/DB forward jumps, smaller jumps within the
sanity bound and a stale repeatable-read snapshot are not fully fenced.
A floor ahead of actual time blocks affected operations until time catches up
or an explicitly reviewed operator recovery is performed. Do not reset it on
startup. The floor begins at zero on migration; pre-upgrade history is not
retroactively verified. Upgrade the schema before new workers and drain old
workers that bypass the guard. A committed owner transaction may still be
subject to cancellation and ambiguous broker effects.

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

## Transaction-bound consumer effect admission (source only)

Notifications intake serializes terminal acceptance and rejection on the
canonical source row before reading either owner outcome. PostgreSQL uses
READ COMMITTED and FOR UPDATE NOWAIT; SQLite reserves its writer before
reading. Decode uses the locked source snapshot. Acceptance writes run in
a savepoint, rolled back before a permanent rejection is persisted; retryable
errors abort the whole transaction. An existing terminal outcome is replayed
without another owner write. A legacy contradictory accepted/rejected pair
fails closed and requires reviewed reconciliation. The terminal owner
outcome, source inbox and generic retention ACK commit together. No broker
or external I/O is allowed in this transaction.

OutboxConsumer::reserve_effect_in_transaction exposes the existing exact
consumer/event receipt insertion result as a transaction-local admission:
true means a new receipt, false means an exact prior committed receipt
(or one inserted earlier in the same transaction). Perform the SQL owner
effect only on true and commit receipt and effect together; roll back on
any error. It is not a reservation that can be committed separately and
does not cover external effects, broker replay after event retention, or
unregistered consumers. Existing late ACK/backfill semantics remain intact.
This does not automatically make arbitrary module handlers idempotent.
SQLite and PostgreSQL regression sources are authored, not executed.
