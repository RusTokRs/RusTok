#![cfg(feature = "ssr")]

use std::any::Any;
use std::collections::HashMap;
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
use rustok_channel::{
    BindChannelModuleInput, ChannelResponse, ChannelService, CreateChannelInput,
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
use rustok_pages::entities::page_publish_operation;
use rustok_pages::services::PageService;
use rustok_pages::{
    PAGES_CACHE_ENTITY_KIND, PAGES_STOREFRONT_CACHE_TTL_SECS, PageCacheError,
    PageCacheGenerationSnapshot, PageCacheInvalidationCause, PageCacheInvalidationEventHandler,
    PageCacheInvalidationPort, PageCacheInvalidationReceipt, PageCacheInvalidationRequest,
    PageCacheScope, PagesCacheInvalidationRuntime, PagesCacheReadPort, PagesCacheReadRuntime,
    PagesModule,
};
use rustok_pages_storefront as _;
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
    publish_operation_id: Uuid,
    expected_artifact_url: String,
}

#[derive(Default)]
struct CacheState {
    generations: PageCacheGenerationSnapshot,
    values: HashMap<String, Vec<u8>>,
    generation_reads: usize,
    get_keys: Vec<String>,
    put_keys: Vec<String>,
    put_ttls: Vec<Duration>,
    requests: Vec<PageCacheInvalidationRequest>,
    receipts: Vec<PageCacheInvalidationReceipt>,
}

struct ContinuityCachePort {
    state: Mutex<CacheState>,
}

impl ContinuityCachePort {
    fn new(generations: PageCacheGenerationSnapshot) -> Self {
        Self {
            state: Mutex::new(CacheState {
                generations,
                ..CacheState::default()
            }),
        }
    }

    fn snapshot(&self) -> CacheSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        CacheSnapshot {
            generations: state.generations,
            keys: state.values.keys().cloned().collect(),
            generation_reads: state.generation_reads,
            get_keys: state.get_keys.clone(),
            put_keys: state.put_keys.clone(),
            put_ttls: state.put_ttls.clone(),
            requests: state.requests.clone(),
            receipts: state.receipts.clone(),
        }
    }
}

struct CacheSnapshot {
    generations: PageCacheGenerationSnapshot,
    keys: Vec<String>,
    generation_reads: usize,
    get_keys: Vec<String>,
    put_keys: Vec<String>,
    put_ttls: Vec<Duration>,
    requests: Vec<PageCacheInvalidationRequest>,
    receipts: Vec<PageCacheInvalidationReceipt>,
}

#[async_trait]
impl PageCacheInvalidationPort for ContinuityCachePort {
    async fn invalidate(
        &self,
        request: PageCacheInvalidationRequest,
    ) -> Result<PageCacheInvalidationReceipt, PageCacheError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.requests.push(request.clone());
        let mut receipt = PageCacheInvalidationReceipt::new(&request);
        for scope in request.scopes() {
            let next = state.generations.generation(*scope) + 1;
            state.generations.record(*scope, next);
            receipt.record(*scope, next);
        }
        state.receipts.push(receipt.clone());
        Ok(receipt)
    }
}

#[async_trait]
impl PagesCacheReadPort for ContinuityCachePort {
    async fn generation_snapshot(
        &self,
        _tenant_id: Uuid,
    ) -> Result<PageCacheGenerationSnapshot, PageCacheError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.generation_reads += 1;
        Ok(state.generations)
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PageCacheError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.get_keys.push(key.to_string());
        Ok(state.values.get(key).cloned())
    }

    async fn put(&self, key: String, value: Vec<u8>, ttl: Duration) -> Result<(), PageCacheError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.put_keys.push(key.clone());
        state.put_ttls.push(ttl);
        state.values.insert(key, value);
        Ok(())
    }
}

struct ContinuityTarget {
    handler: PageCacheInvalidationEventHandler,
    delivered_event_ids: Mutex<Vec<Uuid>>,
}

impl ContinuityTarget {
    fn new(handler: PageCacheInvalidationEventHandler) -> Self {
        Self {
            handler,
            delivered_event_ids: Mutex::new(Vec::new()),
        }
    }

