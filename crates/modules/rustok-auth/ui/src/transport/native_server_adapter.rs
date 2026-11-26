use leptos::prelude::*;
#[cfg(feature = "ssr")]
use serde_json::json;

#[cfg(feature = "ssr")]
use super::RESET_REQUEST_MESSAGE;
use super::{NativeAuthPayload, NativeCurrentUserPayload};
#[cfg(feature = "ssr")]
use crate::{core::RefreshCredential, model::{AuthError, AuthUser}};

#[cfg(feature = "ssr")]
use super::{
    RestApiErrorPayload, RestAuthResponse, RestStatusResponse, RestUserResponse, get_api_url,
    rest_payload_to_native,
};
#[cfg(feature = "ssr")]
use serde::Deserialize;

// ─── BFF refresh-cookie plumbing (SSR host side) ────────────────────────────
//
// The refresh token never crosses into browser-readable state: native
// server functions receive it from the API, stash it in an HttpOnly
// cookie on the SSR host's own origin, and hand the browser only the
// short-lived access token. Refresh and sign-out recover the token
// from that cookie on the server side of the server-function call.

#[cfg(feature = "ssr")]
async fn request_headers() -> Result<axum::http::HeaderMap, ServerFnError> {
    leptos_axum::extract::<axum::http::HeaderMap>()
        .await
        .map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
fn cookie_request_is_secure(headers: &axum::http::HeaderMap) -> bool {
    let forwarded_https = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("https"));
    let origin_https = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("https://"));
    forwarded_https || origin_https
}

#[cfg(feature = "ssr")]
fn set_response_cookie(cookie: String) -> Result<(), ServerFnError> {
    let options = use_context::<leptos_axum::ResponseOptions>()
        .ok_or_else(|| ServerFnError::new("response options are unavailable"))?;
    let mut value = axum::http::HeaderValue::from_str(&cookie)
        .map_err(|_| ServerFnError::new("Invalid authentication cookie"))?;
    value.set_sensitive(true);
    options.insert_header(axum::http::header::SET_COOKIE, value);
    options.insert_header(axum::http::header::CACHE_CONTROL, axum::http::HeaderValue::from_static("no-store"));
    Ok(())
}

#[cfg(feature = "ssr")]
fn persist_refresh_cookie(
    headers: &axum::http::HeaderMap,
    credential: &RefreshCredential,
) -> Result<(), ServerFnError> {
    set_response_cookie(crate::core::refresh_cookie_header(
        credential,
        cookie_request_is_secure(headers),
    ))
}

#[cfg(feature = "ssr")]
fn clear_refresh_cookie(headers: &axum::http::HeaderMap) -> Result<(), ServerFnError> {
    set_response_cookie(crate::core::clear_refresh_cookie_header(
        cookie_request_is_secure(headers),
    ))
}

#[cfg(feature = "ssr")]
fn refresh_cookie_from(
    headers: &axum::http::HeaderMap,
) -> Result<Option<RefreshCredential>, ServerFnError> {
    let mut values = Vec::new();
    for value in headers.get_all(axum::http::header::COOKIE).iter() {
        values.push(
            value
                .to_str()
                .map_err(|_| ServerFnError::new("Invalid authentication cookie"))?,
        );
    }
    crate::core::inspect_refresh_cookie_headers(values)
        .map_err(|_| ServerFnError::new("Invalid authentication cookie"))
}

/// Refresh through one owner endpoint. Only an ambiguous transport or 5xx
/// response may retry; never replay after a definitive rejection.
#[cfg(feature = "ssr")]
pub(super) async fn exchange_refresh_token_ssr(
    credential: RefreshCredential,
) -> Result<(NativeAuthPayload, RefreshCredential), AuthError> {
    exchange_refresh_token_at(&get_api_url(), credential).await
}

