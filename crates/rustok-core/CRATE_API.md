# rustok-core / CRATE_API

## Public Modules
`async_utils`, `cache`, `config`, `content_format`, `context`, `error`, `events`, `field_schema`, `grapesjs`, `health`, `i18n`, `id`, `locale`, `metrics`, `migrations`, `module`, `permissions`, `rbac`, `registry`, `resilience`, `rt_json`, `security`, `state_machine`, `tenant_validation`, `tracing`, `typed_error`, `types`, `utils`.

## Primary Public Types and Signatures
- `pub trait RusToKModule` — base contract for platform modules.
- `pub struct AppContext` — shared application runtime context.
- `pub enum DomainEvent`, `pub struct EventEnvelope` — established root event family and transport wrapper.
- `pub struct EventBus`, `pub struct EventBusStats`, `pub struct MemoryTransport` — event foundation surface for in-memory transport and observability counters.
- `pub struct BackpressureController`, `pub struct BackpressureMetrics`, `pub enum BackpressureState` — queue depth guardrails and backpressure observability for events.
- `pub trait EventTransport` — root and sealed typed-family transport boundary.
- `EventTransport::publish(EventEnvelope)` — established root publication path.
- `EventTransport::publish_contract(ContractEventEnvelope)` — sealed typed-family publication path with a root-compatible default adapter.
- `pub enum Error`, `pub type Result<T>` — unified error model.
- `pub struct ModuleRegistry` — module and dependency registry.
- `pub enum UserRole`, `pub enum UserStatus` — shared identity primitives.
- `pub struct CustomFieldsSchema`, `pub struct FieldDefinition` — flex/custom-fields contract.
- `pub fn generate_id()` — canonical ID generation.

## Events
- Publishes established root events through `EventTransport::publish`.
- Publishes sealed bounded-family envelopes through `EventTransport::publish_contract` when the configured transport supports that family profile.
- Consumes: N/A (infrastructure contract layer).

## Dependencies on Other RusToK Crates
- `rustok-telemetry`
- `rustok-events`

`rustok-outbox` is used only in dev-dependencies for integration tests.
Neutral `Port*` contracts belong to `rustok-api` and are not part of the core public surface.

## Common AI Mistakes
- Confuses `AppContext` from `rustok_core::context` with local service contexts.
- Imports canonical event contracts from legacy paths instead of `rustok-events`.
- Sends a bounded-family `ContractEventEnvelope` through root-only `publish`.
- Overrides `publish_contract` with a raw JSON/string API instead of preserving the typed envelope.
- Considers `rustok-core` a domain module (`RusToKModule`) — it is infrastructure core.

## Minimum Contract Set

### Input DTOs/Commands
- Root events use `EventEnvelope` and `publish`.
- Bounded event families use `ContractEventEnvelope` and `publish_contract`.
- Changes to either public transport signature require synchronized adapter and conformance updates.

### Domain Invariants
- The default `publish_contract` adapter may convert only the root contract family back to `EventEnvelope`.
- Streaming transports that support bounded families override `publish_contract` and preserve the typed contract envelope.
- Multi-tenant boundary invariants remain mandatory for every transport profile.

### Events / Outbox Side Effects
- Domain publication goes through the transactional outbox contract rather than direct transport calls from owner transactions.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.
- EventBus/backpressure metrics are an observability contract: publish/drop/accepted/rejected counters and depth/state must not break without updating tests and documentation.

### Errors / Failure Codes
- Unsupported bounded-family publication must fail explicitly; it must not be silently coerced to an unrelated root event.
- Public `Error`/`Result` semantics must remain stable when mapped by adapters.
