---
id: doc://docs/references/outbox/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# Outbox Reference Package (RusToK)

Last updated: **2026-02-19**.

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
- `pub async fn process_pending_once(&self) -> Result<usize>` (`OutboxRelay`)
- `pub async fn write_to_outbox<C>(&self, txn: &C, envelope: EventEnvelope) -> Result<()> where C: ConnectionTrait` (`OutboxTransport`)

## 4) What not to do (typical incorrect patterns)

1. **Do not replace `publish_in_tx(...)` with `publish(...)` in write-flow with consistency.**
2. **Do not start the relay "sometime later" in production.** Outbox without relay = backlog accumulation without delivery.
3. **Do not write to outbox outside the same transaction as the domain record.**
4. **Do not ignore event validation before publication.**

## 5) Synchronization with code (procedure)

- When changes are made to `crates/rustok-outbox/**` or to the runtime assembly in `apps/server/src/services/event_transport_factory.rs`:
  1) update examples and signatures;
  2) update the date in the header;
  3) verify the relevance of anti-patterns.
