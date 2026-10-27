use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::CacheService;

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
        match (
            service.redis_configuration_present(),
            service.redis_client_initialized(),
        ) {
            (false, _) => tracing::info!("CacheModule: running with in-memory cache only"),
            (true, true) => tracing::info!("CacheModule: Redis client initialized"),
            (true, false) => {
                tracing::error!("CacheModule: Redis is configured but client initialization failed")
            }
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
        "Centralised bounded cache capability — Redis lifecycle/timeouts, weighted fallback, atomic compare-and-set, invalidation and anti-stampede loading."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    async fn health(&self) -> HealthStatus {
        let status = self.service.redis_status().await;
        if status.is_healthy() {
            return HealthStatus::Healthy;
        }

        // Readiness degrades on two distinct causes; logging them separately keeps an operator from
        // treating a recovery backlog as a network outage.
        if status.is_connectivity_healthy() {
            tracing::warn!(
                generation_recovery_pending = status.generation_recovery_pending,
                recovery_error = ?status.recovery_error,
                "CacheModule: Redis reachable but cache recovery is incomplete"
            );
        } else {
            tracing::warn!(
                redis_url_present = status.url_present,
                redis_client_initialized = status.client_initialized,
                error = ?status.last_error,
                "CacheModule: Redis lifecycle is degraded"
            );
        }
        HealthStatus::Degraded
    }
}
