pub mod auth_context;
pub mod block_rest_auth;
#[path = "channel_native_wrapper.rs"]
pub mod channel;
pub mod csp_reports;
pub mod invite_accept;
pub mod locale;
pub mod mcp_scaffold_workspace;
pub mod metrics_auth;
pub mod rate_limit;
pub mod registry_artifact_access;
pub mod registry_publish_policy;
pub mod registry_remote_claim;
pub mod security_headers;

mod tenant_resolution;
mod tenant_route_policy;
#[path = "tenant.rs"]
mod tenant_runtime;

/// Public tenant middleware surface backed by durable cache generations.
pub mod tenant {
    pub use super::tenant_runtime::{resolve, TenantCacheInfrastructure, TenantCacheStats};
    pub(crate) use super::tenant_runtime::resolve_tenant_context_by_slug;
    pub use crate::services::tenant_cache_generation_status::{
        TenantCacheGenerationListenerSnapshot as TenantInvalidationListenerSnapshot,
        TenantCacheGenerationListenerStatus as TenantInvalidationListenerStatus,
    };

    use crate::services::server_runtime_context::ServerRuntimeContext;
    use crate::services::tenant_cache_generation::{
        TENANT_CACHE_BACKEND_PREFIX, TENANT_CACHE_GENERATION_CHANNEL,
    };
    use rustok_cache::{CacheService, DurableCacheInvalidationRecord};
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    /// Initialize tenant resolver caches. Cross-instance invalidation is handled by the
    /// durable generation listener initialized with the application runtime.
    pub async fn init_tenant_cache_infrastructure(
        ctx: &ServerRuntimeContext,
        cache_service: &CacheService,
    ) {
        super::tenant_runtime::init_tenant_cache_infrastructure(ctx, cache_service).await;
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
        let mut stats = super::tenant_runtime::tenant_cache_stats(ctx).await;
        let listener = tenant_invalidation_listener_snapshot(ctx).await;
        stats.invalidation_listener_status = listener.status.metric_value();
        stats
    }

    #[cfg(test)]
    mod tests {
        use super::unix_ms_at;
        use std::time::{Duration, UNIX_EPOCH};

        #[test]
        fn invalidation_timestamp_rejects_pre_epoch_clock() {
            assert_eq!(unix_ms_at(UNIX_EPOCH).expect("epoch"), 0);
            assert_eq!(
                unix_ms_at(UNIX_EPOCH + Duration::from_millis(42)).expect("timestamp"),
                42
            );
            assert!(unix_ms_at(UNIX_EPOCH - Duration::from_secs(1)).is_err());
        }
    }
}