    fn delivered_event_ids(&self) -> Vec<Uuid> {
        self.delivered_event_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl EventTransport for ContinuityTarget {
    async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
        self.handler.handle(&envelope).await?;
        self.delivered_event_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(envelope.id);
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
async fn reviewed_node_published_relay_rotates_registered_native_route_key_before_refill()
-> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let channel_service = ChannelService::new(db.clone());
    let web = create_enabled_channel(&channel_service, tenant_id, "web", "Web").await?;
    let fixture = create_reviewed_published_page(&db, event_bus.clone(), tenant_id).await?;
    let receipt = page_publish_operation::Entity::find_by_id(fixture.publish_operation_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("durable reviewed publish receipt is missing"))?;
    assert_eq!(receipt.page_id, fixture.page_id);
    assert_eq!(receipt.result_version, 2);
    let events = publication_events(&db, tenant_id, fixture.page_id).await?;

    let cache = Arc::new(ContinuityCachePort::new(PageCacheGenerationSnapshot::new(
        3, 5, 7,
    )));
    let invalidation_port: Arc<dyn PageCacheInvalidationPort> = cache.clone();
    let read_port: Arc<dyn PagesCacheReadPort> = cache.clone();
    let handler = PageCacheInvalidationEventHandler::new(PagesCacheInvalidationRuntime::new(
        invalidation_port,
    ));
    let target = Arc::new(ContinuityTarget::new(handler));
    let target_transport: Arc<dyn EventTransport> = target.clone();
    let relay = OutboxRelay::new(db.clone(), target_transport).with_config(relay_config());

    assert_eq!(relay.process_pending_once(Some(1)).await?, 1);
    assert_eq!(target.delivered_event_ids(), vec![events.created_id]);
    assert_outbox_status(&db, events.created_id, SysEventStatus::Dispatched).await?;
    assert_outbox_status(&db, events.updated_id, SysEventStatus::Pending).await?;
    assert_outbox_status(&db, events.published_id, SysEventStatus::Pending).await?;
    let after_created = cache.snapshot();
    assert_eq!(
        after_created.generations,
        PageCacheGenerationSnapshot::new(3, 5, 7)
    );
    assert!(after_created.requests.is_empty());
    assert!(after_created.receipts.is_empty());

    assert_eq!(relay.process_pending_once(Some(1)).await?, 1);
    assert_eq!(
        target.delivered_event_ids(),
        vec![events.created_id, events.updated_id]
    );
    assert_outbox_status(&db, events.updated_id, SysEventStatus::Dispatched).await?;
    assert_outbox_status(&db, events.published_id, SysEventStatus::Pending).await?;
    let after_updated = cache.snapshot();
    assert_eq!(
        after_updated.generations,
        PageCacheGenerationSnapshot::new(4, 6, 7)
    );
    assert_eq!(after_updated.requests.len(), 1);
    assert_eq!(after_updated.receipts.len(), 1);
    assert_eq!(after_updated.requests[0].event_id, events.updated_id);
    assert_eq!(after_updated.requests[0].correlation_id, events.updated_id);
    assert_eq!(
        after_updated.requests[0].cause,
        PageCacheInvalidationCause::Updated
    );
    assert_eq!(
        after_updated.requests[0].scopes(),
        &[PageCacheScope::Route, PageCacheScope::Page]
    );
    assert_eq!(after_updated.receipts[0].event_id, events.updated_id);
    assert_eq!(after_updated.receipts[0].route_generation, Some(4));
    assert_eq!(after_updated.receipts[0].page_generation, Some(6));
    assert_eq!(after_updated.receipts[0].artifact_generation, None);

    let host = HostRuntimeContext::new(db.clone())
        .with_shared_value(event_bus)
        .with_shared_value(PagesCacheReadRuntime::new(read_port));
    let app = native_server_fn_router(host);
    let tenant = tenant_context(tenant_id);
    let channel = channel_context(&web);

    let before_published_delivery = call_storefront(&app, &tenant, &channel).await?;
    assert_eq!(before_published_delivery.status, StatusCode::OK);
    assert!(
        before_published_delivery
            .body
            .contains(&fixture.expected_artifact_url)
    );
    assert!(before_published_delivery.body.contains("fly_artifact_url"));
    let old_cache = cache.snapshot();
    assert_eq!(
        old_cache.generations,
        PageCacheGenerationSnapshot::new(4, 6, 7)
    );
    assert_eq!(old_cache.generation_reads, 1);
    assert_eq!(old_cache.get_keys.len(), 1);
    assert_eq!(old_cache.put_keys.len(), 1);
    assert_eq!(old_cache.get_keys[0], old_cache.put_keys[0]);
    assert_eq!(
        old_cache.put_ttls,
        vec![Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS)]
    );
    let old_key = old_cache.put_keys[0].clone();
    assert!(old_cache.keys.contains(&old_key));

    assert_eq!(relay.process_pending_once(Some(1)).await?, 1);
    assert_eq!(
        target.delivered_event_ids(),
        vec![events.created_id, events.updated_id, events.published_id]
    );
    assert_outbox_status(&db, events.published_id, SysEventStatus::Dispatched).await?;
    let after_published = cache.snapshot();
    assert_eq!(
        after_published.generations,
        PageCacheGenerationSnapshot::new(5, 7, 8)
    );
    assert_eq!(after_published.requests.len(), 2);
    assert_eq!(after_published.receipts.len(), 2);
    assert_eq!(after_published.requests[1].tenant_id, tenant_id);
    assert_eq!(after_published.requests[1].page_id, fixture.page_id);
    assert_eq!(after_published.requests[1].event_id, events.published_id);
    assert_eq!(
        after_published.requests[1].correlation_id,
        events.published_id
    );
    assert_eq!(
        after_published.requests[1].cause,
        PageCacheInvalidationCause::Published
    );
    assert_eq!(
        after_published.requests[1].scopes(),
        &[
            PageCacheScope::Route,
            PageCacheScope::Page,
            PageCacheScope::Artifact,
        ]
    );
    assert_eq!(after_published.receipts[1].event_id, events.published_id);
    assert_eq!(
        after_published.receipts[1].correlation_id,
        events.published_id
    );
    assert_eq!(after_published.receipts[1].route_generation, Some(5));
    assert_eq!(after_published.receipts[1].page_generation, Some(7));
    assert_eq!(after_published.receipts[1].artifact_generation, Some(8));
    assert!(after_published.keys.contains(&old_key));
    assert_eq!(after_published.put_keys.len(), 1);

    let after_rotation = call_storefront(&app, &tenant, &channel).await?;
    assert_eq!(after_rotation.status, StatusCode::OK);
    assert_eq!(after_rotation.body, before_published_delivery.body);
    assert!(after_rotation.body.contains(&fixture.expected_artifact_url));
    let refilled = cache.snapshot();
    assert_eq!(
        refilled.generations,
        PageCacheGenerationSnapshot::new(5, 7, 8)
    );
    assert_eq!(refilled.generation_reads, 2);
    assert_eq!(refilled.get_keys.len(), 2);
    assert_eq!(refilled.put_keys.len(), 2);
    let new_key = refilled.put_keys[1].clone();
    assert_eq!(refilled.get_keys[1], new_key);
    assert_ne!(new_key, old_key);
    assert!(refilled.keys.contains(&old_key));
    assert!(refilled.keys.contains(&new_key));
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
    let final_cache = cache.snapshot();
    assert_eq!(final_cache.generation_reads, 3);
    assert_eq!(final_cache.get_keys.len(), 3);
    assert_eq!(final_cache.get_keys[2], new_key);
    assert_eq!(final_cache.put_keys.len(), 2);
    assert_eq!(final_cache.keys.len(), 2);
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
        "sqlite:file:pages_native_storefront_relay_continuity_{}?mode=memory&cache=shared",
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
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE channels (\
            id TEXT PRIMARY KEY NOT NULL, \
            tenant_id TEXT NOT NULL, \
            slug TEXT NOT NULL, \
            name TEXT NOT NULL, \
            is_active INTEGER NOT NULL, \
            is_default INTEGER NOT NULL, \
            status TEXT NOT NULL, \
            settings TEXT NOT NULL, \
            created_at TEXT NOT NULL, \
            updated_at TEXT NOT NULL\
        )"
        .to_string(),
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE UNIQUE INDEX uq_channels_tenant_slug ON channels (tenant_id, slug)".to_string(),
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE channel_module_bindings (\
            id TEXT PRIMARY KEY NOT NULL, \
            channel_id TEXT NOT NULL, \
            module_slug TEXT NOT NULL, \
            is_enabled INTEGER NOT NULL, \
            settings TEXT NOT NULL, \
            created_at TEXT NOT NULL, \
            updated_at TEXT NOT NULL\
        )"
        .to_string(),
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE UNIQUE INDEX uq_channel_module_binding ON channel_module_bindings (channel_id, module_slug)".to_string(),
    ))
    .await?;

    let manager = SchemaManager::new(&db);
    for migration in OutboxModule
        .migrations()
        .into_iter()
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
                "title": "Relay continuity artifact",
                "description": "Reviewed publish to native storefront relay continuity",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Relay continuity artifact"
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
                    title: "Relay continuity artifact".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: Some("Relay continuity artifact".to_string()),
                    meta_description: Some(
                        "Reviewed publish to native storefront relay continuity".to_string(),
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
        "native-relay-continuity",
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
                idempotency_key: "native-storefront-relay-continuity-v1".to_string(),
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
        publish_operation_id: publish.operation_id,
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
        worker_id: "pages-native-storefront-continuity".to_string(),
    }
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Relay continuity tenant".to_string(),
        slug: "relay-continuity-tenant".to_string(),
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
