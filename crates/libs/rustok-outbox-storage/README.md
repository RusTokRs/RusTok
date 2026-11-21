# rustok-outbox-storage

Shared SQL storage for Outbox events and owner-operation receipts. Apply
`SysEventsMigration` before accepting writes in an independently provisioned
database. The platform migrator uses the same schema helpers.

Use `TransactionalEventBus` and `idempotency::complete` inside the transaction
that persists the owner mutation. Where a receipt coordinates an independent
aggregate claim, `idempotency::renew_lease` postpones reclamation for
operations that outlive the five-minute processing lease, and
`idempotency::lock_current` verifies and locks the receipt in
that transaction before taking the aggregate lock. This is not a lease renewal
or a fence for external side effects. The caller controls commit and rollback.
Configure Outbox to deliver committed events from that database. See the
[Outbox runtime contract](../../modules/rustok-outbox/README.md).
