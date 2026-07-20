use axum::{
    Json, Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    middleware,
    routing::get,
};
use rustok_cache::CacheService;
use rustok_migrations::Migrator;
use rustok_server::{
    common::settings::{RustokSettings, TenantResolutionMode},
    extractors::tenant::CurrentTenant,
    middleware::tenant,
    services::server_runtime_context::ServerRuntimeContext,
};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serial_test::serial;
use tower::ServiceExt;
use uuid::Uuid;

async fn tenant_probe(CurrentTenant(tenant): CurrentTenant) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "id": tenant.id,
        "slug": tenant.slug,
        "is_active": tenant.is_active,
    }))
}

async fn setup_tenant_router(
    settings: RustokSettings,
) -> (DatabaseConnection, ServerRuntimeContext, Router) {
    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let runtime_ctx = ServerRuntimeContext::new(db.clone(), settings);
    let cache_service = CacheService::from_url(None);
    tenant::init_tenant_cache_infrastructure(&runtime_ctx, &cache_service).await;

    let app = Router::new()
        .route("/tenant-probe", get(tenant_probe))
        .route("/api/graphql", get(tenant_probe))
        .route("/storefront/products", get(tenant_probe))
        .route_layer(middleware::from_fn_with_state(
            runtime_ctx.clone(),
            tenant::resolve,
        ))
        .with_state(runtime_ctx.clone());

    (db, runtime_ctx, app)
}

async fn request_path(app: &Router, path: &str, tenant_header: Option<&str>) -> StatusCode {
    let mut builder = Request::builder().uri(path);
    if let Some(tenant_header) = tenant_header {
        builder = builder.header("X-Tenant-ID", tenant_header);
    }
    app.clone()
        .oneshot(builder.body(Body::empty()).expect("request"))
        .await
        .expect("tenant-bound request should complete")
        .status()
}

async fn request_tenant_slug(app: &Router, tenant_header: &str) -> (StatusCode, Option<String>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/tenant-probe")
                .header("X-Tenant-ID", tenant_header)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("tenant probe request should complete");

    let status = response.status();
    if !status.is_success() {
        return (status, None);
    }

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json payload");
    (status, payload["slug"].as_str().map(ToString::to_string))
}

async fn request_host_tenant_slug(app: &Router, host: &str) -> (StatusCode, Option<String>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/tenant-probe")
                .header("host", host)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("tenant host probe request should complete");

    let status = response.status();
    if !status.is_success() {
        return (status, None);
    }

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json payload");
    (status, payload["slug"].as_str().map(ToString::to_string))
}

async fn insert_tenant(
    db: &DatabaseConnection,
    slug: &str,
    domain: Option<&str>,
    is_active: bool,
) -> rustok_server::models::_entities::tenants::Model {
    let now = chrono::Utc::now();

    rustok_server::models::_entities::tenants::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(format!("{slug} tenant")),
        slug: Set(slug.to_string()),
        domain: Set(domain.map(ToString::to_string)),
        settings: Set(serde_json::json!({})),
        default_locale: Set("en".to_string()),
        is_active: Set(is_active),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .expect("tenant should insert")
}

#[tokio::test]
#[serial]
async fn tenant_bound_http_transports_reject_missing_tenant_assertion() {
    let settings = RustokSettings::default();
    let (_db, _runtime_ctx, app) = setup_tenant_router(settings).await;

    for path in ["/tenant-probe", "/api/graphql", "/storefront/products"] {
        assert_eq!(
            request_path(&app, path, None).await,
            StatusCode::BAD_REQUEST,
            "{path} must fail closed without tenant identity"
        );
    }
}

#[tokio::test]
#[serial]
async fn tenant_bound_http_transports_reject_attacker_controlled_identifier() {
    let settings = RustokSettings::default();
    let (_db, _runtime_ctx, app) = setup_tenant_router(settings).await;

    for path in ["/tenant-probe", "/api/graphql", "/storefront/products"] {
        assert_eq!(
            request_path(&app, path, Some("../../other-tenant")).await,
            StatusCode::BAD_REQUEST,
            "{path} must reject malformed tenant identity"
        );
    }
}

