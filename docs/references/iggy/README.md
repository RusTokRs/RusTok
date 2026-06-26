---
id: doc://docs/references/iggy/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Iggy Reference-пакет (RusToK)

Дата последней актуализации: **2026-02-26**.

> Пакет фиксирует рабочий слой интеграции Iggy в RusToK (`rustok-iggy`, `rustok-iggy-connector`, `rustok-outbox`) и защищает от ложных переносов из Kafka/NATS.

## Версии

| Компонент | Версия |
|-----------|--------|
| Rust SDK (`iggy` crate) | `0.9.0` |
| Iggy Server (Docker) | `apache/iggy:0.7.0` |

## 1) Минимальный рабочий пример: поднять transport

```rust
use rustok_iggy::{IggyConfig, IggyTransport};

let config = IggyConfig::default();
let transport = IggyTransport::new(config).await?;

if transport.is_connected() {
    // transport готов для EventTransport::publish
}

transport.shutdown().await?;
```

## 2) Минимальный рабочий пример: write + event через транзакцию

```rust
let txn = db.begin().await?;

// ... write в доменные таблицы

transactional_bus
    .publish_in_tx(&txn, tenant_id, Some(actor_id), event)
    .await?;

txn.commit().await?;
```

Это каноничный путь RusToK для write-flow с событиями.

## 3) Актуальный high-level API (iggy SDK 0.9.0)

SDK 0.9.0 использует высокоуровневый Producer/Consumer API на базе builder-паттерна.
Низкоуровневые методы (`send_messages`, `get_stream`, `create_topic`) доступны, но для
продакшн-кода предпочтителен высокоуровневый подход.

```rust
use iggy::prelude::{IggyClient, IggyDuration, Message, Partitioning};
use std::str::FromStr;

// Создать клиент
let client = IggyClient::from_connection_string("iggy://iggy:iggy@localhost:8090")?;
client.connect().await?;

// Создать producer для stream/topic
let mut producer = client
    .producer("rustok", "domain")?
    .partitioning(Partitioning::balanced())
    .batch_size(100)
    .send_interval(IggyDuration::from_str("5ms")?)
    .build();

producer.init().await?;

// Отправить сообщения
let messages = vec![Message::from_str("payload")?];
producer.send(messages).await?;
```

## 4) Актуальные сигнатуры API (в репозитории)

### `rustok-iggy`
- `pub async fn new(config: IggyConfig) -> Result<Self>`
- `pub async fn shutdown(&self) -> Result<()>`
- `pub async fn subscribe_as_group(&self, group: &str) -> Result<()>`
- `pub async fn replay(&self) -> Result<()>`
- `pub fn config(&self) -> &IggyConfig`
- `pub fn is_connected(&self) -> bool`

### `rustok-iggy-connector`
- `pub async fn connect(&self, config: &ConnectorConfig) -> Result<(), ConnectorError>`
- `pub async fn publish(&self, request: PublishRequest) -> Result<(), ConnectorError>`
- `pub async fn subscribe(&self, stream: &str, topic: &str, partition: u32) -> Result<Box<dyn MessageSubscriber>, ConnectorError>`
- `pub async fn shutdown(&self) -> Result<(), ConnectorError>`
- `pub async fn recv(&mut self) -> Result<Option<Vec<u8>>, ConnectorError>`

### `rustok-outbox`
- `pub async fn publish_in_tx<C>(&self, txn: &C, tenant_id: Uuid, actor_id: Option<Uuid>, event: DomainEvent) -> Result<()> where C: ConnectionTrait`

## 5) Чего делать нельзя (типичные ложные паттерны из Kafka/NATS)

1. **Нельзя предполагать kafka-only semantics (acks/offset commit API), которых нет в текущем abstraction.**
   - Антипаттерн: добавлять в бизнес-код ручные offset-коммиты или direct SDK вызовы Kafka.
   - Правильно: использовать `EventTransport`/`TransactionalEventBus`.

2. **Нельзя использовать fire-and-forget publish для write-flow, требующего консистентности.**
   - Антипаттерн: `publish(...)` до/вместо транзакционного пути.
   - Правильно: `publish_in_tx(...)` при write + event.

3. **Нельзя переносить NATS subject-модель как есть на stream/topic/partition Iggy.**
   - Антипаттерн: проектировать routing только по строковому `subject` без учёта `stream/topic/partition_key`.

4. **Нельзя выдумывать поля конфигурации и режимы коннектора.**
   - В актуальном коде режимы только `Embedded | Remote`, а конфиг идёт через `IggyConfig -> ConnectorConfig`.

5. **Нельзя использовать низкоуровневые методы SDK там, где есть высокоуровневый Producer API.**
   - Антипаттерн: вызывать `client.send_messages(...)` напрямую в бизнес-коде.
   - Правильно: использовать `client.producer(...).build()` → `producer.send(...)`.

## 6) Docker Compose

Iggy-сервер добавлен в `docker-compose.yml` как сервис `iggy`:

```yaml
iggy:
  image: apache/iggy:0.7.0
  ports:
    - "8090:8090"
  environment:
    - IGGY_ROOT_USERNAME=iggy
    - IGGY_ROOT_PASSWORD=iggy
    - IGGY_TCP_ENABLED=true
    - IGGY_TCP_ADDRESS=0.0.0.0:8090
```

## 7) Синхронизация с кодом (регламент)

- При изменениях в `crates/rustok-iggy/**`, `crates/rustok-iggy-connector/**`, `crates/rustok-outbox/**`:
  1) обновить примеры и сигнатуры в этом reference;
  2) обновить дату в шапке;
  3) проверить, что антипаттерны всё ещё релевантны.
- При обновлении версии `iggy` SDK или Docker-образа сервера — обновить таблицу версий в разделе «Версии».
