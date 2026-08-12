use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use chrono::Utc;
use rustok_api::{HostRuntimeContext, TenantContext, TenantContextExtension};
use rustok_core::MigrationSource;
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use rustok_page_builder::static_landing::StaticLandingCompiler;
use rustok_pages::entities::{
    page, page_body, page_published_landing_artifact, page_static_landing_artifact,
};
use rustok_pages::{
    PageCacheError, PageCacheGenerationSnapshot, PageCacheScope, PagesCacheReadPort,
    PagesCacheReadRuntime, PagesModule, controllers,
};
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, Set};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

const RESPONSE_BODY_LIMIT: usize = 3 * 1024 * 1024;
const EXPECTED_VARY: &str = "X-Tenant-ID, X-Channel-Slug, X-Channel-ID";
const EXPECTED_CACHE_CONTROL: &str = "public, max-age=60, stale-while-revalidate=300";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone)]
struct ArtifactFixture {
    tenant_id: Uuid,
    page_id: Uuid,
    body_id: Uuid,
    artifact_id: Uuid,
    artifact_hash: String,
    document_html: String,
}

#[derive(Default)]
struct CacheState {
    generations: PageCacheGenerationSnapshot,
    values: HashMap<String, Vec<u8>>,
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

    fn set_artifact_generation(&self, generation: u64) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generations
            .record(PageCacheScope::Artifact, generation);
    }

    fn snapshot(&self) -> CacheSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        CacheSnapshot {
            generations: state.generations,
            keys: state.values.keys().cloned().collect(),
            get_keys: state.get_keys.clone(),
            put_keys: state.put_keys.clone(),
            put_ttls: state.put_ttls.clone(),
        }
    }
}

