pub mod auth_context;
pub mod block_rest_auth;
#[path = "channel_native_wrapper.rs"]
pub mod channel;
pub mod guest_cart_access;
pub mod invite_accept;
pub mod locale;
pub mod mcp_scaffold_workspace;
pub mod metrics_auth;
pub mod oauth_token_guard;
pub mod rate_limit;
pub mod registry_artifact_access;
pub mod registry_publish_policy;
pub mod registry_remote_claim;
pub mod security_headers;

#[path = "tenant.rs"]
mod tenant_legacy;

/// Public tenant middleware surface backed by durable cache generations.
pub mod tenant {
    pub use super::tenant_legacy::{
        ResolvedTenantIdentifier, TenantCacheInfrastructure, TenantCacheStats, TenantIdentifierKind,
    };
    pub use crate::services::tenant_cache_generation_status::{
        TenantCacheGenerationListenerSnapshot as TenantInvalidationListenerSnapshot,
        TenantCacheGenerationListenerStatus as TenantInvalidationListenerStatus,
    };

    use axum::{
        body::Body,
        extract::State,
        http::{Request, StatusCode},
        middleware::Next,
        response::Response,
    };
    use crate::common::settings::{RustokSettings, TenantFallbackMode};
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use crate::services::tenant_cache_generation::{
        TENANT_CACHE_BACKEND_PREFIX, TENANT_CACHE_GENERATION_CHANNEL,
    };
    use rustok_cache::{CacheService, DurableCacheInvalidationRecord};
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    /// Request-time tenant boundary. Bootstrap validation should make invalid modes
    /// unreachable, but this guard keeps configuration reloads and alternate hosts
    /// fail-closed instead of inheriting the legacy default-tenant catch-all.
    pub async fn resolve(
        State(ctx): State<ServerRuntimeContext>,
        req: Request<Body>,
        next: Next,
    ) -> Result<Response, StatusCode> {
        validate_request_tenant_policy(&req, ctx.settings())?;
        super::tenant_legacy::resolve(State(ctx), req, next).await
    }

    /// Public identifier resolver with the same fail-closed policy as middleware.
    pub fn resolve_identifier(
        req: &Request<Body>,
        settings: &RustokSettings,
    ) -> Result<ResolvedTenantIdentifier, StatusCode> {
        validate_request_tenant_policy(req, settings)?;
        super::tenant_legacy::resolve_identifier(req, settings)
    }

    fn validate_request_tenant_policy(
        req: &Request<Body>,
        settings: &RustokSettings,
    ) -> Result<(), StatusCode> {
        let path = req.uri().path();
        if !tenant_path_requires_resolution(path) {
            return Ok(());
        }

        if let Err(error) = unix_ms_at(SystemTime::now()) {
            tracing::error!(
                %error,
                path,
                "Rejecting tenant-bound request because system time is before the Unix epoch"
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }

        if settings.tenant.enabled && !tenant_resolution_mode_supported(settings) {
            tracing::error!(
                resolution = %settings.tenant.resolution,
                path,
                "Rejecting request because tenant resolution mode is invalid"
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }

        if default_tenant_fallback_will_be_used(req, settings) {
            rustok_telemetry::metrics::record_cache_operation(
                "tenant_resolution",
                "fallback",
                "default_tenant",
            );
            tracing::warn!(
                path,
                header_name = %settings.tenant.header_name,
                tenant_id = %settings.tenant.default_id,
                "Using explicitly configured development default-tenant fallback"
            );
        }

        Ok(())
    }

    fn tenant_path_requires_resolution(path: &str) -> bool {
        !(matches!(path, "/metrics" | "/api/openapi.json" | "/api/openapi.yaml")
            || path == "/api/graphql/schema.graphql"
            || path == "/api/graphql/ws"
            || path == "/api/install"
            || path.starts_with("/api/install/")
            || path == "/v1/catalog"
            || path.starts_with("/v1/catalog/")
            || path == "/catalog"
            || path.starts_with("/catalog/")
            || path.starts_with("/health"))
    }

    fn tenant_resolution_mode_supported(settings: &RustokSettings) -> bool {
        matches!(
            settings.tenant.resolution.as_str(),
            "header" | "host" | "domain" | "subdomain"
        )
    }

    fn default_tenant_fallback_will_be_used(
        req: &Request<Body>,
        settings: &RustokSettings,
    ) -> bool {
        if !tenant_path_requires_resolution(req.uri().path())
            || !settings.tenant.enabled
            || settings.tenant.resolution != "header"
            || !matches!(
                settings.tenant.fallback_mode,
                TenantFallbackMode::DefaultTenant
            )
        {
            return false;
        }

        let primary_present = req
            .headers()
            .get(&settings.tenant.header_name)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| !value.trim().is_empty());
        let slug_present = settings.tenant.header_name != "X-Tenant-Slug"
            && req
                .headers()
                .get("X-Tenant-Slug")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| !value.trim().is_empty());

        !primary_present && !slug_present
    }

