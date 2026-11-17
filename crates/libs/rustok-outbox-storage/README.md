# rustok-outbox-storage

Shared SQL storage for Outbox events and owner-operation receipts. Apply
`SysEventsMigration` before accepting writes in an independently provisioned
database. The platform migrator uses the same schema helpers.

Use `TransactionalEventBus` and `idempotency::complete` inside the transaction
that persists the owner mutation. The caller controls commit and rollback.
Configure Outbox to deliver committed events from that database. See the
[Outbox runtime contract](../../modules/rustok-outbox/README.md).
