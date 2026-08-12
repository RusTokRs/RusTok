#![cfg(feature = "mod-pages")]

use std::any::Any;
use std::error::Error as StdError;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::routing::post;
use leptos::prelude::provide_context;
use leptos_axum::handle_server_fns_with_context;
use rustok_api::{
    ChannelContext, ChannelContextExtension, ChannelResolutionSource, HostRuntimeContext,
    TenantContext, TenantContextExtension,
};
use rustok_cache::CacheService;
use rustok_channel::{
    BindChannelModuleInput, ChannelModule, ChannelResponse, ChannelService, CreateChannelInput,
};
use rustok_core::events::EventHandler;
use rustok_core::{EventTransport, MigrationSource, ReliabilityLevel, SecurityContext};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::entity::{Column as SysEventColumn, SysEventStatus};
use rustok_outbox::{
    OutboxModule, OutboxRelay, OutboxTransport, RelayConfig, SysEvents, TransactionalEventBus,
};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    ReviewedPagePublishRuntimeInput,
};
use rustok_pages::services::PageService;
use rustok_pages::{
    PAGES_CACHE_ENTITY_KIND, PAGES_STOREFRONT_CACHE_TTL_SECS, PageCacheError,
    PageCacheGenerationSnapshot, PageCacheInvalidationEventHandler, PagesCacheInvalidationRuntime,
    PagesCacheReadPort, PagesCacheReadRuntime, PagesModule,
};
use rustok_pages_storefront as _;
use rustok_server::common::settings::RustokSettings;
use rustok_server::services::pages_cache_invalidation::ServerPagesCachePort;
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use rustok_server::services::tenant_cache_generation::start_tenant_cache_generation_listener;
use rustok_server::services::tenant_generation_delivery_gate::TenantGenerationDeliveryGate;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    QueryOrder, Statement,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

const RESPONSE_BODY_LIMIT: usize = 1024 * 1024;
const SERVER_FN_PATH: &str = "/api/fn/pages/storefront-data";

type TestResult<T> = Result<T, Box<dyn StdError + Send + Sync>>;

#[derive(Clone)]
struct ReviewedFixture {
    page_id: Uuid,
    expected_artifact_url: String,
}

#[derive(Default)]
struct ReadState {
    generation_reads: usize,
    get_keys: Vec<String>,
    put_keys: Vec<String>,
    put_ttls: Vec<Duration>,
}

#[derive(Clone)]
struct RecordingReadPort {
    inner: Arc<ServerPagesCachePort>,
    state: Arc<Mutex<ReadState>>,
}

impl RecordingReadPort {
    fn new(inner: Arc<ServerPagesCachePort>) -> Self {
        Self {
            inner,
            state: Arc::new(Mutex::new(ReadState::default())),
        }
    }

    fn snapshot(&self) -> ReadSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        ReadSnapshot {
            generation_reads: state.generation_reads,
            get_keys: state.get_keys.clone(),
            put_keys: state.put_keys.clone(),
            put_ttls: state.put_ttls.clone(),
        }
    }

    async fn raw_get(&self, key: &str) -> Result<Option<Vec<u8>>, PageCacheError> {
        self.inner.get(key).await
    }

    async fn generations(
        &self,
        tenant_id: Uuid,
    ) -> Result<PageCacheGenerationSnapshot, PageCacheError> {
        self.inner.generation_snapshot(tenant_id).await
    }
}

struct ReadSnapshot {
    generation_reads: usize,
    get_keys: Vec<String>,
    put_keys: Vec<String>,
    put_ttls: Vec<Duration>,
}

#[async_trait]
impl PagesCacheReadPort for RecordingReadPort {
    async fn generation_snapshot(
        &self,
        tenant_id: Uuid,
    ) -> Result<PageCacheGenerationSnapshot, PageCacheError> {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.generation_reads += 1;
        }
        self.inner.generation_snapshot(tenant_id).await
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PageCacheError> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_keys
            .push(key.to_string());
        self.inner.get(key).await
    }

    async fn put(&self, key: String, value: Vec<u8>, ttl: Duration) -> Result<(), PageCacheError> {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.put_keys.push(key.clone());
            state.put_ttls.push(ttl);
        }
        self.inner.put(key, value, ttl).await
    }
}

#[derive(Default)]
struct RecordingTransport {
    envelopes: Mutex<Vec<EventEnvelope>>,
}

impl RecordingTransport {
    fn delivered_ids(&self) -> Vec<Uuid> {
        self.envelopes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|envelope| envelope.id)
            .collect()
    }

    fn envelope(&self, event_id: Uuid) -> Option<EventEnvelope> {
        self.envelopes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|envelope| envelope.id == event_id)
            .cloned()
    }
}