struct CacheSnapshot {
    generations: PageCacheGenerationSnapshot,
    keys: Vec<String>,
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
        Ok(self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generations)
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
async fn artifact_http_misses_refills_hits_and_returns_conditional_304_across_generation_change()
-> TestResult<()> {
    let db = setup_db().await?;
    let fixture = insert_published_artifact(&db).await?;
    let cache = Arc::new(RecordingCachePort::new(PageCacheGenerationSnapshot::new(
        3, 5, 7,
    )));
    let cache_port: Arc<dyn PagesCacheReadPort> = cache.clone();
    let host = HostRuntimeContext::new(db.clone())
        .with_shared_value(mock_transactional_event_bus())
        .with_shared_value(PagesCacheReadRuntime::new(cache_port));
    let app = controllers::axum_router(&host)?;

    let first = app
        .clone()
        .oneshot(artifact_request(&fixture, None))
        .await?;
    assert_eq!(first.status(), StatusCode::OK);
    assert_success_headers(&first, &fixture.artifact_hash)?;
    let first_etag = first
        .headers()
        .get(header::ETAG)
        .ok_or_else(|| std::io::Error::other("artifact response is missing ETag"))?
        .to_str()?
        .to_string();
    let first_body = to_bytes(first.into_body(), RESPONSE_BODY_LIMIT).await?;
    assert_eq!(std::str::from_utf8(&first_body)?, fixture.document_html);

    let first_cache = cache.snapshot();
    assert_eq!(first_cache.generations.artifact, 7);
    assert_eq!(first_cache.get_keys.len(), 1);
    assert_eq!(first_cache.put_keys.len(), 1);
    assert_eq!(first_cache.get_keys[0], first_cache.put_keys[0]);
    assert_eq!(first_cache.put_ttls, vec![Duration::from_secs(60)]);
    assert!(first_cache.keys.contains(&first_cache.put_keys[0]));
    let old_generation_key = first_cache.put_keys[0].clone();

    delete_binding(&db, fixture.body_id).await?;
    let second = app
        .clone()
        .oneshot(artifact_request(&fixture, Some(&first_etag)))
        .await?;
    assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    assert_not_modified_headers(&second, &fixture.artifact_hash)?;
    assert!(
        to_bytes(second.into_body(), RESPONSE_BODY_LIMIT)
            .await?
            .is_empty()
    );

    let second_cache = cache.snapshot();
    assert_eq!(second_cache.get_keys.len(), 2);
    assert_eq!(second_cache.put_keys.len(), 1);
    assert_eq!(second_cache.get_keys[1], old_generation_key);

    insert_binding(&db, &fixture).await?;
    cache.set_artifact_generation(8);
    let third = app
        .clone()
        .oneshot(artifact_request(&fixture, None))
        .await?;
    assert_eq!(third.status(), StatusCode::OK);
    assert_success_headers(&third, &fixture.artifact_hash)?;
    let third_body = to_bytes(third.into_body(), RESPONSE_BODY_LIMIT).await?;
    assert_eq!(std::str::from_utf8(&third_body)?, fixture.document_html);

    let third_cache = cache.snapshot();
    assert_eq!(third_cache.generations.artifact, 8);
    assert_eq!(third_cache.get_keys.len(), 3);
    assert_eq!(third_cache.put_keys.len(), 2);
    let new_generation_key = third_cache.put_keys[1].clone();
    assert_eq!(third_cache.get_keys[2], new_generation_key);
    assert_ne!(new_generation_key, old_generation_key);
    assert!(third_cache.keys.contains(&old_generation_key));
    assert!(third_cache.keys.contains(&new_generation_key));
    assert_eq!(
        third_cache.put_ttls,
        vec![Duration::from_secs(60), Duration::from_secs(60)]
    );

    delete_binding(&db, fixture.body_id).await?;
    let fourth = app
        .oneshot(artifact_request(&fixture, Some(&first_etag)))
        .await?;
    assert_eq!(fourth.status(), StatusCode::NOT_MODIFIED);
    assert_not_modified_headers(&fourth, &fixture.artifact_hash)?;
    assert!(
        to_bytes(fourth.into_body(), RESPONSE_BODY_LIMIT)
            .await?
            .is_empty()
    );

    let fourth_cache = cache.snapshot();
    assert_eq!(fourth_cache.get_keys.len(), 4);
    assert_eq!(fourth_cache.put_keys.len(), 2);
    assert_eq!(fourth_cache.get_keys[3], new_generation_key);
    Ok(())
}

async fn setup_db() -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_artifact_http_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    let manager = SchemaManager::new(&db);
    for migration in PagesModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

async fn insert_published_artifact(db: &DatabaseConnection) -> TestResult<ArtifactFixture> {
    let tenant_id = Uuid::new_v4();
    let page_id = Uuid::new_v4();
    let body_id = Uuid::new_v4();
    let artifact_id = Uuid::new_v4();
    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Cached landing",
                "description": "Pages artifact HTTP cache evidence",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Cached landing"
                }]
            }
        }]
    });
    let artifact = StaticLandingCompiler::default().compile_publish(&project)?;
    artifact.verify_integrity()?;
    let rendered = artifact
        .pages
        .first()
        .cloned()
        .ok_or_else(|| std::io::Error::other("compiled landing artifact has no page"))?;
    let now: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();

    page::ActiveModel {
        id: Set(page_id),
        tenant_id: Set(tenant_id),
        author_id: Set(None),
        status: Set("published".to_string()),
        template: Set("default".to_string()),
        metadata: Set(json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        published_at: Set(Some(now)),
        archived_at: Set(None),
        version: Set(1),
    }
    .insert(db)
    .await?;

    page_body::ActiveModel {
        id: Set(body_id),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        locale: Set("en".to_string()),
        content: Set(serde_json::to_string(&project)?),
        format: Set(PAGE_BUILDER_DOCUMENT_FORMAT.to_string()),
        updated_at: Set(now),
    }
    .insert(db)
    .await?;

    page_static_landing_artifact::ActiveModel {
        id: Set(artifact_id),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        locale: Set("en".to_string()),
        source_hash: Set(artifact.identity.source_hash.clone()),
        build_hash: Set(artifact.identity.build_hash.clone()),
        artifact_hash: Set(artifact.artifact_hash.clone()),
        materialization_hash: Set(None),
        materialization_identity: Set(None),
        runtime_snapshots: Set(None),
        renderer_id: Set(artifact.identity.renderer.id.clone()),
        renderer_release: Set(artifact.identity.renderer.release.clone()),
        identity: Set(serde_json::to_value(&artifact.identity)?),
        registry: Set(serde_json::to_value(&artifact.registry)?),
        page_index: Set(i32::try_from(rendered.page_index)?),
        fly_page_id: Set(rendered.page_id.clone()),
        slug: Set(rendered.slug.clone()),
        head: Set(serde_json::to_value(&rendered.head)?),
        document_html: Set(rendered.document_html.clone()),
        body_html: Set(rendered.body_html.clone()),
        css: Set(rendered.css.clone()),
        content_hash: Set(rendered.content_hash.clone()),
        landing_sections: Set(serde_json::to_value(&rendered.landing_sections)?),
        instance_key: Set("canonical".to_string()),
        created_at: Set(now),
    }
    .insert(db)
    .await?;

    let fixture = ArtifactFixture {
        tenant_id,
        page_id,
        body_id,
        artifact_id,
        artifact_hash: artifact.artifact_hash,
        document_html: rendered.document_html,
    };
    insert_binding(db, &fixture).await?;
    Ok(fixture)
}

