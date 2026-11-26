use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Method, Request, header},
    middleware::Next,
    response::Response,
};
use rustok_auth_ui::ui::leptos::ServerAuthSnapshot;

fn cookie_request_is_secure(headers: &HeaderMap) -> bool {
    let forwarded_https = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("https"));
    let origin_https = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("https://"));
    forwarded_https || origin_https
}

/// Parse a single explicit Bearer authorization header. HTTP auth scheme names are
/// case-insensitive; malformed and duplicate headers fail closed.
pub fn bearer_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let raw = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    let (scheme, token) = raw.trim().split_once(' ')?;
    let token = token.trim();
    if !scheme.eq_ignore_ascii_case("bearer")
        || token.is_empty()
        || token.chars().any(char::is_whitespace)
    {
        return None;
    }
    Some(token.to_string())
}

/// A Bearer API request must provide its tenant explicitly. Never infer it from a
/// simultaneously-present browser session.
pub fn bearer_tenant_slug_from_headers(headers: &HeaderMap) -> Option<String> {
    let mut values = headers.get_all("x-tenant-slug").iter();
    let value = values.next()?.to_str().ok()?.trim();
    if values.next().is_some() || value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

/// Exchange the HttpOnly BFF refresh cookie (set by the native auth
/// server functions at sign-in) for a verified auth snapshot.
///
/// The refresh token is never readable by page JavaScript and never
/// rendered into HTML: this host validates it server-to-server and
/// exposes only the resulting short-lived access token and user
/// profile to SSR rendering. The returned `Set-Cookie` value carries
/// the rotated refresh envelope and must be attached to the response. A lost
/// response can recover using the unchanged envelope inside backend grace.
pub async fn verified_snapshot_from_headers(
    headers: &HeaderMap,
) -> (ServerAuthSnapshot, Option<String>) {
    let cookie_headers = headers
        .get_all(header::COOKIE)
        .iter()
        .map(|value| value.to_str())
        .collect::<Result<Vec<_>, _>>();
    let Ok(cookie_headers) = cookie_headers else {
        return (ServerAuthSnapshot::default(), None);
    };
    let Ok(Some(credential)) =
        rustok_auth_ui::core::inspect_refresh_cookie_headers(cookie_headers)
    else {
        // Invalid or duplicate credentials do not produce a verified SSR identity.
        return (ServerAuthSnapshot::default(), None);
    };

    let secure = cookie_request_is_secure(headers);
    match rustok_auth_ui::transport::exchange_refresh_token_ssr(credential).await
    {
        Ok((user, session, rotated)) => (
            ServerAuthSnapshot {
                session: Some(session),
                user: Some(user),
            },
            Some(rustok_auth_ui::core::refresh_cookie_header(
                &rotated, secure,
            )),
        ),
        // An outage and a rejected credential are not distinguishable here. Preserve the
        // cookie rather than silently destroying a potentially live session on failure.
        Err(_) => (ServerAuthSnapshot::default(), None),
    }
}

/// Auth-owned server functions consume and update the browser cookie themselves.
fn auth_function_owns_cookie(path: &str) -> bool {
    matches!(path,
        "/api/fn/auth/sign-in" | "/api/fn/auth/sign-up" |
        "/api/fn/auth/sign-out" | "/api/fn/auth/refresh-token" |
        "/api/fn/auth/accept-invite" | "/api/fn/auth/forgot-password" |
        "/api/fn/auth/current-user")
}

/// Authenticate the request before SSR and update the BFF cookie.
///
/// The snapshot is placed in request extensions (same trust boundary
/// as `request_auth_snapshot`) so both Leptos rendering and plain
/// axum handlers (e.g. the page-builder intent endpoint) consume the
/// single verified exchange performed here — no handler ever
/// re-exchanges the cookie, and rotation is always propagated on the
/// response that consumed the previous token.
pub async fn verify_auth_snapshot(mut request: Request<Body>, next: Next) -> Response {
    // Index AJAX calls relay bearer credentials to the backend. Do not rotate the SSR cookie for
    // these requests: parallel diagnostics reads must not race the auth provider's refresh.
    if request.method() == Method::POST
        && rustok_index_admin::transport::IndexAdminUpstream::is_request_path(request.uri().path())
    {
        return next.run(request).await;
    }
    // Auth functions must not race a second middleware exchange or append an
    // obsolete cookie after the function issued or cleared its own credential.
    if auth_function_owns_cookie(request.uri().path()) {
        request.extensions_mut().insert(ServerAuthSnapshot::default());
        let mut response = next.run(request).await;
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        return response;
    }
    // An explicit Authorization header is the sole authority for API requests. Do not
    // refresh or attach an ambient browser session as a second credential, and insert an
    // anonymous snapshot because some API handlers still require the Extension extractor.
    if request.headers().contains_key(header::AUTHORIZATION) {
        request
            .extensions_mut()
            .insert(ServerAuthSnapshot::default());
        return next.run(request).await;
    }

    let had_cookie = request.headers().contains_key(header::COOKIE);
    let (snapshot, set_cookie) = verified_snapshot_from_headers(request.headers()).await;
    request.extensions_mut().insert(snapshot);

    let mut response = next.run(request).await;
    if had_cookie {
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    }
    if let Some(cookie) = set_cookie
        && let Ok(mut value) = HeaderValue::from_str(&cookie)
    {
        value.set_sensitive(true);
        // A handler's explicit session mutation wins over this snapshot.
        let owns_cookie = response.headers().get_all(header::SET_COOKIE).iter().any(|header| {
            header.to_str().ok().is_some_and(|value| {
                value.split_once('=').is_some_and(|(name, _)| name.trim() == rustok_auth_ui::core::ADMIN_REFRESH_COOKIE)
            })
        });
        if !owns_cookie {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

/// Extract an auth snapshot previously inserted by trusted host middleware.
///
/// The request type and extension bag are crate-private inputs from the SSR host. Do not expose
/// this conversion for untrusted serialized input without re-verifying authority at that boundary.
pub fn request_auth_snapshot(parts: &axum::http::request::Parts) -> ServerAuthSnapshot {
    parts
        .extensions
        .get::<ServerAuthSnapshot>()
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use rustok_auth_ui::model::{AuthSession, AuthUser};

    #[test]
    fn request_snapshot_uses_trusted_host_extension() {
        let mut request = Request::new(());
        request.extensions_mut().insert(ServerAuthSnapshot {
            session: Some(AuthSession {
                token: "verified-access".to_string(),
                expires_at: 123,
                tenant: "demo".to_string(),
            }),
            user: Some(AuthUser {
                id: "user-1".to_string(),
                email: "verified@example.test".to_string(),
                name: Some("Verified".to_string()),
                role: "admin".to_string(),
                permissions: Vec::new(),
            }),
        });
        let (parts, ()) = request.into_parts();
        let snapshot = request_auth_snapshot(&parts);
        assert_eq!(snapshot.user.unwrap().id, "user-1");
        assert_eq!(snapshot.session.unwrap().tenant, "demo");
    }

    #[test]
    fn bearer_authorization_parser_is_case_insensitive_and_rejects_ambiguous_headers() {
        let mut headers = HeaderMap::new();
        assert_eq!(bearer_token_from_headers(&headers), None);

        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("bEaReR access-token"),
        );
        assert_eq!(
            bearer_token_from_headers(&headers).as_deref(),
            Some("access-token")
        );

        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Basic credentials"),
        );
        assert_eq!(bearer_token_from_headers(&headers), None);

        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer token with-space"),
        );
        assert_eq!(bearer_token_from_headers(&headers), None);

        headers.append(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer second-token"),
        );
        assert_eq!(bearer_token_from_headers(&headers), None);
    }

    #[test]
    fn bearer_tenant_must_be_explicit_and_unambiguous() {
        let mut headers = HeaderMap::new();
        assert_eq!(bearer_tenant_slug_from_headers(&headers), None);

        headers.insert("x-tenant-slug", HeaderValue::from_static(" tenant-a "));
        assert_eq!(
            bearer_tenant_slug_from_headers(&headers).as_deref(),
            Some("tenant-a")
        );

        headers.append("x-tenant-slug", HeaderValue::from_static("tenant-b"));
        assert_eq!(bearer_tenant_slug_from_headers(&headers), None);
    }

    #[test]
    fn auth_functions_own_their_cookie_transition() {
        for path in ["/api/fn/auth/sign-in", "/api/fn/auth/sign-up", "/api/fn/auth/sign-out", "/api/fn/auth/refresh-token"] {
            assert!(auth_function_owns_cookie(path));
        }
        assert!(!auth_function_owns_cookie("/api/admin/pages/1/builder/intents"));
        assert!(!auth_function_owns_cookie("/dashboard"));
    }

    #[tokio::test]
    async fn missing_refresh_cookie_yields_anonymous_snapshot() {
        let headers = HeaderMap::new();
        let (snapshot, set_cookie) = verified_snapshot_from_headers(&headers).await;
        assert_eq!(snapshot, ServerAuthSnapshot::default());
        assert!(set_cookie.is_none());
    }

    #[tokio::test]
    async fn ambiguous_refresh_cookies_never_select_or_clear_a_credential() {
        let mut headers = HeaderMap::new();
        let wire = format!("rustok-admin-refresh-v1=shop.{}.{}", "a1".repeat(32), "b2".repeat(32));
        headers.append(header::COOKIE, HeaderValue::from_str(&wire).unwrap());
        headers.append(header::COOKIE, HeaderValue::from_str(&wire).unwrap());
        let (snapshot, set_cookie) = verified_snapshot_from_headers(&headers).await;
        assert_eq!(snapshot, ServerAuthSnapshot::default());
        assert!(set_cookie.is_none());

        headers.clear();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("rustok-admin-refresh-v1=invalid"),
        );
        let (snapshot, set_cookie) = verified_snapshot_from_headers(&headers).await;
        assert_eq!(snapshot, ServerAuthSnapshot::default());
        assert!(set_cookie.is_none());
    }
}
