use leptos::prelude::*;
use leptos_graphql::{execute, GraphqlRequest};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{AuthError, AuthSession, AuthUser};

const SIGN_IN_MUTATION: &str = r#"
mutation SignIn($input: SignInInput!) {
    signIn(input: $input) {
        accessToken
        refreshToken
        tokenType
        expiresIn
        user {
            id
            email
            name
            role
            status
        }
    }
}
"#;

const SIGN_UP_MUTATION: &str = r#"
mutation SignUp($input: SignUpInput!) {
    signUp(input: $input) {
        accessToken
        refreshToken
        tokenType
        expiresIn
        user {
            id
            email
            name
            role
            status
        }
    }
}
"#;

const SIGN_OUT_MUTATION: &str = r#"
mutation SignOut {
    signOut {
        success
    }
}
"#;

const REFRESH_TOKEN_MUTATION: &str = r#"
mutation RefreshToken($input: RefreshTokenInput!) {
    refreshToken(input: $input) {
        accessToken
        refreshToken
        tokenType
        expiresIn
        user {
            id
            email
            name
            role
            status
        }
    }
}
"#;

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
struct SignInResponse {
    #[serde(rename = "signIn")]
    sign_in: AuthPayload,
}

#[derive(Debug, Deserialize)]
struct SignUpResponse {
    #[serde(rename = "signUp")]
    sign_up: AuthPayload,
}

#[derive(Debug, Deserialize)]
struct SignOutResponse {
    #[serde(rename = "signOut")]
    #[allow(dead_code)]
    sign_out: SignOutPayload,
}

#[derive(Debug, Deserialize)]
struct RefreshTokenResponse {
    #[serde(rename = "refreshToken")]
    refresh_token: AuthPayload,
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
struct AuthPayload {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "refreshToken")]
    refresh_token: String,
    #[serde(rename = "expiresIn")]
    expires_in: i32,
    user: AuthUserGraphql,
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
struct SignOutPayload {
    #[allow(dead_code)]
    success: bool,
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

fn auth_payload_to_session(payload: AuthPayload, tenant: String) -> (AuthUser, AuthSession) {
    let user = AuthUser {
        id: payload.user.id,
        email: payload.user.email,
        name: payload.user.name,
        role: payload.user.role,
    };

    let session = AuthSession {
        token: payload.access_token,
        refresh_token: payload.refresh_token,
        expires_at: now_unix_secs() + payload.expires_in as i64,
        tenant,
    };

    (user, session)
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
            refresh_token: payload.refresh_token,
            expires_at: now_unix_secs() + payload.expires_in as i64,
            tenant,
        },
    }
}

fn map_graphql_auth_error(error: leptos_graphql::GraphqlHttpError, is_login: bool) -> AuthError {
    match error {
        leptos_graphql::GraphqlHttpError::Unauthorized => {
            if is_login {
                AuthError::InvalidCredentials
            } else {
                AuthError::Unauthorized
            }
        }
        leptos_graphql::GraphqlHttpError::Graphql(message) => {
            let lower = message.to_ascii_lowercase();
            if is_login && (lower.contains("invalid") || lower.contains("credential")) {
                AuthError::InvalidCredentials
            } else if lower.contains("unauthorized") || lower.contains("unauthenticated") {
                AuthError::Unauthorized
            } else {
                AuthError::Http(400)
            }
        }
        leptos_graphql::GraphqlHttpError::Http(status) => status
            .parse::<u16>()
            .map(AuthError::Http)
            .unwrap_or(AuthError::Http(500)),
        leptos_graphql::GraphqlHttpError::Network => AuthError::Network,
    }
}

async fn sign_in_graphql(
    email: String,
    password: String,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    let variables = json!({
        "input": {
            "email": email,
            "password": password,
        }
    });

    let response: SignInResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(SIGN_IN_MUTATION, Some(variables)),
        None,
        Some(tenant.clone()),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, true))?;

    Ok(auth_payload_to_session(response.sign_in, tenant))
}

async fn sign_up_graphql(
    email: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    let variables = json!({
        "input": {
            "email": email,
            "password": password,
            "name": name,
        }
    });

    let response: SignUpResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(SIGN_UP_MUTATION, Some(variables)),
        None,
        Some(tenant.clone()),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, false))?;

    Ok(auth_payload_to_session(response.sign_up, tenant))
}

async fn sign_out_graphql(token: String, tenant: String) -> Result<(), AuthError> {
    let _response: SignOutResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(SIGN_OUT_MUTATION, None::<serde_json::Value>),
        Some(token),
        Some(tenant),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, false))?;

    Ok(())
}

async fn refresh_token_graphql(
    refresh_tok: String,
    tenant: String,
) -> Result<(AuthSession, AuthUser), AuthError> {
    let variables = json!({
        "input": {
            "refreshToken": refresh_tok,
        }
    });

    let response: RefreshTokenResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(REFRESH_TOKEN_MUTATION, Some(variables)),
        None,
        Some(tenant.clone()),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, false))?;

    let (user, session) = auth_payload_to_session(response.refresh_token, tenant);
    Ok((session, user))
}