#[cfg(feature = "ssr")]
async fn exchange_refresh_token_at(
    api_url: &str,
    credential: RefreshCredential,
) -> Result<(NativeAuthPayload, RefreshCredential), AuthError> {
    use reqwest::header::HeaderValue;
    let mut proof = HeaderValue::from_str(&credential.retry_key()?).map_err(|_| AuthError::Network)?;
    proof.set_sensitive(true);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build().map_err(|_| AuthError::Network)?;
    for attempt in 0..2 {
        let result = client.post(format!("{}/api/auth/refresh", api_url.trim_end_matches('/')))
            .header("X-Tenant-ID", credential.tenant())
            .header("x-rustok-refresh-retry-key", proof.clone())
            .json(&json!({ "refresh_token": credential.token() }))
            .send().await;
        let response = match result {
            Ok(response) => response,
            Err(_) if attempt == 0 => continue,
            Err(_) => return Err(AuthError::Network),
        };
        let status = response.status();
        if status.is_server_error() && attempt == 0 {
            continue;
        }
        if !status.is_success() {
            return Err(AuthError::from_status(status.as_u16(), false));
        }
        let payload = match response.json::<RestAuthResponse>().await {
            Ok(payload) => payload,
            Err(_) if attempt == 0 => continue,
            Err(_) => return Err(AuthError::Network),
        };
        let rotated = match credential.rotated(&payload.refresh_token) {
            Ok(rotated) if !payload.access_token.is_empty() && payload.expires_in > 0 => rotated,
            _ if attempt == 0 => continue,
            _ => return Err(AuthError::Network),
        };
        let tenant = credential.tenant().to_owned();
        return Ok((rest_payload_to_native(payload, tenant), rotated));
    }
    Err(AuthError::Network)
}

#[server(prefix = "/api/fn", endpoint = "auth/sign-in")]
pub(super) async fn sign_in_native(
    email: String,
    password: String,
    tenant: String,
) -> Result<NativeAuthPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let headers = request_headers().await?;
        let seed = RefreshCredential::generate_seed().map_err(|error| ServerFnError::new(error.to_string()))?;
        let response: RestAuthResponse = auth_rest_post(
            "/api/auth/login",
            &json!({
                "email": email,
                "password": password,
            }),
            None,
            Some(tenant.clone()),
        )
        .await?;

        let credential = RefreshCredential::new(&tenant, &response.refresh_token, seed)
            .map_err(|_| ServerFnError::new("Invalid authentication response"))?;
        persist_refresh_cookie(&headers, &credential)?;
        Ok(rest_payload_to_native(response, tenant))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (email, password, tenant);
        Err(ServerFnError::new(
            "auth/sign-in requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/sign-up")]
