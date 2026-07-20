use std::net::TcpListener;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use axum::{Router, body::Body, http::Request, middleware as axum_middleware, routing::get};
use rustok_cache::{CacheInvalidationMessage, CacheService, VersionedCacheInvalidation};
use rustok_migrations::Migrator;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use sha2::{Digest, Sha256};
use tokio::process::{Child, Command};
use tower::ServiceExt;
use uuid::Uuid;

use super::*;
use crate::common::settings::RustokSettings;
use crate::context::{TenantContext, TenantContextExtension};
use crate::middleware::locale as locale_middleware;
use crate::models::_entities::tenants;
use crate::services::cache_runtime::ensure_cache_service;

async fn locale_probe() -> &'static str {
    "ok"
}

fn settings_with_redis(url: &str) -> RustokSettings {
    let mut settings = RustokSettings::default();
    settings.cache.redis_url = Some(url.to_string());
    settings
}

async fn insert_tenant(db: &sea_orm::DatabaseConnection, name: &str) -> tenants::Model {
    let now = chrono::Utc::now();
    tenants::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(name.to_string()),
        slug: Set(format!("tenant-locale-{}", Uuid::new_v4())),
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

async fn insert_locale(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    locale: &str,
    is_default: bool,
    is_enabled: bool,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale, created_at) VALUES ('{}', '{}', '{}', '{}', '{}', {}, {}, NULL, CURRENT_TIMESTAMP)",
        Uuid::new_v4(),
        tenant_id,
        locale,
        locale,
        locale,
        u8::from(is_default),
        u8::from(is_enabled),
    ))
    .await
    .expect("tenant locale should insert");
}

async fn replace_default_locale(db: &sea_orm::DatabaseConnection, tenant_id: Uuid, locale: &str) {
    db.execute_unprepared(&format!(
        "UPDATE tenant_locales SET is_default = 0, is_enabled = 0 WHERE tenant_id = '{tenant_id}'"
    ))
    .await
    .expect("old tenant locales should disable");
    insert_locale(db, tenant_id, locale, true, true).await;
}

fn locale_router(ctx: ServerRuntimeContext) -> Router {
    Router::new()
        .route("/locale-probe", get(locale_probe))
        .route_layer(axum_middleware::from_fn_with_state(
            ctx.clone(),
            locale_middleware::resolve_locale,
        ))
        .with_state(ctx)
}

async fn request_locale(app: &Router, tenant: &TenantContext) -> String {
    let mut request = Request::builder()
        .uri("/locale-probe")
        .body(Body::empty())
        .expect("locale probe request");
    request
        .extensions_mut()
        .insert(TenantContextExtension(tenant.clone()));

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("locale probe should complete");
    assert!(response.status().is_success());
    response
        .headers()
        .get("content-language")
        .expect("locale middleware should set content-language")
        .to_str()
        .expect("content-language should be UTF-8")
        .to_string()
}

