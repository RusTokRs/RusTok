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
  - `CacheTtlPolicy::{none, fixed, deterministic_jitter}` — an opaque policy: the representation is
    private, so these constructors are the only entry point, and both TTL constructors reject a zero
    TTL with `CachePolicyError::ZeroTtl`. No policy that only fails at use time can be built.
  - `CacheNamespaceGenerationStore`, `CacheInvalidationService`
  - `DurableCacheInvalidationConsumer`, `DurableInvalidationProcessOutcome`
  - `BoundedCacheInvalidationGapTracker` (canonical; no unbounded variant), `BoundedCacheEventDedupe`
  - `CacheInvalidationError` (`InvalidMessage`, `SharedTransport`), `DurableCacheInvalidationError::Transport`
- **Settings & Admin Seam**:
  - `CacheSettings` (resolved target, validation, redaction, merge-back), `preserve_url_password`
  - `same_redis_target` / `REDACTED_SECRET` (a password is scoped to a target; a mask is never a
    credential), `resolve_redis_url`, `REDIS_URL_ENV_NAMES`
  - `CacheSettingsAdminPort`, `SharedCacheSettingsAdmin`
  - `CacheHealthReport` (`is_connected()` = transport, `is_healthy()` = reachable **and** fully
    recovered, plus `redis_recovery_pending` / `redis_recovery_error`)
  - `resolve_redis_url` (deployment precedence: settings file → `RUSTOK_REDIS_URL` → `REDIS_URL`)
  - authority for the admin surface is not owned here: both transports evaluate
    `rustok_api::authorizes_host_or_permission` (`settings:read` for diagnostics and the settings
    document, `settings:manage` for a write, or the matching host-global authority)
- **Generation convergence**:
  - `CacheNamespaceGenerationStore::read` heals a lost shared counter from a trusted local
    observation (`GENERATION_CATCH_UP_SCRIPT`, advance-only) and reports
    `rustok_cache_generation_shared_catch_up_{writes,failures}_total`;
  - `CacheGenerationError::SharedBumpAppliedLocalLag` marks a bump whose shared counter moved but
    whose local snapshot did not.
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
- `pub trait CacheSettingsAdminPort` / `pub struct SharedCacheSettingsAdmin`
- `pub struct RedisCacheStatus` (`connectivity_healthy`, `generation_recovery_pending`, `recovery_error`)
- `pub enum CacheRefreshClock` (`System`, `Fixed`)
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
- `CacheBackendOptions` (`metrics_enabled`, `redis_circuit_breaker`)
- `CacheLeaseOptions` (`lease_duration`, `acquire_timeout`)
- `CacheRefreshLoadOptions`
- `TypedCacheLoadOptions`

### Domain Invariants
- Bounded payload size: Values exceeding `DEFAULT_MAX_CACHE_ENVELOPE_BYTES` are rejected to prevent memory exhaustion.
- Bounded key size: Keys exceeding `MAX_CACHE_KEY_INPUT_BYTES` are rejected or hashed deterministically.
- Fallback resilience: If Redis connection drops, the service degrades gracefully to local in-memory caching without panicking; degraded reads serve only locally confirmed degraded writes and fail-closed saturation is exited only through `CacheBackend::reconcile_local_state()`.
- Correctness over acknowledgement: `publish`, `publish_invalidation` and `publish_durable` return an error when Redis is configured but the fan-out failed, and the Redis target is deployment configuration rather than an editable runtime setting.
- Invalidation idempotency: Replayed invalidation events with identical sequence or generation do not cause spurious cache evictions.

### Events / Outbox Side Effects
- Distributed invalidation messages are serialized via `durable_invalidation_to_message` and broadcast to nodes.

### Errors / Failure Codes
- `CacheError`, `CacheKeyError`, `CacheLeaseError`, `CacheEnvelopeError`, `DurableCacheInvalidationError` provide structured, typed error semantics.
