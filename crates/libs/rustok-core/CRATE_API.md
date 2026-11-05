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

The migration foundation also exports `MigrationSafetyAssessment`:
`AdditiveSafe`, `MaintenanceRequired { reasons, locks_table, is_destructive }`,
and `Unknown { reasons }`. `is_additive_safe()` examines the variant;
`denial_reasons()` supplies a nonempty diagnostic for every rejected assessment,
including absent or whitespace-only explanations. No unknown/missing serialized
variant defaults to safe. The producer retains plan identity and evidence scope;
the modules owner retains update authorization. See the
[shared assessment decision](../../../DECISIONS/2026-11-05-migration-safety-assessment.md).

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

- [`RusToKModule`](src/module.rs): Base trait for sovereign native domain modules.
- [`ModuleRegistry`](src/registry.rs): Central registration and lifecycle dispatch engine.
- [`AppContext`](src/context.rs): Runtime container for database connection and platform services.
- [`InMemoryCacheBackend`](src/cache_atomic.rs): Thread-safe in-memory cache backend with atomic CAS semantics.
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

## Acknowledged sealed module delivery (2026-11-01; source only)

- `EventHandler::handles_contract(&ContractEventPayload) -> bool` defaults to false;
  `handle_contract(&ContractEventEnvelope)` defaults to a typed refusal, never fake success.
- `EventDispatcher::start_with_contract_delivery(self) -> Result<RunningDispatcher>` binds
  explicit module handlers to the existing bus. Legacy `start()` and root APIs remain.
- `EventBus::has_contract_handler(&ContractEventEnvelope) -> Result<bool>` requires a live
  registry; it does not infer readiness from root broadcast subscriber counts.
- `EventBus::publish_contract_envelope` and `MemoryTransport::publish_contract` validate sealed
  frames and await all matching handlers. Capacity is bounded with immediate refusal, not an
  extra in-memory queue. No-handler, closed/unbound registry and handler errors propagate.
- The fallible typed startup rejects concurrency/queue settings above Tokio's `MAX_PERMITS`
  before binding or spawning; invalid bounds cannot panic while poisoning the listener registry.

The bus stores a Weak listener binding; the running task owns the strong registration. Stop,
abort, task exit/unwind close it, including a final closure check after awaited handler work.
This avoids a handler-to-bus ownership cycle. Root broadcast remains best-effort; it does not
inherit the sealed acknowledgement guarantee. Core owns no Inventory switch or domain publisher.
Outbox owns durable retry/DLQ; remote transport consumers still own persistent receive/cursor ACK.
Six new delivery definitions are uncompiled/unrun; real runtime evidence remains open. See the
[delivery decision](../../../DECISIONS/2026-07-17-sealed-typed-event-families.md#acknowledged-local-contract-listeners-2026-11-01).

## Required-module delivery continuation (2026-11-02)

Canonical payloads expose compiled `requires_module_delivery()` policy. Inventory source and
reservation facts cannot be acknowledged by unmatched broker-only fan-out. Core's existing
bounded registry already refuses absent/inactive/unhandled/failing listeners and awaits every
match; it accepts the additive family without domain SQL or a second queue. Host fan-out applies
the policy before primary acceptance. No source success/no-op listener or generic Index
readiness is introduced. Remote cursor ACK and selected current-authority source/replay/Index
handling remain separate unfinished boundaries.
