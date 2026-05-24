use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    middleware,
    routing::get,
    Json, Router,
};
use loco_rs::{app::AppContext, tests_cfg::app::get_app_context};
use migration::Migrator;
use rustok_cache::CacheService;
use rustok_server::{
    common::settings::{RustokSettings, SharedRustokSettings},
    extractors::tenant::CurrentTenant,
    middleware::tenant,
};
use sea_orm::{ActiveModelTrait, Set};
use sea_orm_migration::MigratorTrait;
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

async fn setup_tenant_router(settings: RustokSettings) -> (AppContext, Router) {
    let ctx = get_app_context().await;
    Migrator::up(&ctx.db, None)
        .await
        .expect("server migrations should apply for tenant resolver test");

    ctx.shared_store
        .insert(SharedRustokSettings(Arc::new(settings)));

    let cache_service = CacheService::from_url(None);
    tenant::init_tenant_cache_infrastructure(&ctx, &cache_service).await;

    let app = Router::new()
        .route("/tenant-probe", get(tenant_probe))
        .route_layer(middleware::from_fn_with_state(ctx.clone(), tenant::resolve))
        .with_state(ctx.clone());

    (ctx, app)
}

async fn insert_tenant(
    ctx: &AppContext,
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
    .insert(&ctx.db)
    .await
    .expect("tenant should insert")
}

#[tokio::test]
#[serial]
async fn header_resolution_resolves_active_tenant_context() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = "header".to_string();

    let (ctx, app) = setup_tenant_router(settings).await;
    insert_tenant(&ctx, "resolver-header", None, true).await;

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
    settings.tenant.resolution = "host".to_string();

    let (ctx, app) = setup_tenant_router(settings).await;
    insert_tenant(
        &ctx,
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
    settings.tenant.resolution = "subdomain".to_string();
    settings.tenant.base_domains = vec!["example.test".to_string()];

    let (ctx, app) = setup_tenant_router(settings).await;
    insert_tenant(&ctx, "resolver-subdomain", None, true).await;

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
async fn resolver_returns_not_found_for_unknown_tenant() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = "header".to_string();

    let (_ctx, app) = setup_tenant_router(settings).await;

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
    settings.tenant.resolution = "header".to_string();

    let (ctx, app) = setup_tenant_router(settings).await;
    insert_tenant(&ctx, "resolver-disabled", None, false).await;

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
