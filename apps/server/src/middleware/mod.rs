pub mod auth_context;
pub mod block_rest_auth;
pub mod channel;
pub mod guest_cart_access;
pub mod invite_accept;
pub mod locale;
pub mod mcp_scaffold_workspace;
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