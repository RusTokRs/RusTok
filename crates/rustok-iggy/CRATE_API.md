# rustok-iggy / CRATE_API

## Public Modules
`config`, `consumer`, `contract_consumer`, `dlq`, `health`, `partitioning`, `producer`, `replay`, `serialization`, `topology`, `transport`.

## Primary Public Types and Signatures
- `pub struct IggyTransport` (implements `EventTransport`)
- `pub enum IggyMode { Bundled, External }`
- `pub struct LocalConfig`, `RemoteConfig`, `IggyConfig`
- `pub trait EventSerializer` + `JsonSerializer`, `MessagePackSerializer`
- `EventSerializer::{serialize, deserialize}` for established root envelopes
- `EventSerializer::{serialize_contract, deserialize_contract}` for sealed typed-family envelopes
- `pub struct TopologyManager`, `ConsumedEvent`, `PersistentConsumerGroup`
- `pub struct ConsumedContractEvent`, `PersistentContractConsumerGroup`
- `pub fn health_check(...) -> HealthCheckResult`

## Events
- Publishes root `EventEnvelope` and sealed `ContractEventEnvelope` values into Iggy stream/topics.
- Preserves the configured JSON or MessagePack serialization profile for both envelope types.
- JSON timestamps use RFC 3339; MessagePack timestamps use UTC microseconds while
  decoding to the same envelope field.
- Root consumers use `PersistentConsumerGroup`.
- Bounded-family consumers use the explicit `PersistentContractConsumerGroup`.
- Supports replay/DLQ pipelines without silently interpreting family events as `DomainEvent`.

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-events`
- `rustok-iggy-connector`

## Common AI Mistakes
- Skips the tenant partition key and breaks processing order.
- Uses a different serializer profile between producer and consumer.
- Publishes a contract envelope through the root-only producer path.
- Consumes a bounded-family event through `PersistentConsumerGroup` instead of the explicit contract cursor.
- Acknowledges an event with metadata from a different stream/topic/partition cursor.

## Minimum Contract Set

### Input DTOs/Commands
- `IggyTransport::publish` accepts established root envelopes.
- `IggyTransport::publish_contract` accepts sealed typed-family envelopes.
- `open_persistent_consumer_group` and `open_persistent_contract_consumer_group` are explicit and non-interchangeable profiles.

### Domain Invariants
- Event ID, tenant partition key, event type, topic, and configured serialization format are preserved.
- Contract envelopes validate against the canonical schema registry before publish and after consume.
- Receive and acknowledge operate on the same persistent connector cursor.
- Connector metadata must match stream, topic, and partition before acknowledgement.
- Owner code acknowledges only after it persisted a terminal business result or
  recognized a durable idempotent redelivery. The transport has no legacy
  per-partition receive/re-subscribe acknowledgement API.

### Events / Outbox Side Effects
- Root and typed-family events route to the same domain/system topology rules unless a dedicated event type requires another topic.
- Outbox relay calls the matching root or contract transport method.
- Event payload and event-type format remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- Connector, serialization, schema validation, metadata mismatch, and acknowledgement failures remain distinguishable.
- Failed consume/publish operations must not acknowledge broker offsets implicitly.
- `Bundled` starts the module-installed native `iggy-server` process and uses
  the same real TCP SDK path as `External`; it is not an in-memory simulator.
