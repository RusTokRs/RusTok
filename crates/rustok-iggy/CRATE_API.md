# rustok-iggy / CRATE_API

## Public Modules
`config`, `consumer`, `contract_consumer`, `dlq`, `health`, `partitioning`, `position` behind feature `iggy`, `producer`, `serialization`, `topology`, `transport`.

## Primary Public Types and Signatures
- `pub struct IggyTransport` (implements `EventTransport`)
- `pub enum IggyMode { Bundled, External }`
- `pub struct BundledConfig`, `ExternalConfig`, `IggyConfig`
- `pub trait EventSerializer` + `JsonSerializer`, `MessagePackSerializer`
- `EventSerializer::{serialize, deserialize}` for established root envelopes
- `EventSerializer::{serialize_contract, deserialize_contract}` for sealed typed-family envelopes
- `pub struct TopologyManager`, `ConsumedEvent`, `PersistentConsumerGroup`
- `pub struct ConsumedContractEvent`, `PersistentContractConsumerGroup`
- `ConsumedContractEvent::raw_payload()` exposes exact received JSON or MessagePack bytes for lossless owner-directed DLQ publication.
- `DlqEntry::with_broker_message_id(Uuid)` attaches a stable owner-derived Iggy message-header identity without changing the original event ID.
- `IggyConsumerPositionObserver::connect(&IggyConfig, group, topic)` opens a read-only SDK client to the already-running configured endpoint.
- `IggyConsumerPositionObserver::snapshot()` returns `ConsumerPositionSnapshot` with every topic partition, its committed group offset, current high-watermark, and message count.
- `ConsumerPartitionPosition::lag()` returns an exact checked offset difference, zero for an empty partition, and `None` for missing or incoherent checkpoints.
- `ConsumerPositionSnapshot::{is_complete,total_lag,max_lag}` expose aggregate lag only when every partition is coherent.
- `pub fn health_check(...) -> HealthCheckResult`

## Events
- Publishes root `EventEnvelope` and sealed `ContractEventEnvelope` values into Iggy stream/topics.
- Preserves the configured JSON or MessagePack serialization profile for both envelope types.
- JSON timestamps use RFC 3339; MessagePack timestamps use UTC microseconds while decoding to the same envelope field.
- Root consumers use `PersistentConsumerGroup`; bounded-family consumers use `PersistentContractConsumerGroup`.
- A successfully decoded `ConsumedContractEvent` retains exact original broker bytes alongside the canonical envelope and connector metadata.
- Supports DLQ movement and entry-based retry without silently interpreting family events as `DomainEvent`.
- Replay remains unavailable until bounded broker reads, republish, durable progress, and real-broker evidence exist.

## Deterministic DLQ publication
- A `DlqEntry` may carry a non-nil stable broker message UUID while retaining the original event ID, payload, source metadata, error code, and retry count separately.
- `IggyTransport::move_to_dlq` uses the established connector path for entries without a broker ID.
- For an identified entry, `IggyTransport` lazily opens one dedicated SDK publisher connection to the same configured endpoint and publishes to the existing `dlq` topology with `IggyMessage::builder().id(uuid.as_u128())`.
- The publisher uses the same reviewed TCP addresses, credentials, TLS options, one-based partition routing, topic partition count, and replication factor as the primary transport configuration.
- Publication failure drops the dedicated client; the owner worker keeps the source delivery unacknowledged and its bounded retry reconnects.
- Shutdown drops the dedicated publisher before stopping the primary connector/process.
- A deterministic header ID enables Iggy's optional per-partition duplicate suppression only when the deployment enables it and its bounded cache/expiry still covers the retry window.
- The durable owner receipt remains authoritative. This crate does not claim broker exactly-once from a message ID, an optional cache, or a successful SDK return alone.

