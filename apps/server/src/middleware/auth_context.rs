use axum::{
    extract::{FromRequestParts, State},
    http::{HeaderValue, Method, Request, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use rustok_api::AuthPrincipalKind;
use rustok_api::context::{
    AuthContext, AuthContextExtension, AuthPrincipalContext, AuthPrincipalContextExtension,
};
use rustok_api::{HOST_AUTHORITY_REQUIRED, Permission, has_effective_permission};
use rustok_core::SecurityActorKind;

use crate::auth::decode_access_token;
use crate::extractors::auth::{resolve_current_user, resolve_current_user_from_access_token};
use crate::host_authority::{take_host_authority, with_host_authority_scope};
use crate::middleware::metrics_auth::is_observability_auth_path;
use crate::middleware::tenant_route_policy::{TenantRouteScope, tenant_route_scope};
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
    tracing::debug!(
        path_group = super::http_stack::trace_path_family(&request_path),
        "auth_context::resolve_optional entry"
    );
    let pages_inline_authoring_surface = is_pages_inline_authoring_surface(request_path.as_str());
    let pages_inline_authoring = pages_inline_authoring_surface
        || is_pages_inline_authoring_server_fn(request_path.as_str());

    if parts.headers.get_all(AUTHORIZATION).iter().count() > 1 {
        tracing::warn!("Multiple Authorization headers detected; failing closed");
        return pages_inline_authoring_response(
            auth_error_response(
                StatusCode::BAD_REQUEST,
                "Multiple Authorization headers are not allowed",
            ),
            pages_inline_authoring,
            pages_inline_authoring_surface,
        );
    }

    let presented_credentials = parts.headers.contains_key(AUTHORIZATION);
    let host_authority = match take_host_authority(&mut parts.headers) {
        Ok(authority) => authority,
        Err(crate::error::Error::Unauthorized(_)) => {
            return pages_inline_authoring_response(
                auth_error_response(StatusCode::FORBIDDEN, HOST_AUTHORITY_REQUIRED),
                pages_inline_authoring,
                pages_inline_authoring_surface,
            );
        }
        Err(crate::error::Error::BadRequest(message)) => {
            return pages_inline_authoring_response(
                auth_error_response(StatusCode::BAD_REQUEST, &message),
                pages_inline_authoring,
                pages_inline_authoring_surface,
            );
        }
        Err(_error) => {
            tracing::error!(
                error_code = "host_authority_configuration_invalid",
                "Host authority credential configuration is invalid"
            );
            return pages_inline_authoring_response(
                auth_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Host authority configuration is invalid",
                ),
                pages_inline_authoring,
                pages_inline_authoring_surface,
            );
        }
    };
    let direct_user_self_service = is_direct_user_self_service_path(request_path.as_str());
    let human_user_only =
        is_human_user_self_service_path(request_path.as_str()) || pages_inline_authoring;
    let request_method = parts.method.clone();
    let mut rbac_scope = None;

    if is_observability_auth_path(request_path.as_str()) {
        // These tenant-independent observability handlers enforce HostAuthorityContext
        // themselves. Readiness deliberately falls back to an aggregate-only response.
        if let Some(host_authority) = host_authority {
            parts.extensions.insert(host_authority);
        }
        let req = Request::from_parts(parts, body);
        return with_host_authority_scope(host_authority, next.run(req)).await;
    }

    if is_oauth_client_auth_request(request_method.as_str(), request_path.as_str())
        || super::mcp_scaffold_workspace::is_remote_mcp_auth_request(
            &request_method,
            request_path.as_str(),
        )
    {
        // OAuth client endpoints and remote MCP runtime endpoints own their
        // protocol credentials. An MCP bearer is an opaque persisted token,
        // not a user JWT. Preserve independent host authority and let the
        // protocol handler authenticate the credential before any operation.
        if let Some(host_authority) = host_authority {
            parts.extensions.insert(host_authority);
        }
        let req = Request::from_parts(parts, body);
        return with_host_authority_scope(host_authority, next.run(req)).await;
    }

    let current_user_result = if parts
        .extensions
        .get::<crate::context::TenantContextExtension>()
        .is_some()
    {
        resolve_current_user(&mut parts, &ctx).await
    } else if auth_can_resolve_without_tenant_context(request_path.as_str()) {
        resolve_current_user_without_tenant_context(&mut parts, &ctx).await
    } else {
        resolve_current_user(&mut parts, &ctx).await
    };

    match current_user_result {
        Ok(current_user) => {
            if direct_user_self_service
                && !is_direct_user_self_service_principal(current_user.principal_kind)
            {
                return pages_inline_authoring_response(
                    auth_error_response(
                        StatusCode::FORBIDDEN,
                        "Direct authenticated user sessions are required for auth self-service endpoints",
                    ),
                    pages_inline_authoring,
                    pages_inline_authoring_surface,
                );
            }
            if human_user_only && current_user.actor_kind != SecurityActorKind::User {
                return pages_inline_authoring_response(
                    auth_error_response(
                        StatusCode::FORBIDDEN,
                        "Human-user, storefront, and interactive admin endpoints do not accept service credentials",
                    ),
                    pages_inline_authoring,
                    pages_inline_authoring_surface,
                );
            }
            if pages_inline_authoring {
                if !current_user.principal_kind.is_direct_user() {
                    return pages_inline_authoring_response(
                        auth_error_response(
                            StatusCode::FORBIDDEN,
                            "Pages inline authoring requires a direct authenticated user session",
                        ),
                        true,
                        pages_inline_authoring_surface,
                    );
                }
                if current_user.session_id.is_nil() {
                    return pages_inline_authoring_response(
                        auth_error_response(
                            StatusCode::UNAUTHORIZED,
                            "Pages inline authoring requires a non-empty authenticated session",
                        ),
                        true,
                        pages_inline_authoring_surface,
                    );
                }
                if !has_effective_permission(&current_user.permissions, &Permission::PAGES_UPDATE) {
                    return pages_inline_authoring_response(
                        auth_error_response(
                            StatusCode::FORBIDDEN,
                            "pages:update is required for Pages inline authoring",
                        ),
                        true,
                        pages_inline_authoring_surface,
                    );
                }
            }
            if current_user.actor_kind == SecurityActorKind::Service
                && let Some(message) = service_forum_boundary_violation(
                    &request_method,
                    request_path.as_str(),
                    &current_user.permissions,
                )
            {
                return auth_error_response(StatusCode::FORBIDDEN, message);
            }

            rbac_scope = Some(RbacRequestScope::new(
                current_user.user.tenant_id,
                current_user.user.id,
                current_user.permissions.clone(),
                current_user.inferred_role.clone(),
            ));
            let principal_context = AuthPrincipalContext::new(current_user.principal_kind);
            parts
                .extensions
                .insert(AuthPrincipalContextExtension(principal_context));
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
                auth_error_response(status, message),
                pages_inline_authoring,
                pages_inline_authoring_surface,
            );
        }
        Err(_) => {}
    }

    if let Some(host_authority) = host_authority {
        parts.extensions.insert(host_authority);
    }
    tracing::debug!(
        path_group = super::http_stack::trace_path_family(&request_path),
        "auth_context::resolve_optional calling next"
    );
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

