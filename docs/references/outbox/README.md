---
id: doc://docs/references/outbox/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# Outbox Reference Package (RusToK)

Last updated: **2026-10-07**.

> This package captures the correct transactional outbox flow (`rustok-outbox`) and prevents incorrect patterns from "simple publish after commit".

## 1) Minimal working example: transactional publish

```rust
use rustok_outbox::TransactionalEventBus;

let bus = TransactionalEventBus::new(transport);

let txn = db.begin().await?;
// ... domain changes
bus.publish_in_tx(&txn, tenant_id, Some(actor_id), event).await?;
txn.commit().await?;
```

## 2) Minimal working example: starting relay

```rust
use rustok_outbox::{OutboxRelay, RelayConfig};

let relay = OutboxRelay::new(db.clone(), target_transport).with_config(RelayConfig::default());
let processed = relay.process_pending_once().await?;
```

## 3) Current API signatures (in repository)

- `pub fn new(transport: Arc<dyn EventTransport>) -> Self` (`TransactionalEventBus`)
- `pub async fn publish_in_tx<C>(&self, txn: &C, tenant_id: Uuid, actor_id: Option<Uuid>, event: DomainEvent) -> Result<()> where C: ConnectionTrait`
- `pub async fn publish(&self, tenant_id: Uuid, actor_id: Option<Uuid>, event: DomainEvent) -> Result<()>`
- `pub fn new(db: DatabaseConnection, target: Arc<dyn EventTransport>) -> Self` (`OutboxRelay`)
- `pub fn with_config(mut self, config: RelayConfig) -> Self` (`OutboxRelay`)
- `pub async fn process_pending_once(&self, max_batch_hint: Option<u64>) -> Result<usize>` (`OutboxRelay`)
- `pub async fn write_to_outbox<C>(&self, txn: &C, envelope: EventEnvelope) -> Result<()> where C: ConnectionTrait` (`OutboxTransport`)
- `pub async fn create_sys_events_schema(manager: &SchemaManager<'_>) -> Result<(), DbErr>` (`migration`) — the single definition of the `sys_events` table; `SysEventsMigration` (module/test schemas) and the platform wrapper `m20260211_000002_create_sys_events` both call it, so the table shape is never duplicated across migrators.
- `pub async fn create_sys_events_claim_index(manager: &SchemaManager<'_>) -> Result<(), DbErr>` (`migration`) — the `(status, created_at)` index the relay claim batch reads through; `SysEventsMigration` and the platform migration `m20261007_000014_add_sys_events_claim_index` both call it.
- `pub async fn create_sys_events_retention_index(manager: &SchemaManager<'_>) -> Result<(), DbErr>` (`migration`) — the `(status, dispatched_at)` index the retention prune reads through; `SysEventsMigration` and the platform migration `m20261007_000015_add_sys_events_retention_index` both call it.
- `pub async fn drop_sys_events_superseded_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr>` (`migration`) — drops `idx_sys_events_pending_next_attempt` and `idx_sys_events_claimed_at`, the pair the claim index superseded (no query filters on either column alone); the append-only platform migration `m20261007_000016_drop_sys_events_superseded_indexes` calls it and its `down` restores the pair.
- `pub struct OutboxRetention` (`retention`) — the delivered-event pruner. `OutboxRetentionConfig { retention, batch_size }` defaults to 30 days and 500 rows per run and rejects a non-positive window or a zero batch; `pub async fn OutboxRetention::prune_once(&self) -> Result<OutboxPruneReport>` deletes only `status = 'dispatched'` rows with `dispatched_at` older than the window, in bounded batches, and reports `OutboxPruneReport { pruned, batch_exhausted }`.

## 4) What not to do (typical incorrect patterns)

1. **Do not replace `publish_in_tx(...)` with `publish(...)` in write-flow with consistency.**
2. **Do not start the relay "sometime later" in production.** Outbox without relay = backlog accumulation without delivery.
3. **Do not write to outbox outside the same transaction as the domain record.**
4. **Do not ignore event validation before publication.**
5. **Do not prune undelivered or DLQ outbox rows.** Retention is delivered-only
   (`status = 'dispatched'` + a non-null `dispatched_at` older than the window), bounded per run,
   and owned by `OutboxRetention`; a `pending` row still owes a delivery and a `failed` row is the
   DLQ an operator works through. Do not re-implement this inside a host or a module.

## 5) Synchronization with code (procedure)

- When changes are made to `crates/modules/rustok-outbox/**` or to the runtime assembly in `apps/server/src/services/event_transport_factory.rs`:
  1) update examples and signatures;
  2) update the date in the header;
  3) verify the relevance of anti-patterns.
