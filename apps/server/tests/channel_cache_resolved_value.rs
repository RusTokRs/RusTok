use std::{net::TcpListener, process::Stdio, time::Duration};

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    http::Request,
    middleware as axum_middleware,
    routing::get,
};
use rustok_channel::{ChannelService, CreateChannelInput, entities::channel};
use rustok_migrations::Migrator;
use rustok_server::{
    common::settings::RustokSettings,
    context::{OptionalChannel, TenantContext, TenantContextExtension},
    middleware::channel as channel_middleware,
    models::_entities::tenants,
    services::{
        cache_runtime::ensure_cache_service,
        channel_cache_invalidation::{
            CHANNEL_RESOLUTION_INVALIDATION_CHANNEL, ChannelCacheInvalidationListenerHandle,
            publish_channel_resolution_invalidation, start_channel_cache_invalidation_listener,
        },
        server_runtime_context::ServerRuntimeContext,
    },
};
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};
use tokio::process::{Child, Command};
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

async fn insert_default_channel(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
) -> rustok_channel::ChannelResponse {
    ChannelService::new(db.clone())
        .create_channel(CreateChannelInput {
            tenant_id,
            slug: "default".to_string(),
            name: "Before invalidation".to_string(),
            settings: None,
        })
        .await
        .expect("default channel should insert")
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

async fn wait_for_channel_name(
    app: &Router,
    tenant: &TenantContext,
    expected: &str,
    timeout: Duration,
) {
    tokio::time::timeout(timeout, async {
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

async fn wait_for_readiness(
    handle: &ChannelCacheInvalidationListenerHandle,
    expected: bool,
    timeout: Duration,
) {
    tokio::time::timeout(timeout, async {
        while handle.is_ready() != expected {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("channel invalidation readiness did not become {expected}"));
}

async fn install_generation_state(db: &sea_orm::DatabaseConnection, generation: u64) {
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

fn reserve_loopback_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("loopback port should be reservable")
        .local_addr()
        .expect("reserved loopback address")
        .port()
}

async fn wait_for_redis(url: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(client) = redis::Client::open(url)
                && let Ok(mut connection) = client.get_multiplexed_async_connection().await
            {
                let pong = redis::cmd("PING")
                    .query_async::<String>(&mut connection)
                    .await;
                if pong.as_deref() == Ok("PONG") {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("spawned Redis did not become ready");
}

async fn spawn_redis(binary: &str, port: u16) -> Child {
    let child = Command::new(binary)
        .arg("--bind")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--save")
        .arg("")
        .arg("--appendonly")
        .arg("no")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("redis-server should start");
    wait_for_redis(&format!("redis://127.0.0.1:{port}/")).await;
    child
}

async fn stop_redis(child: &mut Child) {
    child.kill().await.expect("redis-server should stop");
    child.wait().await.expect("redis-server should be reaped");
}

async fn wait_for_redis_subscribers(url: &str, expected: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(client) = redis::Client::open(url)
                && let Ok(mut connection) = client.get_multiplexed_async_connection().await
            {
                let counts = redis::cmd("PUBSUB")
                    .arg("NUMSUB")
                    .arg(CHANNEL_RESOLUTION_INVALIDATION_CHANNEL)
                    .query_async::<Vec<(String, usize)>>(&mut connection)
                    .await;
                if counts
                    .ok()
                    .and_then(|counts| counts.into_iter().next())
                    .is_some_and(|(_, count)| count >= expected)
                {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("Redis did not restore {expected} channel subscribers"));
}

#[tokio::test]
async fn missed_publication_refreshes_remote_resolved_value_via_durable_poll() {
    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant = insert_tenant(&db).await;
    let tenant_context = tenant_context(&tenant);
    let channel = insert_default_channel(&db, tenant.id).await;

    let ctx_b = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let cache_b = ensure_cache_service(&ctx_b);
    assert!(!cache_b.redis_configuration_present());
    start_channel_cache_invalidation_listener(&ctx_b, cache_b)
        .await
        .expect("remote listener should start");
    let app_b = channel_router(ctx_b);

    assert_eq!(
        request_channel_name(&app_b, &tenant_context)
            .await
            .as_deref(),
        Some("Before invalidation")
    );
    rename_channel(&db, channel.id, "After invalidation").await;

    // No publication occurs. The stale value must remain visible until the
    // durable generation worker reaches its next database reconciliation.
    assert_eq!(
        request_channel_name(&app_b, &tenant_context)
            .await
            .as_deref(),
        Some("Before invalidation")
    );

    wait_for_channel_name(
        &app_b,
        &tenant_context,
        "After invalidation",
        Duration::from_secs(7),
    )
    .await;
}

#[tokio::test]
async fn database_state_loss_fails_closed_and_recovers_resolved_value() {
    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant = insert_tenant(&db).await;
    let tenant_context = tenant_context(&tenant);
    let channel = insert_default_channel(&db, tenant.id).await;

    let ctx_b = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let cache_b = ensure_cache_service(&ctx_b);
    start_channel_cache_invalidation_listener(&ctx_b, cache_b)
        .await
        .expect("remote listener should start");
    let handle = ctx_b
        .shared_get::<ChannelCacheInvalidationListenerHandle>()
        .expect("remote listener handle");
    let app_b = channel_router(ctx_b);

    assert_eq!(
        request_channel_name(&app_b, &tenant_context)
            .await
            .as_deref(),
        Some("Before invalidation")
    );
    rename_channel(&db, channel.id, "After database recovery").await;
    let generation = rustok_channel::read_resolution_invalidation_generation(&db)
        .await
        .expect("generation should be readable before state loss");
    db.execute_unprepared("DROP TABLE channel_resolution_invalidation_state")
        .await
        .expect("generation state should be removable for the outage fixture");

    assert_eq!(
        request_channel_name(&app_b, &tenant_context)
            .await
            .as_deref(),
        Some("Before invalidation")
    );
    wait_for_readiness(&handle, false, Duration::from_secs(7)).await;

    install_generation_state(&db, generation).await;
    wait_for_channel_name(
        &app_b,
        &tenant_context,
        "After database recovery",
        Duration::from_secs(7),
    )
    .await;
    assert!(handle.is_ready());
}

#[tokio::test]
async fn generation_regression_rebuilds_remote_resolved_value() {
    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant = insert_tenant(&db).await;
    let tenant_context = tenant_context(&tenant);
    let channel = insert_default_channel(&db, tenant.id).await;

    let ctx_b = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let cache_b = ensure_cache_service(&ctx_b);
    start_channel_cache_invalidation_listener(&ctx_b, cache_b)
        .await
        .expect("remote listener should start");
    let app_b = channel_router(ctx_b);

    assert_eq!(
        request_channel_name(&app_b, &tenant_context)
            .await
            .as_deref(),
        Some("Before invalidation")
    );
    rename_channel(&db, channel.id, "Forward generation").await;
    wait_for_channel_name(
        &app_b,
        &tenant_context,
        "Forward generation",
        Duration::from_secs(7),
    )
    .await;

    db.execute_unprepared(
        "UPDATE channel_resolution_invalidation_state SET generation = 0 WHERE scope = 'resolution'",
    )
    .await
    .expect("generation should be regressed for the recovery fixture");
    rename_channel(&db, channel.id, "After generation regression").await;

    assert_eq!(
        request_channel_name(&app_b, &tenant_context)
            .await
            .as_deref(),
        Some("Forward generation")
    );
    wait_for_channel_name(
        &app_b,
        &tenant_context,
        "After generation regression",
        Duration::from_secs(7),
    )
    .await;
}

#[tokio::test]
#[ignore = "requires an isolated Redis instance via RUSTOK_CACHE_REAL_REDIS_URL"]
async fn redis_invalidation_refreshes_remote_resolved_channel_value_before_poll() {
    let url = std::env::var("RUSTOK_CACHE_REAL_REDIS_URL")
        .expect("RUSTOK_CACHE_REAL_REDIS_URL must point to an isolated Redis instance");
    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant = insert_tenant(&db).await;
    let tenant_context = tenant_context(&tenant);
    let channel = insert_default_channel(&db, tenant.id).await;

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

    rename_channel(&db, channel.id, "After invalidation").await;

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

#[tokio::test]
#[ignore = "requires redis-server via RUSTOK_CACHE_REDIS_SERVER_BIN"]
async fn redis_restart_reconnects_existing_replicas_and_refreshes_value_before_poll() {
    let binary = std::env::var("RUSTOK_CACHE_REDIS_SERVER_BIN")
        .expect("RUSTOK_CACHE_REDIS_SERVER_BIN must point to redis-server");
    let port = reserve_loopback_port();
    let url = format!("redis://127.0.0.1:{port}/");
    let mut redis_process = spawn_redis(binary.as_str(), port).await;

    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant = insert_tenant(&db).await;
    let tenant_context = tenant_context(&tenant);
    let channel = insert_default_channel(&db, tenant.id).await;

    let ctx_a = ServerRuntimeContext::new(db.clone(), settings_with_redis(url.as_str()));
    let ctx_b = ServerRuntimeContext::new(db.clone(), settings_with_redis(url.as_str()));
    let cache_a = ensure_cache_service(&ctx_a);
    let cache_b = ensure_cache_service(&ctx_b);
    let app_b = channel_router(ctx_b.clone());

    start_channel_cache_invalidation_listener(&ctx_a, cache_a)
        .await
        .expect("publisher listener should start");
    start_channel_cache_invalidation_listener(&ctx_b, cache_b)
        .await
        .expect("remote listener should start");
    wait_for_redis_subscribers(url.as_str(), 2).await;
    assert_eq!(
        request_channel_name(&app_b, &tenant_context)
            .await
            .as_deref(),
        Some("Before invalidation")
    );

    stop_redis(&mut redis_process).await;
    rename_channel(&db, channel.id, "During Redis outage").await;
    assert_eq!(
        request_channel_name(&app_b, &tenant_context)
            .await
            .as_deref(),
        Some("Before invalidation")
    );
    wait_for_channel_name(
        &app_b,
        &tenant_context,
        "During Redis outage",
        Duration::from_secs(7),
    )
    .await;

    redis_process = spawn_redis(binary.as_str(), port).await;
    wait_for_redis_subscribers(url.as_str(), 2).await;

    // Anchor immediately after a durable poll. The final three-second window
    // is therefore shorter than the next five-second reconciliation tick.
    rename_channel(&db, channel.id, "Reconnect baseline").await;
    wait_for_channel_name(
        &app_b,
        &tenant_context,
        "Reconnect baseline",
        Duration::from_secs(7),
    )
    .await;
    rename_channel(&db, channel.id, "After Redis reconnect").await;
    assert_eq!(
        request_channel_name(&app_b, &tenant_context)
            .await
            .as_deref(),
        Some("Reconnect baseline")
    );

    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            publish_channel_resolution_invalidation(&ctx_a, tenant.id).await;
            if request_channel_name(&app_b, &tenant_context)
                .await
                .as_deref()
                == Some("After Redis reconnect")
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("existing remote replica did not consume invalidation after Redis restart");

    stop_redis(&mut redis_process).await;
}
