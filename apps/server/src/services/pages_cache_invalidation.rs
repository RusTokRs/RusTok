use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_cache::{BoundedCacheEventDedupe, CacheNamespaceGenerationStore, CacheService};
use rustok_core::events::{EventHandler, EventEnvelope};
use rustok_core::{CacheBackend, EventTransport, ReliabilityLevel};
use rustok_pages::{
    PAGE_CACHE_SCOPES, PAGES_CACHE_NAMESPACE_FORMAT, PAGES_STOREFRONT_CACHE_MAX_CAPACITY,
    PageCacheError, PageCacheGenerationSnapshot, PageCacheInvalidationEventHandler,
    PageCacheInvalidationPort, PageCacheInvalidationReceipt, PageCacheInvalidationRequest,
    PagesCacheInvalidationRuntime, PagesCacheReadPort, PagesCacheReadRuntime,
};
use tokio::sync::OnceCell;
use uuid::Uuid;

use crate::services::server_runtime_context::ServerRuntimeContext;

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
            successful_invalidations: Arc::new(BoundedCacheEventDedupe::default()),
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
        let _event_guard = self
            .successful_invalidations
            .serialize_event(request.event_id)
            .await;
        if self.successful_invalidations.is_duplicate(request.event_id) {
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
        let _ = self.successful_invalidations.observe(request.event_id);
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

#[derive(Clone)]
pub struct ServerPagesCacheRuntime {
    invalidation: PagesCacheInvalidationRuntime,
    read: PagesCacheReadRuntime,
}

impl ServerPagesCacheRuntime {
    pub fn new(cache: &CacheService) -> Self {
        let provider = Arc::new(ServerPagesCachePort::new(cache));
        Self {
            invalidation: PagesCacheInvalidationRuntime::new(provider.clone()),
            read: PagesCacheReadRuntime::new(provider),
        }
    }

    pub fn invalidation(&self) -> PagesCacheInvalidationRuntime {
        self.invalidation.clone()
    }

    pub fn read(&self) -> PagesCacheReadRuntime {
        self.read.clone()
    }

    pub fn wrap_relay_target(
        &self,
        inner: Arc<dyn EventTransport>,
    ) -> Arc<dyn EventTransport> {
        Arc::new(PagesCacheInvalidationTransport::new(
            self.invalidation.clone(),
            inner,
        ))
    }
}

pub fn ensure_server_pages_cache_runtime(
    ctx: &ServerRuntimeContext,
    cache: &CacheService,
) -> ServerPagesCacheRuntime {
    if let Some(runtime) = ctx.shared_get::<ServerPagesCacheRuntime>() {
        return runtime;
    }

    let candidate = ServerPagesCacheRuntime::new(cache);
    let _ = ctx.shared_insert_if_absent(candidate.clone());
    ctx.shared_get::<ServerPagesCacheRuntime>()
        .expect("Pages cache runtime must exist after initialization")
}

#[derive(Clone)]
struct PagesCacheInvalidationTransport {
    handler: PageCacheInvalidationEventHandler,
    inner: Arc<dyn EventTransport>,
}

impl PagesCacheInvalidationTransport {
    fn new(runtime: PagesCacheInvalidationRuntime, inner: Arc<dyn EventTransport>) -> Self {
        Self {
            handler: PageCacheInvalidationEventHandler::new(runtime),
            inner,
        }
    }
}

#[async_trait]
impl EventTransport for PagesCacheInvalidationTransport {
    async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
        if self.handler.handles(&envelope.event) {
            self.handler.handle(&envelope).await?;
        }
        self.inner.publish(envelope).await
    }

    async fn acknowledge(&self, event_id: Uuid) -> rustok_core::Result<()> {
        self.inner.acknowledge(event_id).await
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        self.inner.reliability_level()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rustok_events::{DomainEvent, EventEnvelope};
    use rustok_pages::{
        PAGES_CACHE_ENTITY_KIND, PageCacheInvalidationCause, PageCacheInvalidationPort,
        PageCacheInvalidationRequest, PageCacheScope, PagesCacheReadPort,
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

    fn published_envelope() -> EventEnvelope {
        EventEnvelope::new(
            Uuid::from_u128(1),
            None,
            DomainEvent::NodePublished {
                node_id: Uuid::from_u128(2),
                kind: PAGES_CACHE_ENTITY_KIND.to_string(),
            },
        )
    }

    #[derive(Default)]
    struct FailOnceTransport {
        attempts: AtomicUsize,
    }

    #[async_trait]
    impl EventTransport for FailOnceTransport {
        async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                Err(rustok_core::Error::External(
                    "synthetic downstream rejection".to_string(),
                ))
            } else {
                Ok(())
            }
        }

        fn reliability_level(&self) -> ReliabilityLevel {
            ReliabilityLevel::Outbox
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
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
        let first = port
            .invalidate(request(PageCacheInvalidationCause::Published))
            .await
            .unwrap();
        let duplicate = port
            .invalidate(request(PageCacheInvalidationCause::Published))
            .await
            .unwrap();
        assert_eq!(duplicate, first);
        assert_eq!(
            port.generation_snapshot(Uuid::from_u128(1)).await.unwrap(),
            PageCacheGenerationSnapshot::new(1, 1, 1)
        );
    }

    #[tokio::test]
    async fn relay_retry_and_async_listener_share_one_successful_rotation() {
        let cache = local_only_cache();
        let runtime = ServerPagesCacheRuntime::new(&cache);
        let target = runtime.wrap_relay_target(Arc::new(FailOnceTransport::default()));
        let envelope = published_envelope();

        assert!(target.publish(envelope.clone()).await.is_err());
        assert_eq!(
            runtime
                .read()
                .generation_snapshot(envelope.tenant_id)
                .await
                .unwrap(),
            PageCacheGenerationSnapshot::new(1, 1, 1)
        );

        target.publish(envelope.clone()).await.unwrap();
        PageCacheInvalidationEventHandler::new(runtime.invalidation())
            .handle(&envelope)
            .await
            .unwrap();
        assert_eq!(
            runtime
                .read()
                .generation_snapshot(envelope.tenant_id)
                .await
                .unwrap(),
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
        assert_eq!(
            port.get("pages:test").await.unwrap(),
            Some(b"value".to_vec())
        );
    }
}