async fn forgot_password_graphql(email: String, tenant: String) -> Result<String, AuthError> {
    let variables = json!({
        "input": {
            "email": email,
        }
    });

    let response: ForgotPasswordResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(FORGOT_PASSWORD_MUTATION, Some(variables)),
        None,
        Some(tenant),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, false))?;

    Ok(response.forgot_password.message)
}

async fn fetch_current_user_graphql(
    token: String,
    tenant: String,
) -> Result<Option<AuthUser>, AuthError> {
    let response: CurrentUserResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(CURRENT_USER_QUERY, None::<serde_json::Value>),
        Some(token),
        Some(tenant),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, false))?;

    Ok(response.me.map(|user| AuthUser {
        id: user.id,
        email: user.email,
        name: user.name,
        role: user.role,
    }))
}

pub async fn sign_in(
    email: String,
    password: String,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    match sign_in_native(email.clone(), password.clone(), tenant.clone()).await {
        Ok(payload) => Ok((payload.user, payload.session)),
        Err(_) => sign_in_graphql(email, password, tenant).await,
    }
}

pub async fn sign_up(
    email: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    match sign_up_native(
        email.clone(),
        password.clone(),
        name.clone(),
        tenant.clone(),
    )
    .await
    {
        Ok(payload) => Ok((payload.user, payload.session)),
        Err(_) => sign_up_graphql(email, password, name, tenant).await,
    }
}

pub async fn sign_out(
    token: String,
    refresh_token: String,
    tenant: String,
) -> Result<(), AuthError> {
    match sign_out_native(refresh_token.clone(), tenant.clone()).await {
        Ok(()) => Ok(()),
        Err(_) => sign_out_graphql(token, tenant).await,
    }
}

pub async fn refresh_token(
    refresh_tok: String,
    tenant: String,
) -> Result<(AuthSession, AuthUser), AuthError> {
    match refresh_token_native(refresh_tok.clone(), tenant.clone()).await {
        Ok(payload) => Ok((payload.session, payload.user)),
        Err(_) => refresh_token_graphql(refresh_tok, tenant).await,
    }
}

pub async fn forgot_password(email: String, tenant: String) -> Result<String, AuthError> {
    match forgot_password_native(email.clone(), tenant.clone()).await {
        Ok(message) => Ok(message),
        Err(_) => forgot_password_graphql(email, tenant).await,
    }
}

pub async fn fetch_current_user(
    token: String,
    tenant: String,
) -> Result<Option<AuthUser>, AuthError> {
    match current_user_native(token.clone(), tenant.clone()).await {
        Ok(payload) => Ok(payload.user),
        Err(_) => fetch_current_user_graphql(token, tenant).await,
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/sign-in")]
async fn sign_in_native(
    email: String,
    password: String,
    tenant: String,
) -> Result<NativeAuthPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
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
async fn sign_up_native(
    email: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<NativeAuthPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
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
async fn sign_out_native(refresh_token: String, tenant: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let _response: RestStatusResponse = auth_rest_post(
            "/api/auth/logout",
            &json!({
                "refresh_token": refresh_token,
            }),
            None,
            Some(tenant),
        )
        .await?;

        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (refresh_token, tenant);
        Err(ServerFnError::new(
            "auth/sign-out requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/refresh-token")]
async fn refresh_token_native(
    refresh_tok: String,
    tenant: String,
) -> Result<NativeAuthPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let response: RestAuthResponse = auth_rest_post(
            "/api/auth/refresh",
            &json!({
                "refresh_token": refresh_tok,
            }),
            None,
            Some(tenant.clone()),
        )
        .await?;

        Ok(rest_payload_to_native(response, tenant))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (refresh_tok, tenant);
        Err(ServerFnError::new(
            "auth/refresh-token requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/forgot-password")]
async fn forgot_password_native(email: String, tenant: String) -> Result<String, ServerFnError> {
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
async fn current_user_native(
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

    let client = reqwest::Client::new();
    let mut request = client
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

    let client = reqwest::Client::new();
    let mut request = client.get(format!("{}{}", get_api_url(), path));

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

    if let Ok(payload) = serde_json::from_str::<RestApiErrorPayload>(trimmed) {
        if let Some(message) = payload
            .message
            .as_deref()
            .or(payload.error.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
    }

    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_in_mutation() {
        assert!(SIGN_IN_MUTATION.contains("mutation SignIn"));
        assert!(SIGN_IN_MUTATION.contains("signIn"));
        assert!(SIGN_IN_MUTATION.contains("accessToken"));
    }

    #[test]
    fn test_sign_up_mutation() {
        assert!(SIGN_UP_MUTATION.contains("mutation SignUp"));
        assert!(SIGN_UP_MUTATION.contains("signUp"));
        assert!(SIGN_UP_MUTATION.contains("user"));
    }

    #[test]
    fn test_graphql_url_shape() {
        let url = get_graphql_url();
        assert!(url.contains("/api/graphql"));
    }
}
