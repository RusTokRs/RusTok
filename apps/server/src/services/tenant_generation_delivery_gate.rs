use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_cache::CacheService;
use rustok_core::events::{EventEnvelope, EventTransport, ReliabilityLevel};
use rustok_core::{Error, Result};

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
#[derive(Clone)]
pub struct TenantGenerationDeliveryGate {
    inner: Arc<dyn EventTransport>,
    ctx: ServerRuntimeContext,
    cache: CacheService,
}

impl TenantGenerationDeliveryGate {
    pub fn new(
        inner: Arc<dyn EventTransport>,
        ctx: ServerRuntimeContext,
        cache: CacheService,
    ) -> Self {
        Self { inner, ctx, cache }
    }

    async fn ensure_local_listener_ready(&self) -> Result<()> {
        if self.cache.redis_configuration_present() {
            return Ok(());
        }

        let snapshot = tenant_cache_generation_listener_snapshot(&self.ctx).await;
        if snapshot.status == TenantCacheGenerationListenerStatus::Healthy && snapshot.local_ready {
            return Ok(());
        }

        Err(Error::Cache(
            snapshot.last_error.unwrap_or_else(|| {
                "canonical tenant cache generation listener is not ready".to_string()
            }),
        ))
    }
}

#[async_trait]
impl EventTransport for TenantGenerationDeliveryGate {
    async fn publish(&self, envelope: EventEnvelope) -> Result<()> {
        self.ensure_local_listener_ready().await?;
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
    use super::*;
    use rustok_core::events::MemoryTransport;
    use rustok_events::DomainEvent;
    use uuid::Uuid;

    async fn context() -> ServerRuntimeContext {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        ServerRuntimeContext::new(db, crate::common::settings::RustokSettings::default())
    }

    fn tenant_event(id: u128) -> EventEnvelope {
        let tenant_id = Uuid::from_u128(id);
        EventEnvelope::new(
            tenant_id,
            None,
            DomainEvent::TenantUpdated { tenant_id },
        )
    }

    #[tokio::test]
    async fn unrelated_cache_subscriber_cannot_satisfy_the_tenant_listener_gate() {
        let cache = CacheService::from_url(None);
        let _unrelated = cache
            .invalidations()
            .subscribe_local_channel("unrelated.cache.channel");
        let downstream = MemoryTransport::with_capacity(8);
        let mut receiver = downstream.subscribe();
        let gate = TenantGenerationDeliveryGate::new(
            Arc::new(downstream),
            context().await,
            cache,
        );

        assert!(gate.publish(tenant_event(1)).await.is_err());
        assert!(tokio::time::timeout(std::time::Duration::from_millis(10), receiver.recv())
            .await
            .is_err());
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
        let gate = TenantGenerationDeliveryGate::new(
            Arc::new(downstream),
            ctx,
            cache,
        );
        let envelope = tenant_event(2);

        gate.publish(envelope.clone()).await.unwrap();
        assert_eq!(receiver.recv().await.unwrap().id, envelope.id);
    }
}