#[tokio::test]
#[serial]
async fn header_resolution_resolves_active_tenant_context() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = TenantResolutionMode::Header;

    let (db, _runtime_ctx, app) = setup_tenant_router(settings).await;
    insert_tenant(&db, "resolver-header", None, true).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/tenant-probe")
                .header("X-Tenant-ID", "resolver-header")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("header resolver request should complete");

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");

    assert_eq!(status, StatusCode::OK);
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json payload");
    assert_eq!(payload["slug"], "resolver-header");
}

#[tokio::test]
#[serial]
async fn host_resolution_resolves_tenant_by_domain() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = TenantResolutionMode::Host;

    let (db, _runtime_ctx, app) = setup_tenant_router(settings).await;
    insert_tenant(
        &db,
        "resolver-host",
        Some("resolver-host.example.test"),
        true,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/tenant-probe")
                .header("host", "resolver-host.example.test")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("host resolver request should complete");

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");

    assert_eq!(status, StatusCode::OK);
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json payload");
    assert_eq!(payload["slug"], "resolver-host");
}

#[tokio::test]
#[serial]
async fn subdomain_resolution_extracts_slug_and_resolves_tenant() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = TenantResolutionMode::Subdomain;
    settings.tenant.base_domains = vec!["example.test".to_string()];

    let (db, _runtime_ctx, app) = setup_tenant_router(settings).await;
    insert_tenant(&db, "resolver-subdomain", None, true).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/tenant-probe")
                .header("host", "resolver-subdomain.example.test")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("subdomain resolver request should complete");

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");

    assert_eq!(status, StatusCode::OK);
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json payload");
    assert_eq!(payload["slug"], "resolver-subdomain");
}

#[tokio::test]
#[serial]
async fn conflicting_id_and_slug_headers_are_rejected() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = TenantResolutionMode::Header;

    let (db, _runtime_ctx, app) = setup_tenant_router(settings).await;
    let tenant = insert_tenant(&db, "canonical-slug", None, true).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/tenant-probe")
                .header("X-Tenant-ID", tenant.id.to_string())
                .header("X-Tenant-Slug", "different-slug")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("conflicting tenant request should complete");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn resolver_returns_not_found_for_unknown_tenant() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = TenantResolutionMode::Header;

    let (_db, _runtime_ctx, app) = setup_tenant_router(settings).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/tenant-probe")
                .header("X-Tenant-ID", "missing-tenant")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("missing tenant request should complete");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial]
async fn resolver_returns_forbidden_for_inactive_tenant() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = TenantResolutionMode::Header;

    let (db, _runtime_ctx, app) = setup_tenant_router(settings).await;
    insert_tenant(&db, "resolver-disabled", None, false).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/tenant-probe")
                .header("X-Tenant-ID", "resolver-disabled")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("disabled tenant request should complete");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[serial]
async fn slug_cache_invalidation_refreshes_deactivated_tenant_state() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = TenantResolutionMode::Header;

    let (db, runtime_ctx, app) = setup_tenant_router(settings).await;
    let tenant_model = insert_tenant(&db, "resolver-deactivate-cache", None, true).await;

    let (status, slug) = request_tenant_slug(&app, "resolver-deactivate-cache").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(slug.as_deref(), Some("resolver-deactivate-cache"));

    let mut active: rustok_server::models::_entities::tenants::ActiveModel = tenant_model.into();
    active.is_active = Set(false);
    active.updated_at = Set(chrono::Utc::now().into());
    active
        .update(&db)
        .await
        .expect("tenant deactivation should persist");

    let (stale_status, stale_slug) = request_tenant_slug(&app, "resolver-deactivate-cache").await;
    assert_eq!(stale_status, StatusCode::OK);
    assert_eq!(stale_slug.as_deref(), Some("resolver-deactivate-cache"));

    tenant::invalidate_tenant_cache_by_slug(&runtime_ctx, "resolver-deactivate-cache").await;

    let (refreshed_status, refreshed_slug) =
        request_tenant_slug(&app, "resolver-deactivate-cache").await;
    assert_eq!(refreshed_status, StatusCode::FORBIDDEN);
    assert_eq!(refreshed_slug, None);
}

