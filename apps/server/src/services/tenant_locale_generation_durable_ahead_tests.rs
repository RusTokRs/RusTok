use std::sync::Arc;

use axum::{Router, body::Body, http::Request, middleware as axum_middleware, routing::get};
use rustok_cache::{CacheService, VersionedCacheInvalidation};
use rustok_migrations::Migrator;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use tower::ServiceExt;
use uuid::Uuid;

use super::*;
use crate::common::settings::RustokSettings;
use crate::context::{TenantContext, TenantContextExtension};
use crate::middleware::locale as locale_middleware;
use crate::models::_entities::tenants;

async fn locale_probe() -> &'static str {
    "ok"
}

async fn insert_tenant(db: &sea_orm::DatabaseConnection, name: &str) -> tenants::Model {
    let now = chrono::Utc::now();
    tenants::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(name.to_string()),
        slug: Set(format!("tenant-locale-durable-ahead-{}", Uuid::new_v4())),
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

#[tokio::test]
#[serial_test::serial]
async fn durable_generation_ahead_of_exact_event_forces_full_clear() {
    let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
    let tenant_a = insert_tenant(&db, "Durable ahead A").await;
    let tenant_b = insert_tenant(&db, "Durable ahead B").await;
    insert_locale(&db, tenant_a.id, "en", true, true).await;
    insert_locale(&db, tenant_b.id, "en", true, true).await;
    let tenant_a_context = tenant_context(&tenant_a);
    let tenant_b_context = tenant_context(&tenant_b);

    let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let app = locale_router(ctx.clone());
    let cache = CacheService::from_url(None);
    assert!(!cache.redis_configuration_present());
    let health = Arc::new(TenantLocaleGenerationHealth::default());
    let listener = TenantLocaleGenerationListener::new(ctx, cache.clone(), Arc::clone(&health));
    listener.recover_if_advanced().await.unwrap();

    assert_eq!(request_locale(&app, &tenant_a_context).await, "en");
    assert_eq!(request_locale(&app, &tenant_b_context).await, "en");

    replace_default_locale(&db, tenant_a.id, "fr").await;
    replace_default_locale(&db, tenant_b.id, "de").await;
    let received_generation = cache
        .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
        .await
        .expect("first tenant generation should advance")
        .generation;
    let durable_generation = cache
        .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
        .await
        .expect("second tenant generation should advance")
        .generation;
    assert_eq!(durable_generation, received_generation + 1);

    let tenant_a_key = tenant_a.id.to_string();
    let message = VersionedCacheInvalidation::new(
        TENANT_CACHE_GENERATION_CHANNEL,
        tenant_a_key,
        received_generation,
        received_generation,
    )
    .expect("tenant locale invalidation should be valid")
    .to_message()
    .expect("tenant locale invalidation should encode");
    listener.handle_message(message).await.unwrap();

    // Although the received key targets tenant A, the durable generation proves
    // that another invalidation was missed. Both tenant entries must be rebuilt.
    assert_eq!(request_locale(&app, &tenant_a_context).await, "fr");
    assert_eq!(request_locale(&app, &tenant_b_context).await, "de");
    assert_eq!(
        listener
            .tracker
            .last_generation(TENANT_CACHE_GENERATION_CHANNEL),
        Some(durable_generation)
    );
    assert!(health.is_ready());
}
