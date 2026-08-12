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
use chrono::Utc;
use leptos::prelude::provide_context;
use leptos_axum::handle_server_fns_with_context;
use rustok_api::{HostRuntimeContext, TenantContext, TenantContextExtension};
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::dto::{CreatePageInput, PageBodyInput, PageTranslationInput};
use rustok_pages::entities::page_body;
use rustok_pages::services::PageService;
use rustok_pages::{
    PAGES_STOREFRONT_CACHE_TTL_SECS, PageCacheError, PageCacheGenerationSnapshot,
    PagesCacheReadPort, PagesCacheReadRuntime, PagesModule,
};
use rustok_pages_storefront as _;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait,
    QueryFilter, Set,
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
    generation_error: bool,
    get_error: bool,
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

    fn set_generations(&self, generations: PageCacheGenerationSnapshot) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generations = generations;
    }

    fn set_get_error(&self, enabled: bool) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_error = enabled;
    }

    fn set_generation_error(&self, enabled: bool) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generation_error = enabled;
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
        if state.generation_error {
            return Err(PageCacheError::Provider(
                "injected native storefront generation failure".to_string(),
            ));
        }
        Ok(state.generations)
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PageCacheError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.get_keys.push(key.to_string());
        if state.get_error {
            return Err(PageCacheError::Provider(
                "injected native storefront cache read failure".to_string(),
            ));
        }
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
async fn native_storefront_server_fn_misses_hits_rotates_and_fails_open() -> TestResult<()> {
    let db = setup_db().await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let tenant_id = Uuid::new_v4();
    let page_id = create_published_page(&db, event_bus.clone(), tenant_id).await?;
    let cache = Arc::new(RecordingCachePort::new(PageCacheGenerationSnapshot::new(
        3, 5, 7,
    )));
    let cache_port: Arc<dyn PagesCacheReadPort> = cache.clone();
    let host = HostRuntimeContext::new(db.clone())
        .with_shared_value(event_bus)
        .with_shared_value(PagesCacheReadRuntime::new(cache_port));
    let app = native_server_fn_router(host);
    let tenant = tenant_context(tenant_id);

    let first = call_storefront(&app, &tenant).await?;
    assert!(first.contains("source-v1"));
    assert!(first.contains("selected_page"));
    assert!(first.contains("\"format\":\"html\""));

    let first_cache = cache.snapshot();
    assert_eq!(first_cache.generation_reads, 1);
    assert_eq!(first_cache.get_keys.len(), 1);
    assert_eq!(first_cache.put_keys.len(), 1);
    assert_eq!(first_cache.get_keys[0], first_cache.put_keys[0]);
    assert_eq!(
        first_cache.put_ttls,
        vec![Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS)]
    );
    let old_generation_key = first_cache.put_keys[0].clone();

    update_body(&db, page_id, "<main>source-v2</main>").await?;
    let second = call_storefront(&app, &tenant).await?;
    assert_eq!(second, first);
    assert!(!second.contains("source-v2"));
    let second_cache = cache.snapshot();
    assert_eq!(second_cache.get_keys.len(), 2);
    assert_eq!(second_cache.put_keys.len(), 1);
    assert_eq!(second_cache.get_keys[1], old_generation_key);

    cache.set_generations(PageCacheGenerationSnapshot::new(4, 6, 8));
    let third = call_storefront(&app, &tenant).await?;
    assert!(third.contains("source-v2"));
    let rotated_cache = cache.snapshot();
    assert_eq!(rotated_cache.put_keys.len(), 2);
    let new_generation_key = rotated_cache.put_keys[1].clone();
    assert_ne!(new_generation_key, old_generation_key);
    assert!(rotated_cache.keys.contains(&old_generation_key));
    assert!(rotated_cache.keys.contains(&new_generation_key));

    update_body(&db, page_id, "<main>source-v3</main>").await?;
    cache.set_generations(PageCacheGenerationSnapshot::new(5, 7, 9));
    cache.set_get_error(true);
    let fourth = call_storefront(&app, &tenant).await?;
    assert!(fourth.contains("source-v3"));
    let read_failure_cache = cache.snapshot();
    assert_eq!(read_failure_cache.put_keys.len(), 3);
    assert!(
        read_failure_cache
            .put_ttls
            .iter()
            .all(|ttl| *ttl == Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS))
    );

    update_body(&db, page_id, "<main>source-v4</main>").await?;
    cache.set_get_error(false);
    cache.set_generation_error(true);
    let before_generation_failure = cache.snapshot();
    let fifth = call_storefront(&app, &tenant).await?;
    assert!(fifth.contains("source-v4"));
    let after_generation_failure = cache.snapshot();
    assert_eq!(
        after_generation_failure.generation_reads,
        before_generation_failure.generation_reads + 1
    );
    assert_eq!(
        after_generation_failure.get_keys,
        before_generation_failure.get_keys
    );
    assert_eq!(
        after_generation_failure.put_keys,
        before_generation_failure.put_keys
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

async fn call_storefront(app: &Router, tenant: &TenantContext) -> TestResult<String> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(SERVER_FN_PATH)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("X-Tenant-ID", tenant.id.to_string())
        .body(Body::from(SERVER_FN_FORM))?;
    request
        .extensions_mut()
        .insert(TenantContextExtension(tenant.clone()));

    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), RESPONSE_BODY_LIMIT).await?;
    Ok(std::str::from_utf8(&bytes)?.to_string())
}

async fn setup_db() -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_native_storefront_server_fn_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    let manager = SchemaManager::new(&db);
    SysEventsMigration.up(&manager).await?;
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
                    title: "Native storefront".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: Some("Native storefront".to_string()),
                    meta_description: Some("Server function cache packet".to_string()),
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: serde_json::json!({
                        "pages": [],
                        "test_content": "source-v1",
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

async fn update_body(db: &DatabaseConnection, page_id: Uuid, content: &str) -> TestResult<()> {
    let body = page_body::Entity::find()
        .filter(page_body::Column::PageId.eq(page_id))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("published page body is missing"))?;
    let mut active: page_body::ActiveModel = body.into();
    active.content = Set(content.to_string());
    active.updated_at = Set(Utc::now().into());
    active.update(db).await?;
    Ok(())
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Native storefront tenant".to_string(),
        slug: "native-storefront-tenant".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}