#[tokio::test]
#[serial]
async fn slug_negative_cache_invalidation_allows_created_tenant_to_resolve() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = TenantResolutionMode::Header;

    let (db, runtime_ctx, app) = setup_tenant_router(settings).await;

    let (missing_status, _) = request_tenant_slug(&app, "resolver-created-after-miss").await;
    assert_eq!(missing_status, StatusCode::NOT_FOUND);

    insert_tenant(&db, "resolver-created-after-miss", None, true).await;

    let (cached_miss_status, cached_miss_slug) =
        request_tenant_slug(&app, "resolver-created-after-miss").await;
    assert_eq!(cached_miss_status, StatusCode::NOT_FOUND);
    assert_eq!(cached_miss_slug, None);

    tenant::invalidate_tenant_cache_by_slug(&runtime_ctx, "resolver-created-after-miss").await;

    let (refreshed_status, refreshed_slug) =
        request_tenant_slug(&app, "resolver-created-after-miss").await;
    assert_eq!(refreshed_status, StatusCode::OK);
    assert_eq!(
        refreshed_slug.as_deref(),
        Some("resolver-created-after-miss")
    );
}

#[tokio::test]
#[serial]
async fn host_cache_invalidation_refreshes_domain_change() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = TenantResolutionMode::Host;

    let (db, runtime_ctx, app) = setup_tenant_router(settings).await;
    let tenant_model = insert_tenant(
        &db,
        "resolver-domain-change",
        Some("old-domain.example.test"),
        true,
    )
    .await;

    let (old_status, old_slug) = request_host_tenant_slug(&app, "old-domain.example.test").await;
    assert_eq!(old_status, StatusCode::OK);
    assert_eq!(old_slug.as_deref(), Some("resolver-domain-change"));

    let mut active: rustok_server::models::_entities::tenants::ActiveModel = tenant_model.into();
    active.domain = Set(Some("new-domain.example.test".to_string()));
    active.updated_at = Set(chrono::Utc::now().into());
    active
        .update(&db)
        .await
        .expect("tenant domain change should persist");

    let (stale_old_status, stale_old_slug) =
        request_host_tenant_slug(&app, "old-domain.example.test").await;
    assert_eq!(stale_old_status, StatusCode::OK);
    assert_eq!(stale_old_slug.as_deref(), Some("resolver-domain-change"));

    tenant::invalidate_tenant_cache_by_host(&runtime_ctx, "old-domain.example.test").await;
    tenant::invalidate_tenant_cache_by_host(&runtime_ctx, "new-domain.example.test").await;

    let (old_refreshed_status, old_refreshed_slug) =
        request_host_tenant_slug(&app, "old-domain.example.test").await;
    assert_eq!(old_refreshed_status, StatusCode::NOT_FOUND);
    assert_eq!(old_refreshed_slug, None);

    let (new_refreshed_status, new_refreshed_slug) =
        request_host_tenant_slug(&app, "new-domain.example.test").await;
    assert_eq!(new_refreshed_status, StatusCode::OK);
    assert_eq!(
        new_refreshed_slug.as_deref(),
        Some("resolver-domain-change")
    );
}

#[tokio::test]
#[serial]
async fn uuid_cache_invalidation_refreshes_updated_tenant_state() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = TenantResolutionMode::Header;

    let (db, runtime_ctx, app) = setup_tenant_router(settings).await;
    let tenant_model = insert_tenant(&db, "resolver-uuid-cache", None, true).await;
    let tenant_id = tenant_model.id;

    let (status, slug) = request_tenant_slug(&app, &tenant_id.to_string()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(slug.as_deref(), Some("resolver-uuid-cache"));

    let mut active: rustok_server::models::_entities::tenants::ActiveModel = tenant_model.into();
    active.is_active = Set(false);
    active.updated_at = Set(chrono::Utc::now().into());
    active
        .update(&db)
        .await
        .expect("tenant update should persist");

    tenant::invalidate_tenant_cache_by_uuid(&runtime_ctx, tenant_id).await;

    let (refreshed_status, refreshed_slug) =
        request_tenant_slug(&app, &tenant_id.to_string()).await;
    assert_eq!(refreshed_status, StatusCode::FORBIDDEN);
    assert_eq!(refreshed_slug, None);
}