pub(super) async fn sign_up_native(
    email: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<NativeAuthPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let headers = request_headers().await?;
        let seed = RefreshCredential::generate_seed().map_err(|error| ServerFnError::new(error.to_string()))?;
        let response: RestAuthResponse = auth_rest_post(
            "/api/auth/register",
            &json!({
                "email": email,
                "password": password,
                "name": name,
            }),
            None,
            Some(tenant.clone()),
        )
        .await?;

        let credential = RefreshCredential::new(&tenant, &response.refresh_token, seed)
            .map_err(|_| ServerFnError::new("Invalid authentication response"))?;
        persist_refresh_cookie(&headers, &credential)?;
        Ok(rest_payload_to_native(response, tenant))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (email, password, name, tenant);
        Err(ServerFnError::new(
            "auth/sign-up requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/sign-out")]
pub(super) async fn sign_out_native(tenant: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let headers = request_headers().await?;
        // Do not report logout or clear a different tenant's cookie. If revocation fails,
        // preserve the cookie for recovery and leave the caller's scope unverified.
        if let Some(credential) = refresh_cookie_from(&headers)? {
            if credential.tenant() != tenant {
                return Err(ServerFnError::new("Authentication tenant mismatch"));
            }
            // Use the API's idempotent cookie-session logout contract: an already expired
            // or revoked refresh token is a completed logout, while persistence failures
            // remain errors. This Cookie header is server-to-server only, never rendered.
            auth_rest_cookie_logout(&tenant, &credential.api_cookie_value()).await?;
        }
        clear_refresh_cookie(&headers)?;
        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = tenant;
        Err(ServerFnError::new(
            "auth/sign-out requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/refresh-token")]
pub(super) async fn refresh_token_native(
    tenant: String,
) -> Result<Option<NativeAuthPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let headers = request_headers().await?;
        let Some(credential) = refresh_cookie_from(&headers)? else {
            // Only an explicitly absent cookie is evidence of a guest. Invalid, duplicate,
            // foreign, expired or revoked cookies all follow an error path instead.
            return Ok(None);
        };
        if credential.tenant() != tenant {
            return Err(ServerFnError::new("Authentication tenant mismatch"));
        }

        let (payload, rotated) = exchange_refresh_token_ssr(credential)
            .await.map_err(|error| ServerFnError::new(error.to_string()))?;
        persist_refresh_cookie(&headers, &rotated)?;
        Ok(Some(payload))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = tenant;
        Err(ServerFnError::new(
            "auth/refresh-token requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize)]
struct RestAcceptInviteResponse {
    #[allow(dead_code)]
    status: String,
    email: String,
    #[allow(dead_code)]
    role: String,
}

#[server(prefix = "/api/fn", endpoint = "auth/accept-invite")]
pub(super) async fn accept_invite_native(
    invite_token: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let response: RestAcceptInviteResponse = auth_rest_post(
            "/api/auth/invite/accept",
            &json!({
                "token": invite_token,
                "password": password,
                "name": name,
            }),
            None,
            Some(tenant),
        )
        .await?;
        Ok(response.email)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (invite_token, password, name, tenant);
        Err(ServerFnError::new(
            "auth/accept-invite requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/forgot-password")]
pub(super) async fn forgot_password_native(
    email: String,
    tenant: String,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let _response: RestStatusResponse = auth_rest_post(
            "/api/auth/reset/request",
            &json!({
                "email": email,
            }),
            None,
            Some(tenant),
        )
        .await?;

        Ok(RESET_REQUEST_MESSAGE.to_string())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (email, tenant);
        Err(ServerFnError::new(
            "auth/forgot-password requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/current-user")]
pub(super) async fn current_user_native(
    token: String,
    tenant: String,
) -> Result<NativeCurrentUserPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let response: RestUserResponse =
            auth_rest_get("/api/auth/me", Some(token), Some(tenant)).await?;

        Ok(NativeCurrentUserPayload {
            user: Some(AuthUser {
                id: response.id,
                email: response.email,
                name: response.name,
                role: response.role,
                permissions: response.permissions,
            }),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant);
        Err(ServerFnError::new(
            "auth/current-user requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn api_session_cookie(envelope: &str) -> Result<reqwest::header::HeaderValue, ServerFnError> {
    // Reuse the strict envelope parser before formatting the API cookie.
    RefreshCredential::parse(&format!("logout.{envelope}"))
        .map_err(|_| ServerFnError::new("Invalid authentication cookie"))?;
    let mut cookie =
        reqwest::header::HeaderValue::from_str(&format!("rustok-session={envelope}"))
            .map_err(|_| ServerFnError::new("Invalid authentication cookie"))?;
    cookie.set_sensitive(true);
    Ok(cookie)
}

#[cfg(feature = "ssr")]
fn confirm_cookie_logout(response: RestStatusResponse) -> Result<(), ServerFnError> {
    if response.status == "ok" {
        Ok(())
    } else {
        Err(ServerFnError::new("Logout could not be confirmed"))
    }
}

#[cfg(feature = "ssr")]
async fn auth_rest_cookie_logout(tenant: &str, refresh_token: &str) -> Result<(), ServerFnError> {
    auth_rest_cookie_logout_at(&get_api_url(), tenant, refresh_token).await
}

#[cfg(feature = "ssr")]
async fn auth_rest_cookie_logout_at(
    api_url: &str,
    tenant: &str,
    refresh_token: &str,
) -> Result<(), ServerFnError> {
    let cookie = api_session_cookie(refresh_token)?;
    // A redirect must never forward a bearer-like cookie to a different endpoint.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| ServerFnError::new("Logout request failed"))?;
    let response = client
        .post(format!(
            "{}/api/auth/session/logout",
            api_url.trim_end_matches('/')
        ))
        .header(reqwest::header::COOKIE, cookie)
        .header("X-Tenant-ID", tenant)
        .header("x-rustok-session-request", "1")
        .send()
        .await
        .map_err(|_| ServerFnError::new("Logout request failed"))?;
    if !response.status().is_success() {
        // Do not copy a potentially credential-bearing upstream body into diagnostics.
        return Err(ServerFnError::new(format!(
            "Logout request failed with status {}",
            response.status().as_u16()
        )));
    }
    let response: RestStatusResponse = response
        .json()
        .await
        .map_err(|_| ServerFnError::new("Logout could not be confirmed"))?;
    confirm_cookie_logout(response)
}

#[cfg(feature = "ssr")]
static HTTP_CLIENT: std::sync::LazyLock<Result<reqwest::Client, reqwest::Error>> =
    std::sync::LazyLock::new(|| reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build());

#[cfg(feature = "ssr")]
fn http_client() -> Result<&'static reqwest::Client, ServerFnError> {
    HTTP_CLIENT.as_ref().map_err(|_| ServerFnError::new("Network error"))
}

#[cfg(feature = "ssr")]
async fn auth_rest_post<T>(
    path: &str,
    body: &serde_json::Value,
    token: Option<String>,
    tenant: Option<String>,
) -> Result<T, ServerFnError>
where
    T: for<'de> Deserialize<'de>,
{
    use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

    let mut request = http_client()?
        .post(format!("{}{}", get_api_url(), path))
        .header(CONTENT_TYPE, "application/json")
        .json(body);

    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant) = tenant {
        request = request.header("X-Tenant-ID", tenant);
    }

    let response = request.send().await.map_err(ServerFnError::new)?;
    if !response.status().is_success() {
        return Err(ServerFnError::new(rest_error_message(response).await));
    }

    response.json::<T>().await.map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
async fn auth_rest_get<T>(
    path: &str,
    token: Option<String>,
    tenant: Option<String>,
) -> Result<T, ServerFnError>
where
    T: for<'de> Deserialize<'de>,
{
    use reqwest::header::AUTHORIZATION;

    let mut request = http_client()?.get(format!("{}{}", get_api_url(), path));

    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant) = tenant {
        request = request.header("X-Tenant-ID", tenant);
    }

    let response = request.send().await.map_err(ServerFnError::new)?;
    if !response.status().is_success() {
        return Err(ServerFnError::new(rest_error_message(response).await));
    }

    response.json::<T>().await.map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
async fn rest_error_message(response: reqwest::Response) -> String {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return format!("request failed with status {status}");
    }

    if let Ok(payload) = serde_json::from_str::<RestApiErrorPayload>(trimmed)
        && let Some(message) = payload
            .message
            .as_deref()
            .or(payload.error.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    {
        return message.to_string();
    }

    trimmed.to_string()
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::{
        RestStatusResponse, api_session_cookie, auth_rest_cookie_logout_at, confirm_cookie_logout,
        refresh_cookie_from, exchange_refresh_token_at, RefreshCredential, AuthError,
    };
    use axum::http::{HeaderMap, HeaderValue, header::COOKIE};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn mock_logout_api(response: String) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let received = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = socket.read(&mut chunk).await.unwrap();
                assert!(read > 0 && request.len() + read <= 16_384);
                request.extend_from_slice(&chunk[..read]);
            }
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8(request).unwrap()
        });
        (format!("http://{address}"), received)
    }

    fn http_reply(status: &str, body: &str, extra_headers: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n{extra_headers}\r\n{body}",
            body.len()
        )
    }

    // Read the full request, including JSON, before responding; retry must send
    // exactly the original bearer and proof even when the response is lost.
    async fn mock_refresh_api(replies: Vec<String>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let received = tokio::spawn(async move {
            let mut requests = Vec::new();
            for reply in replies {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                let (body_start, body_len) = loop {
                    let read = socket.read(&mut buffer).await.unwrap();
                    assert!(read > 0 && request.len() + read < 16_384);
                    request.extend_from_slice(&buffer[..read]);
                    if let Some(index) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = std::str::from_utf8(&request[..index]).unwrap();
                        let len = headers.lines().find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        }).unwrap();
                        break (index + 4, len);
                    }
                };
                while request.len() < body_start + body_len {
                    let read = socket.read(&mut buffer).await.unwrap();
                    assert!(read > 0 && request.len() + read < 16_384);
                    request.extend_from_slice(&buffer[..read]);
                }
                requests.push(String::from_utf8(request).unwrap());
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
            requests
        });
        (format!("http://{address}"), received)
    }

    #[tokio::test]
    async fn native_refresh_repeats_identical_proof_after_ambiguous_result() {
        let bearer = "a1".repeat(32);
        let seed = "b2".repeat(32);
        let successor = "c3".repeat(32);
        let credential = RefreshCredential::parse(&format!("shop.{bearer}.{seed}")).unwrap();
        let proof = credential.retry_key().unwrap();
        let body = serde_json::json!({
            "access_token": "access", "refresh_token": successor,
            "token_type": "Bearer", "expires_in": 900,
            "user": {"id":"user", "email":"user@example.test", "name":null, "role":"member", "status":"active"}
        }).to_string();
        let (url, requests) = mock_refresh_api(vec![
            http_reply("503 Service Unavailable", "unknown", ""),
            http_reply("200 OK", &body, "content-type: application/json\r\n"),
        ]).await;
        let (payload, rotated) = exchange_refresh_token_at(&url, credential).await.unwrap();
        assert_eq!(payload.session.token, "access");
        assert_eq!(rotated.token(), successor);
        assert_ne!(rotated.retry_key().unwrap(), proof);
        let requests = requests.await.unwrap();
        assert_eq!(requests.len(), 2);
        for request in requests {
            assert!(request.to_ascii_lowercase().contains(&format!("\r\nx-rustok-refresh-retry-key: {proof}\r\n")));
            assert!(request.contains(&bearer));
            assert!(!request.contains(&seed));
        }
    }

    #[tokio::test]
    async fn native_refresh_preserves_conflict_without_retry() {
        let credential = RefreshCredential::parse(&format!("shop.{}.{}", "a1".repeat(32), "b2".repeat(32))).unwrap();
        let (url, requests) = mock_refresh_api(vec![http_reply("409 Conflict", "conflict", "")]).await;
        assert_eq!(exchange_refresh_token_at(&url, credential).await.unwrap_err(), AuthError::Http(409));
        assert_eq!(requests.await.unwrap().len(), 1);
    }

    fn logout_envelope() -> String {
        format!("{}.{}", "a1".repeat(32), "b2".repeat(32))
    }

    #[tokio::test]
    async fn native_logout_uses_cookie_session_contract_and_requires_confirmation() {
        let (url, received) = mock_logout_api(http_reply("200 OK", r#"{"status":"ok"}"#, "")).await;
        assert!(
            auth_rest_cookie_logout_at(&url, "shop", &logout_envelope())
                .await
                .is_ok()
        );
        let request = received.await.unwrap().to_ascii_lowercase();
        assert!(request.starts_with("post /api/auth/session/logout http/1.1\r\n"));
        assert!(request.contains(&format!("\r\ncookie: rustok-session={}\r\n", logout_envelope())));
        assert!(request.contains("\r\nx-tenant-id: shop\r\n"));
        assert!(request.contains("\r\nx-rustok-session-request: 1\r\n"));

        let (url, received) =
            mock_logout_api(http_reply("503 Service Unavailable", "fake-token", "")).await;
        let error = auth_rest_cookie_logout_at(&url, "shop", &logout_envelope())
            .await
            .unwrap_err();
        assert!(!error.to_string().contains("fake-token"));
        received.await.unwrap();

        let (url, received) =
            mock_logout_api(http_reply("200 OK", r#"{"status":"unknown"}"#, "")).await;
        assert!(
            auth_rest_cookie_logout_at(&url, "shop", &logout_envelope())
                .await
                .is_err()
        );
        received.await.unwrap();
    }

    #[tokio::test]
    async fn native_logout_never_follows_a_redirect_with_the_cookie() {
        let (url, received) = mock_logout_api(http_reply(
            "307 Temporary Redirect",
            "",
            "location: http://127.0.0.1:1/not-the-auth-service\r\n",
        ))
        .await;
        let error = auth_rest_cookie_logout_at(&url, "shop", &logout_envelope())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("status 307"));
        received.await.unwrap();
    }

    #[test]
    fn logout_forwarding_is_sensitive_and_never_accepts_cookie_injection() {
        let cookie = api_session_cookie(&logout_envelope()).unwrap();
        assert!(cookie.is_sensitive());
        assert_eq!(cookie.to_str().unwrap(), format!("rustok-session={}", logout_envelope()));
        for invalid in ["", "has space", "x;other=y", "x,y", "x\"y", "x\\y", "x\ny"] {
            assert!(api_session_cookie(invalid).is_err());
        }
        assert!(api_session_cookie(&"a".repeat(4097)).is_err());
    }

    #[test]
    fn logout_requires_the_expected_api_confirmation() {
        assert!(
            confirm_cookie_logout(RestStatusResponse {
                status: "ok".into()
            })
            .is_ok()
        );
        assert!(
            confirm_cookie_logout(RestStatusResponse {
                status: "error".into()
            })
            .is_err()
        );
    }

    #[test]
    fn native_refresh_distinguishes_absence_from_invalid_and_duplicate_cookies() {
        let mut headers = HeaderMap::new();
        assert_eq!(refresh_cookie_from(&headers).unwrap(), None);
        headers.append(COOKIE, HeaderValue::from_static("other=value"));
        assert_eq!(refresh_cookie_from(&headers).unwrap(), None);
        let wire = format!("rustok-admin-refresh-v1=shop.{}.{}", "a1".repeat(32), "b2".repeat(32));
        headers.append(COOKIE, HeaderValue::from_str(&wire).unwrap());
        assert_eq!(refresh_cookie_from(&headers).unwrap().unwrap().tenant(), "shop");
        headers.append(COOKIE, HeaderValue::from_str(&wire).unwrap());
        assert!(refresh_cookie_from(&headers).is_err());

        let mut malformed = HeaderMap::new();
        malformed.insert(
            COOKIE,
            HeaderValue::from_static("rustok-admin-refresh-v1=not-a-token"),
        );
        assert!(refresh_cookie_from(&malformed).is_err());
        malformed.insert(COOKIE, HeaderValue::from_bytes(&[0xff]).unwrap());
        assert!(refresh_cookie_from(&malformed).is_err());
    }
}