#[async_trait]
impl EventTransport for RecordingTransport {
    async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
        self.envelopes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(envelope);
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::Outbox
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone, Copy)]
struct PublicationEvents {
    created_id: Uuid,
    updated_id: Uuid,
    published_id: Uuid,
}

#[tokio::test]
async fn production_relay_gate_rotates_registered_native_route_before_outbox_ack() -> TestResult<()>
{
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let channel_service = ChannelService::new(db.clone());
    let web = create_enabled_channel(&channel_service, tenant_id, "web", "Web").await?;
    let fixture = create_reviewed_published_page(&db, event_bus.clone(), tenant_id).await?;
    let events = publication_events(&db, tenant_id, fixture.page_id).await?;

    let cache = CacheService::from_url(None);
    let runtime_ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    start_tenant_cache_generation_listener(&runtime_ctx, cache.clone()).await?;

    let read_port = RecordingReadPort::new(Arc::new(ServerPagesCachePort::new(&cache)));
    let host = HostRuntimeContext::new(db.clone())
        .with_shared_value(event_bus)
        .with_shared_value(PagesCacheReadRuntime::new(Arc::new(read_port.clone())));
    let app = native_server_fn_router(host);
    let tenant = tenant_context(tenant_id);
    let channel = channel_context(&web);

    let downstream = Arc::new(RecordingTransport::default());
    let target: Arc<dyn EventTransport> = Arc::new(TenantGenerationDeliveryGate::new(
        downstream.clone(),
        runtime_ctx,
        cache.clone(),
    ));
    let relay = OutboxRelay::new(db.clone(), target).with_config(relay_config());

    assert_eq!(relay.process_pending_once(Some(1)).await?, 1);
    assert_eq!(downstream.delivered_ids(), vec![events.created_id]);
    assert_outbox_status(&db, events.created_id, SysEventStatus::Dispatched).await?;
    assert_eq!(
        read_port.generations(tenant_id).await?,
        PageCacheGenerationSnapshot::default()
    );

    assert_eq!(relay.process_pending_once(Some(1)).await?, 1);
    assert_eq!(
        downstream.delivered_ids(),
        vec![events.created_id, events.updated_id]
    );
    assert_outbox_status(&db, events.updated_id, SysEventStatus::Dispatched).await?;
    assert_outbox_status(&db, events.published_id, SysEventStatus::Pending).await?;
    assert_eq!(
        read_port.generations(tenant_id).await?,
        PageCacheGenerationSnapshot::new(1, 1, 0)
    );

    let before_published_delivery = call_storefront(&app, &tenant, &channel).await?;
    assert_eq!(before_published_delivery.status, StatusCode::OK);
    assert!(
        before_published_delivery
            .body
            .contains(&fixture.expected_artifact_url)
    );
    assert!(before_published_delivery.body.contains("fly_artifact_url"));
    let before_rotation = read_port.snapshot();
    assert_eq!(before_rotation.generation_reads, 1);
    assert_eq!(before_rotation.get_keys.len(), 1);
    assert_eq!(before_rotation.put_keys.len(), 1);
    assert_eq!(before_rotation.get_keys[0], before_rotation.put_keys[0]);
    assert_eq!(
        before_rotation.put_ttls,
        vec![Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS)]
    );
    let old_key = before_rotation.put_keys[0].clone();
    assert!(read_port.raw_get(&old_key).await?.is_some());

    assert_eq!(relay.process_pending_once(Some(1)).await?, 1);
    assert_eq!(
        downstream.delivered_ids(),
        vec![events.created_id, events.updated_id, events.published_id]
    );
    assert_outbox_status(&db, events.published_id, SysEventStatus::Dispatched).await?;
    assert_eq!(
        read_port.generations(tenant_id).await?,
        PageCacheGenerationSnapshot::new(2, 2, 1)
    );

    let published_envelope = downstream
        .envelope(events.published_id)
        .ok_or_else(|| std::io::Error::other("downstream NodePublished envelope is missing"))?;
    let listener_provider = Arc::new(ServerPagesCachePort::new(&cache));
    PageCacheInvalidationEventHandler::new(PagesCacheInvalidationRuntime::new(listener_provider))
        .handle(&published_envelope)
        .await?;
    assert_eq!(
        read_port.generations(tenant_id).await?,
        PageCacheGenerationSnapshot::new(2, 2, 1)
    );
    assert!(read_port.raw_get(&old_key).await?.is_some());

    let after_rotation = call_storefront(&app, &tenant, &channel).await?;
    assert_eq!(after_rotation.status, StatusCode::OK);
    assert_eq!(after_rotation.body, before_published_delivery.body);
    let refilled = read_port.snapshot();
    assert_eq!(refilled.generation_reads, 2);
    assert_eq!(refilled.get_keys.len(), 2);
    assert_eq!(refilled.put_keys.len(), 2);
    let new_key = refilled.put_keys[1].clone();
    assert_eq!(refilled.get_keys[1], new_key);
    assert_ne!(new_key, old_key);
    assert!(read_port.raw_get(&old_key).await?.is_some());
    assert!(read_port.raw_get(&new_key).await?.is_some());
    assert_eq!(
        refilled.put_ttls,
        vec![
            Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS),
            Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS),
        ]
    );

    let hit = call_storefront(&app, &tenant, &channel).await?;
    assert_eq!(hit.status, StatusCode::OK);
    assert_eq!(hit.body, after_rotation.body);
    let final_cache = read_port.snapshot();
    assert_eq!(final_cache.generation_reads, 3);
    assert_eq!(final_cache.get_keys.len(), 3);
    assert_eq!(final_cache.get_keys[2], new_key);
    assert_eq!(final_cache.put_keys.len(), 2);

    let metrics = relay.metrics();
    assert_eq!(metrics.success_total, 3);
    assert_eq!(metrics.failure_total, 0);
    assert_eq!(metrics.processed_total, 3);
    Ok(())
}

