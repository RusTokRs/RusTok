use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_cache::{BoundedCacheEventDedupe, CacheService};
use rustok_core::events::{EventEnvelope, EventTransport, ReliabilityLevel};
use rustok_core::{Error, Result};
use rustok_events::{ContractEventEnvelope, ContractEventPayload};

#[cfg(feature = "mod-pages")]
use rustok_core::events::EventHandler;
#[cfg(feature = "mod-pages")]
use rustok_pages::{PageCacheInvalidationEventHandler, PagesCacheInvalidationRuntime};

#[cfg(feature = "mod-pages")]
use crate::services::pages_cache_invalidation::ServerPagesCachePort;
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::tenant_cache_generation::tenant_cache_generation_listener_snapshot;
use crate::services::tenant_cache_generation_status::{
    TenantCacheGenerationListenerSnapshot, TenantCacheGenerationListenerStatus,
};

fn tenant_generation_listener_ready(
    redis_configured: bool,
    snapshot: &TenantCacheGenerationListenerSnapshot,
) -> bool {
    if redis_configured {
        snapshot.redis_required
            && snapshot.status == TenantCacheGenerationListenerStatus::Healthy
            && snapshot.subscriber_ready
            && snapshot.reconciliation_healthy
    } else {
        !snapshot.redis_required
            && snapshot.status == TenantCacheGenerationListenerStatus::Healthy
            && snapshot.local_ready
    }
}

