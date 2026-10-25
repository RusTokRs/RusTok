mod cookie_adapter;
mod graphql_adapter;
mod native_server_adapter;

use rustok_ui_transport::UiTransportPath;
use serde::{Deserialize, Serialize};

use crate::{AuthError, AuthSession, AuthUser};

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

const FORGOT_PASSWORD_MUTATION: &str = r#"
mutation ForgotPassword($input: ForgotPasswordInput!) {
    forgotPassword(input: $input) {
        success
        message
    }
}
"#;

const CURRENT_USER_QUERY: &str = r#"
query CurrentUser {
    me {
        id
        email
        name
        role
        status
    }
}
"#;

#[cfg(feature = "ssr")]
const RESET_REQUEST_MESSAGE: &str = "If the email exists, a password reset link has been sent";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NativeAuthPayload {
    user: AuthUser,
    session: AuthSession,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NativeCurrentUserPayload {
    user: Option<AuthUser>,
}

#[derive(Debug, Deserialize)]
struct ForgotPasswordResponse {
    #[serde(rename = "forgotPassword")]
    forgot_password: ForgotPasswordPayload,
}

#[derive(Debug, Deserialize)]
struct CurrentUserResponse {
    me: Option<AuthUserGraphql>,
}

#[derive(Debug, Deserialize)]
struct AuthUserGraphql {
    id: String,
    email: String,
    name: Option<String>,
    role: String,
    #[allow(dead_code)]
    status: String,
}

#[derive(Debug, Deserialize)]
struct ForgotPasswordPayload {
    #[allow(dead_code)]
    success: bool,
    message: String,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize)]
struct RestAuthResponse {
    access_token: String,
    /// Server-side only: moved into the host's HttpOnly BFF cookie and
    /// never included in the payload returned to the browser.
    refresh_token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: u64,
    user: RestUserInfo,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize)]
struct RestUserInfo {
    id: String,
    email: String,
    name: Option<String>,
    role: String,
    #[allow(dead_code)]
    status: String,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize)]
struct RestUserResponse {
    id: String,
    email: String,
    name: Option<String>,
    role: String,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize)]
struct RestStatusResponse {
    #[allow(dead_code)]
    status: String,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize)]
struct RestApiErrorPayload {
    error: Option<String>,
    message: Option<String>,
}

fn get_api_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.trim_end_matches("/api/graphql").to_string();
    }

    if let Some(url) = option_env!("RUSTOK_API_URL") {
        return url.trim_end_matches('/').to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string())
    }
}

fn get_graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    format!("{}/api/graphql", get_api_url())
}

fn now_unix_secs() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0) as i64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

#[cfg(feature = "ssr")]
fn rest_payload_to_native(payload: RestAuthResponse, tenant: String) -> NativeAuthPayload {
    NativeAuthPayload {
        user: AuthUser {
            id: payload.user.id,
            email: payload.user.email,
            name: payload.user.name,
            role: payload.user.role,
        },
        session: AuthSession {
            token: payload.access_token,
            expires_at: now_unix_secs() + payload.expires_in as i64,
            tenant,
        },
    }
}

fn map_graphql_auth_error(error: rustok_graphql::GraphqlHttpError, is_login: bool) -> AuthError {
    match error {
        rustok_graphql::GraphqlHttpError::Unauthorized => {
            if is_login {
                AuthError::InvalidCredentials
            } else {
                AuthError::Unauthorized
            }
        }
        rustok_graphql::GraphqlHttpError::Graphql(message) => {
            let lower = message.to_ascii_lowercase();
            if is_login && (lower.contains("invalid") || lower.contains("credential")) {
                AuthError::InvalidCredentials
            } else if lower.contains("unauthorized") || lower.contains("unauthenticated") {
                AuthError::Unauthorized
            } else {
                AuthError::Http(400)
            }
        }
        rustok_graphql::GraphqlHttpError::Http(status) => status
            .parse::<u16>()
            .map(AuthError::Http)
            .unwrap_or(AuthError::Http(500)),
        rustok_graphql::GraphqlHttpError::Network => AuthError::Network,
    }
}