fn native_server_fn_router(host: HostRuntimeContext) -> Router {
    Router::new().route(
        "/api/fn/{*fn_name}",
        post(move |request| {
            let host = host.clone();
            async move {
                handle_server_fns_with_context(
                    move || {
                        provide_context(host.clone());
                    },
                    request,
                )
                .await
            }
        }),
    )
}

struct ServerFnResponse {
    status: StatusCode,
    body: String,
}

async fn call_storefront(
    app: &Router,
    tenant: &TenantContext,
    channel: &ChannelContext,
) -> TestResult<ServerFnResponse> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(SERVER_FN_PATH)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("X-Tenant-ID", tenant.id.to_string())
        .body(Body::from("page_slug=home&locale=en"))?;
    request
        .extensions_mut()
        .insert(TenantContextExtension(tenant.clone()));
    request
        .extensions_mut()
        .insert(ChannelContextExtension(channel.clone()));

    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), RESPONSE_BODY_LIMIT).await?;
    Ok(ServerFnResponse {
        status,
        body: std::str::from_utf8(&bytes)?.to_string(),
    })
}

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_production_relay_native_route_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)".to_string(),
    ))
    .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenant_modules (\
            id TEXT PRIMARY KEY NOT NULL, \
            tenant_id TEXT NOT NULL, \
            module_slug TEXT NOT NULL, \
            enabled INTEGER NOT NULL, \
            settings TEXT NOT NULL, \
            created_at TEXT NOT NULL, \
            updated_at TEXT NOT NULL\
        )"
        .to_string(),
    ))
    .await?;

    let manager = SchemaManager::new(&db);
    for migration in OutboxModule
        .migrations()
        .into_iter()
        .chain(ChannelModule.migrations())
        .chain(PagesModule.migrations())
    {
        migration.up(&manager).await?;
    }
    Ok(db)
}

async fn create_reviewed_published_page(
    db: &DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant_id: Uuid,
) -> TestResult<ReviewedFixture> {
    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Production relay native route artifact",
                "description": "Production generation gate to registered native route",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Production relay native route artifact"
                }]
            }
        }]
    });
    let service = PageService::new(db.clone(), event_bus);
    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Production relay native route artifact".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: Some("Production relay native route artifact".to_string()),
                    meta_description: Some(
                        "Production generation gate to registered native route".to_string(),
                    ),
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project.clone(),
                }),
                channel_slugs: Some(vec!["web".to_string()]),
                publish: false,
            },
        )
        .await?;
    let body_revision = draft
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("reviewed draft is missing its body"))?
        .updated_at
        .clone();
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "production-relay-native-route",
        json!({ "surface": "native-storefront", "channel": "web" }),
    )?;
    let publish = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: draft.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: body_revision,
                }],
                idempotency_key: "production-relay-native-route-v1".to_string(),
                runtime: ReviewedPagePublishRuntimeInput {
                    format: reviewed.format,
                    scenario_id: reviewed.scenario_id,
                    context: reviewed.context,
                    review_hash: reviewed.review_hash,
                },
            },
        )
        .await?;
    assert_eq!(publish.page_id, draft.id);
    assert!(!publish.replayed);
    assert_eq!(publish.review_hash.len(), 64);
    assert_eq!(publish.sanitized_set_hash.len(), 64);
    assert_eq!(publish.artifact_set_hash.len(), 64);

    Ok(ReviewedFixture {
        page_id: draft.id,
        expected_artifact_url: format!("/api/pages/{}/artifact?locale=en&channel=web", draft.id),
    })
}

