use axum::{extract::State, http::Request, middleware::Next, response::Response};
use rustok_api::context::{AuthContext, AuthContextExtension};

use crate::extractors::auth::resolve_current_user;
use crate::services::server_runtime_context::ServerAuthRuntime;

pub async fn resolve_optional(
    State(ctx): State<ServerAuthRuntime>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let (mut parts, body) = req.into_parts();

    if let Ok(current_user) = resolve_current_user(&mut parts, &ctx).await {
        parts.extensions.insert(AuthContextExtension(AuthContext {
            user_id: current_user.user.id,
            session_id: current_user.session_id,
            tenant_id: current_user.user.tenant_id,
            permissions: current_user.permissions,
            client_id: current_user.client_id,
            scopes: current_user.scopes,
            grant_type: current_user.grant_type,
        }));
    }

    req = Request::from_parts(parts, body);
    next.run(req).await
}
