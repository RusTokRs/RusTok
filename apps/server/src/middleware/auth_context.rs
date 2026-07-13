use axum::{
    extract::State,
    http::{header::AUTHORIZATION, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustok_api::context::{AuthContext, AuthContextExtension};

use crate::extractors::auth::resolve_current_user;
use crate::services::rbac_request_scope::{with_rbac_request_scope, RbacRequestScope};
use crate::services::server_runtime_context::ServerAuthRuntime;

pub async fn resolve_optional(
    State(ctx): State<ServerAuthRuntime>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let (mut parts, body) = req.into_parts();
    let presented_credentials = parts.headers.contains_key(AUTHORIZATION);
    let mut rbac_scope = None;

    match resolve_current_user(&mut parts, &ctx).await {
        Ok(current_user) => {
            rbac_scope = Some(RbacRequestScope::new(
                current_user.user.tenant_id,
                current_user.user.id,
                current_user.permissions.clone(),
                current_user.inferred_role.clone(),
            ));
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
        Err((status, message)) if presented_credentials => {
            return (status, message).into_response();
        }
        Err(_) => {}
    }

    let req = Request::from_parts(parts, body);
    with_rbac_request_scope(rbac_scope, next.run(req)).await
}

#[cfg(test)]
mod tests {
    use axum::http::{header::AUTHORIZATION, HeaderMap};

    #[test]
    fn authorization_presence_distinguishes_anonymous_from_invalid_credentials() {
        let mut headers = HeaderMap::new();
        assert!(!headers.contains_key(AUTHORIZATION));
        headers.insert(AUTHORIZATION, "Bearer invalid".parse().unwrap());
        assert!(headers.contains_key(AUTHORIZATION));
    }
}
