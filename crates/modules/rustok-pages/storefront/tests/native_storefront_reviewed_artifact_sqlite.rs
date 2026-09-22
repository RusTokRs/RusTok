#![cfg(feature = "ssr")]

use std::collections::HashMap;
use std::error::Error;
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
    BindChannelModuleInput, ChannelModule, ChannelResponse, ChannelService, CreateChannelInput,
};
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::page_static_landing_artifact;
use rustok_pages::services::PageService;
use rustok_pages::{
    PAGES_STOREFRONT_CACHE_TTL_SECS, PageCacheError, PageCacheGenerationSnapshot,
    PagesCacheReadPort, PagesCacheReadRuntime, PagesModule,
};
use rustok_pages_storefront as _;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

const RESPONSE_BODY_LIMIT: usize = 1024 * 1024;
const SERVER_FN_PATH: &str = "/api/fn/pages/storefront-data";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone)]
struct ReviewedArtifactFixture {
    page_id: Uuid,
    artifact_id: Uuid,
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
}

struct RecordingCachePort {
    state: Mutex<CacheState>,
}

impl RecordingCachePort {
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
            key_count: state.values.len(),
            generation_reads: state.generation_reads,
            get_keys: state.get_keys.clone(),
            put_keys: state.put_keys.clone(),
            put_ttls: state.put_ttls.clone(),
        }
    }
}

struct CacheSnapshot {
    key_count: usize,
    generation_reads: usize,
    get_keys: Vec<String>,
    put_keys: Vec<String>,
    put_ttls: Vec<Duration>,
}

#[async_trait]
impl PagesCacheReadPort for RecordingCachePort {
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

#[tokio::test]
async fn native_storefront_returns_reviewed_artifact_for_visible_channel_and_refuses_unverified_fill()
-> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let fixture = create_reviewed_published_page(&db, event_bus.clone(), tenant_id).await?;

    let channel_service = ChannelService::new(db.clone());
    let web = create_enabled_channel(&channel_service, tenant_id, "web", "Web").await?;
    let mobile = create_enabled_channel(&channel_service, tenant_id, "mobile", "Mobile").await?;

    let cache = Arc::new(RecordingCachePort::new(PageCacheGenerationSnapshot::new(
        11, 13, 17,
    )));
    let cache_port: Arc<dyn PagesCacheReadPort> = cache.clone();
    let host = HostRuntimeContext::new(db.clone())
        .with_shared_value(event_bus)
        .with_shared_value(PagesCacheReadRuntime::new(cache_port));
    let app = native_server_fn_router(host);
    let tenant = tenant_context(tenant_id);

    let visible = call_storefront(&app, &tenant, &channel_context(&web), "en").await?;
    assert_eq!(visible.status, StatusCode::OK);
    assert!(visible.body.contains("fly_artifact_url"));
    assert!(visible.body.contains(&fixture.expected_artifact_url));
    assert!(visible.body.contains(&fixture.page_id.to_string()));
    assert!(visible.body.contains("Reviewed native artifact"));
    let visible_cache = cache.snapshot();
    assert_eq!(visible_cache.generation_reads, 1);
    assert_eq!(visible_cache.get_keys.len(), 1);
    assert_eq!(visible_cache.put_keys.len(), 1);
    assert_eq!(visible_cache.get_keys[0], visible_cache.put_keys[0]);
    assert_eq!(visible_cache.key_count, 1);
    assert_eq!(
        visible_cache.put_ttls,
        vec![Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS)]
    );

    let hidden = call_storefront(&app, &tenant, &channel_context(&mobile), "en").await?;
    assert_eq!(hidden.status, StatusCode::OK);
    assert!(!hidden.body.contains(&fixture.expected_artifact_url));
    assert!(!hidden.body.contains("fly_artifact_url"));
    let hidden_cache = cache.snapshot();
    assert_eq!(hidden_cache.generation_reads, 2);
    assert_eq!(hidden_cache.get_keys.len(), 2);
    assert_eq!(hidden_cache.put_keys.len(), 2);
    assert_ne!(hidden_cache.get_keys[0], hidden_cache.get_keys[1]);
    assert_eq!(hidden_cache.key_count, 2);

    corrupt_artifact_document(&db, fixture.artifact_id).await?;
    let before_corrupt_read = cache.snapshot();
    let corrupt = call_storefront(&app, &tenant, &channel_context(&web), "fr").await?;
    assert_ne!(corrupt.status, StatusCode::OK);
    assert!(corrupt.body.to_ascii_lowercase().contains("integrity"));
    let after_corrupt_read = cache.snapshot();
    assert_eq!(
        after_corrupt_read.generation_reads,
        before_corrupt_read.generation_reads + 1
    );
    assert_eq!(
        after_corrupt_read.get_keys.len(),
        before_corrupt_read.get_keys.len() + 1
    );
    assert_eq!(after_corrupt_read.put_keys, before_corrupt_read.put_keys);
    assert_eq!(after_corrupt_read.key_count, before_corrupt_read.key_count);
    assert_ne!(
        after_corrupt_read.get_keys.last(),
        before_corrupt_read.get_keys.first()
    );
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
    locale: &str,
) -> TestResult<ServerFnResponse> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(SERVER_FN_PATH)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("X-Tenant-ID", tenant.id.to_string())
        .body(Body::from(format!("page_slug=home&locale={locale}")))?;
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
        "sqlite:file:pages_native_storefront_reviewed_artifact_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)".to_string(),
    ))
    .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await?;
    db.execute_raw(Statement::from_string(
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
    SysEventsMigration.up(&manager).await?;
    for migration in ChannelModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in PagesModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

async fn create_reviewed_published_page(
    db: &DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant_id: Uuid,
) -> TestResult<ReviewedArtifactFixture> {
    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Reviewed native artifact",
                "description": "Registered native storefront reviewed artifact packet",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Reviewed native artifact"
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
                    title: "Reviewed native artifact".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: Some("Reviewed native artifact".to_string()),
                    meta_description: Some(
                        "Registered native storefront reviewed artifact packet".to_string(),
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
        "native-reviewed-artifact",
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
                idempotency_key: "native-storefront-reviewed-artifact-v1".to_string(),
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

    let artifact = page_static_landing_artifact::Entity::find()
        .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
        .filter(page_static_landing_artifact::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("reviewed publish did not retain an artifact"))?;
    assert!(artifact.materialization_hash.is_some());
    assert!(artifact.materialization_identity.is_some());
    assert!(artifact.runtime_snapshots.is_some());
    assert_eq!(artifact.artifact_hash.len(), 64);
    assert_eq!(artifact.content_hash.len(), 64);

    Ok(ReviewedArtifactFixture {
        page_id: draft.id,
        artifact_id: artifact.id,
        expected_artifact_url: format!("/api/pages/{}/artifact?locale=en&channel=web", draft.id),
    })
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

async fn corrupt_artifact_document(db: &DatabaseConnection, artifact_id: Uuid) -> TestResult<()> {
    let artifact = page_static_landing_artifact::Entity::find_by_id(artifact_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("artifact to corrupt was not found"))?;
    let corrupted_document = format!("{}<!--corrupted-after-publish-->", artifact.document_html);
    let mut active: page_static_landing_artifact::ActiveModel = artifact.into();
    active.document_html = Set(corrupted_document);
    active.update(db).await?;
    Ok(())
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Reviewed artifact tenant".to_string(),
        slug: "reviewed-artifact-tenant".to_string(),
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
