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
