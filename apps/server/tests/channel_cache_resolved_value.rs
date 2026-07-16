use std::time::Duration;

use axum::{
    body::{to_bytes, Body},
    http::Request,
    middleware as axum_middleware,
    routing::get,
    Json, Router,
};
use rustok_channel::{entities::channel, ChannelService, CreateChannelInput};
use rustok_migrations::Migrator;
use rustok_server::{
    common::settings::RustokSettings,
    context::{OptionalChannel, TenantContext, TenantContextExtension},
    middleware::channel as channel_middleware,
    models::_entities::tenants,
    services::{
        cache_runtime::ensure_cache_service,
        channel_cache_invalidation::{
            publish_channel_resolution_invalidation, start_channel_cache_invalidation_listener,
        },
        server_runtime_context::ServerRuntimeContext,
    },
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use tower::ServiceExt;
use uuid::Uuid;

async fn channel_probe(OptionalChannel(channel): OptionalChannel) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": channel.map(|channel| channel.name),
    }))
}

fn settings_with_redis(url: &str) -> RustokSettings {
    let mut settings = RustokSettings::default();
    settings.cache.redis_url = Some(url.to_string());
    settings
}

async fn insert_tenant(db: &sea_orm::DatabaseConnection) -> tenants::Model {
    let now = chrono::Utc::now();
    let slug = format!("channel-cache-{}", Uuid::new_v4());
    tenants::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set("Channel cache tenant".to_string()),
        slug: Set(slug),
        domain: Set(None),
        settings: Set(serde_json::json!({})),
        default_locale: Set("en".to_string()),
        is_active: Set(true),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .expect("tenant should insert")
}

fn tenant_context(tenant: &tenants::Model) -> TenantContext {
    TenantContext {
        id: tenant.id,
        name: tenant.name.clone(),
        slug: tenant.slug.clone(),
        domain: tenant.domain.clone(),
        settings: tenant.settings.clone(),
        default_locale: tenant.default_locale.clone(),
        is_active: tenant.is_active,
    }
}

fn channel_router(ctx: ServerRuntimeContext) -> Router {
    Router::new()
        .route("/channel-probe", get(channel_probe))
        .route_layer(axum_middleware::from_fn_with_state(
            ctx.clone(),
            channel_middleware::resolve,
        ))
        .with_state(ctx)
}

async fn request_channel_name(app: &Router, tenant: &TenantContext) -> Option<String> {
    let mut request = Request::builder()
        .uri("/channel-probe")
        .body(Body::empty())
        .expect("channel probe request");
    request
        .extensions_mut()
        .insert(TenantContextExtension(tenant.clone()));

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("channel probe should complete");
    assert!(response.status().is_success());
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("channel probe body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("channel probe JSON");
    payload["name"].as_str().map(ToString::to_string)
}

#[tokio::test]
#[ignore = "requires an isolated Redis instance via RUSTOK_CACHE_REAL_REDIS_URL"]
async fn redis_invalidation_refreshes_remote_resolved_channel_value_before_poll() {
    let url = std::env::var("RUSTOK_CACHE_REAL_REDIS_URL")
        .expect("RUSTOK_CACHE_REAL_REDIS_URL must point to an isolated Redis instance");
    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant = insert_tenant(&db).await;
    let tenant_context = tenant_context(&tenant);
    let service = ChannelService::new(db.clone());
    let channel = service
        .create_channel(CreateChannelInput {
            tenant_id: tenant.id,
            slug: "default".to_string(),
            name: "Before invalidation".to_string(),
            settings: None,
        })
        .await
        .expect("default channel should insert");

    let ctx_a = ServerRuntimeContext::new(db.clone(), settings_with_redis(url.as_str()));
    let ctx_b = ServerRuntimeContext::new(db.clone(), settings_with_redis(url.as_str()));
    let cache_a = ensure_cache_service(&ctx_a);
    let cache_b = ensure_cache_service(&ctx_b);
    assert!(cache_a.redis_client_initialized());
    assert!(cache_b.redis_client_initialized());

    let app_b = channel_router(ctx_b.clone());
    assert_eq!(
        request_channel_name(&app_b, &tenant_context)
            .await
            .as_deref(),
        Some("Before invalidation")
    );

    // Start the five-second reconciliation clocks only after replica B has
    // definitely cached the old value. A successful result inside the
    // three-second bound must therefore come from Redis delivery.
    start_channel_cache_invalidation_listener(&ctx_a, cache_a)
        .await
        .expect("publisher listener should start");
    start_channel_cache_invalidation_listener(&ctx_b, cache_b)
        .await
        .expect("remote listener should start");

    let model = channel::Entity::find_by_id(channel.id)
        .one(&db)
        .await
        .expect("channel lookup should succeed")
        .expect("channel should exist");
    let mut active: channel::ActiveModel = model.into();
    active.name = Set("After invalidation".to_string());
    active
        .update(&db)
        .await
        .expect("channel update should commit");

    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            publish_channel_resolution_invalidation(&ctx_a, tenant.id).await;
            if request_channel_name(&app_b, &tenant_context)
                .await
                .as_deref()
                == Some("After invalidation")
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("remote resolved channel value stayed stale until the periodic poll");
}