async fn wait_for_locale(app: &Router, tenant: &TenantContext, expected: &str, timeout: Duration) {
    tokio::time::timeout(timeout, async {
        loop {
            if request_locale(app, tenant).await == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("tenant locale did not converge to {expected}"));
}

async fn wait_for_readiness(
    handle: &TenantLocaleGenerationListenerHandle,
    expected: bool,
    timeout: Duration,
) {
    tokio::time::timeout(timeout, async {
        while handle.is_ready() != expected {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("tenant locale readiness did not become {expected}"));
}

fn invalidation_message(key: &str, generation: u64) -> CacheInvalidationMessage {
    VersionedCacheInvalidation::new(TENANT_CACHE_GENERATION_CHANNEL, key, generation, generation)
        .expect("tenant locale invalidation should be valid")
        .to_message()
        .expect("tenant locale invalidation should encode")
}

async fn bump_generation(cache: &CacheService) -> u64 {
    cache
        .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
        .await
        .expect("tenant cache generation should advance")
        .generation
}

async fn publish_until_locale(
    cache: &CacheService,
    app: &Router,
    tenant: &TenantContext,
    key: &str,
    generation: u64,
    expected: &str,
) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let outcome = cache
                .publish_invalidation(invalidation_message(key, generation))
                .await;
            assert!(outcome.redis_published);
            if request_locale(app, tenant).await == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("remote tenant locale stayed stale instead of becoming {expected}"));
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
                    .arg(TENANT_CACHE_GENERATION_CHANNEL)
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
    .unwrap_or_else(|_| panic!("Redis did not restore {expected} tenant locale subscribers"));
}

fn shared_generation_key() -> String {
    let digest = Sha256::digest(TENANT_CACHE_BACKEND_PREFIX.as_bytes());
    format!("rustok:cache-generation:v1:{}", hex::encode(digest))
}

async fn redis_generation_connection(url: &str) -> redis::aio::MultiplexedConnection {
    redis::Client::open(url)
        .expect("Redis URL should be valid")
        .get_multiplexed_async_connection()
        .await
        .expect("Redis generation fixture connection should open")
}

async fn restore_shared_generation(url: &str, generation: u64) {
    let mut connection = redis_generation_connection(url).await;
    let reply = redis::cmd("SET")
        .arg(shared_generation_key())
        .arg(generation)
        .query_async::<String>(&mut connection)
        .await
        .expect("tenant cache generation should be restored");
    assert_eq!(reply, "OK");
}

async fn align_shared_generation(url: &str) -> u64 {
    let snapshot = cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
        .expect("process tenant generation snapshot should be readable");
    let process_generation = snapshot.trusted.then_some(snapshot.generation).unwrap_or(0);
    let mut connection = redis_generation_connection(url).await;
    let shared_generation = redis::cmd("GET")
        .arg(shared_generation_key())
        .query_async::<Option<u64>>(&mut connection)
        .await
        .expect("shared tenant generation should be readable")
        .unwrap_or(0);
    let baseline = process_generation.max(shared_generation);
    let reply = redis::cmd("SET")
        .arg(shared_generation_key())
        .arg(baseline)
        .query_async::<String>(&mut connection)
        .await
        .expect("shared tenant generation should align");
    assert_eq!(reply, "OK");
    baseline
}

#[tokio::test]
#[serial_test::serial]
async fn exact_and_wildcard_invalidation_refresh_two_replica_locale_values() {
    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant_a = insert_tenant(&db, "Tenant locale A").await;
    let tenant_b = insert_tenant(&db, "Tenant locale B").await;
    insert_locale(&db, tenant_a.id, "en", true, true).await;
    insert_locale(&db, tenant_b.id, "en", true, true).await;
    let tenant_a_context = tenant_context(&tenant_a);
    let tenant_b_context = tenant_context(&tenant_b);

    let ctx_a = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let ctx_b = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let app_a = locale_router(ctx_a.clone());
    let app_b = locale_router(ctx_b.clone());
    let cache_a = CacheService::from_url(None);
    let cache_b = CacheService::from_url(None);
    assert!(!cache_a.redis_configuration_present());
    assert!(!cache_b.redis_configuration_present());

    let health_a = Arc::new(TenantLocaleGenerationHealth::default());
    let health_b = Arc::new(TenantLocaleGenerationHealth::default());
    let listener_a = TenantLocaleGenerationListener::new(ctx_a, cache_a.clone(), health_a.clone());
    let listener_b = TenantLocaleGenerationListener::new(ctx_b, cache_b, health_b.clone());
    listener_a.recover_if_advanced().await.unwrap();
    listener_b.recover_if_advanced().await.unwrap();

    assert_eq!(request_locale(&app_a, &tenant_a_context).await, "en");
    assert_eq!(request_locale(&app_a, &tenant_b_context).await, "en");
    assert_eq!(request_locale(&app_b, &tenant_a_context).await, "en");
    assert_eq!(request_locale(&app_b, &tenant_b_context).await, "en");

    replace_default_locale(&db, tenant_a.id, "fr").await;
    replace_default_locale(&db, tenant_b.id, "de").await;
    let exact_generation = bump_generation(&cache_a).await;
    let tenant_a_key = tenant_a.id.to_string();
    let exact = invalidation_message(tenant_a_key.as_str(), exact_generation);
    listener_a.handle_message(exact.clone()).await.unwrap();
    listener_b.handle_message(exact).await.unwrap();

    assert_eq!(request_locale(&app_a, &tenant_a_context).await, "fr");
    assert_eq!(request_locale(&app_b, &tenant_a_context).await, "fr");
    assert_eq!(request_locale(&app_a, &tenant_b_context).await, "en");
    assert_eq!(request_locale(&app_b, &tenant_b_context).await, "en");

    let wildcard_generation = bump_generation(&cache_a).await;
    let wildcard = invalidation_message("*", wildcard_generation);
    listener_a.handle_message(wildcard.clone()).await.unwrap();
    listener_b.handle_message(wildcard).await.unwrap();

    assert_eq!(request_locale(&app_a, &tenant_b_context).await, "de");
    assert_eq!(request_locale(&app_b, &tenant_b_context).await, "de");
    assert!(health_a.is_ready());
    assert!(health_b.is_ready());
}

#[tokio::test]
#[serial_test::serial]
async fn deterministic_local_lag_recovers_two_replica_locale_values() {
    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant = insert_tenant(&db, "Tenant locale lag").await;
    insert_locale(&db, tenant.id, "en", true, true).await;
    let tenant_context = tenant_context(&tenant);

    let ctx_a = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let ctx_b = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let app_a = locale_router(ctx_a.clone());
    let app_b = locale_router(ctx_b.clone());
    let cache = CacheService::from_url(None);
    assert!(!cache.redis_configuration_present());

    start_tenant_locale_generation_listener(&ctx_a, cache.clone()).await;
    start_tenant_locale_generation_listener(&ctx_b, cache.clone()).await;
    let handle_a = ctx_a
        .shared_get::<TenantLocaleGenerationListenerHandle>()
        .expect("first tenant locale listener handle");
    let handle_b = ctx_b
        .shared_get::<TenantLocaleGenerationListenerHandle>()
        .expect("second tenant locale listener handle");
    assert!(handle_a.is_ready());
    assert!(handle_b.is_ready());
    assert_eq!(request_locale(&app_a, &tenant_context).await, "en");
    assert_eq!(request_locale(&app_b, &tenant_context).await, "en");

    replace_default_locale(&db, tenant.id, "fr").await;
    let generation = bump_generation(&cache).await;
    let mut probe = cache
        .invalidations()
        .subscribe_local_channel(TENANT_CACHE_GENERATION_CHANNEL);

    // The local bus holds 256 messages. With no Redis, publication has no
    // suspension point, so both serving listeners and the probe lag together.
    for _ in 0..300 {
        let outcome = cache
            .publish_invalidation(invalidation_message("*", generation))
            .await;
        assert_eq!(outcome.local_subscribers, 3);
        assert!(!outcome.redis_published);
    }
    match probe.recv().await {
        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
            assert!(skipped >= 44);
        }
        other => panic!("expected deterministic tenant locale lag, got {other:?}"),
    }
    drop(probe);

    wait_for_locale(&app_a, &tenant_context, "fr", Duration::from_secs(2)).await;
    wait_for_locale(&app_b, &tenant_context, "fr", Duration::from_secs(2)).await;
    wait_for_readiness(&handle_a, true, Duration::from_secs(2)).await;
    wait_for_readiness(&handle_b, true, Duration::from_secs(2)).await;
}

#[tokio::test]
#[ignore = "requires an isolated Redis instance via RUSTOK_CACHE_REAL_REDIS_URL"]
#[serial_test::serial]
async fn missed_redis_publication_recovers_remote_locale_via_periodic_generation() {
    let url = std::env::var("RUSTOK_CACHE_REAL_REDIS_URL")
        .expect("RUSTOK_CACHE_REAL_REDIS_URL must point to an isolated Redis instance");
    let _baseline = align_shared_generation(url.as_str()).await;
    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant = insert_tenant(&db, "Tenant locale periodic").await;
    insert_locale(&db, tenant.id, "en", true, true).await;
    let tenant_context = tenant_context(&tenant);

    let ctx_a = ServerRuntimeContext::new(db.clone(), settings_with_redis(url.as_str()));
    let ctx_b = ServerRuntimeContext::new(db.clone(), settings_with_redis(url.as_str()));
    let cache_a = ensure_cache_service(&ctx_a);
    let cache_b = ensure_cache_service(&ctx_b);
    assert!(cache_a.redis_client_initialized());
    assert!(cache_b.redis_client_initialized());
    let app_b = locale_router(ctx_b.clone());

    let health_b = Arc::new(TenantLocaleGenerationHealth::default());
    let listener_b = TenantLocaleGenerationListener::new(ctx_b, cache_b, Arc::clone(&health_b));
    listener_b.recover_if_advanced().await.unwrap();
    assert_eq!(request_locale(&app_b, &tenant_context).await, "en");

    replace_default_locale(&db, tenant.id, "fr").await;
    let _generation = bump_generation(&cache_a).await;
    assert_eq!(request_locale(&app_b, &tenant_context).await, "en");

    // No PubSub publication occurs. A short test interval exercises the same
    // production reconciliation loop without waiting thirty seconds.
    let periodic = tokio::spawn(run_periodic_reconciliation_with_interval(
        listener_b,
        Duration::from_millis(50),
    ));
    wait_for_locale(&app_b, &tenant_context, "fr", Duration::from_secs(3)).await;
    assert!(health_b.is_ready());
    periodic.abort();
}

#[tokio::test]
#[ignore = "requires redis-server via RUSTOK_CACHE_REDIS_SERVER_BIN"]
#[serial_test::serial]
async fn redis_restart_fails_closed_until_generation_is_restored() {
    let binary = std::env::var("RUSTOK_CACHE_REDIS_SERVER_BIN")
        .expect("RUSTOK_CACHE_REDIS_SERVER_BIN must point to redis-server");
    let port = reserve_loopback_port();
    let url = format!("redis://127.0.0.1:{port}/");
    let mut redis_process = spawn_redis(binary.as_str(), port).await;
    let _baseline = align_shared_generation(url.as_str()).await;

    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant = insert_tenant(&db, "Tenant locale reconnect").await;
    insert_locale(&db, tenant.id, "en", true, true).await;
    let tenant_context = tenant_context(&tenant);
    let tenant_key = tenant.id.to_string();

    let ctx_a = ServerRuntimeContext::new(db.clone(), settings_with_redis(url.as_str()));
    let ctx_b = ServerRuntimeContext::new(db.clone(), settings_with_redis(url.as_str()));
    let cache_a = ensure_cache_service(&ctx_a);
    let cache_b = ensure_cache_service(&ctx_b);
    let app_b = locale_router(ctx_b.clone());

    start_tenant_locale_generation_listener(&ctx_a, cache_a.clone()).await;
    start_tenant_locale_generation_listener(&ctx_b, cache_b).await;
    let handle_a = ctx_a
        .shared_get::<TenantLocaleGenerationListenerHandle>()
        .expect("publisher tenant locale listener handle");
    let handle_b = ctx_b
        .shared_get::<TenantLocaleGenerationListenerHandle>()
        .expect("remote tenant locale listener handle");
    wait_for_redis_subscribers(url.as_str(), 2).await;
    assert!(handle_a.is_ready());
    assert!(handle_b.is_ready());
    assert_eq!(request_locale(&app_b, &tenant_context).await, "en");

    replace_default_locale(&db, tenant.id, "fr").await;
    let before_restart = bump_generation(&cache_a).await;
    publish_until_locale(
        &cache_a,
        &app_b,
        &tenant_context,
        tenant_key.as_str(),
        before_restart,
        "fr",
    )
    .await;

    stop_redis(&mut redis_process).await;
    wait_for_readiness(&handle_a, false, Duration::from_secs(5)).await;
    wait_for_readiness(&handle_b, false, Duration::from_secs(5)).await;
    replace_default_locale(&db, tenant.id, "de").await;
    assert_eq!(request_locale(&app_b, &tenant_context).await, "fr");

    redis_process = spawn_redis(binary.as_str(), port).await;
    wait_for_redis_subscribers(url.as_str(), 2).await;

    // Empty Redis has generation zero while both replicas retain a trusted
    // higher snapshot. Ready recovery must clear stale values but remain failed.
    wait_for_locale(&app_b, &tenant_context, "de", Duration::from_secs(3)).await;
    assert!(!handle_a.is_ready());
    assert!(!handle_b.is_ready());

    restore_shared_generation(url.as_str(), before_restart).await;
    replace_default_locale(&db, tenant.id, "ru").await;
    let after_restore = bump_generation(&cache_a).await;
    assert_eq!(after_restore, before_restart + 1);
    publish_until_locale(
        &cache_a,
        &app_b,
        &tenant_context,
        tenant_key.as_str(),
        after_restore,
        "ru",
    )
    .await;
    wait_for_readiness(&handle_a, true, Duration::from_secs(3)).await;
    wait_for_readiness(&handle_b, true, Duration::from_secs(3)).await;

    stop_redis(&mut redis_process).await;
}