async fn insert_binding(db: &DatabaseConnection, fixture: &ArtifactFixture) -> TestResult<()> {
    page_published_landing_artifact::ActiveModel {
        page_body_id: Set(fixture.body_id),
        tenant_id: Set(fixture.tenant_id),
        page_id: Set(fixture.page_id),
        locale: Set("en".to_string()),
        artifact_id: Set(fixture.artifact_id),
        published_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn delete_binding(db: &DatabaseConnection, body_id: Uuid) -> TestResult<()> {
    let deleted = page_published_landing_artifact::Entity::delete_by_id(body_id)
        .exec(db)
        .await?;
    assert_eq!(deleted.rows_affected, 1);
    Ok(())
}

fn artifact_request(fixture: &ArtifactFixture, if_none_match: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .uri(format!("/api/pages/{}/artifact?locale=en", fixture.page_id))
        .header("X-Tenant-ID", fixture.tenant_id.to_string());
    if let Some(etag) = if_none_match {
        builder = builder.header(header::IF_NONE_MATCH, etag);
    }
    let mut request = builder
        .body(Body::empty())
        .expect("artifact HTTP request must be valid");
    request
        .extensions_mut()
        .insert(TenantContextExtension(tenant_context(fixture.tenant_id)));
    request
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Artifact HTTP tenant".to_string(),
        slug: "artifact-http-tenant".to_string(),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

fn assert_success_headers(
    response: &axum::response::Response,
    artifact_hash: &str,
) -> TestResult<()> {
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .ok_or_else(|| std::io::Error::other("artifact response is missing Content-Type"))?
            .to_str()?,
        "text/html; charset=utf-8"
    );
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_LANGUAGE)
            .ok_or_else(|| std::io::Error::other("artifact response is missing Content-Language"))?
            .to_str()?,
        "en"
    );
    let expected_etag = format!("\"{artifact_hash}\"");
    assert_eq!(
        response
            .headers()
            .get(header::ETAG)
            .ok_or_else(|| std::io::Error::other("artifact response is missing ETag"))?
            .to_str()?,
        expected_etag
    );
    assert_eq!(
        response
            .headers()
            .get(header::VARY)
            .ok_or_else(|| std::io::Error::other("artifact response is missing Vary"))?
            .to_str()?,
        EXPECTED_VARY
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .ok_or_else(|| std::io::Error::other("artifact response is missing Cache-Control"))?
            .to_str()?,
        EXPECTED_CACHE_CONTROL
    );
    let csp = response
        .headers()
        .get("content-security-policy")
        .ok_or_else(|| std::io::Error::other("artifact response is missing CSP"))?
        .to_str()?;
    assert!(csp.contains("style-src 'sha256-"));
    assert_eq!(
        response
            .headers()
            .get("referrer-policy")
            .ok_or_else(|| std::io::Error::other("artifact response is missing Referrer-Policy"))?
            .to_str()?,
        "strict-origin-when-cross-origin"
    );
    assert_eq!(
        response
            .headers()
            .get("x-content-type-options")
            .ok_or_else(|| std::io::Error::other(
                "artifact response is missing X-Content-Type-Options"
            ))?
            .to_str()?,
        "nosniff"
    );
    assert_eq!(
        response
            .headers()
            .get("cross-origin-resource-policy")
            .ok_or_else(|| std::io::Error::other("artifact response is missing CORP"))?
            .to_str()?,
        "same-origin"
    );
    Ok(())
}

fn assert_not_modified_headers(
    response: &axum::response::Response,
    artifact_hash: &str,
) -> TestResult<()> {
    let expected_etag = format!("\"{artifact_hash}\"");
    assert_eq!(
        response
            .headers()
            .get(header::ETAG)
            .ok_or_else(|| std::io::Error::other("artifact 304 response is missing ETag"))?
            .to_str()?,
        expected_etag
    );
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_LANGUAGE)
            .ok_or_else(|| std::io::Error::other(
                "artifact 304 response is missing Content-Language"
            ))?
            .to_str()?,
        "en"
    );
    assert_eq!(
        response
            .headers()
            .get(header::VARY)
            .ok_or_else(|| std::io::Error::other("artifact 304 response is missing Vary"))?
            .to_str()?,
        EXPECTED_VARY
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .ok_or_else(|| std::io::Error::other("artifact 304 response is missing Cache-Control"))?
            .to_str()?,
        EXPECTED_CACHE_CONTROL
    );
    Ok(())
}
