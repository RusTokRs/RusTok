# Crate API: rustok-events-module

## Public surface

### Primary Public Types
- `EventsModule`: Core runtime module implementing `RusToKModule` for event capabilities.

### Public modules
- `module`: Module definition and RusToKModule lifecycle trait implementation.

## Platform Capability Bindings
- Runtime module adapter for platform events capability.
- Core lifecycle integration with `rustok-core::module::RusToKModule`.

## Domain Collaborations
- Registers runtime hooks alongside `rustok-outbox`.
- Binds with platform event brokers and event dispatchers.

## Minimum Contract Set

### Input DTOs/Commands
- `EventsModule`: Module descriptor for host module registry registration.

### Domain Invariants
- Module kind is `ModuleKind::Core`.
- Declares explicit dependency on `outbox`.
- Health status defers concrete readiness checks to server event runtime.

### Events / Outbox Side Effects
- Enables platform event pipeline and transactional outbox dispatching.

### Errors / Failure Codes
- Lifecycle errors propagated via `rustok_core::Result`.

