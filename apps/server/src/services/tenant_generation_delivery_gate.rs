use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_cache::CacheService;
use rustok_core::events::{EventEnvelope, EventTransport, ReliabilityLevel};
use rustok_core::{Error, Result};

#[cfg(feature = "mod-pages")]
use rustok_core::events::EventHandler;
#[cfg(feature = "mod-pages")]
use rustok_pages::{PageCacheInvalidationEventHandler, PagesCacheInvalidationRuntime};

#[cfg(feature = "mod-pages")]
use crate::services::pages_cache_invalidation::ServerPagesCachePort;
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::tenant_cache_generation::tenant_cache_generation_listener_snapshot;
use crate::services::tenant_cache_generation_status::TenantCacheGenerationListenerStatus;

/// Prevent local-only event delivery from treating an unrelated cache invalidation subscriber as
/// the canonical tenant generation listener.
///
/// The invalidation transport exposes a transport-wide receiver count that may include receivers
/// for other channels. This gate uses the context-owned generation listener state immediately
/// before downstream event delivery. A retry can therefore resume after the listener recovers
/// without rotating the same event generation again.
///
/// With the Pages module enabled, the same delivery gate also runs the Pages cache invalidation
/// handler synchronously before downstream transport acceptance. `ServerPagesCachePort` shares a
/// bounded event-id dedupe across the gate and the asynchronous module listener, so relay retries
/// and later listener delivery cannot rotate the same Pages event twice in one process.
#[derive(Clone)]
pub struct TenantGenerationDeliveryGate {
    inner: Arc<dyn EventTransport>,
    ctx: ServerRuntimeContext,
    cache: CacheService,
    #[cfg(feature = "mod-pages")]
    pages_handler: PageCacheInvalidationEventHandler,
}

impl TenantGenerationDeliveryGate {
    pub fn new(
        inner: Arc<dyn EventTransport>,
        ctx: ServerRuntimeContext,
        cache: CacheService,
    ) -> Self {
        #[cfg(feature = "mod-pages")]
        let pages_handler = {
            let provider = Arc::new(ServerPagesCachePort::new(&cache));
            PageCacheInvalidationEventHandler::new(PagesCacheInvalidationRuntime::new(provider))
        };

        Self {
            inner,
            ctx,
            cache,
            #[cfg(feature = "mod-pages")]
            pages_handler,
        }
    }

    async fn ensure_local_listener_ready(&self) -> Result<()> {
        if self.cache.redis_configuration_present() {
            return Ok(());
        }

        let snapshot = tenant_cache_generation_listener_snapshot(&self.ctx).await;
        if snapshot.status == TenantCacheGenerationListenerStatus::Healthy && snapshot.local_ready {
            return Ok(());
        }

        Err(Error::Cache(snapshot.last_error.unwrap_or_else(|| {
            "canonical tenant cache generation listener is not ready".to_string()
        })))
    }
}

#[async_trait]
impl EventTransport for TenantGenerationDeliveryGate {
    async fn publish(&self, envelope: EventEnvelope) -> Result<()> {
        self.ensure_local_listener_ready().await?;
        #[cfg(feature = "mod-pages")]
        if self.pages_handler.handles(&envelope.event) {
            self.pages_handler.handle(&envelope).await?;
        }
        self.inner.publish(envelope).await
    }

    async fn acknowledge(&self, event_id: uuid::Uuid) -> Result<()> {
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
    #[cfg(feature = "mod-pages")]
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use rustok_core::events::MemoryTransport;
    use rustok_events::DomainEvent;
    #[cfg(feature = "mod-pages")]
    use rustok_pages::{PAGES_CACHE_ENTITY_KIND, PageCacheGenerationSnapshot, PagesCacheReadPort};
    use uuid::Uuid;

    async fn context() -> ServerRuntimeContext {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        ServerRuntimeContext::new(db, crate::common::settings::RustokSettings::default())
    }

    fn tenant_event(id: u128) -> EventEnvelope {
        let tenant_id = Uuid::from_u128(id);
        EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id })
    }

    #[cfg(feature = "mod-pages")]
    fn page_published_event() -> EventEnvelope {
        EventEnvelope::new(
            Uuid::new_v4(),
            None,
            DomainEvent::NodePublished {
                node_id: Uuid::new_v4(),
                kind: PAGES_CACHE_ENTITY_KIND.to_string(),
            },
        )
    }

    #[cfg(feature = "mod-pages")]
    #[derive(Default)]
    struct FailOnceTransport {
        attempts: AtomicUsize,
    }

    #[cfg(feature = "mod-pages")]
    #[async_trait]
    impl EventTransport for FailOnceTransport {
        async fn publish(&self, _envelope: EventEnvelope) -> Result<()> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                Err(Error::External(
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
    async fn unrelated_cache_subscriber_cannot_satisfy_the_tenant_listener_gate() {
        let cache = CacheService::from_url(None);
        let _unrelated = cache
            .invalidations()
            .subscribe_local_channel("unrelated.cache.channel");
        let downstream = MemoryTransport::with_capacity(8);
        let mut receiver = downstream.subscribe();
        let gate = TenantGenerationDeliveryGate::new(Arc::new(downstream), context().await, cache);

        assert!(gate.publish(tenant_event(1)).await.is_err());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), receiver.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn canonical_local_listener_allows_downstream_delivery() {
        let cache = CacheService::from_url(None);
        let ctx = context().await;
        crate::services::tenant_cache_generation::start_tenant_cache_generation_listener(
            &ctx,
            cache.clone(),
        )
        .await
        .unwrap();
        let downstream = MemoryTransport::with_capacity(8);
        let mut receiver = downstream.subscribe();
        let gate = TenantGenerationDeliveryGate::new(Arc::new(downstream), ctx, cache);
        let envelope = tenant_event(2);

        gate.publish(envelope.clone()).await.unwrap();
        assert_eq!(receiver.recv().await.unwrap().id, envelope.id);
    }

    #[cfg(feature = "mod-pages")]
    #[tokio::test]
    async fn pages_rotation_precedes_downstream_retry_and_async_listener_is_duplicate_safe() {
        let cache = CacheService::from_url(None);
        let ctx = context().await;
        crate::services::tenant_cache_generation::start_tenant_cache_generation_listener(
            &ctx,
            cache.clone(),
        )
        .await
        .unwrap();
        let gate = TenantGenerationDeliveryGate::new(
            Arc::new(FailOnceTransport::default()),
            ctx,
            cache.clone(),
        );
        let envelope = page_published_event();

        assert!(gate.publish(envelope.clone()).await.is_err());
        gate.publish(envelope.clone()).await.unwrap();

        let listener_provider = Arc::new(ServerPagesCachePort::new(&cache));
        PageCacheInvalidationEventHandler::new(PagesCacheInvalidationRuntime::new(
            listener_provider.clone(),
        ))
        .handle(&envelope)
        .await
        .unwrap();
        assert_eq!(
            listener_provider
                .generation_snapshot(envelope.tenant_id)
                .await
                .unwrap(),
            PageCacheGenerationSnapshot::new(1, 1, 1)
        );
    }
}
