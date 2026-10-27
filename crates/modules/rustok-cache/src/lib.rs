//! # RusToK Cache Module
//!
//! Centralised bounded cache capability for the RusToK platform.
//!
//! Manages Redis connection lifecycle, backend factories, degradation-aware fallbacks,
//! atomic compare-and-set (CAS), distributed leases, anti-stampede refresh, and durable invalidations.

mod backend_generation;
mod backend_generation_recovery;
mod bounded_invalidation;
mod cas_observability;
mod clock;
mod durable_invalidation;
mod durable_invalidation_service;
mod durable_invalidation_transport;
mod envelope;
mod error;
mod event_dedupe;
#[cfg(feature = "redis-cache")]
mod fallback;
mod generation;
mod invalidation;
mod invalidation_consumer;
mod instrumentation;
mod invalidation_processor;
mod key;
mod key_generation;
mod lease;
pub mod module;
mod negative;
mod observability;
mod policy;
mod redis_status;
mod refresh;
mod service;
pub mod settings;
mod settings_admin;
mod shared_backend;
#[cfg(all(test, feature = "redis-cache"))]
mod startup_recovery_tests;
mod tenant_generation_observability;
mod typed;
mod weighted;

#[cfg(test)]
mod contract_tests;

pub use module::CacheModule;
pub use settings::{
    CacheSettings, DEFAULT_CACHE_MODE, DEFAULT_REDIS_DB, DEFAULT_REDIS_HOST, DEFAULT_REDIS_MODE,
    DEFAULT_REDIS_PORT, REDACTED_SECRET, REDIS_URL_ENV_NAMES, preserve_url_password,
    resolve_redis_url, same_redis_target,
};
pub use settings_admin::{CacheSettingsAdminPort, SharedCacheSettingsAdmin};


pub use backend_generation::{
    CacheBackendGenerationError, CacheBackendGenerationSnapshot,
    DEFAULT_MAX_CACHE_BACKEND_GENERATIONS, MAX_CACHE_BACKEND_PREFIX_BYTES,
    bind_cache_backend_generation_aliases, cache_backend_generation_registry_size,
    cache_backend_generation_snapshot, observe_cache_backend_generation,
};
pub use bounded_invalidation::{
    BoundedCacheInvalidationGapTracker, BoundedInvalidationTrackerError,
    DEFAULT_MAX_TRACKED_INVALIDATION_CHANNELS,
};
pub use cas_observability::{
    CacheCompareAndSetMetrics, CacheCompareAndSetStats,
    format_cache_compare_and_set_prometheus_metrics, observe_cache_compare_and_set,
};
pub use durable_invalidation::{
    DEFAULT_MAX_DURABLE_INVALIDATION_BYTES, DURABLE_CACHE_INVALIDATION_FORMAT_VERSION,
    DurableCacheInvalidationError, DurableCacheInvalidationRecord,
    MAX_DURABLE_INVALIDATION_CAUSE_BYTES, MAX_DURABLE_INVALIDATION_TRACE_ID_BYTES,
};
pub use durable_invalidation_transport::{
    durable_invalidation_from_message, durable_invalidation_to_message,
};
pub use envelope::{
    CACHE_ENVELOPE_FORMAT_VERSION, CacheEnvelope, CacheEnvelopeError, CacheEnvelopeFreshness,
    DEFAULT_MAX_CACHE_ENVELOPE_BYTES,
};
pub use error::CacheError;
pub use event_dedupe::{
    BoundedCacheEventDedupe, CacheEventDedupeDecision, CacheEventDedupeError,
    CacheEventDedupeStats, DEFAULT_CACHE_EVENT_DEDUPE_TTL, DEFAULT_MAX_CACHE_EVENT_DEDUPE_ENTRIES,
};
#[cfg(feature = "redis-cache")]
pub use fallback::DegradationAwareFallbackBackend;
pub use generation::{
    CacheGenerationError, CacheGenerationSource, CacheGenerationStats, CacheNamespaceGeneration,
    CacheNamespaceGenerationStore, DEFAULT_MAX_LOCAL_GENERATION_SNAPSHOTS,
};
pub use invalidation::{
    CacheInvalidationObservation, CacheInvalidationPayloadError, VersionedCacheInvalidation,
};
pub use invalidation_consumer::{
    DurableCacheInvalidationConsumer, DurableInvalidationConsumerStats,
    DurableInvalidationDecision, format_durable_invalidation_prometheus_metrics,
};
pub use invalidation_processor::{
    DurableInvalidationProcessError, DurableInvalidationProcessOutcome,
};
pub use key::{
    CacheKeyBuilder, CacheKeyError, MAX_CACHE_IDENTITY_BYTES, MAX_CACHE_KEY_DYNAMIC_COMPONENTS,
    MAX_CACHE_KEY_INPUT_BYTES,
};
pub use lease::{CacheLeaseError, CacheLeaseOptions, CacheLeaseOutcome, DistributedCacheLease};
pub use negative::{
    DEFAULT_MAX_NEGATIVE_CACHE_BYTES, NegativeCacheEntry, NegativeCacheHit, NegativeCachePolicy,
    NegativeCachePolicyError,
};
pub use observability::{
    format_cache_generation_prometheus_metrics, format_cache_refresh_prometheus_metrics,
};
pub use policy::{CacheLoadPolicy, CachePolicyError, CacheTtlPolicy};
pub use redis_status::{RedisCacheStatus, format_redis_cache_status_prometheus_metrics};
pub use refresh::{
    CacheRefreshClock, CacheRefreshCoordinator, CacheRefreshCoordinatorError,
    CacheRefreshLoadOptions,
    CacheRefreshSchedule, CacheRefreshStats, MAX_CACHE_REFRESH_KEY_BYTES,
    StaleWhileRevalidateResult,
};
pub use rustok_core::CacheCompareAndSetOutcome;
pub use service::{
    CacheBackendOptions, CacheHealthReport, CacheInvalidationError, CacheInvalidationMessage,
    CacheInvalidationMessageError, CacheInvalidationOutcome, CacheInvalidationService,
    CacheInvalidationStats, CacheLoadResult, CacheLoadSource, CacheService,
    DEFAULT_MAX_IN_FLIGHT_CACHE_LOADS, DEFAULT_MAX_SHARED_CACHE_BACKENDS,
    LocalCacheInvalidationSubscription, MAX_CACHE_INVALIDATION_CHANNEL_BYTES,
    MAX_CACHE_INVALIDATION_KEY_BYTES, MAX_CACHE_LOAD_KEY_BYTES, MIN_CACHE_BACKEND_TTL,
    format_cache_service_prometheus_metrics,
};
pub use tenant_generation_observability::{
    TenantGenerationListenerMetrics, format_tenant_generation_listener_prometheus_metrics,
    record_tenant_generation_listener_metrics,
};
pub use typed::{TypedCacheLoadOptions, TypedCacheLoadResult};

