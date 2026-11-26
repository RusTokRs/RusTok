//! Direct browser authentication against the server's HttpOnly cookie
//! session endpoints (`/api/auth/session/*`).
//!
//! The refresh token is held by the server in an HttpOnly cookie and
//! never enters JavaScript-readable memory: responses contain only
//! the short-lived access token and the user profile. This path
//! requires the API to be same-origin with the page (the default
//! deployment); cross-origin API bases cannot persist the cookie and
//! should use the SSR (native) transport instead.

use serde::Deserialize;
use serde_json::json;

use super::{get_api_url, now_unix_secs};
use crate::model::{AuthError, AuthSession, AuthUser};

#[derive(Debug, Deserialize)]
struct CookieSessionResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: u64,
    user: CookieUserInfo,
}

#[derive(Debug, Deserialize)]
struct CookieUserInfo {
    id: String,
    email: String,
    name: Option<String>,
    role: String,
    #[allow(dead_code)]
    status: String,
    #[serde(default)]
    permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    status: String,
}

fn confirmed_logout(response: StatusResponse) -> Result<(), AuthError> {
    if response.status == "ok" {
        Ok(())
    } else {
        Err(AuthError::Network)
    }
}

fn to_user_session(payload: CookieSessionResponse, tenant: String) -> (AuthUser, AuthSession) {
    let user = AuthUser {
        id: payload.user.id,
        email: payload.user.email,
        name: payload.user.name,
        role: payload.user.role,
        permissions: payload.user.permissions,
    };
    let session = AuthSession {
        token: payload.access_token,
        expires_at: now_unix_secs() + payload.expires_in as i64,
        tenant,
    };
    (user, session)
}

#[cfg(any(test, target_arch = "wasm32"))]
fn cookie_session_is_same_origin(endpoint: &reqwest::Url, page_origin: &str) -> bool {
    endpoint.origin().ascii_serialization() == page_origin
}

fn cookie_session_endpoint(api_url: &str, path: &str) -> Result<reqwest::Url, AuthError> {
    if !matches!(
        path,
        "/api/auth/session/login"
            | "/api/auth/session/register"
            | "/api/auth/session/refresh"
            | "/api/auth/session/logout"
    ) {
        return Err(AuthError::Network);
    }
    let endpoint = reqwest::Url::parse(&format!("{}{path}", api_url.trim_end_matches('/')))
        .map_err(|_| AuthError::Network)?;
    if !matches!(endpoint.scheme(), "http" | "https")
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
        || !endpoint.path().ends_with(path)
    {
        return Err(AuthError::Network);
    }
    #[cfg(target_arch = "wasm32")]
    {
        // Fetch sends same-origin cookies by default. A cross-origin logout could otherwise
        // return "ok" for an absent cookie while the real session remains live.
        let page_origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .ok_or(AuthError::Network)?;
        if !cookie_session_is_same_origin(&endpoint, &page_origin) {
            return Err(AuthError::Network);
        }
    }
    Ok(endpoint)
}

static HTTP_CLIENT: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(reqwest::Client::new);

async fn post_session(
    path: &str,
    tenant: &str,
    body: Option<serde_json::Value>,
    session_request: bool,
    is_login: bool,
    expected_session_id: Option<&str>,
) -> Result<CookieSessionResponse, AuthError> {
    let endpoint = cookie_session_endpoint(&get_api_url(), path)?;
    request_session(endpoint, tenant, body, session_request, is_login, expected_session_id).await
}

