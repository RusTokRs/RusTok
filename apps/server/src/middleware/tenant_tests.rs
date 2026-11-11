use super::{
    CachedTenantContext, CachedTenantMiss, TenantCacheKeyBuilder, TenantContextLoadError,
    TenantResolutionTransport, core_error_to_load_error, duration_millis_ceil,
    normalize_identifier_value, tenant_context_from_projection, tenant_port_error_to_core_error,
    tenant_read_context, tenant_read_request, unix_ms_at,
};
use crate::middleware::tenant_resolution::{ResolvedTenantIdentifier, TenantIdentifierKind};
use rustok_api::PortActorKind;
use rustok_core::Error as CoreError;
use rustok_tenant::{PortError, TenantReadProjection, TenantReadSelector};
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
fn tenant_cache_keys_change_with_generation_without_exposing_raw_identity() {
    let builder = TenantCacheKeyBuilder::new("v2");
    let current = builder.kind_key(TenantIdentifierKind::Slug, "demo", 10);
    let next = builder.kind_key(TenantIdentifierKind::Slug, "demo", 11);
    let current_negative = builder.kind_negative_key(TenantIdentifierKind::Slug, "demo", 10);
    let next_negative = builder.kind_negative_key(TenantIdentifierKind::Slug, "demo", 11);

    assert_ne!(current, next);
    assert_ne!(current_negative, next_negative);
    assert_ne!(current, current_negative);
    assert!(!current.contains("demo"));
    assert!(!next.contains("demo"));
}

