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
use rustok_pages::dto::{CreatePageInput, PageBodyInput, PageTranslationInput};
use rustok_pages::services::PageService;
use rustok_pages::{
    PAGES_STOREFRONT_CACHE_TTL_SECS, PageCacheError, PageCacheGenerationSnapshot,
    PagesCacheReadPort, PagesCacheReadRuntime, PagesModule,
};
use rustok_pages_storefront as _;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use tower::ServiceExt;
use uuid::Uuid;

const RESPONSE_BODY_LIMIT: usize = 1024 * 1024;
const SERVER_FN_PATH: &str = "/api/fn/pages/storefront-data";
const SERVER_FN_FORM: &str = "page_slug=home&locale=en";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

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
            keys: state.values.keys().cloned().collect(),
            generation_reads: state.generation_reads,
            get_keys: state.get_keys.clone(),
            put_keys: state.put_keys.clone(),
            put_ttls: state.put_ttls.clone(),
        }
    }
}

struct CacheSnapshot {
    keys: Vec<String>,
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
async fn native_storefront_channel_admission_precedes_cache_lookup() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    create_published_page(&db, event_bus.clone(), tenant_id).await?;

    let channel_service = ChannelService::new(db.clone());
    let channel = channel_service
        .create_channel(CreateChannelInput {
            tenant_id,
            slug: "web".to_string(),
            name: "Web".to_string(),
            settings: None,
        })
        .await?;
    channel_service
        .bind_module(
            channel.id,
            BindChannelModuleInput {
                module_slug: "pages".to_string(),
                is_enabled: false,
                settings: None,
            },
        )
        .await?;

    let cache = Arc::new(RecordingCachePort::new(PageCacheGenerationSnapshot::new(
        3, 5, 7,
    )));
    let cache_port: Arc<dyn PagesCacheReadPort> = cache.clone();
    let host = HostRuntimeContext::new(db.clone())
        .with_shared_value(event_bus)
        .with_shared_value(PagesCacheReadRuntime::new(cache_port));
    let app = native_server_fn_router(host);
    let tenant = tenant_context(tenant_id);
    let channel_context = channel_context(&channel);

    let disabled = call_storefront(&app, &tenant, &channel_context).await?;
    assert_ne!(disabled.status, StatusCode::OK);
    assert!(disabled.body.contains("pages"));
    assert!(disabled.body.contains("not enabled"));
    assert!(disabled.body.contains("web"));
    let disabled_cache = cache.snapshot();
    assert_eq!(disabled_cache.generation_reads, 0);
    assert!(disabled_cache.get_keys.is_empty());
    assert!(disabled_cache.put_keys.is_empty());
    assert!(disabled_cache.keys.is_empty());

    channel_service
        .bind_module(
            channel.id,
            BindChannelModuleInput {
                module_slug: "pages".to_string(),
                is_enabled: true,
                settings: None,
            },
        )
        .await?;
    let enabled = call_storefront(&app, &tenant, &channel_context).await?;
    assert_eq!(enabled.status, StatusCode::OK);
    assert!(enabled.body.contains("channel-admission-source"));
    let enabled_cache = cache.snapshot();
    assert_eq!(enabled_cache.generation_reads, 1);
    assert_eq!(enabled_cache.get_keys.len(), 1);
    assert_eq!(enabled_cache.put_keys.len(), 1);
    assert_eq!(enabled_cache.get_keys[0], enabled_cache.put_keys[0]);
    assert_eq!(
        enabled_cache.put_ttls,
        vec![Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS)]
    );
    assert_eq!(enabled_cache.keys.len(), 1);

    channel_service
        .bind_module(
            channel.id,
            BindChannelModuleInput {
                module_slug: "pages".to_string(),
                is_enabled: false,
                settings: None,
            },
        )
        .await?;
    let before_second_denial = cache.snapshot();
    let disabled_with_cached_value = call_storefront(&app, &tenant, &channel_context).await?;
    assert_ne!(disabled_with_cached_value.status, StatusCode::OK);
    assert!(disabled_with_cached_value.body.contains("not enabled"));
    let after_second_denial = cache.snapshot();
    assert_eq!(
        after_second_denial.generation_reads,
        before_second_denial.generation_reads
    );
    assert_eq!(after_second_denial.get_keys, before_second_denial.get_keys);
    assert_eq!(after_second_denial.put_keys, before_second_denial.put_keys);
    assert_eq!(after_second_denial.keys, before_second_denial.keys);
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
        .body(Body::from(SERVER_FN_FORM))?;
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
        "sqlite:file:pages_native_storefront_channel_admission_{}?mode=memory&cache=shared",
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

async fn create_published_page(
    db: &DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant_id: Uuid,
) -> TestResult<Uuid> {
    let service = PageService::new(db.clone(), event_bus);
    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Channel admission".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: Some("Channel admission".to_string()),
                    meta_description: Some("Native storefront admission packet".to_string()),
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: serde_json::json!({
                        "pages": [],
                        "test_content": "channel-admission-source",
                    }),
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;
    let published = service
        .publish_non_builder_if_current(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            Some(draft.version),
        )
        .await?;
    Ok(published.id)
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Channel admission tenant".to_string(),
        slug: "channel-admission-tenant".to_string(),
        domain: None,
        settings: serde_json::json!({}),
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