async fn resolve_current_user_without_tenant_context(
    parts: &mut axum::http::request::Parts,
    ctx: &ServerAuthRuntime,
) -> Result<crate::extractors::auth::CurrentUser, (StatusCode, &'static str)> {
    let TypedHeader(Authorization(bearer)) =
        TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, ctx)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Missing or invalid token"))?;

    let auth_config = ctx.auth_config().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "JWT secret not configured",
    ))?;
    let tenant_id = verified_tenant_id_from_access_token(auth_config, bearer.token())?;

    resolve_current_user_from_access_token(ctx, tenant_id, bearer.token()).await
}

fn verified_tenant_id_from_access_token(
    auth_config: &crate::auth::AuthConfig,
    access_token: &str,
) -> Result<uuid::Uuid, (StatusCode, &'static str)> {
    let claims = decode_access_token(auth_config, access_token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token signature"))?;
    if claims.tenant_id.is_nil() {
        return Err((StatusCode::UNAUTHORIZED, "Token tenant claim is invalid"));
    }
    Ok(claims.tenant_id)
}

fn auth_error_response(status: StatusCode, message: &str) -> Response {
    let (code, public_message) = match status {
        StatusCode::BAD_REQUEST => ("bad_request", message),
        StatusCode::UNAUTHORIZED => ("unauthorized", message),
        StatusCode::FORBIDDEN => ("forbidden", message),
        _ => {
            tracing::error!(status = %status, "authentication middleware failed internally");
            ("internal_error", "Internal server error")
        }
    };
    rustok_web::HttpError::new(status, code, public_message).into_response()
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

fn auth_can_resolve_without_tenant_context(path: &str) -> bool {
    matches!(
        tenant_route_scope(path),
        TenantRouteScope::GlobalOperator | TenantRouteScope::SelfResolvingHandshake
    )
}

fn is_oauth_client_auth_request(method: &str, path: &str) -> bool {
    method == "POST" && matches!(path, "/api/oauth/token" | "/api/oauth/revoke")
}

fn is_direct_user_self_service_principal(principal_kind: AuthPrincipalKind) -> bool {
    principal_kind.is_direct_user()
}

fn is_direct_user_self_service_path(path: &str) -> bool {
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

/// Axum's Path extractor percent-decodes captured segments, while middleware
/// still sees the encoded URI path. Normalize the resource ID before applying
/// service-principal restrictions so percent-encoded UUIDs cannot skip them.
fn forum_path_segment_is_uuid(segment: &str) -> bool {
    if uuid::Uuid::parse_str(segment).is_ok() {
        return true;
    }

    let encoded_pair = format!("id={segment}");
    url::form_urlencoded::parse(encoded_pair.as_bytes())
        .next()
        .is_some_and(|(_, value)| uuid::Uuid::parse_str(value.as_ref()).is_ok())
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
            && forum_path_segment_is_uuid(segments[3])
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

    // Quote relations mutate the same topic/reply content boundary as direct
    // edits. Keep service credentials on the moderation-only path rather than
    // allowing update-only grants to bypass the stricter topic/reply rule.
    if *method == Method::PUT
        && segments.len() == 5
        && segments[0] == "api"
        && segments[1] == "forum"
        && forum_path_segment_is_uuid(segments[3])
        && segments[4] == "quotes"
    {
        let required = match segments[2] {
            "topics" => Some(Permission::FORUM_TOPICS_MODERATE),
            "replies" => Some(Permission::FORUM_REPLIES_MODERATE),
            _ => None,
        };
        if required.is_some_and(|required| !has_effective_permission(permissions, &required)) {
            return Some(
                "Service credentials require explicit forum moderation authority for quote relation changes",
            );
        }
    }

    if segments.len() == 4
        && segments[0] == "api"
        && segments[1] == "forum"
        && forum_path_segment_is_uuid(segments[3])
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
        auth_can_resolve_without_tenant_context, auth_error_response,
        is_direct_user_self_service_path, is_direct_user_self_service_principal,
        is_human_user_self_service_path, is_oauth_client_auth_request, is_observability_auth_path,
        is_pages_inline_authoring_server_fn, is_pages_inline_authoring_surface,
        pages_inline_authoring_response, service_forum_boundary_violation,
        verified_tenant_id_from_access_token,
    };
    use axum::http::{HeaderMap, Method, StatusCode, header::AUTHORIZATION};
    use axum::response::IntoResponse;
    use rustok_api::{AuthPrincipalKind, Permission};
    use uuid::Uuid;

    #[test]
    fn tenantless_auth_resolution_is_limited_to_non_tenant_route_scopes() {
        for path in [
            "/health/ready",
            "/catalog/modules",
            "/v2/catalog/publish",
            "/.well-known/openid-configuration",
            "/.well-known/oauth-authorization-server",
            "/api/graphql/ws",
        ] {
            assert!(auth_can_resolve_without_tenant_context(path), "{path}");
        }

        for path in [
            "/api/users",
            "/api/forum/topics",
            "/api/graphql",
            "/modules/pages-authoring",
        ] {
            assert!(!auth_can_resolve_without_tenant_context(path), "{path}");
        }
    }

    #[test]
    fn global_auth_uses_only_the_verified_jwt_tenant_claim() {
        let config = crate::auth::test_auth_config(
            "test-secret-key-for-auth-context-32bytes-long".to_string(),
        );
        let tenant_id = Uuid::new_v4();
        let token = crate::auth::encode_access_token(
            &config,
            Uuid::new_v4(),
            tenant_id,
            rustok_core::UserRole::Customer,
            Uuid::new_v4(),
        )
        .expect("encode access token");

        assert_eq!(
            verified_tenant_id_from_access_token(&config, &token).expect("tenant claim"),
            tenant_id
        );
    }

    #[test]
    fn invalid_global_auth_token_cannot_provide_a_tenant_claim() {
        let config = crate::auth::test_auth_config(
            "test-secret-key-for-auth-context-32bytes-long".to_string(),
        );

        assert_eq!(
            verified_tenant_id_from_access_token(&config, "not-a-jwt"),
            Err((StatusCode::UNAUTHORIZED, "Invalid token signature"))
        );
    }

    #[test]
    fn oauth_client_authentication_is_not_interpreted_as_user_bearer_auth() {
        assert!(is_oauth_client_auth_request("POST", "/api/oauth/token"));
        assert!(is_oauth_client_auth_request("POST", "/api/oauth/revoke"));
        assert!(!is_oauth_client_auth_request("GET", "/api/oauth/token"));
        assert!(!is_oauth_client_auth_request(
            "POST",
            "/api/oauth/authorize"
        ));
        assert!(!is_oauth_client_auth_request("POST", "/api/auth/login"));
    }

    #[tokio::test]
    async fn remote_mcp_credentials_reach_protocol_handler_without_user_jwt() {
        use axum::{Router, body::Body, http::Request, middleware, routing::post};
        use tower::ServiceExt;

        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            crate::common::settings::RustokSettings::default(),
        );
        let state = crate::services::server_runtime_context::ServerAuthRuntime::without_auth(ctx);
        let app = Router::new()
            .route(
                "/api/mcp/runtime/bootstrap",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/api/mcp/runtime/tools/call",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/api/mcp/runtime/tools/stream",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/api/mcp/clients",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .layer(middleware::from_fn_with_state(
                state,
                super::resolve_optional,
            ));

        for path in [
            "/api/mcp/runtime/bootstrap",
            "/api/mcp/runtime/tools/call",
            "/api/mcp/runtime/tools/stream",
            "/api/mcp/clients",
        ] {
            let request = Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(AUTHORIZATION, "Bearer mcp_opaque_test_credential")
                .body(Body::empty())
                .expect("request");
            let response = app.clone().oneshot(request).await.expect("response");
            if path == "/api/mcp/clients" {
                assert!(
                    !response.status().is_success(),
                    "management still requires user JWT"
                );
            } else {
                assert_eq!(response.status(), StatusCode::NO_CONTENT, "{path}");
            }
        }

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/mcp/runtime/bootstrap")
            .header(AUTHORIZATION, "Bearer mcp_opaque_test_credential")
            .header(AUTHORIZATION, "Bearer mcp_opaque_test_credential")
            .body(Body::empty())
            .expect("duplicate credential request");
        assert_eq!(
            app.oneshot(request).await.expect("response").status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn observability_auth_paths_are_excluded_from_user_jwt_resolution() {
        for path in [
            "/metrics",
            "/metrics/",
            "/api/_health/metrics",
            "/health/ready",
            "/health/runtime",
            "/health/modules",
        ] {
            assert!(is_observability_auth_path(path), "{path}");
        }

        for path in [
            "/api/auth/me",
            "/api/graphql",
            "/api/forum/topics",
            "/catalog/modules",
            "/v2/catalog/publish",
        ] {
            assert!(!is_observability_auth_path(path), "{path}");
        }
    }
    #[test]
    fn authorization_presence_distinguishes_anonymous_from_invalid_credentials() {
        let mut headers = HeaderMap::new();
        assert!(!headers.contains_key(AUTHORIZATION));
        headers.insert(AUTHORIZATION, "Bearer invalid".parse().unwrap());
        assert!(headers.contains_key(AUTHORIZATION));
    }

    #[test]
    fn auth_self_service_admission_rejects_delegated_and_service_principals() {
        assert!(is_direct_user_self_service_principal(
            AuthPrincipalKind::DirectUser
        ));
        assert!(!is_direct_user_self_service_principal(
            AuthPrincipalKind::DelegatedUser
        ));
        assert!(!is_direct_user_self_service_principal(
            AuthPrincipalKind::Service
        ));
    }

    #[test]
    fn auth_self_service_routes_require_a_direct_user_principal() {
        for path in [
            "/api/auth/me",
            "/api/auth/sessions",
            "/api/auth/sessions/revoke-all",
            "/api/auth/change-password",
            "/api/auth/profile",
            "/api/auth/history",
            "/api/auth/sessions/00000000-0000-0000-0000-000000000001",
        ] {
            assert!(is_direct_user_self_service_path(path), "{path}");
        }

        for path in [
            "/api/oauth/authorize",
            "/api/oauth/userinfo",
            "/store",
            "/store/customers/me",
            "/api/fn/ai/overview",
            "/api/fn/ai/create-provider",
        ] {
            assert!(!is_direct_user_self_service_path(path), "{path}");
        }
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

    #[tokio::test]
    async fn auth_middleware_errors_use_the_shared_json_envelope_and_redact_internal_details() {
        use axum::body::to_bytes;
        use rustok_web::ErrorBody;

        for (status, message, expected_code, expected_message) in [
            (
                StatusCode::BAD_REQUEST,
                "Multiple Authorization headers are not allowed",
                "bad_request",
                "Multiple Authorization headers are not allowed",
            ),
            (
                StatusCode::UNAUTHORIZED,
                "Invalid token signature",
                "unauthorized",
                "Invalid token signature",
            ),
            (
                StatusCode::FORBIDDEN,
                "Access denied",
                "forbidden",
                "Access denied",
            ),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "secret key configuration details",
                "internal_error",
                "Internal server error",
            ),
        ] {
            let response = auth_error_response(status, message);
            assert_eq!(response.status(), status);
            assert_eq!(
                response
                    .headers()
                    .get(axum::http::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok()),
                Some("application/json")
            );
            let bytes = to_bytes(response.into_body(), 4096)
                .await
                .expect("read error body");
            let body: ErrorBody = serde_json::from_slice(&bytes).expect("JSON error envelope");
            assert_eq!(body.code, expected_code);
            assert_eq!(body.message, expected_message);
            assert!(!body.message.contains("secret key"));
        }
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
    fn forum_service_quote_updates_require_moderation_authority() {
        let topic_id = Uuid::new_v4();
        let reply_id = Uuid::new_v4();

        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &format!("/api/forum/topics/{topic_id}/quotes"),
                &[Permission::FORUM_TOPICS_UPDATE],
            )
            .is_some()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &format!("/api/forum/topics/{topic_id}/quotes"),
                &[Permission::FORUM_TOPICS_MODERATE],
            )
            .is_none()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &format!("/api/forum/replies/{reply_id}/quotes"),
                &[Permission::FORUM_REPLIES_UPDATE],
            )
            .is_some()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &format!("/api/forum/replies/{reply_id}/quotes"),
                &[Permission::FORUM_REPLIES_MODERATE],
            )
            .is_none()
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

    #[test]
    fn forum_service_boundary_decodes_percent_encoded_resource_ids() {
        let id = Uuid::new_v4();
        let encoded_id = id.to_string().replace('-', "%2d");

        assert!(
            service_forum_boundary_violation(
                &Method::POST,
                &format!("/api/forum/topics/{encoded_id}/replies"),
                &[Permission::FORUM_REPLIES_CREATE],
            )
            .is_some()
        );

        let topic_quotes = format!("/api/forum/topics/{encoded_id}/quotes");
        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &topic_quotes,
                &[Permission::FORUM_TOPICS_UPDATE],
            )
            .is_some()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &topic_quotes,
                &[Permission::FORUM_TOPICS_MODERATE],
            )
            .is_none()
        );

        let reply_quotes = format!("/api/forum/replies/{encoded_id}/quotes");
        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &reply_quotes,
                &[Permission::FORUM_REPLIES_UPDATE],
            )
            .is_some()
        );

        let topic = format!("/api/forum/topics/{encoded_id}");
        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &topic,
                &[Permission::FORUM_TOPICS_UPDATE],
            )
            .is_some()
        );

        let reply = format!("/api/forum/replies/{encoded_id}");
        assert!(
            service_forum_boundary_violation(
                &Method::DELETE,
                &reply,
                &[Permission::FORUM_REPLIES_UPDATE],
            )
            .is_some()
        );

        assert!(!forum_path_segment_is_uuid("not%2duuid"));
    }

    #[test]
    fn nil_tenant_id_in_access_token_is_rejected() {
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate()
            .expect("generate independent RSA test keyring");
        let document = fixture.document(1).to_string();
        let (generation, config) = crate::auth::auth_config_from_host_document(
            "test-secret-key-for-auth-context-32bytes-long".to_string(),
            900,
            None,
            document.as_bytes(),
        )
        .expect("prepare complete RS256 test keyring");
        assert_eq!(generation, 1);

        // The canonical encoder refuses nil identifiers, so forge the
        // token directly with the same RSA signer to exercise decode policy.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_secs();
        let claims = crate::auth::Claims {
            sub: Uuid::new_v4(),
            tenant_id: Uuid::nil(),
            role: rustok_core::UserRole::Customer,
            session_id: Uuid::new_v4(),
            iss: config.issuer.clone(),
            aud: config.audience.clone(),
            exp: (now + 3600) as usize,
            iat: now as usize,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
            jti: None,
        };
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some("access".into());
        let key = jsonwebtoken::EncodingKey::from_rsa_pem(fixture.access.private_pem().as_bytes())
            .expect("fixture RSA private key");
        // Positive control: the exact same key/header accepts a valid tenant.
        let mut valid_claims = serde_json::to_value(&claims).unwrap();
        let valid_tenant = Uuid::new_v4();
        valid_claims["tenant_id"] = serde_json::json!(valid_tenant);
        let valid = jsonwebtoken::encode(&header, &valid_claims, &key).unwrap();
        assert_eq!(
            verified_tenant_id_from_access_token(&config, &valid),
            Ok(valid_tenant)
        );
        let token = jsonwebtoken::encode(&header, &claims, &key).expect("forge access token");

        assert_eq!(
            verified_tenant_id_from_access_token(&config, &token),
            Err((StatusCode::UNAUTHORIZED, "Invalid token signature"))
        );
    }

    #[test]
    fn duplicate_authorization_headers_are_detected() {
        let mut headers = HeaderMap::new();
        headers.append(AUTHORIZATION, "Bearer token-1".parse().unwrap());
        headers.append(AUTHORIZATION, "Bearer token-2".parse().unwrap());
        assert_eq!(headers.get_all(AUTHORIZATION).iter().count(), 2);
    }

    #[test]
    fn forum_service_solution_and_replies_moderation_rules() {
        assert!(
            service_forum_boundary_violation(&Method::POST, "/api/products", &[]).is_none(),
            "non-forum paths must not be flagged"
        );

        let topic_id = Uuid::new_v4();
        let solution_path = format!("/api/forum/topics/{topic_id}/solution");
        assert!(
            service_forum_boundary_violation(
                &Method::POST,
                &solution_path,
                &[Permission::FORUM_TOPICS_UPDATE]
            )
            .is_some()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::POST,
                &solution_path,
                &[Permission::FORUM_TOPICS_MODERATE]
            )
            .is_none()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::DELETE,
                &solution_path,
                &[Permission::FORUM_TOPICS_MODERATE]
            )
            .is_none()
        );

        let reply_id = Uuid::new_v4();
        let reply_path = format!("/api/forum/replies/{reply_id}");
        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &reply_path,
                &[Permission::FORUM_REPLIES_UPDATE]
            )
            .is_some()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::PUT,
                &reply_path,
                &[Permission::FORUM_REPLIES_MODERATE]
            )
            .is_none()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::DELETE,
                &reply_path,
                &[Permission::FORUM_REPLIES_MODERATE]
            )
            .is_none()
        );

        let topic_path = format!("/api/forum/topics/{topic_id}");
        assert!(
            service_forum_boundary_violation(
                &Method::DELETE,
                &topic_path,
                &[Permission::FORUM_TOPICS_MODERATE]
            )
            .is_none()
        );
        assert!(
            service_forum_boundary_violation(
                &Method::DELETE,
                &topic_path,
                &[Permission::FORUM_TOPICS_READ]
            )
            .is_some()
        );
    }
}