async fn publication_events(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<PublicationEvents> {
    let rows = SysEvents::find()
        .order_by_asc(SysEventColumn::CreatedAt)
        .all(db)
        .await?;
    assert_eq!(rows.len(), 3);
    let mut created_id = None;
    let mut updated_id = None;
    let mut published_id = None;
    for row in rows {
        assert_eq!(row.status, SysEventStatus::Pending);
        let envelope: EventEnvelope = serde_json::from_value(row.payload)?;
        envelope.validate_registered_schema()?;
        assert_eq!(envelope.id, row.id);
        assert_eq!(envelope.correlation_id, row.id);
        assert_eq!(envelope.tenant_id, tenant_id);
        match envelope.event {
            DomainEvent::NodeCreated { node_id, kind, .. }
                if node_id == page_id && kind == PAGES_CACHE_ENTITY_KIND =>
            {
                created_id = Some(row.id);
            }
            DomainEvent::NodeUpdated { node_id, kind }
                if node_id == page_id && kind == PAGES_CACHE_ENTITY_KIND =>
            {
                updated_id = Some(row.id);
            }
            DomainEvent::NodePublished { node_id, kind }
                if node_id == page_id && kind == PAGES_CACHE_ENTITY_KIND =>
            {
                published_id = Some(row.id);
            }
            other => panic!("unexpected durable Pages publication event: {other:?}"),
        }
    }
    Ok(PublicationEvents {
        created_id: created_id
            .ok_or_else(|| std::io::Error::other("durable NodeCreated event is missing"))?,
        updated_id: updated_id
            .ok_or_else(|| std::io::Error::other("durable NodeUpdated event is missing"))?,
        published_id: published_id
            .ok_or_else(|| std::io::Error::other("durable NodePublished event is missing"))?,
    })
}

async fn assert_outbox_status(
    db: &DatabaseConnection,
    event_id: Uuid,
    expected: SysEventStatus,
) -> TestResult<()> {
    let row = SysEvents::find_by_id(event_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("durable outbox event is missing"))?;
    assert_eq!(row.status, expected);
    match &expected {
        SysEventStatus::Pending => assert!(row.dispatched_at.is_none()),
        SysEventStatus::Dispatched => {
            assert!(row.dispatched_at.is_some());
            assert!(row.last_error.is_none());
            assert!(row.next_attempt_at.is_none());
            assert!(row.claimed_by.is_none());
            assert!(row.claimed_at.is_none());
        }
        SysEventStatus::Failed => {}
    }
    Ok(())
}

async fn create_enabled_channel(
    service: &ChannelService,
    tenant_id: Uuid,
    slug: &str,
    name: &str,
) -> TestResult<ChannelResponse> {
    let channel = service
        .create_channel(CreateChannelInput {
            tenant_id,
            slug: slug.to_string(),
            name: name.to_string(),
            settings: None,
        })
        .await?;
    service
        .bind_module(
            channel.id,
            BindChannelModuleInput {
                module_slug: "pages".to_string(),
                is_enabled: true,
                settings: None,
            },
        )
        .await?;
    Ok(channel)
}

fn relay_config() -> RelayConfig {
    RelayConfig {
        batch_size: 1,
        max_attempts: 3,
        backoff_base: Duration::ZERO,
        backoff_max: Duration::ZERO,
        max_concurrency: 1,
        claim_ttl: Duration::from_secs(1),
        worker_id: "pages-production-relay-native-route".to_string(),
    }
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Production relay native route tenant".to_string(),
        slug: "production-relay-native-route-tenant".to_string(),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

fn channel_context(channel: &ChannelResponse) -> ChannelContext {
    ChannelContext {
        id: channel.id,
        tenant_id: channel.tenant_id,
        slug: channel.slug.clone(),
        name: channel.name.clone(),
        is_active: channel.is_active,
        status: channel.status.clone(),
        target_type: None,
        target_value: None,
        settings: channel.settings.clone(),
        resolution_source: ChannelResolutionSource::HeaderSlug,
        resolution_trace: Vec::new(),
    }
}
