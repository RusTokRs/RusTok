use super::{
    resolve_identifier, should_bypass_tenant_resolution, subdomain_identifier,
    tenant_context_from_projection, CachedTenantMiss, TenantCacheKeyBuilder,
};
use crate::common::{RustokSettings, TenantFallbackMode};
use axum::{body::Body, http::Request};
use rustok_tenant::TenantReadProjection;
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
    let key = builder.kind_key(super::TenantIdentifierKind::Slug, &identifier);
    let negative = builder.kind_negative_key(super::TenantIdentifierKind::Slug, &identifier);

    assert!(key.len() <= 512);
    assert!(negative.len() <= 512);
    assert!(!key.contains(&identifier));
    assert!(!negative.contains(&identifier));
}

#[test]
fn bypasses_operator_endpoints_from_tenant_resolution() {
    assert!(should_bypass_tenant_resolution("/health/live"));
    assert!(should_bypass_tenant_resolution("/health/runtime"));
    assert!(should_bypass_tenant_resolution("/metrics"));
    assert!(should_bypass_tenant_resolution("/api/openapi.json"));
    assert!(should_bypass_tenant_resolution(
        "/api/graphql/schema.graphql"
    ));
    assert!(should_bypass_tenant_resolution("/api/graphql/ws"));
    assert!(should_bypass_tenant_resolution("/api/install/status"));
    assert!(should_bypass_tenant_resolution(
        "/api/install/jobs/018f2b7a-9d07-7f0a-9f71-0c9960e9168a"
    ));
    assert!(!should_bypass_tenant_resolution("/api/blog/posts"));
    assert!(!should_bypass_tenant_resolution("/api/installer/status"));
}

#[test]
fn strict_header_resolution_requires_tenant_header() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = "header".to_string();
    settings.tenant.fallback_mode = TenantFallbackMode::Disabled;

    let request = Request::builder()
        .uri("/api/users")
        .body(Body::empty())
        .expect("request");

    let result = resolve_identifier(&request, &settings);
    assert!(matches!(result, Err(axum::http::StatusCode::BAD_REQUEST)));
}

#[test]
fn header_resolution_can_fallback_to_default_tenant() {
    let mut settings = RustokSettings::default();
    settings.tenant.enabled = true;
    settings.tenant.resolution = "header".to_string();
    settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;

    let request = Request::builder()
        .uri("/api/users")
        .body(Body::empty())
        .expect("request");

    let result = resolve_identifier(&request, &settings).expect("identifier");
    assert_eq!(result.kind.as_str(), "uuid");
    assert_eq!(result.uuid, settings.tenant.default_id);
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
fn subdomain_resolution_extracts_single_label_slug() {
    let slug =
        subdomain_identifier("store.example.test", &[String::from("example.test")]).expect("slug");
    assert_eq!(slug, "store");
    assert_eq!(
        subdomain_identifier("example.test", &[String::from("example.test")]),
        Err(axum::http::StatusCode::BAD_REQUEST)
    );
    assert_eq!(
        subdomain_identifier("a.b.example.test", &[String::from("example.test")]),
        Err(axum::http::StatusCode::BAD_REQUEST)
    );
}
