use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::context::TenantContextExt;
use crate::services::server_runtime_context::ServerRuntimeContext;

#[path = "channel.rs"]
mod base;

pub use base::invalidate_tenant_channel_cache;

const NATIVE_CHANNEL_MUTATION_PATHS: &[&str] = &[
    "/api/fn/channel/create-channel",
    "/api/fn/channel/set-default",
    "/api/fn/channel/create-target",
    "/api/fn/channel/update-target",
    "/api/fn/channel/delete-target",
    "/api/fn/channel/bind-module",
    "/api/fn/channel/delete-module-binding",
    "/api/fn/channel/bind-oauth-app",
    "/api/fn/channel/delete-oauth-app-binding",
    "/api/fn/channel/create-resolution-policy-set",
    "/api/fn/channel/activate-resolution-policy-set",
    "/api/fn/channel/create-resolution-rule",
    "/api/fn/channel/update-resolution-rule",
    "/api/fn/channel/reorder-resolution-rules",
    "/api/fn/channel/delete-resolution-rule",
];

fn is_native_channel_mutation(path: &str) -> bool {
    NATIVE_CHANNEL_MUTATION_PATHS.contains(&path)
}

pub async fn resolve(
    State(ctx): State<ServerRuntimeContext>,
    req: Request,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    let should_invalidate = is_native_channel_mutation(req.uri().path());
    let tenant_id = req.extensions().tenant_context().map(|tenant| tenant.id);

    let response = base::resolve(State(ctx.clone()), req, next).await?;
    if should_invalidate && response.status().is_success() {
        if let Some(tenant_id) = tenant_id {
            base::invalidate_tenant_channel_cache(&ctx, tenant_id).await;
        }
    }

    Ok(response)
}

#[cfg(test)]
mod wrapper_tests {
    use super::is_native_channel_mutation;

    #[test]
    fn matcher_accepts_only_channel_mutation_server_functions() {
        assert!(is_native_channel_mutation(
            "/api/fn/channel/create-channel"
        ));
        assert!(is_native_channel_mutation(
            "/api/fn/channel/delete-resolution-rule"
        ));
        assert!(!is_native_channel_mutation("/api/fn/channel/bootstrap"));
        assert!(!is_native_channel_mutation("/api/channels/"));
        assert!(!is_native_channel_mutation("/api/fn/other/mutation"));
    }
}
