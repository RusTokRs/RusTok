# rustok-cache / CRATE_API

## Public Modules
`module`, `settings`.

## Platform Capability Bindings
- **Core Module Registration**: Exposes `CacheModule` implementing `RusToKModule` (slug `"cache"`, kind `ModuleKind::Core`) and `MigrationSource`.
- **Cache Service & Backends**:
  - `CacheService` / `CacheBackendOptions` / `CacheLoadResult`
  - `DegradationAwareFallbackBackend` (feature `redis-cache`)
  - `RedisCacheStatus` / `CacheHealthReport`
- **Atomic Operations & Leases**:
  - Compare-and-set: `CacheCompareAndSetStats`, `observe_cache_compare_and_set`
  - Leases: `DistributedCacheLease`, `CacheLeaseOptions`, `CacheLeaseOutcome`
- **Invalidation & Anti-Stampede**:
  - `CacheRefreshCoordinator`, `StaleWhileRevalidateResult`
  - `CacheNamespaceGenerationStore`, `CacheInvalidationService`
  - `DurableCacheInvalidationConsumer`, `DurableInvalidationProcessOutcome`
  - `BoundedCacheInvalidationGapTracker`, `BoundedCacheEventDedupe`
- **Metrics & Observability**:
  - `format_cache_service_prometheus_metrics`
  - `format_cache_generation_prometheus_metrics`
  - `format_cache_refresh_prometheus_metrics`
  - `format_cache_compare_and_set_prometheus_metrics`

## Domain Collaborations
- **Core Engine**: Integrates with `rustok-core` cache traits, compare-and-set primitives, and envelope formats.
- **Telemetry**: Emits Prometheus metrics for hits, misses, latency, CAS operations, and Redis reconnection states.
- **External Dependencies**: Manages connection pools and failover for optional `redis` backend.

## Primary Public Types and Signatures
- `pub struct CacheModule`
- `pub struct CacheService`
- `pub struct CacheSettings`
- `pub enum CacheError`
- `pub struct CacheKeyBuilder`
- `pub struct CacheEnvelope<T>`

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-telemetry`

## Common AI Mistakes
- Creates raw Redis connections directly instead of injecting or retrieving `CacheService`.
- Bypasses key length bounds checks (`MAX_CACHE_KEY_INPUT_BYTES`).
- Ignores fallback modes, causing application panic when Redis is degraded or unavailable.

## Minimum Contract Set

### Input DTOs/Commands
- `CacheBackendOptions` (`memory_capacity`, `default_ttl`, `max_ttl`)
- `CacheLeaseOptions` (`lease_duration`, `acquire_timeout`)
- `CacheRefreshLoadOptions`
- `TypedCacheLoadOptions`

### Domain Invariants
- Bounded payload size: Values exceeding `DEFAULT_MAX_CACHE_ENVELOPE_BYTES` are rejected to prevent memory exhaustion.
- Bounded key size: Keys exceeding `MAX_CACHE_KEY_INPUT_BYTES` are rejected or hashed deterministically.
- Fallback resilience: If Redis connection drops, the service degrades gracefully to local in-memory caching without panicking.
- Invalidation idempotency: Replayed invalidation events with identical sequence or generation do not cause spurious cache evictions.

### Events / Outbox Side Effects
- Distributed invalidation messages are serialized via `durable_invalidation_to_message` and broadcast to nodes.

### Errors / Failure Codes
- `CacheError`, `CacheKeyError`, `CacheLeaseError`, `CacheEnvelopeError`, `DurableCacheInvalidationError` provide structured, typed error semantics.
