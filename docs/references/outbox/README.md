---
id: doc://docs/references/outbox/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Outbox Reference-пакет (RusToK)

Дата последней актуализации: **2026-02-19**.

> Пакет фиксирует корректный transactional outbox flow (`rustok-outbox`) и предотвращает ложные паттерны из «простого publish после commit».

## 1) Минимальный рабочий пример: transactional publish

```rust
use rustok_outbox::TransactionalEventBus;

let bus = TransactionalEventBus::new(transport);

let txn = db.begin().await?;
// ... доменные изменения
bus.publish_in_tx(&txn, tenant_id, Some(actor_id), event).await?;
txn.commit().await?;
```

## 2) Минимальный рабочий пример: запуск relay

```rust
use rustok_outbox::{OutboxRelay, RelayConfig};

let relay = OutboxRelay::new(db.clone(), target_transport).with_config(RelayConfig::default());
let processed = relay.process_pending_once().await?;
```

## 3) Актуальные сигнатуры API (в репозитории)

- `pub fn new(transport: Arc<dyn EventTransport>) -> Self` (`TransactionalEventBus`)
- `pub async fn publish_in_tx<C>(&self, txn: &C, tenant_id: Uuid, actor_id: Option<Uuid>, event: DomainEvent) -> Result<()> where C: ConnectionTrait`
- `pub async fn publish(&self, tenant_id: Uuid, actor_id: Option<Uuid>, event: DomainEvent) -> Result<()>`
- `pub fn new(db: DatabaseConnection, target: Arc<dyn EventTransport>) -> Self` (`OutboxRelay`)
- `pub fn with_config(mut self, config: RelayConfig) -> Self` (`OutboxRelay`)
- `pub async fn process_pending_once(&self) -> Result<usize>` (`OutboxRelay`)
- `pub async fn write_to_outbox<C>(&self, txn: &C, envelope: EventEnvelope) -> Result<()> where C: ConnectionTrait` (`OutboxTransport`)

## 4) Чего делать нельзя (типичные ложные паттерны)

1. **Нельзя заменять `publish_in_tx(...)` на `publish(...)` в write-flow с консистентностью.**
2. **Нельзя запускать relay «когда-нибудь потом» в production.** Outbox без relay = накопление backlog без доставки.
3. **Нельзя писать в outbox вне той же транзакции, где доменная запись.**
4. **Нельзя игнорировать валидацию event перед публикацией.**

## 5) Синхронизация с кодом (регламент)

- При изменениях в `crates/rustok-outbox/**` или в runtime-сборке `apps/server/src/services/event_transport_factory.rs`:
  1) обновить примеры и сигнатуры;
  2) обновить дату в шапке;
  3) проверить актуальность anti-patterns.
