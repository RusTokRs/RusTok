use axum::{
    extract::State,
    http::{header::AUTHORIZATION, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustok_api::context::{AuthContext, AuthContextExtension};
use rustok_core::SecurityActorKind;

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
    let human_user_only = is_human_user_self_service_path(parts.uri.path());
    let mut rbac_scope = None;

    match resolve_current_user(&mut parts, &ctx).await {
        Ok(current_user) => {
            if human_user_only && current_user.actor_kind != SecurityActorKind::User {
                return (
                    StatusCode::FORBIDDEN,
                    "User self-service endpoints do not accept service credentials",
                )
                    .into_response();
            }

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

fn is_human_user_self_service_path(path: &str) -> bool {
    matches!(
        path,
        "/api/auth/me"
            | "/api/auth/sessions"
            | "/api/auth/sessions/revoke-all"
            | "/api/auth/change-password"
            | "/api/auth/profile"
            | "/api/auth/history"
    ) || path.starts_with("/api/auth/sessions/")
}

#[cfg(test)]
mod tests {
    use super::is_human_user_self_service_path;
    use axum::http::{header::AUTHORIZATION, HeaderMap};

    #[test]
    fn authorization_presence_distinguishes_anonymous_from_invalid_credentials() {
        let mut headers = HeaderMap::new();
        assert!(!headers.contains_key(AUTHORIZATION));
        headers.insert(AUTHORIZATION, "Bearer invalid".parse().unwrap());
        assert!(headers.contains_key(AUTHORIZATION));
    }

    #[test]
    fn only_user_self_service_routes_reject_service_credentials() {
        assert!(is_human_user_self_service_path("/api/auth/me"));
        assert!(is_human_user_self_service_path(
            "/api/auth/sessions/00000000-0000-0000-0000-000000000001"
        ));
        assert!(is_human_user_self_service_path("/api/auth/profile"));
        assert!(!is_human_user_self_service_path("/api/auth/reset/request"));
        assert!(!is_human_user_self_service_path("/api/oauth/token"));
    }
}
