use axum::{
    extract::State,
    http::{HeaderValue, Method, Request, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustok_api::context::{
    AuthContext, AuthContextExtension, AuthPrincipalContext, AuthPrincipalContextExtension,
};
use rustok_api::{HOST_AUTHORITY_REQUIRED, Permission, has_effective_permission};
use rustok_core::SecurityActorKind;

use crate::extractors::auth::resolve_current_user;
use crate::host_authority::{take_host_authority, with_host_authority_scope};
use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
use crate::services::server_runtime_context::ServerAuthRuntime;

const PAGES_AUTHORING_CACHE_CONTROL: &str = "private, no-store";
const PAGES_AUTHORING_ROBOTS_POLICY: &str = "noindex, nofollow, noarchive";

pub async fn resolve_optional(
    State(ctx): State<ServerAuthRuntime>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let (mut parts, body) = req.into_parts();
    let request_path = parts.uri.path().to_string();
    let pages_inline_authoring_surface = is_pages_inline_authoring_surface(request_path.as_str());
    let pages_inline_authoring = pages_inline_authoring_surface
        || is_pages_inline_authoring_server_fn(request_path.as_str());
    let presented_credentials = parts.headers.contains_key(AUTHORIZATION);
    let host_authority = match take_host_authority(&mut parts.headers) {
        Ok(authority) => authority,
        Err(crate::error::Error::Unauthorized(_)) => {
            return pages_inline_authoring_response(
                (StatusCode::FORBIDDEN, HOST_AUTHORITY_REQUIRED).into_response(),
                pages_inline_authoring,
                pages_inline_authoring_surface,
            );
        }
        Err(error) => {
            tracing::error!(
                error = %error,
                code = "host_authority.configuration_invalid",
                "host authority credential configuration is invalid"
            );
            return pages_inline_authoring_response(
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Host authority configuration is invalid",
                )
                    .into_response(),
                pages_inline_authoring,
                pages_inline_authoring_surface,
            );
        }
    };
    let human_user_only =
        is_human_user_self_service_path(request_path.as_str()) || pages_inline_authoring;
    let request_method = parts.method.clone();
    let mut rbac_scope = None;

    match resolve_current_user(&mut parts, &ctx).await {
        Ok(current_user) => {
            if human_user_only && current_user.actor_kind != SecurityActorKind::User {
                return pages_inline_authoring_response(
                    (
                        StatusCode::FORBIDDEN,
                        "Human-user, storefront, and interactive admin endpoints do not accept service credentials",
                    )
                        .into_response(),
                    pages_inline_authoring,
                    pages_inline_authoring_surface,
                );
            }
            if pages_inline_authoring {
                if !current_user.principal_kind.is_direct_user() {
                    return pages_inline_authoring_response(
                        (
                            StatusCode::FORBIDDEN,
                            "Pages inline authoring requires a direct authenticated user session",
                        )
                            .into_response(),
                        true,
                        pages_inline_authoring_surface,
                    );
                }
                if current_user.session_id.is_nil() {
                    return pages_inline_authoring_response(
                        (
                            StatusCode::UNAUTHORIZED,
                            "Pages inline authoring requires a non-empty authenticated session",
                        )
                            .into_response(),
                        true,
                        pages_inline_authoring_surface,
                    );
                }
                if !has_effective_permission(&current_user.permissions, &Permission::PAGES_UPDATE) {
                    return pages_inline_authoring_response(
                        (
                            StatusCode::FORBIDDEN,
                            "pages:update is required for Pages inline authoring",
                        )
                            .into_response(),
                        true,
                        pages_inline_authoring_surface,
                    );
                }
            }
            if current_user.actor_kind == SecurityActorKind::Service {
                if let Some(message) = service_forum_boundary_violation(
                    &request_method,
                    request_path.as_str(),
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
            parts
                .extensions
                .insert(AuthPrincipalContextExtension(AuthPrincipalContext::new(
                    current_user.principal_kind,
                )));
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
        Err((status, message)) if presented_credentials || pages_inline_authoring => {
            return pages_inline_authoring_response(
                (status, message).into_response(),
                pages_inline_authoring,
                pages_inline_authoring_surface,
            );
        }
        Err(_) => {}
    }

    if let Some(host_authority) = host_authority {
        parts.extensions.insert(host_authority);
    }
    let req = Request::from_parts(parts, body);
    let response = with_host_authority_scope(
        host_authority,
        with_rbac_request_scope(rbac_scope, next.run(req)),
    )
    .await;
    pages_inline_authoring_response(
        response,
        pages_inline_authoring,
        pages_inline_authoring_surface,
    )
}

fn pages_inline_authoring_response(
    mut response: Response,
    pages_inline_authoring: bool,
    pages_inline_authoring_surface: bool,
) -> Response {
    if pages_inline_authoring {
        response.headers_mut().insert(
            "cache-control",
            HeaderValue::from_static(PAGES_AUTHORING_CACHE_CONTROL),
        );
    }
    if pages_inline_authoring_surface {
        response.headers_mut().insert(
            "x-robots-tag",
            HeaderValue::from_static(PAGES_AUTHORING_ROBOTS_POLICY),
        );
    }
    response
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

fn is_pages_inline_authoring_surface(path: &str) -> bool {
    let segments = path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    matches!(
        segments.as_slice(),
        ["modules", "pages-authoring"] | [_, "modules", "pages-authoring"]
    )
}

fn is_pages_inline_authoring_server_fn(path: &str) -> bool {
    matches!(
        path,
        "/api/fn/pages/inline-edit/bootstrap" | "/api/fn/pages/inline-edit/commit"
    )
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
    use super::{
        PAGES_AUTHORING_CACHE_CONTROL, PAGES_AUTHORING_ROBOTS_POLICY,
        is_human_user_self_service_path, is_pages_inline_authoring_server_fn,
        is_pages_inline_authoring_surface, pages_inline_authoring_response,
        service_forum_boundary_violation,
    };
    use axum::http::{HeaderMap, Method, StatusCode, header::AUTHORIZATION};
    use axum::response::IntoResponse;
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
    fn pages_inline_authoring_paths_are_explicit_and_bounded() {
        assert!(is_pages_inline_authoring_surface(
            "/modules/pages-authoring"
        ));
        assert!(is_pages_inline_authoring_surface(
            "/en/modules/pages-authoring"
        ));
        assert!(is_pages_inline_authoring_server_fn(
            "/api/fn/pages/inline-edit/bootstrap"
        ));
        assert!(is_pages_inline_authoring_server_fn(
            "/api/fn/pages/inline-edit/commit"
        ));
        assert!(!is_pages_inline_authoring_surface("/modules/pages"));
        assert!(!is_pages_inline_authoring_surface(
            "/en/modules/pages-authoring/extra"
        ));
        assert!(!is_pages_inline_authoring_server_fn(
            "/api/fn/pages/inline-edit/other"
        ));
    }

    #[test]
    fn pages_authoring_responses_are_private_and_html_is_non_indexable() {
        let html = pages_inline_authoring_response(StatusCode::OK.into_response(), true, true);
        assert_eq!(
            html.headers()
                .get("cache-control")
                .and_then(|value| value.to_str().ok()),
            Some(PAGES_AUTHORING_CACHE_CONTROL)
        );
        assert_eq!(
            html.headers()
                .get("x-robots-tag")
                .and_then(|value| value.to_str().ok()),
            Some(PAGES_AUTHORING_ROBOTS_POLICY)
        );

        let server_fn =
            pages_inline_authoring_response(StatusCode::OK.into_response(), true, false);
        assert_eq!(
            server_fn
                .headers()
                .get("cache-control")
                .and_then(|value| value.to_str().ok()),
            Some(PAGES_AUTHORING_CACHE_CONTROL)
        );
        assert!(server_fn.headers().get("x-robots-tag").is_none());
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