#[test]
fn canonical_tenant_keys_are_bounded_and_do_not_embed_long_identifiers() {
    let builder = TenantCacheKeyBuilder::new("v2");
    let identifier = format!("tenant-{}", "x".repeat(2_048));
    let key = builder.kind_key(TenantIdentifierKind::Slug, &identifier, 1);
    let negative = builder.kind_negative_key(TenantIdentifierKind::Slug, &identifier, 1);

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
fn cached_tenant_context_round_trips_structured_settings() {
    let projection = sample_tenant_projection(true);
    let mut context = tenant_context_from_projection(projection).expect("active tenant");
    context.settings = serde_json::json!({
        "checkout": {
            "guest_enabled": true,
            "limits": [1, 2, 3]
        }
    });

    let cached = CachedTenantContext::from(context.clone());
    let envelope = rustok_cache::CacheEnvelope::new(3, 1_000, cached).expect("build envelope");
    let encoded = envelope.encode().expect("encode Postcard envelope");
    let decoded = rustok_cache::CacheEnvelope::<CachedTenantContext>::decode(&encoded, 3)
        .expect("decode Postcard envelope");
    let restored = crate::context::TenantContext::from(decoded.into_payload());

    assert_eq!(restored.id, context.id);
    assert_eq!(restored.name, context.name);
    assert_eq!(restored.slug, context.slug);
    assert_eq!(restored.domain, context.domain);
    assert_eq!(restored.settings, context.settings);
    assert_eq!(restored.default_locale, context.default_locale);
    assert_eq!(restored.is_active, context.is_active);
}

#[test]
fn tenant_context_from_projection_rejects_disabled_tenant_as_forbidden() {
    let result = tenant_context_from_projection(sample_tenant_projection(false));
    assert!(matches!(result, Err(CachedTenantMiss::Disabled)));
    let error = TenantContextLoadError::from(CachedTenantMiss::Disabled);
    assert_eq!(error.status_code(), axum::http::StatusCode::FORBIDDEN);
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

#[test]
fn tenant_context_load_error_codes_and_responses() {
    let not_found = TenantContextLoadError::NotFound;
    assert_eq!(not_found.error_code(), "tenant_not_found");
    assert_eq!(not_found.status_code(), axum::http::StatusCode::NOT_FOUND);
    assert_eq!(not_found.client_message(), "Tenant not found");

    let disabled = TenantContextLoadError::Disabled;
    assert_eq!(disabled.error_code(), "tenant_disabled");
    assert_eq!(disabled.status_code(), axum::http::StatusCode::FORBIDDEN);
    assert_eq!(disabled.client_message(), "Tenant is disabled");

    let invalid_assertion = TenantContextLoadError::InvalidAssertion("mismatch".to_string());
    assert_eq!(
        invalid_assertion.error_code(),
        "conflicting_tenant_assertions"
    );
    assert_eq!(
        invalid_assertion.status_code(),
        axum::http::StatusCode::BAD_REQUEST
    );

    let infra = TenantContextLoadError::InfrastructureUnavailable;
    assert_eq!(infra.error_code(), "tenant_infrastructure_unavailable");
    assert_eq!(
        infra.status_code(),
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(infra.client_message(), "Failed to resolve tenant");
}

#[test]
fn normalize_identifier_value_and_key_builder_host_lowercasing() {
    assert_eq!(
        normalize_identifier_value(TenantIdentifierKind::Host, "Demo.Example.COM"),
        "demo.example.com"
    );
    assert_eq!(
        normalize_identifier_value(TenantIdentifierKind::Slug, "DemoSlug"),
        "DemoSlug"
    );
    assert_eq!(
        normalize_identifier_value(
            TenantIdentifierKind::Uuid,
            "550e8400-e29b-41d4-a716-446655440000"
        ),
        "550e8400-e29b-41d4-a716-446655440000"
    );

    let builder = TenantCacheKeyBuilder::new("v2");
    let key_upper = builder.kind_key(TenantIdentifierKind::Host, "SHOP.RusToK.Test", 42);
    let key_lower = builder.kind_key(TenantIdentifierKind::Host, "shop.rustok.test", 42);
    assert_eq!(key_upper, key_lower, "host-based keys must normalize case");

    let neg_upper =
        builder.kind_negative_key(TenantIdentifierKind::Host, "SHOP.RusToK.Test", 42);
    let neg_lower =
        builder.kind_negative_key(TenantIdentifierKind::Host, "shop.rustok.test", 42);
    assert_eq!(
        neg_upper, neg_lower,
        "host-based negative keys must normalize case"
    );
}

#[test]
fn duration_millis_ceil_behavior_and_extremes() {
    assert_eq!(duration_millis_ceil(Duration::ZERO), 0);
    assert_eq!(duration_millis_ceil(Duration::from_nanos(1)), 1);
    assert_eq!(duration_millis_ceil(Duration::from_nanos(999_999)), 1);
    assert_eq!(duration_millis_ceil(Duration::from_nanos(1_000_000)), 1);
    assert_eq!(duration_millis_ceil(Duration::from_nanos(1_000_001)), 2);
    assert_eq!(duration_millis_ceil(Duration::from_millis(50)), 50);
    assert_eq!(duration_millis_ceil(Duration::from_secs(3)), 3000);
    assert_eq!(duration_millis_ceil(Duration::MAX), u64::MAX);
}

#[test]
fn core_error_to_load_error_complete_mapping() {
    assert!(matches!(
        core_error_to_load_error(CoreError::NotFound("tenant missing".into())),
        TenantContextLoadError::NotFound
    ));
    assert!(matches!(
        core_error_to_load_error(CoreError::Forbidden("tenant disabled".into())),
        TenantContextLoadError::Disabled
    ));
    assert!(matches!(
        core_error_to_load_error(CoreError::Validation("invalid slug".into())),
        TenantContextLoadError::InvalidIdentifier(ref s) if s == "invalid slug"
    ));
    assert!(matches!(
        core_error_to_load_error(CoreError::Cache("redis timeout".into())),
        TenantContextLoadError::CacheUnavailable(ref s) if s == "redis timeout"
    ));
    assert!(matches!(
        core_error_to_load_error(CoreError::Database(sea_orm::DbErr::RecordNotFound(
            "none".into()
        ))),
        TenantContextLoadError::BackendUnavailable(_)
    ));
}

#[test]
fn tenant_port_error_to_core_error_complete_mapping() {
    let nf = tenant_port_error_to_core_error(PortError::not_found("test", "not found"));
    assert!(matches!(nf, CoreError::NotFound(msg) if msg == "not found"));

    let val = tenant_port_error_to_core_error(PortError::validation("test", "bad val"));
    assert!(matches!(val, CoreError::Validation(msg) if msg == "bad val"));

    let timeout = tenant_port_error_to_core_error(PortError::timeout("test", "timeout"));
    assert!(matches!(timeout, CoreError::Database(_)));

    let unavail = tenant_port_error_to_core_error(PortError::unavailable("test", "offline"));
    assert!(matches!(unavail, CoreError::Database(_)));

    let inv = tenant_port_error_to_core_error(PortError::invariant_violation("test", "broken"));
    assert!(matches!(inv, CoreError::Database(_)));

    let forb = tenant_port_error_to_core_error(PortError::forbidden("test", "forbidden"));
    assert!(matches!(forb, CoreError::Forbidden(msg) if msg == "forbidden"));

    let conflict = tenant_port_error_to_core_error(PortError::conflict("test", "conflict"));
    assert!(matches!(conflict, CoreError::Forbidden(msg) if msg == "conflict"));
}

#[test]
fn tenant_read_request_selector_and_inactive_inclusion() {
    let test_uuid = Uuid::new_v4();
    let req_uuid = tenant_read_request(&ResolvedTenantIdentifier::Uuid(test_uuid));
    assert_eq!(req_uuid.selector, TenantReadSelector::Id(test_uuid));
    assert!(
        req_uuid.include_inactive,
        "must include inactive to distinguish disabled from missing"
    );

    let req_slug = tenant_read_request(&ResolvedTenantIdentifier::Slug("demo-slug".into()));
    assert_eq!(
        req_slug.selector,
        TenantReadSelector::Slug("demo-slug".into())
    );
    assert!(req_slug.include_inactive);

    let req_host =
        tenant_read_request(&ResolvedTenantIdentifier::Host("demo.domain.test".into()));
    assert_eq!(
        req_host.selector,
        TenantReadSelector::Domain("demo.domain.test".into())
    );
    assert!(req_host.include_inactive);
}

#[test]
fn tenant_read_context_construction() {
    let identifier = ResolvedTenantIdentifier::Slug("alpha".into());
    let context = tenant_read_context(&identifier);
    assert!(matches!(context.actor.kind, PortActorKind::Service));
    assert_eq!(context.actor.id, "rustok-server.tenant-resolver");
    assert_eq!(context.correlation_id, "tenant-resolver:slug:alpha");
    assert!(context.deadline_ms.is_some());
}

#[test]
fn tenant_resolution_transport_representation() {
    assert_eq!(TenantResolutionTransport::Http.as_str(), "http");
    assert_eq!(
        TenantResolutionTransport::GraphqlWebSocket.as_str(),
        "graphql_ws"
    );
}

#[test]
fn tenant_context_load_error_display_and_metrics() {
    let cases = [
        (
            TenantContextLoadError::InvalidIdentifier("bad id".into()),
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_tenant_identifier",
            "invalid_identifier",
            "Invalid tenant identifier",
        ),
        (
            TenantContextLoadError::InvalidAssertion("bad slug".into()),
            axum::http::StatusCode::BAD_REQUEST,
            "conflicting_tenant_assertions",
            "invalid_assertion",
            "Conflicting tenant assertions",
        ),
        (
            TenantContextLoadError::InfrastructureUnavailable,
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "tenant_infrastructure_unavailable",
            "infrastructure_unavailable",
            "Failed to resolve tenant",
        ),
        (
            TenantContextLoadError::NotFound,
            axum::http::StatusCode::NOT_FOUND,
            "tenant_not_found",
            "not_found",
            "Tenant not found",
        ),
        (
            TenantContextLoadError::Disabled,
            axum::http::StatusCode::FORBIDDEN,
            "tenant_disabled",
            "disabled",
            "Tenant is disabled",
        ),
        (
            TenantContextLoadError::CacheUnavailable("err".into()),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "tenant_cache_unavailable",
            "cache_unavailable",
            "Failed to resolve tenant",
        ),
        (
            TenantContextLoadError::ClockUnavailable("err".into()),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "tenant_clock_unavailable",
            "clock_unavailable",
            "Failed to resolve tenant",
        ),
        (
            TenantContextLoadError::BackendUnavailable("err".into()),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "tenant_backend_unavailable",
            "backend_unavailable",
            "Failed to resolve tenant",
        ),
    ];

    for (err, status, error_code, metric_outcome, client_msg) in cases {
        assert_eq!(err.status_code(), status);
        assert_eq!(err.error_code(), error_code);
        assert_eq!(err.metric_outcome(), metric_outcome);
        assert_eq!(err.client_message(), client_msg);
        let display = format!("{err}");
        assert!(!display.is_empty());
    }
}
