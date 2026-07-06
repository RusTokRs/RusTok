# rustok-core / CRATE_API

## Public Modules
`async_utils`, `cache`, `config`, `content_format`, `context`, `error`, `events`, `field_schema`, `grapesjs`, `health`, `i18n`, `id`, `locale`, `metrics`, `migrations`, `module`, `permissions`, `rbac`, `registry`, `resilience`, `rt_json`, `security`, `state_machine`, `tenant_validation`, `tracing`, `typed_error`, `types`, `utils`.

## Primary Public Types and Signatures
- `pub trait RusToKModule` — base contract for platform modules.
- `pub struct AppContext` — shared application runtime context.
- `pub enum DomainEvent`, `pub struct EventEnvelope` — domain events and transport wrapper.
- `pub struct EventBus`, `pub struct EventBusStats`, `pub struct MemoryTransport` — event foundation-surface for in-memory transport and observability counters.
- `pub struct BackpressureController`, `pub struct BackpressureMetrics`, `pub enum BackpressureState` — queue depth guardrails and backpressure observability for events.
- `pub trait EventTransport` — event transport.
- `pub enum Error`, `pub type Result<T>` — unified error model.
- `pub struct ModuleRegistry` — module and dependency registry.
- `pub enum UserRole`, `pub enum UserStatus` — shared identity primitives.
- `pub struct CustomFieldsSchema`, `pub struct FieldDefinition` — flex/custom-fields contract.
- `pub fn generate_id()` — canonical ID generation.

## Events
- Publishes: base domain events via `DomainEvent` (defines the contract, not a business emitter).
- Consumes: N/A (infrastructure contract layer).

## Dependencies on Other RusToK Crates
- `rustok-telemetry`
- `rustok-events`

`rustok-outbox` is used only in dev-dependencies for integration tests.
Neutral `Port*` contracts belong to `rustok-api` and are not part of the core public surface.

## Common AI Mistakes
- Confuses `AppContext` from `rustok_core::context` with local service contexts.
- Imports `DomainEvent` from old paths instead of `rustok_core`/`rustok-events`.
- Considers `rustok-core` a domain module (`RusToKModule`) — it is infrastructure core.

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
- EventBus/backpressure metrics are an observability contract: publish/drop/accepted/rejected counters and depth/state must not break without updating tests and documentation.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.
