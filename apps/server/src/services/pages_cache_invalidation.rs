use std::any::Any;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use async_trait::async_trait;
use rustok_cache::{BoundedCacheEventDedupe, CacheNamespaceGenerationStore, CacheService};
use rustok_core::CacheBackend;
use rustok_pages::{
    PAGE_CACHE_SCOPES, PAGES_CACHE_NAMESPACE_FORMAT, PAGES_STOREFRONT_CACHE_MAX_CAPACITY,
    PageCacheError, PageCacheGenerationSnapshot, PageCacheInvalidationPort,
    PageCacheInvalidationReceipt, PageCacheInvalidationRequest, PagesCacheReadPort,
};
use tokio::sync::OnceCell;
use uuid::Uuid;

// A cache service's generation owner, not its value backend, owns success
// deduplication. Invalidating generations must never initialize value storage.
const MAX_PAGE_CACHE_DEDUPE_OWNERS: usize = 256;
type SharedPagesDedupeRegistry =
    Vec<(Weak<dyn Any + Send + Sync>, Weak<BoundedCacheEventDedupe>)>;
static PAGE_CACHE_DEDUPES: OnceLock<Mutex<SharedPagesDedupeRegistry>> = OnceLock::new();

fn successful_page_invalidations_for(
    generation_owner: &Arc<dyn Any + Send + Sync>,
) -> Arc<BoundedCacheEventDedupe> {
    let registry = PAGE_CACHE_DEDUPES.get_or_init(|| Mutex::new(Vec::new()));
    let mut entries = registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    entries.retain(|(owner, dedupe)| owner.strong_count() > 0 && dedupe.strong_count() > 0);
    for (owner, dedupe) in entries.iter() {
        if let (Some(registered_owner), Some(registered_dedupe)) =
            (owner.upgrade(), dedupe.upgrade())
        {
            if Arc::ptr_eq(&registered_owner, generation_owner) {
                return registered_dedupe;
            }
        }
    }

    let dedupe = Arc::new(BoundedCacheEventDedupe::default());
    if entries.len() < MAX_PAGE_CACHE_DEDUPE_OWNERS {
        entries.push((Arc::downgrade(generation_owner), Arc::downgrade(&dedupe)));
    } else {
        // Saturation may rotate an extra generation, but never skip one.
        tracing::warn!("Pages cache dedupe registry reached its live owner capacity");
    }
    dedupe
}

#[derive(Clone)]
pub struct ServerPagesCachePort {
    cache: CacheService,
    generations: CacheNamespaceGenerationStore,
    backend: Arc<OnceCell<Arc<dyn CacheBackend>>>,
    successful_invalidations: Arc<BoundedCacheEventDedupe>,
}

