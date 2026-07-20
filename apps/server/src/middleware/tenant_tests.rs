use super::{CachedTenantMiss, TenantCacheKeyBuilder, tenant_context_from_projection, unix_ms_at};
use crate::middleware::tenant_resolution::TenantIdentifierKind;
use rustok_tenant::TenantReadProjection;
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

fn sample_tenant_projection(is_active: bool) -> TenantReadProjection {
    TenantReadProjection {
        id: Uuid::new_v4(),
        name: "Demo tenant".to_string(),
        slug: "demo".to_string(),
        domain: Some("demo.example.test".to_string()),
        is_active,
        default_locale: "en".to_string(),
        settings: serde_json::json!({}),
    }
}

#[test]
fn canonical_tenant_keys_are_bounded_and_do_not_embed_long_identifiers() {
    let builder = TenantCacheKeyBuilder::new("v2");
    let identifier = format!("tenant-{}", "x".repeat(2_048));
    let key = builder.kind_key(TenantIdentifierKind::Slug, &identifier);
    let negative = builder.kind_negative_key(TenantIdentifierKind::Slug, &identifier);

    assert!(key.len() <= 512);
    assert!(negative.len() <= 512);
    assert!(!key.contains(&identifier));
    assert!(!negative.contains(&identifier));
}

#[test]
fn tenant_context_from_projection_maps_active_tenant() {
    let tenant = sample_tenant_projection(true);
    let context = tenant_context_from_projection(tenant.clone()).expect("active tenant");
    assert_eq!(context.id, tenant.id);
    assert_eq!(context.slug, tenant.slug);
    assert_eq!(context.default_locale, tenant.default_locale);
    assert!(context.is_active);
}

#[test]
fn tenant_context_from_projection_rejects_disabled_tenant_as_forbidden() {
    let result = tenant_context_from_projection(sample_tenant_projection(false));
    assert!(matches!(result, Err(CachedTenantMiss::Disabled)));
    assert_eq!(
        CachedTenantMiss::Disabled.status_code(),
        axum::http::StatusCode::FORBIDDEN
    );
}

#[test]
fn tenant_cache_timestamp_rejects_pre_epoch_clock() {
    assert_eq!(unix_ms_at(UNIX_EPOCH).expect("epoch"), 0);
    assert_eq!(
        unix_ms_at(UNIX_EPOCH + Duration::from_millis(42)).expect("timestamp"),
        42
    );
    assert!(unix_ms_at(UNIX_EPOCH - Duration::from_secs(1)).is_err());
}