fn map_server_auth_error(error: impl std::fmt::Display, is_login: bool) -> AuthError {
    let message = error.to_string();
    let clean_message = message
        .strip_prefix("error running server function: ")
        .unwrap_or(&message);
    let lower = clean_message.to_ascii_lowercase();

    if is_login && (lower.contains("invalid") || lower.contains("credential")) {
        AuthError::InvalidCredentials
    } else if lower.contains("unauthorized") || lower.contains("unauthenticated") {
        AuthError::Unauthorized
    } else if lower.contains("network") {
        AuthError::Network
    } else if let Some(status) = clean_message
        .strip_prefix("HTTP error: ")
        .or_else(|| clean_message.strip_prefix("Http error: "))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u16>().ok())
    {
        AuthError::Http(status)
    } else {
        AuthError::Http(500)
    }
}

pub async fn sign_in(
    email: String,
    password: String,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            let payload = native_server_adapter::sign_in_native(email, password, tenant)
                .await
                .map_err(|error| map_server_auth_error(error, true))?;
            Ok((payload.user, payload.session))
        }
        // Browser-direct mode talks to the HttpOnly cookie session
        // endpoints; the refresh token never reaches JavaScript.
        UiTransportPath::Graphql => cookie_adapter::sign_in_cookie(email, password, tenant).await,
    }
}

pub async fn sign_up(
    email: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            let payload = native_server_adapter::sign_up_native(email, password, name, tenant)
                .await
                .map_err(|error| map_server_auth_error(error, false))?;
            Ok((payload.user, payload.session))
        }
        UiTransportPath::Graphql => {
            cookie_adapter::sign_up_cookie(email, password, name, tenant).await
        }
    }
}

pub async fn sign_out(tenant: String) -> Result<(), AuthError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::sign_out_native(tenant)
            .await
            .map_err(|error| map_server_auth_error(error, false)),
        UiTransportPath::Graphql => cookie_adapter::sign_out_cookie(tenant).await,
    }
}

/// Rotate the cookie-held refresh token and return fresh in-memory
/// session state. Used both for proactive refresh and for restoring
/// a session after a page reload (silent refresh).
pub async fn refresh_session(
    tenant: String,
) -> Result<(AuthSession, AuthUser), AuthError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            let payload = native_server_adapter::refresh_token_native(tenant)
                .await
                .map_err(|error| map_server_auth_error(error, false))?;
            Ok((payload.session, payload.user))
        }
        UiTransportPath::Graphql => cookie_adapter::refresh_session_cookie(tenant).await,
    }
}

/// Server-side (SSR host) refresh-token exchange for building the
/// request auth snapshot. Returns the rotated refresh token so the
/// host can re-issue its BFF cookie.
#[cfg(feature = "ssr")]
pub async fn exchange_refresh_token_ssr(
    refresh_token: String,
    tenant: String,
) -> Result<(AuthUser, AuthSession, String), AuthError> {
    let (payload, rotated) =
        native_server_adapter::exchange_refresh_token_ssr(refresh_token, tenant)
            .await
            .map_err(|error| map_server_auth_error(error, false))?;
    Ok((payload.user, payload.session, rotated))
}

pub async fn forgot_password(email: String, tenant: String) -> Result<String, AuthError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::forgot_password_native(email, tenant)
                .await
                .map_err(|error| map_server_auth_error(error, false))
        }
        UiTransportPath::Graphql => graphql_adapter::forgot_password_graphql(email, tenant).await,
    }
}

pub async fn fetch_current_user(
    token: String,
    tenant: String,
) -> Result<Option<AuthUser>, AuthError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            let payload = native_server_adapter::current_user_native(token, tenant)
                .await
                .map_err(|error| map_server_auth_error(error, false))?;
            Ok(payload.user)
        }
        UiTransportPath::Graphql => {
            graphql_adapter::fetch_current_user_graphql(token, tenant).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphql_url_shape() {
        let url = get_graphql_url();
        assert!(url.contains("/api/graphql"));
    }
}