    /// Initialize tenant resolver caches. Cross-instance invalidation is handled by the
    /// durable generation listener initialized with the application runtime.
    pub async fn init_tenant_cache_infrastructure(
        ctx: &ServerRuntimeContext,
        cache_service: &CacheService,
    ) {
        super::tenant_legacy::init_tenant_cache_infrastructure(ctx, cache_service).await;
        crate::services::tenant_locale_generation::start_tenant_locale_generation_listener(
            ctx,
            cache_service.clone(),
        )
        .await;
    }

    /// Invalidate the tenant resolver namespace through a durable generation rotation.
    pub async fn invalidate_tenant_cache(ctx: &ServerRuntimeContext, _identifier: &str) {
        rotate_tenant_cache_generation(ctx, None, "manual_tenant_invalidation").await;
    }

    /// Compatibility entry point for callers that used to publish a per-key host invalidation.
    /// Tenant resolver backends share one generation, so rotating it safely invalidates all aliases.
    pub async fn invalidate_tenant_cache_by_host(ctx: &ServerRuntimeContext, _host: &str) {
        rotate_tenant_cache_generation(ctx, None, "manual_host_invalidation").await;
    }

    /// Compatibility entry point for callers that used to publish a per-key slug invalidation.
    pub async fn invalidate_tenant_cache_by_slug(ctx: &ServerRuntimeContext, _slug: &str) {
        rotate_tenant_cache_generation(ctx, None, "manual_slug_invalidation").await;
    }

    /// Compatibility entry point for callers that used to publish a per-key UUID invalidation.
    pub async fn invalidate_tenant_cache_by_uuid(ctx: &ServerRuntimeContext, tenant_id: Uuid) {
        rotate_tenant_cache_generation(ctx, Some(tenant_id), "manual_uuid_invalidation").await;
    }

    async fn rotate_tenant_cache_generation(
        ctx: &ServerRuntimeContext,
        tenant_id: Option<Uuid>,
        cause: &'static str,
    ) {
        let Some(cache) = ctx.shared_get::<CacheService>() else {
            tracing::warn!(
                cause,
                "Tenant cache generation rotation skipped: cache service missing"
            );
            return;
        };

        let generation = match cache
            .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
            .await
        {
            Ok(generation) => generation,
            Err(error) => {
                tracing::warn!(%error, cause, "Tenant cache generation rotation failed");
                return;
            }
        };
        let emitted_at_unix_ms = match unix_ms_at(SystemTime::now()) {
            Ok(timestamp) => timestamp,
            Err(error) => {
                tracing::error!(
                    %error,
                    cause,
                    "Tenant cache generation publication aborted because system time is before the Unix epoch"
                );
                return;
            }
        };
        let invalidation_key = tenant_id
            .map(|tenant_id| tenant_id.to_string())
            .unwrap_or_else(|| "*".to_string());
        let record = match DurableCacheInvalidationRecord::new(
            Uuid::new_v4(),
            tenant_id,
            TENANT_CACHE_GENERATION_CHANNEL,
            invalidation_key,
            generation.generation,
            emitted_at_unix_ms,
            cause,
            None,
        ) {
            Ok(record) => record,
            Err(error) => {
                tracing::warn!(%error, cause, "Tenant cache generation record creation failed");
                return;
            }
        };
        let outcome = match cache.invalidations().publish_durable(&record).await {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::warn!(%error, cause, "Tenant cache generation publication failed");
                return;
            }
        };

