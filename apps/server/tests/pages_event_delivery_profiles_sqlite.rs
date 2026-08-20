#![cfg(feature = "mod-pages")]

use std::error::Error as StdError;
use std::sync::Arc;
use std::time::Duration;

use rustok_cache::CacheService;
use rustok_core::events::{EventHandler, ReliabilityLevel};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_migrations::SqliteTestMigrator as Migrator;
use rustok_outbox::SysEvents;
use rustok_outbox::entity::SysEventStatus;
use rustok_pages::{
    PAGES_CACHE_ENTITY_KIND, PageCacheGenerationSnapshot, PageCacheInvalidationEventHandler,
    PagesCacheInvalidationRuntime, PagesCacheReadPort,
};
use rustok_server::common::settings::{EventDeliveryProfile, RustokSettings};
use rustok_server::services::event_transport_factory::{EventRuntime, build_event_runtime};
use rustok_server::services::pages_cache_invalidation::ServerPagesCachePort;
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm::EntityTrait;
use tokio::sync::broadcast::error::TryRecvError;
use uuid::Uuid;

const LISTENER_TIMEOUT: Duration = Duration::from_millis(50);

type TestResult<T> = Result<T, Box<dyn StdError + Send + Sync>>;

struct ProfileFixture {
    ctx: ServerRuntimeContext,
    cache: CacheService,
    runtime: EventRuntime,
}

impl ProfileFixture {
    async fn build(profile: EventDeliveryProfile) -> TestResult<Self> {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let mut settings = RustokSettings::default();
        settings.events.delivery_profile = profile;
        settings.events.channel_capacity = 8;
        settings.events.relay_batch_size = 1;
        settings.events.relay_max_concurrency = 1;
        settings.events.relay_interval_ms = 1;
        settings.events.relay_retry_policy.base_backoff_ms = 0;
        settings.events.relay_retry_policy.max_backoff_ms = 0;

        let ctx = ServerRuntimeContext::new(db, settings);
        let cache = CacheService::from_url(None);
        ctx.shared_insert(cache.clone());
        let runtime = build_event_runtime(&ctx).await?;
        Ok(Self {
            ctx,
            cache,
            runtime,
        })
    }

    fn page_published(&self) -> EventEnvelope {
        EventEnvelope::new(
            Uuid::new_v4(),
            None,
            DomainEvent::NodePublished {
                node_id: Uuid::new_v4(),
                kind: PAGES_CACHE_ENTITY_KIND.to_string(),
            },
        )
    }

    fn pages_port(&self) -> Arc<ServerPagesCachePort> {
        Arc::new(ServerPagesCachePort::new(&self.cache))
    }

    async fn generations(&self, tenant_id: Uuid) -> TestResult<PageCacheGenerationSnapshot> {
        Ok(self.pages_port().generation_snapshot(tenant_id).await?)
    }

    async fn invoke_ordinary_pages_listener(&self, envelope: &EventEnvelope) -> TestResult<()> {
        PageCacheInvalidationEventHandler::new(PagesCacheInvalidationRuntime::new(
            self.pages_port(),
        ))
        .handle(envelope)
        .await?;
        Ok(())
    }
}

#[tokio::test]
async fn outbox_profile_defers_rotation_and_listener_delivery_until_relay() -> TestResult<()> {
    let fixture = ProfileFixture::build(EventDeliveryProfile::Outbox).await?;
    assert_eq!(
        fixture.runtime.delivery_profile,
        EventDeliveryProfile::Outbox
    );
    assert_eq!(
        fixture.runtime.transport.reliability_level(),
        ReliabilityLevel::Outbox
    );
    let relay = fixture
        .runtime
        .relay_config
        .as_ref()
        .ok_or_else(|| std::io::Error::other("Outbox runtime is missing its relay"))?
        .relay
        .clone();

    let mut listener = fixture.runtime.listener_bus.subscribe();
    let envelope = fixture.page_published();
    fixture.runtime.transport.publish(envelope.clone()).await?;

    let pending = SysEvents::find_by_id(envelope.id)
        .one(fixture.ctx.db())
        .await?
        .ok_or_else(|| std::io::Error::other("Outbox publish did not persist an event"))?;
    assert_eq!(pending.status, SysEventStatus::Pending);
    assert_eq!(pending.retry_count, 0);
    assert!(pending.dispatched_at.is_none());
    assert_eq!(
        fixture.generations(envelope.tenant_id).await?,
        PageCacheGenerationSnapshot::default()
    );
    assert!(matches!(listener.try_recv(), Err(TryRecvError::Empty)));

    assert_eq!(relay.process_pending_once(Some(1)).await?, 1);

    let delivered = tokio::time::timeout(LISTENER_TIMEOUT, listener.recv()).await??;
    assert_eq!(delivered.id, envelope.id);
    assert_eq!(delivered.correlation_id, envelope.correlation_id);
    assert_eq!(
        fixture.generations(envelope.tenant_id).await?,
        PageCacheGenerationSnapshot::new(1, 1, 1)
    );

    let dispatched = SysEvents::find_by_id(envelope.id)
        .one(fixture.ctx.db())
        .await?
        .ok_or_else(|| std::io::Error::other("relayed Outbox event disappeared"))?;
    assert_eq!(dispatched.status, SysEventStatus::Dispatched);
    assert_eq!(dispatched.retry_count, 0);
    assert!(dispatched.dispatched_at.is_some());
    assert!(dispatched.last_error.is_none());
    assert!(dispatched.next_attempt_at.is_none());
    assert!(dispatched.claimed_by.is_none());
    assert!(dispatched.claimed_at.is_none());

    fixture.invoke_ordinary_pages_listener(&delivered).await?;
    assert_eq!(
        fixture.generations(envelope.tenant_id).await?,
        PageCacheGenerationSnapshot::new(1, 1, 1)
    );
    Ok(())
}
