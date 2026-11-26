//! Shared server-side cookie admission and response policy. Not a principal verifier.
use axum::{http::{HeaderMap, HeaderValue, Method, header}, response::Response};
use super::ServerAuthSnapshot;
use crate::core::{ADMIN_REFRESH_COOKIE, RefreshCredential, inspect_refresh_cookie_headers};

#[derive(Clone, Copy)]
pub enum SnapshotTenant<'a> {
    CookieScoped,
    RequestScoped { slug: &'a str, id: &'a str },
}

fn credential_from_headers(headers: &HeaderMap, scope: SnapshotTenant<'_>) -> Option<RefreshCredential> {
    if headers.contains_key(header::AUTHORIZATION) { return None; }
    let values = headers.get_all(header::COOKIE).iter()
        .map(|value| value.to_str()).collect::<Result<Vec<_>, _>>().ok()?;
    let credential = inspect_refresh_cookie_headers(values).ok().flatten()?;
    if let SnapshotTenant::RequestScoped { slug, id } = scope {
        if slug != credential.tenant() || !crate::core::canonical_session_id(id) { return None; }
    }
    Some(credential)
}

pub async fn verified_snapshot_from_headers(
    headers: &HeaderMap, scope: SnapshotTenant<'_>,
) -> (ServerAuthSnapshot, Option<HeaderValue>) {
    let Some(credential) = credential_from_headers(headers, scope) else {
        return (ServerAuthSnapshot::default(), None);
    };
    let tenant_id = match scope {
        SnapshotTenant::CookieScoped => None,
        SnapshotTenant::RequestScoped { id, .. } => Some(id),
    };
    let Ok((user, session, rotated)) =
        crate::transport::exchange_refresh_token_ssr(credential, tenant_id).await
    else { return (ServerAuthSnapshot::default(), None); };
    let Ok(mut cookie) = HeaderValue::from_str(&crate::core::refresh_cookie_header(
        &rotated, crate::transport::cookie_request_is_secure(headers),
    )) else { return (ServerAuthSnapshot::default(), None); };
    cookie.set_sensitive(true);
    (ServerAuthSnapshot { session: Some(session), user: Some(user) }, Some(cookie))
}

/// Do not consume credentials for HEAD, prefetch or subresources.
pub fn is_document_navigation(method: &Method, headers: &HeaderMap) -> bool {
    if method != Method::GET || ["purpose", "sec-purpose"].iter().any(|name| {
        headers.get_all(*name).iter().any(|value| value.to_str().ok()
            .is_none_or(|value| value.to_ascii_lowercase().contains("prefetch")
                || value.to_ascii_lowercase().contains("prerender")))
    }) { return false; }
    if let Some(dest) = headers.get("sec-fetch-dest") {
        return matches!(dest.to_str().ok(), Some("document" | "iframe" | "frame"));
    }
    headers.get_all(header::ACCEPT).iter().any(|value| value.to_str().ok()
        .is_some_and(|value| value.split(',').any(|part| {
            matches!(part.split(';').next().map(str::trim), Some("text/html" | "application/xhtml+xml"))
        })))
}

pub fn mark_private(response: &mut Response) {
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    response.headers_mut().remove(header::ETAG);
    response.headers_mut().remove(header::LAST_MODIFIED);
}

pub fn apply_snapshot_cookie(response: &mut Response, cookie: Option<HeaderValue>) {
    let Some(cookie) = cookie else { return; };
    mark_private(response);
    let owns_cookie = response.headers().get_all(header::SET_COOKIE).iter().any(|value| {
        value.to_str().ok().is_some_and(|value| value.split_once('=')
            .is_some_and(|(name, _)| name.trim() == ADMIN_REFRESH_COOKIE))
    });
    if !owns_cookie { response.headers_mut().append(header::SET_COOKIE, cookie); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    #[test]
    fn tenant_and_bearer_fence_cookie_admission() {
        let mut headers = HeaderMap::new();
        let value = format!("{ADMIN_REFRESH_COOKIE}=shop.{}.{}", "a1".repeat(32), "b2".repeat(32));
        headers.insert(header::COOKIE, HeaderValue::from_str(&value).unwrap());
        let id = "a1111111-1111-4111-8111-111111111111";
        assert!(credential_from_headers(&headers, SnapshotTenant::RequestScoped { slug: "shop", id }).is_some());
        assert!(credential_from_headers(&headers, SnapshotTenant::RequestScoped { slug: "other", id }).is_none());
        assert!(credential_from_headers(&headers, SnapshotTenant::RequestScoped { slug: "shop", id: "" }).is_none());
        headers.append(header::COOKIE, HeaderValue::from_str(&value).unwrap());
        assert!(credential_from_headers(&headers, SnapshotTenant::CookieScoped).is_none());
        headers.insert(header::COOKIE, HeaderValue::from_str(&value).unwrap());
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("invalid bearer"));
        assert!(credential_from_headers(&headers, SnapshotTenant::CookieScoped).is_none());
    }

    #[test]
    fn only_document_navigation_consumes_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml"));
        assert!(is_document_navigation(&Method::GET, &headers));
        assert!(!is_document_navigation(&Method::HEAD, &headers));
        assert!(!is_document_navigation(&Method::POST, &headers));
        headers.insert("sec-purpose", HeaderValue::from_static("prefetch;prerender"));
        assert!(!is_document_navigation(&Method::GET, &headers));
        headers.remove("sec-purpose");
        headers.insert("sec-fetch-dest", HeaderValue::from_static("script"));
        assert!(!is_document_navigation(&Method::GET, &headers));
        headers.insert("sec-fetch-dest", HeaderValue::from_static("document"));
        assert!(is_document_navigation(&Method::GET, &headers));
    }

    #[test]
    fn explicit_cookie_wins_and_response_is_private() {
        let mut response = "page".into_response();
        response.headers_mut().insert(header::ETAG, HeaderValue::from_static("\"public\""));
        response.headers_mut().append(header::SET_COOKIE, HeaderValue::from_static("other=1"));
        let mut cookie = HeaderValue::from_static("rustok-admin-refresh-v1=rotated");
        cookie.set_sensitive(true);
        apply_snapshot_cookie(&mut response, Some(cookie.clone()));
        assert_eq!(response.headers()[header::CACHE_CONTROL], "private, no-store");
        assert!(!response.headers().contains_key(header::ETAG));
        assert_eq!(response.headers().get_all(header::SET_COOKIE).iter().count(), 2);
        assert!(response.headers().get_all(header::SET_COOKIE).iter().last().unwrap().is_sensitive());
        let mut response = "page".into_response();
        response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_static("rustok-admin-refresh-v1=; Max-Age=0"));
        apply_snapshot_cookie(&mut response, Some(cookie));
        assert_eq!(response.headers().get_all(header::SET_COOKIE).iter().count(), 1);
        assert_eq!(response.headers()[header::SET_COOKIE], "rustok-admin-refresh-v1=; Max-Age=0");
    }
}
