mod service;

pub use service::{
    format_cache_service_prometheus_metrics, CacheBackendOptions, CacheHealthReport,
    CacheInvalidationMessage, CacheInvalidationOutcome, CacheInvalidationService, CacheLoadResult,
    CacheLoadSource, CacheService,
};

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
