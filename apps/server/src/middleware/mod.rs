pub mod auth_context;
pub mod block_rest_auth;
pub mod channel;
pub mod guest_cart_access;
pub mod invite_accept;
pub mod locale;
pub mod mcp_scaffold_workspace;
pub mod metrics_auth;
pub mod oauth_token_guard;
pub mod rate_limit;
pub mod security_headers;

#[path = "tenant.rs"]
mod tenant_legacy;

/// Public tenant middleware surface.
///
/// The resolver implementation remains in the historical `tenant.rs` module while cache
/// invalidation readiness and listener metrics are intentionally overridden with the canonical
/// generation listener. Explicit items win over names imported through the glob re-export, so
/// existing call sites keep `crate::middleware::tenant::*` without observing the dead per-key
/// Pub/Sub status.
pub mod tenant {
    pub use super::tenant_legacy::*;
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

    /// Initialize the tenant resolver caches without leaving the superseded per-key Redis
    /// subscriber running. Durable namespace generations are the only cross-instance invalidation
    /// authority. The historical listener stores an unwrapped `JoinHandle<()>`; preserve any
    /// pre-existing value in that generic slot while extracting and aborting only the listener
    /// created by the legacy initializer.
    pub async fn init_tenant_cache_infrastructure(
        ctx: &ServerRuntimeContext,
        cache_service: &CacheService,
    ) {
        let previous_task = ctx.shared_take::<tokio::task::JoinHandle<()>>();
        super::tenant_legacy::init_tenant_cache_infrastructure(ctx, cache_service).await;
        if let Some(legacy_listener) = ctx.shared_take::<tokio::task::JoinHandle<()>>() {
            legacy_listener.abort();
        }
        if let Some(previous_task) = previous_task {
            ctx.shared_insert(previous_task);
        }
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
        let emitted_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        let record = match DurableCacheInvalidationRecord::new(
            Uuid::new_v4(),
            tenant_id,
            TENANT_CACHE_GENERATION_CHANNEL,
            "tenant-manual-invalidation",
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
            if listener.status != TenantInvalidationListenerStatus::Healthy
                || !listener.local_ready
            {
                tracing::warn!(
                    cause,
                    status = ?listener.status,
                    error = listener.last_error.as_deref(),
                    "Tenant generation advanced without the canonical local listener"
                );
            }
        }
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
}