        if cache.redis_configuration_present() && !outcome.redis_published {
            tracing::warn!(
                cause,
                "Tenant generation advanced but Redis publication was unavailable"
            );
        } else if !cache.redis_configuration_present() {
            let listener = tenant_invalidation_listener_snapshot(ctx).await;
            if listener.status != TenantInvalidationListenerStatus::Healthy || !listener.local_ready
            {
                tracing::warn!(
                    cause,
                    status = ?listener.status,
                    error = ?listener.last_error,
                    "Tenant generation advanced without the canonical local listener"
                );
            }
        }
    }

    fn unix_ms_at(time: SystemTime) -> Result<u64, std::time::SystemTimeError> {
        Ok(time
            .duration_since(UNIX_EPOCH)?
            .as_millis()
            .min(u128::from(u64::MAX)) as u64)
    }

    pub async fn tenant_invalidation_listener_snapshot(
        ctx: &ServerRuntimeContext,
    ) -> TenantInvalidationListenerSnapshot {
        crate::services::tenant_cache_generation::tenant_cache_generation_listener_snapshot(ctx)
            .await
    }

    pub async fn tenant_cache_stats(ctx: &ServerRuntimeContext) -> TenantCacheStats {
        let mut stats = super::tenant_legacy::tenant_cache_stats(ctx).await;
        let listener = tenant_invalidation_listener_snapshot(ctx).await;
        stats.invalidation_listener_status = listener.status.metric_value();
        stats
    }

    #[cfg(test)]
    mod tests {
        use super::{
            default_tenant_fallback_will_be_used, resolve_identifier,
            tenant_path_requires_resolution, tenant_resolution_mode_supported, unix_ms_at,
        };
        use crate::common::settings::{RustokSettings, TenantFallbackMode};
        use axum::{body::Body, http::Request};
        use std::time::{Duration, UNIX_EPOCH};

        #[test]
        fn unix_timestamp_conversion_accepts_epoch() {
            assert_eq!(unix_ms_at(UNIX_EPOCH).expect("epoch timestamp"), 0);
            assert_eq!(
                unix_ms_at(UNIX_EPOCH + Duration::from_millis(42)).expect("timestamp"),
                42
            );
        }

        #[test]
        fn unix_timestamp_conversion_rejects_pre_epoch_clock() {
            assert!(unix_ms_at(UNIX_EPOCH - Duration::from_secs(1)).is_err());
        }

        #[test]
        fn request_boundary_rejects_unknown_resolution() {
            let mut settings = RustokSettings::default();
            settings.tenant.resolution = "automatic".to_string();
            let request = Request::builder()
                .uri("/api/users")
                .body(Body::empty())
                .expect("request");

            assert!(!tenant_resolution_mode_supported(&settings));
            assert!(matches!(
                resolve_identifier(&request, &settings),
                Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
            ));
        }

        #[test]
        fn detects_default_fallback_only_when_headers_are_absent() {
            let mut settings = RustokSettings::default();
            settings.tenant.resolution = "header".to_string();
            settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;

            let missing = Request::builder()
                .uri("/api/users")
                .body(Body::empty())
                .expect("request");
            assert!(default_tenant_fallback_will_be_used(&missing, &settings));

            let present = Request::builder()
                .uri("/api/users")
                .header("X-Tenant-ID", settings.tenant.default_id.to_string())
                .body(Body::empty())
                .expect("request");
            assert!(!default_tenant_fallback_will_be_used(&present, &settings));
        }

        #[test]
        fn operator_and_global_routes_never_count_as_tenant_fallbacks() {
            let mut settings = RustokSettings::default();
            settings.tenant.resolution = "header".to_string();
            settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;

            for path in ["/metrics", "/health/ready", "/v1/catalog", "/catalog/module"] {
                let request = Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request");
                assert!(!default_tenant_fallback_will_be_used(&request, &settings));
            }
        }

        #[test]
        fn only_read_only_registry_catalog_bypasses_tenant_resolution() {
            assert!(!tenant_path_requires_resolution("/v1/catalog"));
            assert!(!tenant_path_requires_resolution("/v1/catalog/blog"));
            assert!(tenant_path_requires_resolution("/v2/catalog/publish"));
            assert!(tenant_path_requires_resolution(
                "/v2/catalog/publish/request/approve"
            ));
        }
    }
}