#[cfg(not(target_arch = "wasm32"))]
async fn request_session<T: serde::de::DeserializeOwned>(
    endpoint: reqwest::Url,
    tenant: &str,
    body: Option<serde_json::Value>,
    session_request: bool,
    is_login: bool,
    expected_session_id: Option<&str>,
) -> Result<T, AuthError> {
    let mut request = HTTP_CLIENT.post(endpoint.clone())
        .timeout(std::time::Duration::from_secs(10))
        .header("X-Tenant-ID", tenant);
    if session_request {
        request = request.header("x-rustok-session-request", "1");
    }
    if let Some(expected) = expected_session_id {
        request = request.header(crate::core::EXPECTED_SESSION_ID_HEADER, expected);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.map_err(|_| AuthError::Network)?;
    if response.url() != &endpoint {
        return Err(AuthError::Network);
    }
    if !response.status().is_success() {
        return Err(AuthError::from_status(response.status().as_u16(), is_login));
    }
    response.json::<T>().await.map_err(|_| AuthError::Network)
}

#[cfg(target_arch = "wasm32")]
async fn request_session<T: serde::de::DeserializeOwned>(
    endpoint: reqwest::Url,
    tenant: &str,
    body: Option<serde_json::Value>,
    session_request: bool,
    is_login: bool,
    expected_session_id: Option<&str>,
) -> Result<T, AuthError> {
    use leptos::wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    let window = web_sys::window().ok_or(AuthError::Network)?;
    let headers = web_sys::Headers::new().map_err(|_| AuthError::Network)?;
    headers.set("X-Tenant-ID", tenant).map_err(|_| AuthError::Network)?;
    if session_request {
        headers.set("x-rustok-session-request", "1").map_err(|_| AuthError::Network)?;
    }
    if let Some(expected) = expected_session_id {
        headers.set(crate::core::EXPECTED_SESSION_ID_HEADER, expected).map_err(|_| AuthError::Network)?;
    }
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_mode(web_sys::RequestMode::SameOrigin);
    init.set_credentials(web_sys::RequestCredentials::SameOrigin);
    init.set_redirect(web_sys::RequestRedirect::Error);
    init.set_cache(web_sys::RequestCache::NoStore);
    let signal = super::browser_lock::timeout_signal(10_000)?;
    init.set_signal(Some(&signal));
    if let Some(body) = body {
        headers.set("content-type", "application/json").map_err(|_| AuthError::Network)?;
        init.set_body(&JsValue::from_str(&body.to_string()));
    }
    init.set_headers(headers.as_ref());
    let request = web_sys::Request::new_with_str_and_init(endpoint.as_str(), &init)
        .map_err(|_| AuthError::Network)?;
    let response = JsFuture::from(window.fetch_with_request(&request)).await
        .map_err(|_| AuthError::Network)?
        .dyn_into::<web_sys::Response>().map_err(|_| AuthError::Network)?;
    if response.redirected() || response.url() != endpoint.as_str() {
        return Err(AuthError::Network);
    }
    if !response.ok() {
        return Err(AuthError::from_status(response.status(), is_login));
    }
    let text = JsFuture::from(response.text().map_err(|_| AuthError::Network)?).await
        .map_err(|_| AuthError::Network)?.as_string().ok_or(AuthError::Network)?;
    serde_json::from_str(&text).map_err(|_| AuthError::Network)
}

pub(super) async fn sign_in_cookie(
    email: String,
    password: String,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    let payload = post_session(
        "/api/auth/session/login",
        &tenant,
        Some(json!({ "email": email, "password": password })),
        false,
        true,
        None,
    )
    .await?;
    Ok(to_user_session(payload, tenant))
}

pub(super) async fn sign_up_cookie(
    email: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    let payload = post_session(
        "/api/auth/session/register",
        &tenant,
        Some(json!({ "email": email, "password": password, "name": name })),
        false,
        false,
        None,
    )
    .await?;
    Ok(to_user_session(payload, tenant))
}

pub(super) async fn refresh_session_cookie(
    tenant: String,
    expected_session_id: Option<String>,
) -> Result<(AuthSession, AuthUser), AuthError> {
    // The API owns the proof in the HttpOnly envelope. If the old cookie
    // is still present, an ambiguous response can recover the same successor.
    // If Set-Cookie was already applied, the retry starts from the new generation.
    let first = post_session("/api/auth/session/refresh", &tenant, None, true, false, expected_session_id.as_deref()).await;
    let payload = match first {
        Err(AuthError::Network | AuthError::Http(500..=599)) =>
            post_session("/api/auth/session/refresh", &tenant, None, true, false, expected_session_id.as_deref()).await?,
        other => other?,
    };
    let (user, session) = to_user_session(payload, tenant);
    Ok((session, user))
}

#[derive(Debug, Deserialize)]
struct AcceptInviteResponse {
    #[allow(dead_code)]
    status: String,
    email: String,
    #[allow(dead_code)]
    role: String,
}

/// Accept an admin-issued invite directly against the REST API.
/// Returns the new account's email; the caller signs in with the
/// password it just set to establish the cookie session.
pub(super) async fn accept_invite_direct(
    invite_token: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<String, AuthError> {
    let response = HTTP_CLIENT
        .post(format!("{}/api/auth/invite/accept", get_api_url()))
        .header("X-Tenant-ID", tenant)
        .json(&json!({
            "token": invite_token,
            "password": password,
            "name": name,
        }))
        .send()
        .await
        .map_err(|_| AuthError::Network)?;
    if !response.status().is_success() {
        return Err(AuthError::from_status(response.status().as_u16(), false));
    }
    let payload = response
        .json::<AcceptInviteResponse>()
        .await
        .map_err(|_| AuthError::Network)?;
    Ok(payload.email)
}

pub(super) async fn sign_out_cookie(tenant: String, expected_session_id: Option<String>) -> Result<(), AuthError> {
    sign_out_cookie_at(&get_api_url(), tenant, expected_session_id.as_deref()).await
}

async fn sign_out_cookie_at(api_url: &str, tenant: String, expected_session_id: Option<&str>) -> Result<(), AuthError> {
    let endpoint = cookie_session_endpoint(api_url, "/api/auth/session/logout")?;
    let payload: StatusResponse = request_session(endpoint, &tenant, None, true, false, expected_session_id).await?;
    confirmed_logout(payload)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{
        AuthError, cookie_session_endpoint, cookie_session_is_same_origin, sign_out_cookie_at,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn mock_api(
        status: &str,
        body: &str,
        headers: &str,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n{headers}\r\n{body}",
            body.len()
        );
        let received = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = socket.read(&mut buffer).await.unwrap();
                assert!(read > 0 && request.len() + read <= 16_384);
                request.extend_from_slice(&buffer[..read]);
            }
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8(request).unwrap()
        });
        (format!("http://{address}"), received)
    }

    #[tokio::test]
    async fn direct_cookie_logout_forwards_expected_lineage_without_following_conflict() {
        let expected = "a1111111-1111-4111-8111-111111111111";
        let (url, received) = mock_api("409 Conflict", "conflict", "").await;
        assert_eq!(sign_out_cookie_at(&url, "shop".into(), Some(expected)).await, Err(AuthError::Http(409)));
        let request = received.await.unwrap().to_ascii_lowercase();
        assert!(request.contains(&format!("\r\nx-rustok-expected-session-id: {expected}\r\n")));
    }

    #[tokio::test]
    async fn direct_cookie_logout_requires_an_explicit_ok_response() {
        let (url, request) = mock_api("200 OK", r#"{"status":"ok"}"#, "").await;
        assert_eq!(sign_out_cookie_at(&url, "shop".into(), None).await, Ok(()));
        let request = request.await.unwrap().to_ascii_lowercase();
        assert!(request.starts_with("post /api/auth/session/logout http/1.1\r\n"));
        assert!(request.contains("\r\nx-tenant-id: shop\r\n"));
        assert!(request.contains("\r\nx-rustok-session-request: 1\r\n"));

        for (status, body) in [
            ("200 OK", r#"{"status":"unknown"}"#),
            ("200 OK", "<html>login</html>"),
            ("503 Service Unavailable", r#"{"status":"ok"}"#),
        ] {
            let (url, request) = mock_api(status, body, "").await;
            assert!(matches!(
                sign_out_cookie_at(&url, "shop".into(), None).await,
                Err(AuthError::Network | AuthError::Http(_))
            ));
            request.await.unwrap();
        }
    }

    #[test]
    fn direct_cookie_session_accepts_only_the_expected_endpoint_paths() {
        for path in [
            "/api/auth/session/login",
            "/api/auth/session/register",
            "/api/auth/session/refresh",
            "/api/auth/session/logout",
        ] {
            let endpoint = cookie_session_endpoint("https://shop.example/prefix/", path).unwrap();
            assert_eq!(endpoint.path(), format!("/prefix{path}"));
        }
        assert!(cookie_session_endpoint("https://shop.example", "/another-endpoint").is_err());
    }

    #[test]
    fn direct_cookie_session_requires_the_browser_page_origin() {
        let endpoint = reqwest::Url::parse("https://shop.example/api/auth/session/logout").unwrap();
        assert!(cookie_session_is_same_origin(
            &endpoint,
            "https://shop.example"
        ));
        assert!(!cookie_session_is_same_origin(
            &endpoint,
            "https://other.example"
        ));
        assert!(!cookie_session_is_same_origin(
            &endpoint,
            "http://shop.example"
        ));
        assert!(!cookie_session_is_same_origin(&endpoint, "null"));
        let other_port =
            reqwest::Url::parse("https://shop.example:8443/api/auth/session/logout").unwrap();
        assert!(!cookie_session_is_same_origin(
            &other_port,
            "https://shop.example"
        ));
    }

    #[tokio::test]
    async fn direct_cookie_logout_rejects_ambiguous_api_bases_before_sending() {
        for api_url in [
            "not a url",
            "ftp://shop.example",
            "https://user@shop.example",
            "https://shop.example/?query=1",
            "https://shop.example/#fragment",
        ] {
            assert_eq!(
                sign_out_cookie_at(api_url, "shop".into(), None).await,
                Err(AuthError::Network),
                "invalid API base should fail before the request"
            );
        }
    }

    #[tokio::test]
    async fn direct_cookie_logout_does_not_accept_a_redirected_ok() {
        let (target, target_request) = mock_api("200 OK", r#"{"status":"ok"}"#, "").await;
        let headers = format!("location: {target}/another-endpoint\r\n");
        let (url, request) = mock_api("302 Found", "", &headers).await;
        assert_eq!(
            sign_out_cookie_at(&url, "shop".into(), None).await,
            Err(AuthError::Network)
        );
        assert!(request.await.unwrap().starts_with("POST "));
        assert!(
            target_request
                .await
                .unwrap()
                .starts_with("GET /another-endpoint ")
        );
    }
}
