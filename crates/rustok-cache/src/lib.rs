mod backend_generation;
mod bounded_invalidation;
mod cas_observability;
mod durable_invalidation;
mod durable_invalidation_service;
mod durable_invalidation_transport;
mod envelope;
mod fallback;
mod generation;
mod invalidation;
mod invalidation_consumer;
mod invalidation_processor;
mod key;
mod key_generation;
mod lease;
mod negative;
mod observability;
mod policy;
mod refresh;
mod service;
mod shared_backend;
mod typed;
mod weighted;

pub use backend_generation::{
    cache_backend_generation_registry_size, cache_backend_generation_snapshot,
    observe_cache_backend_generation, CacheBackendGenerationError,
    CacheBackendGenerationSnapshot, DEFAULT_MAX_CACHE_BACKEND_GENERATIONS,
    MAX_CACHE_BACKEND_PREFIX_BYTES,
};
pub use bounded_invalidation::{
    BoundedCacheInvalidationGapTracker, BoundedInvalidationTrackerError,
    DEFAULT_MAX_TRACKED_INVALIDATION_CHANNELS,
};
pub use cas_observability::{
    format_cache_compare_and_set_prometheus_metrics, observe_cache_compare_and_set,
    CacheCompareAndSetMetrics, CacheCompareAndSetStats,
};
pub use durable_invalidation::{
    DurableCacheInvalidationError, DurableCacheInvalidationRecord,
    DEFAULT_MAX_DURABLE_INVALIDATION_BYTES, DURABLE_CACHE_INVALIDATION_FORMAT_VERSION,
    MAX_DURABLE_INVALIDATION_CAUSE_BYTES, MAX_DURABLE_INVALIDATION_TRACE_ID_BYTES,
};
pub use durable_invalidation_transport::{
    durable_invalidation_from_message, durable_invalidation_to_message,
};
pub use envelope::{
    CacheEnvelope, CacheEnvelopeError, CacheEnvelopeFreshness, CACHE_ENVELOPE_FORMAT_VERSION,
    DEFAULT_MAX_CACHE_ENVELOPE_BYTES,
};
pub use generation::{
    CacheGenerationError, CacheGenerationSource, CacheGenerationStats, CacheNamespaceGeneration,
    CacheNamespaceGenerationStore, DEFAULT_MAX_LOCAL_GENERATION_SNAPSHOTS,
};
pub use invalidation::{
    CacheInvalidationGapTracker, CacheInvalidationObservation, CacheInvalidationPayloadError,
    VersionedCacheInvalidation,
};
pub use invalidation_consumer::{
    format_durable_invalidation_prometheus_metrics, DurableCacheInvalidationConsumer,
    DurableInvalidationConsumerStats, DurableInvalidationDecision,
};
pub use invalidation_processor::{
    DurableInvalidationProcessError, DurableInvalidationProcessOutcome,
};
pub use key::{
    CacheKeyBuilder, CacheKeyError, MAX_CACHE_IDENTITY_BYTES, MAX_CACHE_KEY_DYNAMIC_COMPONENTS,
    MAX_CACHE_KEY_INPUT_BYTES,
};
pub use lease::{
    CacheLeaseError, CacheLeaseOptions, CacheLeaseOutcome, DistributedCacheLease,
};
pub use negative::{
    NegativeCacheEntry, NegativeCacheHit, NegativeCachePolicy, NegativeCachePolicyError,
    DEFAULT_MAX_NEGATIVE_CACHE_BYTES,
};
pub use observability::{
    format_cache_generation_prometheus_metrics, format_cache_refresh_prometheus_metrics,
};
pub use policy::{CacheLoadPolicy, CachePolicyError, CacheTtlPolicy};
pub use refresh::{
    CacheRefreshCoordinator, CacheRefreshCoordinatorError, CacheRefreshSchedule,
    CacheRefreshStats, StaleWhileRevalidateResult, MAX_CACHE_REFRESH_KEY_BYTES,
};
pub use rustok_core::CacheCompareAndSetOutcome;
pub use service::{
    format_cache_service_prometheus_metrics, CacheBackendOptions, CacheHealthReport,
    CacheInvalidationMessage, CacheInvalidationMessageError, CacheInvalidationOutcome,
    CacheInvalidationService, CacheInvalidationStats, CacheLoadResult, CacheLoadSource,
    CacheService, LocalCacheInvalidationSubscription, DEFAULT_MAX_IN_FLIGHT_CACHE_LOADS,
    MAX_CACHE_INVALIDATION_CHANNEL_BYTES, MAX_CACHE_INVALIDATION_KEY_BYTES,
    MAX_CACHE_LOAD_KEY_BYTES,
};
pub use typed::TypedCacheLoadResult;

use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

/// Core cache module — owns Redis connection lifecycle and cache backend factory.
///
/// Other modules obtain cache backends via `CacheService` instead of resolving
/// Redis URLs themselves. This centralises connection management and health checks.
pub struct CacheModule {
    service: CacheService,
}

impl CacheModule {
    pub fn new() -> Self {
        let service = CacheService::from_env();
        if service.has_redis() {
            tracing::info!(url = ?service.redis_url(), "CacheModule: Redis backend available");
        } else {
            tracing::info!("CacheModule: running with in-memory cache only");
        }
        Self { service }
    }

    pub fn service(&self) -> &CacheService {
        &self.service
    }

    pub fn into_service(self) -> CacheService {
        self.service
    }
}

impl Default for CacheModule {
    fn default() -> Self {
        Self::new()
    }
}

impl MigrationSource for CacheModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[async_trait]
impl RusToKModule for CacheModule {
    fn slug(&self) -> &'static str {
        "cache"
    }

    fn name(&self) -> &'static str {
        "Cache"
    }

    fn description(&self) -> &'static str {
        "Centralised cache backend factory — Redis lifecycle, fallback, health checks."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    async fn health(&self) -> HealthStatus {
        let report = self.service.health().await;
        if report.is_healthy() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded
        }
    }
}