impl ServerPagesCachePort {
    pub fn new(cache: &CacheService) -> Self {
        Self {
            cache: cache.clone(),
            generations: cache.namespace_generations(),
            backend: Arc::new(OnceCell::new()),
            successful_invalidations: successful_page_invalidations_for(
                &cache.generation_store_identity(),
            ),
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

    async fn current_receipt(
        &self,
        request: &PageCacheInvalidationRequest,
    ) -> Result<PageCacheInvalidationReceipt, PageCacheError> {
        let mut receipt = PageCacheInvalidationReceipt::new(request);
        for scope in request.scopes() {
            let namespace = request.namespace(*scope);
            let generation = self.generations.read(&namespace).await.map_err(|error| {
                PageCacheError::Provider(format!(
                    "unable to read duplicate {} namespace `{namespace}` for tenant {} and page {}: {error}",
                    scope.as_str(),
                    request.tenant_id,
                    request.page_id,
                ))
            })?;
            receipt.record(*scope, generation.value());
        }
        receipt.validate_for(request)?;
        Ok(receipt)
    }
}

#[async_trait]
impl PageCacheInvalidationPort for ServerPagesCachePort {
    async fn invalidate(
        &self,
        request: PageCacheInvalidationRequest,
    ) -> Result<PageCacheInvalidationReceipt, PageCacheError> {
        let successful_invalidations = Arc::clone(&self.successful_invalidations);
        let _event_guard = successful_invalidations.serialize_event(request.event_id).await;
        if successful_invalidations.is_duplicate(request.event_id) {
            tracing::debug!(
                event_id = %request.event_id,
                correlation_id = %request.correlation_id,
                tenant_id = %request.tenant_id,
                page_id = %request.page_id,
                cause = request.cause.as_str(),
                "Pages cache invalidation already completed for event"
            );
            return self.current_receipt(&request).await;
        }

        let mut receipt = PageCacheInvalidationReceipt::new(&request);
        for scope in request.scopes() {
            let namespace = request.namespace(*scope);
            let generation = self.generations.bump(&namespace).await.map_err(|error| {
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
        // INVARIANT: event dedupe observation after successful cache invalidation.
        successful_invalidations.observe(request.event_id);
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

    async fn put(&self, key: String, value: Vec<u8>, ttl: Duration) -> Result<(), PageCacheError> {
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
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
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
    async fn duplicate_event_returns_current_receipt_without_second_rotation() {
        let cache = local_only_cache();
        let port = ServerPagesCachePort::new(&cache);
        let request = request(PageCacheInvalidationCause::Published);
        let first = port.invalidate(request.clone()).await.unwrap();
        let duplicate = ServerPagesCachePort::new(&cache)
            .invalidate(request.clone())
            .await
            .unwrap();
        assert_eq!(duplicate, first);
        assert_eq!(
            port.generation_snapshot(request.tenant_id).await.unwrap(),
            PageCacheGenerationSnapshot::new(1, 1, 1)
        );
    }

    #[tokio::test]
    async fn invalidation_does_not_construct_the_cache_value_backend() {
        let cache = local_only_cache();
        let port = ServerPagesCachePort::new(&cache);
        assert_eq!(cache.shared_backend_construction_count(), 0);
        port.invalidate(request(PageCacheInvalidationCause::Updated))
            .await
            .unwrap();
        assert_eq!(
            cache.shared_backend_construction_count(),
            0,
            "rotating generations must not initialize value storage"
        );
    }

    #[tokio::test]
    async fn concurrent_ports_from_one_cache_service_rotate_each_event_only_once() {
        let cache = local_only_cache();
        let first = ServerPagesCachePort::new(&cache);
        let second = ServerPagesCachePort::new(&cache.clone());
        let event = request(PageCacheInvalidationCause::Published);
        let (first_receipt, second_receipt) =
            tokio::join!(first.invalidate(event.clone()), second.invalidate(event.clone()));
        assert_eq!(first_receipt.unwrap(), second_receipt.unwrap());
        assert_eq!(
            first.generation_snapshot(event.tenant_id).await.unwrap(),
            PageCacheGenerationSnapshot::new(1, 1, 1)
        );
    }

    #[tokio::test]
    async fn independent_cache_services_do_not_suppress_each_others_invalidation() {
        let first_cache = local_only_cache();
        let second_cache = local_only_cache();
        let first = ServerPagesCachePort::new(&first_cache);
        let second = ServerPagesCachePort::new(&second_cache);
        let event = request(PageCacheInvalidationCause::Published);

        let first_receipt = first.invalidate(event.clone()).await.unwrap();
        let second_receipt = second.invalidate(event.clone()).await.unwrap();
        assert_eq!(first_receipt.route_generation, Some(1));
        assert_eq!(second_receipt.route_generation, Some(1));
        assert_eq!(
            second.generation_snapshot(event.tenant_id).await.unwrap(),
            PageCacheGenerationSnapshot::new(1, 1, 1),
            "another generation-store owner must not be treated as already invalidated"
        );

        // Cloned ports and separately constructed ports over the same service
        // still share the optimization and must not rotate twice.
        let duplicate = ServerPagesCachePort::new(&second_cache)
            .invalidate(event.clone())
            .await
            .unwrap();
        assert_eq!(duplicate, second_receipt);
        assert_eq!(
            second.generation_snapshot(event.tenant_id).await.unwrap(),
            PageCacheGenerationSnapshot::new(1, 1, 1)
        );
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
        let tenant_id = Uuid::new_v4();
        assert_eq!(
            port.generation_snapshot(tenant_id).await.unwrap(),
            PageCacheGenerationSnapshot::default()
        );
        port.put(
            "pages:test".to_string(),
            b"value".to_vec(),
            Duration::from_secs(60),
        )
        .await
        .unwrap();
        assert_eq!(
            port.get("pages:test").await.unwrap(),
            Some(b"value".to_vec())
        );
    }
}
