use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_cache::{CacheNamespaceGenerationStore, CacheService};
use rustok_core::CacheBackend;
use rustok_pages::{
    PAGE_CACHE_SCOPES, PAGES_CACHE_NAMESPACE_FORMAT, PAGES_STOREFRONT_CACHE_MAX_CAPACITY,
    PageCacheError, PageCacheGenerationSnapshot, PageCacheInvalidationPort,
    PageCacheInvalidationReceipt, PageCacheInvalidationRequest, PagesCacheReadPort,
};
use tokio::sync::OnceCell;
use uuid::Uuid;

#[derive(Clone)]
pub struct ServerPagesCachePort {
    cache: CacheService,
    generations: CacheNamespaceGenerationStore,
    backend: Arc<OnceCell<Arc<dyn CacheBackend>>>,
}

impl ServerPagesCachePort {
    pub fn new(cache: &CacheService) -> Self {
        Self {
            cache: cache.clone(),
            generations: cache.namespace_generations(),
            backend: Arc::new(OnceCell::new()),
        }
    }

    async fn backend(&self) -> Arc<dyn CacheBackend> {
        self.backend
            .get_or_init(|| async {
                self.cache
                    .backend(
                        PAGES_CACHE_NAMESPACE_FORMAT,
                        Duration::from_secs(rustok_pages::PAGES_STOREFRONT_CACHE_TTL_SECS),
                        PAGES_STOREFRONT_CACHE_MAX_CAPACITY,
                    )
                    .await
            })
            .await
            .clone()
    }
}

#[async_trait]
impl PageCacheInvalidationPort for ServerPagesCachePort {
    async fn invalidate(
        &self,
        request: PageCacheInvalidationRequest,
    ) -> Result<PageCacheInvalidationReceipt, PageCacheError> {
        let mut receipt = PageCacheInvalidationReceipt::new(&request);
        for scope in request.scopes() {
            let namespace = request.namespace(*scope);
            let generation = self
                .generations
                .bump(&namespace)
                .await
                .map_err(|error| {
                    PageCacheError::Provider(format!(
                        "unable to bump {} namespace `{namespace}` for tenant {} and page {}: {error}",
                        scope.as_str(),
                        request.tenant_id,
                        request.page_id,
                    ))
                })?;
            receipt.record(*scope, generation.value());
        }
        receipt.validate_for(&request)?;
        Ok(receipt)
    }
}

#[async_trait]
impl PagesCacheReadPort for ServerPagesCachePort {
    async fn generation_snapshot(
        &self,
        tenant_id: Uuid,
    ) -> Result<PageCacheGenerationSnapshot, PageCacheError> {
        let mut snapshot = PageCacheGenerationSnapshot::default();
        for scope in PAGE_CACHE_SCOPES {
            let namespace = rustok_pages::page_cache_namespace(scope, tenant_id);
            let generation = self.generations.read(&namespace).await.map_err(|error| {
                PageCacheError::Provider(format!(
                    "unable to read {} namespace `{namespace}` for tenant {tenant_id}: {error}",
                    scope.as_str(),
                ))
            })?;
            snapshot.record(scope, generation.value());
        }
        Ok(snapshot)
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PageCacheError> {
        self.backend()
            .await
            .get(key)
            .await
            .map_err(|error| PageCacheError::Provider(error.to_string()))
    }

    async fn put(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Duration,
    ) -> Result<(), PageCacheError> {
        self.backend()
            .await
            .set_with_ttl(key, value, ttl)
            .await
            .map_err(|error| PageCacheError::Provider(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use rustok_pages::{
        PageCacheInvalidationCause, PageCacheInvalidationPort, PageCacheInvalidationRequest,
        PageCacheScope, PagesCacheReadPort,
    };

    use super::*;

    fn request(cause: PageCacheInvalidationCause) -> PageCacheInvalidationRequest {
        PageCacheInvalidationRequest::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
            Uuid::from_u128(4),
            Some("trace".to_string()),
            cause,
        )
        .unwrap()
    }

    fn local_only_cache() -> CacheService {
        CacheService::from_url(Some("unsupported-cache-scheme://local-only"))
    }

    #[tokio::test]
    async fn published_event_bumps_every_owner_declared_generation() {
        let cache = local_only_cache();
        let port = ServerPagesCachePort::new(&cache);
        let receipt = port
            .invalidate(request(PageCacheInvalidationCause::Published))
            .await
            .unwrap();
        assert_eq!(receipt.generation(PageCacheScope::Route), Some(1));
        assert_eq!(receipt.generation(PageCacheScope::Page), Some(1));
        assert_eq!(receipt.generation(PageCacheScope::Artifact), Some(1));
    }

    #[tokio::test]
    async fn updated_event_does_not_rotate_immutable_artifact_namespace() {
        let cache = local_only_cache();
        let port = ServerPagesCachePort::new(&cache);
        let receipt = port
            .invalidate(request(PageCacheInvalidationCause::Updated))
            .await
            .unwrap();
        assert_eq!(receipt.generation(PageCacheScope::Route), Some(1));
        assert_eq!(receipt.generation(PageCacheScope::Page), Some(1));
        assert_eq!(receipt.generation(PageCacheScope::Artifact), None);
    }

    #[tokio::test]
    async fn read_port_uses_initial_generation_and_round_trips_bytes() {
        let cache = local_only_cache();
        let port = ServerPagesCachePort::new(&cache);
        assert_eq!(
            port.generation_snapshot(Uuid::from_u128(1)).await.unwrap(),
            PageCacheGenerationSnapshot::default()
        );
        port.put(
            "pages:test".to_string(),
            b"value".to_vec(),
            Duration::from_secs(60),
        )
        .await
        .unwrap();
        assert_eq!(port.get("pages:test").await.unwrap(), Some(b"value".to_vec()));
    }
}