## Consumer-position observation
- Position observation is read-only and feature-gated with the real Iggy SDK.
- Bundled mode connects to the already-running loopback endpoint; it never starts or stops another broker process.
- External mode uses the same reviewed TCP addresses, credentials, and TLS connection-string options as transport configuration.
- The observer reads `TopicDetails.partitions` and the consumer-group offset for each partition.
- High-watermark is the maximum of the topic partition offset and the offset API observation from the same snapshot pass.
- A non-empty partition without a committed group offset makes the complete snapshot unavailable rather than assuming offset zero.
- Aggregate total/max lag uses checked arithmetic and is never inferred from event timestamps, processing duration, or one global offset.

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-events`
- `rustok-iggy-connector`
- optional upstream `iggy` SDK for deterministic DLQ headers, the read-only position observer, and real transport features

## Common AI Mistakes
- Skips the tenant partition key and breaks processing order.
- Uses a different serializer profile between producer and consumer.
- Publishes a contract envelope through the root-only producer path.
- Consumes a bounded-family event through `PersistentConsumerGroup` instead of the explicit contract cursor.
- Re-serializes a decoded contract envelope for DLQ instead of preserving exact received bytes.
- Acknowledges an event with metadata from another stream/topic/partition cursor.
- Lets transport choose poison policy before the owner defines its durable-result boundary.
- Treats a deterministic Iggy message ID as durable exactly-once or assumes server deduplication is enabled/unbounded.
- Generates a random DLQ message ID on each retry or includes retry count/time in the ID.
- Starts another bundled broker instead of opening a client connection to the shared endpoint.
- Calls event age, processing duration, a delivered offset, or a single partition observation consumer lag.
- Treats an incomplete position snapshot as zero lag.

## Minimum Contract Set

### Input DTOs/Commands
- `IggyTransport::publish` accepts established root envelopes.
- `IggyTransport::publish_contract` accepts sealed typed-family envelopes.
- `open_persistent_consumer_group` and `open_persistent_contract_consumer_group` are explicit and non-interchangeable profiles.
- `DlqEntry` accepts exact original payload and connector metadata; an owner may additionally attach a stable broker message UUID; DLQ publication does not acknowledge the source cursor.
- Position observation requires non-empty group/topic names and a valid reviewed TCP endpoint configuration.

### Domain Invariants
- Event ID, tenant partition key, event type, topic, and configured serialization format are preserved.
- Contract envelopes validate against the canonical schema registry before publish and after consume.
- Receive and acknowledge operate on the same persistent connector cursor.
- Connector metadata must match stream, topic, and partition before acknowledgement.
- Exact raw payload retention applies after successful contract-envelope decoding; malformed bytes remain unacknowledged until a connector poison contract exists.
- Owner code acknowledges only after a terminal durable result or recognized idempotent redelivery.
- An attached DLQ broker ID is non-random and stable for the owner identity; it never replaces the durable receipt.
- Position snapshots are partition-qualified and aggregate lag is valid only when all partitions are complete and coherent.

### Events / Outbox Side Effects
- Root and typed-family events route to the same domain/system topology rules unless a dedicated family requires another topic.
- Outbox relay calls the matching root or contract transport method.
- DLQ publication and source acknowledgement remain separate result-first operations.
- Identified DLQ publication may open one lazy SDK connection to the same endpoint; it does not create or supervise another broker process.
- Position observation has no publish, consume, offset-store, delete, or acknowledgement side effect.

### Errors / Failure Codes
- Connector, serialization, schema validation, metadata mismatch, acknowledgement, deterministic-DLQ configuration/connection/publication, and position failures remain distinguishable.
- Deterministic DLQ publication exposes bounded `iggy.dlq_publisher.*` stable codes internally for operational logs.
- Position observer exposes bounded stable codes for invalid configuration, connection unavailable, topic unavailable, and snapshot failure.
- Failed consume/publish/position operations never acknowledge broker offsets implicitly.
- `Bundled` manages the module-installed native `iggy-server`; extra publisher/observer clients only connect to that existing process.