/// Prevent event delivery until the canonical tenant generation listener is ready.
///
/// The invalidation transport exposes a transport-wide receiver count that may include receivers
/// for other channels. This gate uses the context-owned listener state immediately before
/// downstream delivery: the local effective-policy cache is fenced synchronously, local-only mode
/// requires its canonical local listener, and Redis-backed mode requires the Redis subscriber and
/// durable reconciliation to be healthy. A retry can therefore resume after the listener recovers
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
    policy_cache_invalidations: Arc<BoundedCacheEventDedupe>,
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
            policy_cache_invalidations: Arc::new(BoundedCacheEventDedupe::default()),
            #[cfg(feature = "mod-pages")]
            pages_handler,
        }
    }

    async fn ensure_listener_ready(&self) -> Result<()> {
        let redis_configured = self.cache.redis_configuration_present();
        let snapshot = tenant_cache_generation_listener_snapshot(&self.ctx).await;
        if tenant_generation_listener_ready(redis_configured, &snapshot) {
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
        {
            let _event_guard = self
                .policy_cache_invalidations
                .serialize_event(envelope.id)
                .await;
            if !self.policy_cache_invalidations.is_duplicate(envelope.id)
                && crate::services::tenant_cache_generation::invalidate_effective_policy_cache_for_event(
                    &self.ctx,
                    &envelope,
                )
            {
                self.policy_cache_invalidations.observe(envelope.id);
            }
        }
        #[cfg(feature = "mod-pages")]
        if self.pages_handler.handles(&envelope.event) {
            self.pages_handler.handle(&envelope).await?;
        }
        self.ensure_listener_ready().await?;
        self.inner.publish(envelope).await
    }

    async fn publish_contract(&self, envelope: ContractEventEnvelope) -> Result<()> {
        if matches!(
            envelope
                .payload()
                .map_err(|error| Error::Validation(error.to_string()))?,
            ContractEventPayload::Root(_)
        ) {
            let root = envelope
                .into_root_envelope()
                .map_err(|error| Error::Validation(error.to_string()))?;
            return self.publish(root).await;
        }
        // This wrapper's root-specific work does not reinterpret bounded-family facts.
        self.inner.publish_contract(envelope).await
    }

    async fn acknowledge(&self, event_id: uuid::Uuid) -> Result<()> {
        self.inner.acknowledge(event_id).await
    }

    // Transactional appends are storage-only; listener gating stays a
    // publish-time concern of the relay path above.
    fn supports_transactional_writes(&self) -> bool {
        self.inner.supports_transactional_writes()
    }

    async fn write_root_in_transaction(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        envelope: EventEnvelope,
    ) -> Result<()> {
        self.inner
            .write_root_in_transaction(transaction, envelope)
            .await
    }

    async fn write_contract_in_transaction(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        envelope: ContractEventEnvelope,
    ) -> Result<()> {
        self.inner
            .write_contract_in_transaction(transaction, envelope)
            .await
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

    use super::*;
    use rustok_core::events::MemoryTransport;
    use rustok_events::DomainEvent;
    use rustok_modules::{ModuleEffectivePolicyCache, PolicyCacheReadStamp};
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

    struct PolicyCacheStampCheckingTransport {
        cache: ModuleEffectivePolicyCache,
        before: PolicyCacheReadStamp,
    }

    #[async_trait]
    impl EventTransport for PolicyCacheStampCheckingTransport {
        async fn publish(&self, _envelope: EventEnvelope) -> Result<()> {
            assert!(
                !self
                    .cache
                    .is_stamp_current(&self.before)
                    .expect("policy cache lock must be healthy"),
                "local effective-policy cache must be invalidated before downstream delivery"
            );
            Ok(())
        }

        fn reliability_level(&self) -> ReliabilityLevel {
            ReliabilityLevel::Outbox
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    struct FailOncePolicyCacheStampCheckingTransport {
        cache: ModuleEffectivePolicyCache,
        before: PolicyCacheReadStamp,
        attempts: AtomicUsize,
    }

    #[async_trait]
    impl EventTransport for FailOncePolicyCacheStampCheckingTransport {
        async fn publish(&self, _envelope: EventEnvelope) -> Result<()> {
            assert!(
                !self
                    .cache
                    .is_stamp_current(&self.before)
                    .expect("policy cache lock must be healthy"),
                "local effective-policy cache must be invalidated before downstream delivery"
            );
            if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(Error::External(
                    "synthetic downstream rejection".to_string(),
                ));
            }
            Ok(())
        }

        fn reliability_level(&self) -> ReliabilityLevel {
            ReliabilityLevel::Outbox
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    fn listener_snapshot(
        redis_required: bool,
        status: TenantCacheGenerationListenerStatus,
        local_ready: bool,
        subscriber_ready: bool,
        reconciliation_healthy: bool,
    ) -> TenantCacheGenerationListenerSnapshot {
        TenantCacheGenerationListenerSnapshot {
            status,
            last_error: None,
            redis_required,
            local_ready,
            subscriber_ready,
            reconciliation_healthy,
        }
    }

    #[test]
    fn local_delivery_requires_the_canonical_local_listener() {
        assert!(!tenant_generation_listener_ready(
            false,
            &listener_snapshot(
                false,
                TenantCacheGenerationListenerStatus::Starting,
                false,
                false,
                true,
            ),
        ));
        assert!(tenant_generation_listener_ready(
            false,
            &listener_snapshot(
                false,
                TenantCacheGenerationListenerStatus::Healthy,
                true,
                false,
                true,
            ),
        ));
    }

    #[test]
    fn redis_delivery_requires_a_healthy_subscriber_and_reconciliation() {
        let starting = listener_snapshot(
            true,
            TenantCacheGenerationListenerStatus::Starting,
            true,
            false,
            true,
        );
        assert!(!tenant_generation_listener_ready(true, &starting));

        let healthy = listener_snapshot(
            true,
            TenantCacheGenerationListenerStatus::Healthy,
            false,
            true,
            true,
        );
        assert!(tenant_generation_listener_ready(true, &healthy));
        assert!(!tenant_generation_listener_ready(false, &healthy));
    }

    #[tokio::test]
    async fn local_effective_policy_cache_is_invalidated_before_downstream_delivery() {
        let cache = CacheService::from_url(None);
        let ctx = context().await;
        let policy_cache = ModuleEffectivePolicyCache::new();
        ctx.shared_insert(policy_cache.clone());
        crate::services::tenant_cache_generation::start_tenant_cache_generation_listener(
            &ctx,
            cache.clone(),
        )
        .await
        .unwrap();

        let before = policy_cache.begin_read().unwrap();
        let downstream = Arc::new(PolicyCacheStampCheckingTransport {
            cache: policy_cache,
            before,
        });
        let gate = TenantGenerationDeliveryGate::new(downstream, ctx, cache);

        gate.publish(tenant_event(42)).await.unwrap();
    }

    #[tokio::test]
    async fn policy_cache_invalidation_is_not_repeated_for_downstream_retries() {
        let cache = CacheService::from_url(None);
        let ctx = context().await;
        let policy_cache = ModuleEffectivePolicyCache::new();
        ctx.shared_insert(policy_cache.clone());
        crate::services::tenant_cache_generation::start_tenant_cache_generation_listener(
            &ctx,
            cache.clone(),
        )
        .await
        .unwrap();

        let before = policy_cache.begin_read().unwrap();
        let downstream = Arc::new(FailOncePolicyCacheStampCheckingTransport {
            cache: policy_cache.clone(),
            before,
            attempts: AtomicUsize::new(0),
        });
        let gate = TenantGenerationDeliveryGate::new(downstream, ctx, cache);
        let envelope = tenant_event(43);

        assert!(gate.publish(envelope.clone()).await.is_err());
        let after_first_attempt = policy_cache.begin_read().unwrap();
        gate.publish(envelope).await.unwrap();
        assert!(policy_cache.is_stamp_current(&after_first_attempt).unwrap());
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
