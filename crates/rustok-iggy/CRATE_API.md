# rustok-iggy / CRATE_API

## Public Modules
`config`, `consumer`, `dlq`, `health`, `partitioning`, `producer`, `replay`, `serialization`, `topology`, `transport`.

## Primary Public Types and Signatures
- `pub struct IggyTransport` (implements `EventTransport`)
- `pub trait EventSerializer` + `JsonSerializer`, `PostcardSerializer` (serialize/deserialize)
- `pub struct TopologyManager`, `ConsumerGroupManager`, `ConsumedEvent`, `DlqManager`, `ReplayManager`
- `pub fn health_check(...) -> HealthCheckResult`

## Events
- Publishes: serialized `EventEnvelope` into Iggy stream/topics.
- Consumes: messages from Iggy consumer groups, including replay/DLQ pipeline.

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-iggy-connector`

## Common AI Mistakes
- Skips partition key and breaks processing order.
- Uses the wrong serializer between producer/consumer.

## Minimum Contract Set

### Input DTOs/Commands
- Input contract is defined by the public DTOs/commands from the crate (see sections with `Create*Input`/`Update*Input`/query/filter above and corresponding `pub` exports in `src/lib.rs`).
- All changes to public DTO fields are considered breaking changes and require synchronized updates to transport adapters in `apps/server`.

### Domain Invariants
- Module invariants are enforced in services/state machines and DTO validation; invalid transitions/parameters must result in a domain error.
- Multi-tenant boundary invariants (tenant/resource isolation, auth context) are considered a mandatory part of the contract.

### Events / Outbox Side Effects
- If the module publishes domain events, publication must go through the transactional outbox/transport contract without local workarounds.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.
