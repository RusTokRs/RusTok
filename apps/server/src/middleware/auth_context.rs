use axum::{
    extract::State,
    http::{Method, Request, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustok_api::context::{AuthContext, AuthContextExtension};
use rustok_api::{Permission, has_effective_permission};
use rustok_core::SecurityActorKind;

use crate::extractors::auth::resolve_current_user;
use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
use crate::services::server_runtime_context::ServerAuthRuntime;

pub async fn resolve_optional(
    State(ctx): State<ServerAuthRuntime>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let (mut parts, body) = req.into_parts();
    let presented_credentials = parts.headers.contains_key(AUTHORIZATION);
    let human_user_only = is_human_user_self_service_path(parts.uri.path());
    let request_method = parts.method.clone();
    let request_path = parts.uri.path().to_string();
    let mut rbac_scope = None;

    match resolve_current_user(&mut parts, &ctx).await {
        Ok(current_user) => {
            if human_user_only && current_user.actor_kind != SecurityActorKind::User {
                return (
                    StatusCode::FORBIDDEN,
                    "Human-user, storefront, and interactive admin endpoints do not accept service credentials",
                )
                    .into_response();
            }
            if current_user.actor_kind == SecurityActorKind::Service {
                if let Some(message) = service_forum_boundary_violation(
                    &request_method,
                    &request_path,
                    &current_user.permissions,
                ) {
                    return (StatusCode::FORBIDDEN, message).into_response();
                }
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
        || path == "/store"
        || path.starts_with("/store/")
        || path.starts_with("/api/fn/ai/")
}

fn service_forum_boundary_violation(
    method: &Method,
    path: &str,
    permissions: &[Permission],
) -> Option<&'static str> {
    if !path.starts_with("/api/forum/") {
        return None;
    }

    let segments = path.trim_matches('/').split('/').collect::<Vec<_>>();
    let personal_interaction = path.contains("/vote")
        || path.ends_with("/subscription")
        || (method == Method::POST && path == "/api/forum/topics")
        || (method == Method::POST
            && segments.len() == 5
            && segments[0] == "api"
            && segments[1] == "forum"
            && segments[2] == "topics"
            && uuid::Uuid::parse_str(segments[3]).is_ok()
            && segments[4] == "replies");
    if personal_interaction {
        return Some(
            "Forum authorship, voting, and personal subscriptions require human-user credentials",
        );
    }

    if path.contains("/solution")
        && (method == Method::POST || method == Method::DELETE)
        && !has_effective_permission(permissions, &Permission::FORUM_TOPICS_MODERATE)
    {
        return Some("Service credentials require forum_topics:moderate for solution changes");
    }

    if segments.len() == 4
        && segments[0] == "api"
        && segments[1] == "forum"
        && uuid::Uuid::parse_str(segments[3]).is_ok()
        && (method == Method::PUT || method == Method::DELETE)
    {
        let required = match segments[2] {
            "topics" => Some(Permission::FORUM_TOPICS_MODERATE),
            "replies" => Some(Permission::FORUM_REPLIES_MODERATE),
            _ => None,
        };
        if required.is_some_and(|required| !has_effective_permission(permissions, &required)) {
            return Some(
                "Service credentials require explicit forum moderation authority for update/delete",
            );
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{is_human_user_self_service_path, service_forum_boundary_violation};
    use axum::http::{HeaderMap, Method, header::AUTHORIZATION};
    use rustok_api::Permission;
    use uuid::Uuid;

    #[test]
    fn authorization_presence_distinguishes_anonymous_from_invalid_credentials() {
        let mut headers = HeaderMap::new();
        assert!(!headers.contains_key(AUTHORIZATION));
        headers.insert(AUTHORIZATION, "Bearer invalid".parse().unwrap());
        assert!(headers.contains_key(AUTHORIZATION));
    }

    #[test]
    fn user_storefront_and_ai_admin_routes_reject_service_credentials() {
        assert!(is_human_user_self_service_path("/api/auth/me"));
        assert!(is_human_user_self_service_path(
            "/api/auth/sessions/00000000-0000-0000-0000-000000000001"
        ));
        assert!(is_human_user_self_service_path("/api/auth/profile"));
        assert!(is_human_user_self_service_path("/store"));
        assert!(is_human_user_self_service_path("/store/customers/me"));
        assert!(is_human_user_self_service_path("/store/carts"));
        assert!(is_human_user_self_service_path("/api/fn/ai/overview"));
        assert!(is_human_user_self_service_path(
            "/api/fn/ai/create-provider"
        ));
        assert!(!is_human_user_self_service_path("/admin/products"));
        assert!(!is_human_user_self_service_path("/api/auth/reset/request"));
        assert!(!is_human_user_self_service_path("/api/oauth/token"));
    }

    #[test]
    fn forum_personal_interactions_are_human_only() {
        let topic_id = Uuid::new_v4();
        assert!(
            service_forum_boundary_violation(
                &Method::POST,
                "/api/forum/topics",
                &[Permission::FORUM_TOPICS_CREATE],
            )
            .is_some()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::POST,
                &format!("/api/forum/topics/{topic_id}/replies"),
                &[Permission::FORUM_REPLIES_CREATE],
            )
            .is_some()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::POST,
                &format!("/api/forum/topics/{topic_id}/vote/1"),
                &[Permission::FORUM_TOPICS_UPDATE],
            )
            .is_some()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::GET,
                &format!("/api/forum/topics/{topic_id}/subscription"),
                &[Permission::FORUM_TOPICS_READ],
            )
            .is_some()
        );
    }

    #[test]
    fn forum_service_updates_require_moderation_authority() {
        let topic_id = Uuid::new_v4();
        let path = format!("/api/forum/topics/{topic_id}");
        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &path,
                &[Permission::FORUM_TOPICS_UPDATE],
            )
            .is_some()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &path,
                &[Permission::FORUM_TOPICS_MODERATE],
            )
            .is_none()
        );
    }
}
