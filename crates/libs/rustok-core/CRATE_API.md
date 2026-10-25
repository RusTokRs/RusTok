# rustok-core / CRATE_API

## Role

`rustok-core` is the platform foundation runtime crate for RusToK.
It owns the minimal platform-wide runtime contracts, kernel types, and foundational primitives
consumed across the host server (`apps/server`) and all sovereign domain modules (`crates/modules/*`).

`rustok-core` contains **no domain-specific business logic** (e.g., cart operations, checkout flows, catalog structures)
and no transport-level port declarations (which are owned by `rustok-api`).

## Public surface

### 1. Module Lifecycle & Registry
- `RusToKModule` — base trait defining module metadata, lifecycle hooks, migration sources, and event subscribers.
- `ModuleRegistry` — container managing module registration, topological sorting, and lifecycle orchestration.
- `ModuleContext`, `ModuleEventListenerRegistry`, `ModuleKind` — execution context and event listener registration.
- `ModuleMigration`, `MigrationSafetyMetadata` — phased database migration descriptors and invariants.

### 2. Runtime Context & Caching
- `AppContext` — platform execution context providing access to database, cache, and search backends.
- `CacheBackend` — async cache trait supporting atomic compare-and-set (`CacheCompareAndSetOutcome`), TTL, and invalidation.
- `InMemoryCacheBackend` — lock-free/atomic in-memory cache backend implementation.
- `CacheStats` — operational hit/miss and key eviction counters.
- `SearchBackend` — neutral search abstraction trait.

### 3. In-Process Events & Backpressure
- `EventBus` — decoupled publish/subscribe bus for asynchronous domain events.
- `EventDispatcher` — reliable event dispatch loop supporting retry semantics and backpressure.
- `MemoryTransport` — bounded, reliable in-memory transport implementation.
- `BackpressureController`, `BackpressureConfig`, `BackpressureMetrics` — adaptive flow control and concurrency throttling.
- `DomainEvent`, `EventEnvelope`, `EventSchema` — generic event payload envelopes.

### 4. Canonical Money Arithmetic (`money`)
- ISO 4217 minor-unit exponent resolution (`currency_exponent`).
- Currency-aware exact rounding (`round_to_currency`, `round_to_fixed`).
- Major-unit and minor-unit conversions (`to_minor_units`, `from_minor_units`, `to_fixed_point_units`, `from_fixed_point_units`).
- `MoneyError` — typed arithmetic failure modes preventing silent truncation or zero-value bugs.

### 5. Resilience & Asynchronous Utilities
- `CircuitBreaker`, `CircuitBreakerConfig`, `CircuitBreakerError`, `CircuitState` — stateful failure isolation.
- `RetryPolicy`, `RetryStrategy` — exponential and constant backoff policies.
- `retry`, `timeout`, `batch`, `parallel` — asynchronous functional flow combinators.
- `Throttler`, `Debouncer`, `Coalescer` — request rate regulation and duplicate suppression.

### 6. Identity, RBAC & Security Context
- `generate_id`, `parse_id` — ULID/UUID platform identity generators.
- `Rbac`, `SecurityContext`, `PermissionScope`, `SecurityActorKind` — runtime authorization evaluation context.
- `security_context_from_access_token` — token claim extraction to security context.
- `RateLimiter`, `InputValidator`, `SecurityHeaders` — defensive HTTP and payload validation primitives.

### 7. Flex & Custom Fields Schema (`field_schema`)
- `CustomFieldsSchema`, `FieldDefinition`, `FieldType`, `ValidationRule` — extensible JSON schema validation engine.
- `HasCustomFields` — trait for entities supporting user-defined custom attributes.
- JSON utility helpers (`json_field_extract`, `json_field_eq`, `json_object_depth`).

### 8. Typed Errors (`typed_error` & `error`)
- `DomainError`, `ErrorCode`, `ErrorCategory`, `TypedResult` — structured machine-readable error responses.
- `Error`, `RichError`, `ValidationErrorBuilder` — flexible operational error types.

### 9. Prelude (`rustok_core::prelude`)
The prelude exports foundational types (`AppContext`, `CacheBackend`, `InMemoryCacheBackend`,
`EventBus`, `generate_id`, `DomainError`, `ErrorCode`, `TypedResult`, `CircuitBreaker`,
`SecurityContext`, `Rbac`, `UserRole`, `UserStatus`) commonly needed across domain crates.

## Feature flags

- `default` — Core runtime contracts, in-memory caching, events, resilience, money, and error types.
- `server` — Enables `rustok-api/server` feature propagation for host integration.
- `redis-cache` — Empty compatibility feature for workspace build configurations.

## Primary entry points

- [`RusToKModule`](src/module/mod.rs): Base trait for sovereign native domain modules.
- [`ModuleRegistry`](src/registry/mod.rs): Central registration and lifecycle dispatch engine.
- [`AppContext`](src/context/mod.rs): Runtime container for database connection and platform services.
- [`InMemoryCacheBackend`](src/cache_atomic/mod.rs): Thread-safe in-memory cache backend with atomic CAS semantics.
- [`EventBus`](src/events/mod.rs): Platform in-process asynchronous event bus.
- [`money::round_to_currency`](src/money.rs): Canonical ISO 4217 rounding and exponent resolution.

## Invariants

1. **Zero domain business logic**: `rustok-core` contains no domain models, checkout logic, or user credential schemas.
2. **Safe currency calculations**: Money conversions never use floating-point types; exponents are governed by ISO 4217.
3. **Atomic cache consistency**: All cache backends provide deterministic compare-and-set semantics for concurrency safety.
4. **Structured error propagation**: Errors must not be swallowed; all failures produce typed domain or operational results.

## Minimum Contract Set

### Input DTOs/Commands
- `ModuleContext` carries database connection handles and configuration access during module initialization.
- `EventBus::publish` accepts `EventEnvelope<DomainEvent>` or generic typed event envelopes.
- `CacheBackend::compare_and_set` accepts key, expected version/value token, new payload, and TTL.
- `money::round_to_currency` accepts `Decimal` amount and ISO 4217 currency code.
- `RateLimiter::check_rate_limit` accepts client key, configured rate limits, and returns `RateLimitResult`.

### Domain Invariants
- `AppContext` preserves isolation across tenant scopes; cache keys must be namespace-qualified.
- Currency math strictly validates currency codes against known ISO 4217 exponents (rejecting invalid exponents).
- Compare-and-set operations guarantee race-free atomic updates; on mismatch, return `CacheCompareAndSetOutcome::Mismatch`.
- Circuit breakers transition across `Closed`, `Open`, and `HalfOpen` states deterministically without panicking.

### Events / Outbox Side Effects
- `EventBus` delivers in-process events to registered handlers with configurable backpressure and timeout semantics.
- `BackpressureController` monitors event queue depth, dropping or queueing per configured `ReliabilityLevel`.
- Caching side-effects must remain decoupled from durable database state transitions.

### Errors / Failure Codes
- `DomainError` uses strongly-typed `ErrorCode` with mapped HTTP status codes.
- `MoneyError` distinguishes invalid currency codes, overflow, and unsupported exponent ranges.
- `CircuitBreakerError::Open` explicitly signals downstream service unavailability to avoid cascading failure.
- `ConfigError` and `FieldError` provide granular validation failure reasons.
