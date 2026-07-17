use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{CacheBackend, CacheCompareAndSetOutcome, CacheStats};

use crate::{
    cache_backend_generation_snapshot, observe_cache_backend_generation,
    CacheNamespaceGenerationStore, CacheService,
};

pub(crate) struct GenerationRecoveryHealthBackend {
    inner: Arc<dyn CacheBackend>,
    prefix: String,
    generations: CacheNamespaceGenerationStore,
    redis_client_initialized: bool,
}

impl CacheService {
    pub(crate) fn wrap_generation_recovery_health(
        &self,
        prefix: &str,
        inner: Arc<dyn CacheBackend>,
    ) -> Arc<dyn CacheBackend> {
        if !self.redis_configuration_present() {
            return inner;
        }

        Arc::new(GenerationRecoveryHealthBackend {
            inner,
            prefix: prefix.to_string(),
            generations: self.namespace_generations(),
            redis_client_initialized: self.redis_client_initialized(),
        })
    }
}

impl GenerationRecoveryHealthBackend {
    async fn ensure_shared_generation(&self) -> rustok_core::Result<()> {
        let snapshot = cache_backend_generation_snapshot(&self.prefix)
            .map_err(|error| rustok_core::Error::Cache(error.to_string()))?;
        if snapshot.trusted {
            return Ok(());
        }
        if !self.redis_client_initialized {
            return Err(rustok_core::Error::Cache(
                "Redis is configured but shared cache generation recovery has no initialized client"
                    .to_string(),
            ));
        }

        let generation = self
            .generations
            .read(&self.prefix)
            .await
            .map_err(|error| rustok_core::Error::Cache(error.to_string()))?;
        observe_cache_backend_generation(&self.prefix, generation.value())
            .map_err(|error| rustok_core::Error::Cache(error.to_string()))?;
        tracing::info!(
            prefix = %self.prefix,
            generation = generation.value(),
            source = ?generation.source(),
            "Recovered trusted shared cache generation"
        );
        Ok(())
    }
}

#[async_trait]
impl CacheBackend for GenerationRecoveryHealthBackend {
    async fn health(&self) -> rustok_core::Result<()> {
        self.inner.health().await?;
        self.ensure_shared_generation().await
    }

    async fn get(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
        self.inner.get(key).await
    }

    async fn set(&self, key: String, value: Vec<u8>) -> rustok_core::Result<()> {
        self.inner.set(key, value).await
    }

    async fn set_with_ttl(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Duration,
    ) -> rustok_core::Result<()> {
        self.inner.set_with_ttl(key, value, ttl).await
    }

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> rustok_core::Result<CacheCompareAndSetOutcome> {
        self.inner.compare_and_set(key, expected, value, ttl).await
    }

    async fn invalidate(&self, key: &str) -> rustok_core::Result<()> {
        self.inner.invalidate(key).await
    }

    fn stats(&self) -> CacheStats {
        self.inner.stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn invalid_redis_configuration_keeps_generation_health_degraded() {
        let service = CacheService::from_url(Some("://invalid-redis-url"));
        let prefix = format!("generation-health:{}", uuid::Uuid::new_v4().simple());
        let backend = service.wrap_generation_recovery_health(
            &prefix,
            service.memory_backend(Duration::from_secs(60), 16),
        );

        assert!(backend.health().await.is_err());
        assert!(!cache_backend_generation_snapshot(&prefix).unwrap().trusted);
    }

    #[tokio::test]
    async fn memory_only_backend_does_not_require_shared_generation() {
        let service = CacheService::from_url(None);
        let prefix = format!("generation-health-memory:{}", uuid::Uuid::new_v4().simple());
        let inner = service.memory_backend(Duration::from_secs(60), 16);
        let backend = service.wrap_generation_recovery_health(&prefix, Arc::clone(&inner));

        assert!(Arc::ptr_eq(&backend, &inner));
        assert!(backend.health().await.is_ok());
    }
}
