use async_trait::async_trait;
use rustok_cache::{CacheNamespaceGenerationStore, CacheService};
use rustok_pages::{
    PageCacheInvalidationError, PageCacheInvalidationPort, PageCacheInvalidationReceipt,
    PageCacheInvalidationRequest,
};

#[derive(Clone)]
pub struct ServerPagesCacheInvalidationPort {
    generations: CacheNamespaceGenerationStore,
}

impl ServerPagesCacheInvalidationPort {
    pub fn new(cache: &CacheService) -> Self {
        Self {
            generations: cache.namespace_generations(),
        }
    }
}

#[async_trait]
impl PageCacheInvalidationPort for ServerPagesCacheInvalidationPort {
    async fn invalidate(
        &self,
        request: PageCacheInvalidationRequest,
    ) -> Result<PageCacheInvalidationReceipt, PageCacheInvalidationError> {
        let mut receipt = PageCacheInvalidationReceipt::new(&request);
        for scope in request.scopes() {
            let namespace = request.namespace(*scope);
            let generation = self
                .generations
                .bump(&namespace)
                .await
                .map_err(|error| {
                    PageCacheInvalidationError::Provider(format!(
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rustok_pages::{
        PageCacheInvalidationCause, PageCacheInvalidationPort, PageCacheInvalidationRequest,
        PageCacheScope,
    };
    use uuid::Uuid;

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

    #[tokio::test]
    async fn published_event_bumps_every_owner_declared_generation() {
        let cache = CacheService::from_url(None);
        let port = ServerPagesCacheInvalidationPort::new(&cache);
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
        let cache = CacheService::from_url(None);
        let port = Arc::new(ServerPagesCacheInvalidationPort::new(&cache));
        let receipt = port
            .invalidate(request(PageCacheInvalidationCause::Updated))
            .await
            .unwrap();
        assert_eq!(receipt.generation(PageCacheScope::Route), Some(1));
        assert_eq!(receipt.generation(PageCacheScope::Page), Some(1));
        assert_eq!(receipt.generation(PageCacheScope::Artifact), None);
    }
}
