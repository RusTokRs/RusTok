---
id: doc://docs/references/iggy/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# Iggy Reference Package (RusToK)

Last updated: **2026-02-26**.

> This package captures the working Iggy integration layer in RusToK (`rustok-iggy`, `rustok-iggy-connector`, `rustok-outbox`) and protects against incorrect migrations from Kafka/NATS.

## Versions

| Component | Version |
|-----------|--------|
| Rust SDK (`iggy` crate) | `0.9.0` |
| Iggy Server (Docker) | `apache/iggy:0.7.0` |

## 1) Minimal working example: bring up transport

```rust
use rustok_iggy::{IggyConfig, IggyTransport};

let config = IggyConfig::default();
let transport = IggyTransport::new(config).await?;

if transport.is_connected() {
    // transport ready for EventTransport::publish
}

transport.shutdown().await?;
```

## 2) Minimal working example: write + event through transaction

```rust
let txn = db.begin().await?;

// ... write to domain tables

transactional_bus
    .publish_in_tx(&txn, tenant_id, Some(actor_id), event)
    .await?;

txn.commit().await?;
```

This is the canonical RusToK path for write-flow with events.

## 3) Current high-level API (iggy SDK 0.9.0)

SDK 0.9.0 uses the high-level Producer/Consumer API based on the builder pattern.
Low-level methods (`send_messages`, `get_stream`, `create_topic`) are available, but for
production code the high-level approach is preferred.

```rust
use iggy::prelude::{IggyClient, IggyDuration, Message, Partitioning};
use std::str::FromStr;

// Create client
let client = IggyClient::from_connection_string("iggy://iggy:iggy@localhost:8090")?;
client.connect().await?;

// Create producer for stream/topic
let mut producer = client
    .producer("rustok", "domain")?
    .partitioning(Partitioning::balanced())
    .batch_size(100)
    .send_interval(IggyDuration::from_str("5ms")?)
    .build();

producer.init().await?;

// Send messages
let messages = vec![Message::from_str("payload")?];
producer.send(messages).await?;
```

## 4) Current API signatures (in repository)

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

## 5) What not to do (typical incorrect patterns from Kafka/NATS)

1. **Do not assume kafka-only semantics (acks/offset commit API) that do not exist in the current abstraction.**
   - Anti-pattern: adding manual offset commits or direct SDK calls to Kafka in business code.
   - Correct: use `EventTransport`/`TransactionalEventBus`.

2. **Do not use fire-and-forget publish for write-flow requiring consistency.**
   - Anti-pattern: `publish(...)` before/instead of the transactional path.
   - Correct: `publish_in_tx(...)` for write + event.

3. **Do not migrate the NATS subject model as-is onto Iggy stream/topic/partition.**
   - Anti-pattern: designing routing only by string `subject` without considering `stream/topic/partition_key`.

4. **Do not invent configuration fields and connector modes.**
   - In the current code, modes are only `Embedded | Remote`, and config goes through `IggyConfig -> ConnectorConfig`.

5. **Do not use low-level SDK methods where a high-level Producer API exists.**
   - Anti-pattern: calling `client.send_messages(...)` directly in business code.
   - Correct: use `client.producer(...).build()` → `producer.send(...)`.

## 6) Docker Compose

The Iggy server is added to `docker-compose.yml` as the `iggy` service:

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

## 7) Synchronization with code (procedure)

- When changes are made to `crates/rustok-iggy/**`, `crates/rustok-iggy-connector/**`, `crates/rustok-outbox/**`:
  1) update examples and signatures in this reference;
  2) update the date in the header;
  3) verify that the anti-patterns are still relevant.
- When updating the version of the `iggy` SDK or Docker server image — update the version table in the "Versions" section.
