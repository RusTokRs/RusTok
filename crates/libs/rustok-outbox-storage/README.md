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
