use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Method, Request, header},
    middleware::Next,
    response::Response,
};
use rustok_auth_ui::ui::leptos::{
    ServerAuthSnapshot,
    ssr::{SnapshotTenant, apply_snapshot_cookie, is_document_navigation, mark_private, verified_snapshot_from_headers},
};

/// Parse a single explicit Bearer authorization header. HTTP auth scheme names are
/// case-insensitive; malformed and duplicate headers fail closed.
/// Read one unambiguous header value. Duplicate values fail closed rather than
/// letting different HTTP layers choose different CSRF or locale values.
pub fn single_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?.trim();
    if values.next().is_some() || value.is_empty() {
        return None;
    }
    Some(value.to_string())
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

/// Auth-owned server functions consume and update the browser cookie themselves.
fn auth_function_owns_cookie(path: &str) -> bool {
    path.starts_with("/api/fn/auth/")
}

fn request_needs_snapshot(method: &Method, path: &str, headers: &HeaderMap) -> bool {
    if auth_function_owns_cookie(path) || headers.contains_key(header::AUTHORIZATION) {
        return false;
    }
    if matches!(path, "/health" | "/install" | "/favicon.ico")
        || ["/assets/", "/pkg/", "/richtext/"].iter().any(|prefix| path.starts_with(prefix))
    { return false; }
    // Explicit page-builder consumers and existing cookie-based native calls.
    if let Some(rest) = path.strip_prefix("/api/admin/pages/") {
        let mut segments = rest.splitn(2, '/');
        let page = segments.next().unwrap_or_default();
        return !page.is_empty() && match segments.next() {
            Some("preview") => method == Method::GET,
            Some("builder/intents") => method == Method::POST,
            _ => false,
        };
    }
    if path.starts_with("/api/fn/") { return method == Method::POST; }
    !path.starts_with("/api/") && is_document_navigation(method, headers)
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
    let needs_snapshot = request_needs_snapshot(request.method(), request.uri().path(), request.headers());
    let credential_request = request.headers().contains_key(header::COOKIE)
        || request.headers().contains_key(header::AUTHORIZATION);
    let (snapshot, set_cookie) = if needs_snapshot {
        verified_snapshot_from_headers(request.headers(), SnapshotTenant::CookieScoped).await
    } else { (ServerAuthSnapshot::default(), None) };
    request.extensions_mut().insert(snapshot);
    let mut response = next.run(request).await;
    if needs_snapshot || (credential_request && response.headers().get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok()).is_some_and(|value| value.starts_with("text/html")))
    { mark_private(&mut response); }
    apply_snapshot_cookie(&mut response, set_cookie);
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
    fn single_header_value_rejects_duplicate_values() {
        let mut headers = HeaderMap::new();
        headers.append("x-csrf-token", HeaderValue::from_static("valid-token"));
        assert_eq!(
            single_header_value(&headers, "x-csrf-token").as_deref(),
            Some("valid-token")
        );
        headers.append("x-csrf-token", HeaderValue::from_static("second-token"));
        assert_eq!(single_header_value(&headers, "x-csrf-token"), None);
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

    #[test]
    fn assets_health_head_and_prefetch_do_not_rotate() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, HeaderValue::from_static("text/html"));
        assert!(request_needs_snapshot(&Method::GET, "/dashboard", &headers));
        for path in ["/health", "/pkg/app.wasm", "/assets/app.css", "/richtext/frame", "/favicon.ico", "/install", "/api/other", "/api/fn/auth/refresh-token"] {
            assert!(!request_needs_snapshot(&Method::GET, path, &headers), "{path}");
        }
        assert!(!request_needs_snapshot(&Method::HEAD, "/dashboard", &headers));
        headers.insert("sec-purpose", HeaderValue::from_static("prefetch"));
        assert!(!request_needs_snapshot(&Method::GET, "/dashboard", &headers));
        headers.remove("sec-purpose");
        assert!(request_needs_snapshot(&Method::POST, "/api/admin/pages/123/builder/intents", &headers));
        assert!(request_needs_snapshot(&Method::GET, "/api/admin/pages/123/preview", &headers));
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer explicit"));
        assert!(!request_needs_snapshot(&Method::POST, "/api/admin/pages/123/builder/intents", &headers));
    }

    #[tokio::test]
    async fn missing_refresh_cookie_yields_anonymous_snapshot() {
        let headers = HeaderMap::new();
        let (snapshot, set_cookie) = verified_snapshot_from_headers(&headers, SnapshotTenant::CookieScoped).await;
        assert_eq!(snapshot, ServerAuthSnapshot::default());
        assert!(set_cookie.is_none());
    }

    #[tokio::test]
    async fn ambiguous_refresh_cookies_never_select_or_clear_a_credential() {
        let mut headers = HeaderMap::new();
        let wire = format!("rustok-admin-refresh-v1=shop.{}.{}", "a1".repeat(32), "b2".repeat(32));
        headers.append(header::COOKIE, HeaderValue::from_str(&wire).unwrap());
        headers.append(header::COOKIE, HeaderValue::from_str(&wire).unwrap());
        let (snapshot, set_cookie) = verified_snapshot_from_headers(&headers, SnapshotTenant::CookieScoped).await;
        assert_eq!(snapshot, ServerAuthSnapshot::default());
        assert!(set_cookie.is_none());

        headers.clear();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("rustok-admin-refresh-v1=invalid"),
        );
        let (snapshot, set_cookie) = verified_snapshot_from_headers(&headers, SnapshotTenant::CookieScoped).await;
        assert_eq!(snapshot, ServerAuthSnapshot::default());
        assert!(set_cookie.is_none());
    }
}
