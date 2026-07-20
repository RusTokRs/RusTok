use std::time::Duration;

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    http::Request,
    middleware as axum_middleware,
    routing::get,
};
use rustok_cache::{CacheInvalidationMessage, VersionedCacheInvalidation};
use rustok_channel::{ChannelService, CreateChannelInput, entities::channel};
use rustok_migrations::Migrator;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};
use tower::ServiceExt;
use uuid::Uuid;

use super::cache_runtime::ensure_cache_service;
use super::channel_cache_invalidation::{
    CHANNEL_RESOLUTION_INVALIDATION_CHANNEL, ChannelCacheInvalidationListenerHandle,
    start_channel_cache_invalidation_listener,
};
use super::server_runtime_context::ServerRuntimeContext;
use crate::common::settings::RustokSettings;
use crate::context::{OptionalChannel, TenantContext, TenantContextExtension};
use crate::middleware::channel as channel_middleware;
use crate::models::_entities::tenants;

const CHANNEL_RESOLUTION_INVALIDATION_KEY: &str = "*";

async fn channel_probe(OptionalChannel(channel): OptionalChannel) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": channel.map(|channel| channel.name),
    }))
}

async fn insert_tenant(db: &sea_orm::DatabaseConnection) -> tenants::Model {
    let now = chrono::Utc::now();
    tenants::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set("Channel lag tenant".to_string()),
        slug: Set(format!("channel-lag-{}", Uuid::new_v4())),
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

async fn rename_channel(db: &sea_orm::DatabaseConnection, channel_id: Uuid, name: &str) {
    let model = channel::Entity::find_by_id(channel_id)
        .one(db)
        .await
        .expect("channel lookup should succeed")
        .expect("channel should exist");
    let mut active: channel::ActiveModel = model.into();
    active.name = Set(name.to_string());
    active
        .update(db)
        .await
        .expect("channel update should commit");
}

fn invalidation_message(generation: u64) -> CacheInvalidationMessage {
    VersionedCacheInvalidation::new(
        CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
        CHANNEL_RESOLUTION_INVALIDATION_KEY,
        generation,
        generation,
    )
    .expect("versioned invalidation should be valid")
    .to_message()
    .expect("invalidation message should encode")
}

async fn wait_for_readiness(handle: &ChannelCacheInvalidationListenerHandle, expected: bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while handle.is_ready() != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("channel invalidation readiness did not become {expected}"));
}

async fn wait_for_channel_name(app: &Router, tenant: &TenantContext, expected: &str) {
    tokio::time::timeout(Duration::from_secs(7), async {
        loop {
            if request_channel_name(app, tenant).await.as_deref() == Some(expected) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("channel value did not converge to {expected}"));
}

async fn restore_generation_state(db: &sea_orm::DatabaseConnection, generation: u64) {
    db.execute_unprepared(
        "CREATE TABLE channel_resolution_invalidation_state (scope TEXT PRIMARY KEY, generation BIGINT NOT NULL)",
    )
    .await
    .expect("generation state table should be restored");
    db.execute_unprepared(&format!(
        "INSERT INTO channel_resolution_invalidation_state (scope, generation) VALUES ('resolution', {generation})"
    ))
    .await
    .expect("generation state row should be restored");
}

#[tokio::test]
async fn local_listener_lag_recovers_readiness_and_remote_resolved_value() {
    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant = insert_tenant(&db).await;
    let tenant_context = tenant_context(&tenant);
    let channel = ChannelService::new(db.clone())
        .create_channel(CreateChannelInput {
            tenant_id: tenant.id,
            slug: "default".to_string(),
            name: "Before listener lag".to_string(),
            settings: None,
        })
        .await
        .expect("default channel should insert");

    let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let cache = ensure_cache_service(&ctx);
    assert!(!cache.redis_configuration_present());
    start_channel_cache_invalidation_listener(&ctx, cache.clone())
        .await
        .expect("channel invalidation listener should start");
    let handle = ctx
        .shared_get::<ChannelCacheInvalidationListenerHandle>()
        .expect("channel invalidation listener handle");
    assert!(handle.is_ready());

    let app = channel_router(ctx);
    assert_eq!(
        request_channel_name(&app, &tenant_context).await.as_deref(),
        Some("Before listener lag")
    );

    rename_channel(&db, channel.id, "After listener lag").await;
    let generation = rustok_channel::read_resolution_invalidation_generation(&db)
        .await
        .expect("committed generation should be readable");
    db.execute_unprepared("DROP TABLE channel_resolution_invalidation_state")
        .await
        .expect("generation state should be removable for the lag fixture");

    let mut probe = cache
        .invalidations()
        .subscribe_local_channel(CHANNEL_RESOLUTION_INVALIDATION_CHANNEL);

    // The local bus holds 256 messages. With no Redis configured, publication
    // has no suspension point after broadcast::send, so this burst overflows
    // both the serving listener and the probe before either can drain.
    for _ in 0..300 {
        let outcome = cache
            .publish_invalidation(invalidation_message(generation))
            .await;
        assert_eq!(outcome.local_subscribers, 2);
        assert!(!outcome.redis_published);
    }

    match probe.recv().await {
        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
            assert!(skipped >= 44);
        }
        other => panic!("expected deterministic local invalidation lag, got {other:?}"),
    }
    drop(probe);

    wait_for_readiness(&handle, false).await;
    assert_eq!(
        request_channel_name(&app, &tenant_context).await.as_deref(),
        Some("Before listener lag")
    );

    // No replacement fast-path publication is sent. Restoring the durable
    // source must let reconciliation rotate the namespace, recover readiness
    // and replace the actual value returned by the serving middleware.
    restore_generation_state(&db, generation).await;
    wait_for_channel_name(&app, &tenant_context, "After listener lag").await;
    wait_for_readiness(&handle, true).await;
}
