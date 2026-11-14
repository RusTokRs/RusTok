use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Request, header},
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

/// Exchange the HttpOnly BFF refresh cookie (set by the native auth
/// server functions at sign-in) for a verified auth snapshot.
///
/// The refresh token is never readable by page JavaScript and never
/// rendered into HTML: this host validates it server-to-server and
/// exposes only the resulting short-lived access token and user
/// profile to SSR rendering. The returned `Set-Cookie` value carries
/// the rotated refresh token and must be attached to the response — dropping it
/// would make the browser replay the previous token and trip reuse detection.
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
    let Ok(Some((tenant, refresh_token))) =
        rustok_auth_ui::core::inspect_refresh_cookie_headers(cookie_headers)
    else {
        // Invalid or duplicate credentials do not produce a verified SSR identity.
        return (ServerAuthSnapshot::default(), None);
    };

    let secure = cookie_request_is_secure(headers);
    match rustok_auth_ui::transport::exchange_refresh_token_ssr(refresh_token, tenant.clone()).await
    {
        Ok((user, session, rotated)) => (
            ServerAuthSnapshot {
                session: Some(session),
                user: Some(user),
            },
            Some(rustok_auth_ui::core::refresh_cookie_header(
                &tenant, &rotated, secure,
            )),
        ),
        // An outage and a rejected credential are not distinguishable here. Preserve the
        // cookie rather than silently destroying a potentially live session on failure.
        Err(_) => (ServerAuthSnapshot::default(), None),
    }
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
    let (snapshot, set_cookie) = verified_snapshot_from_headers(request.headers()).await;
    request.extensions_mut().insert(snapshot);

    let mut response = next.run(request).await;
    if let Some(cookie) = set_cookie
        && let Ok(value) = HeaderValue::from_str(&cookie)
    {
        response.headers_mut().append(header::SET_COOKIE, value);
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
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("rustok-admin-refresh-v1=shop.one"),
        );
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("rustok-admin-refresh-v1=shop.two"),
        );
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
