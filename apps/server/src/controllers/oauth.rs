//! OAuth2 REST endpoints (RFC 6749)
//!
//! `POST /oauth/token` — Token endpoint (client_credentials flow)

use axum::{
    extract::{ConnectInfo, Form, Query, State},
    http::{
        header::{AUTHORIZATION, COOKIE, LOCATION, SET_COOKIE},
        HeaderMap, StatusCode,
    },
    response::{Html, IntoResponse},
    routing::{get, post},
    Json,
};
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

use crate::auth::auth_config_from_ctx;
use crate::common::{extract_effective_proto, RustokSettings};
use crate::context::TenantContext;
use crate::extractors::auth::{resolve_current_user_from_access_token, CurrentUser};
use crate::services::oauth_app::OAuthAppService;

const OAUTH_BROWSER_SESSION_COOKIE: &str = "rustok_oauth_browser_session";
const OAUTH_BROWSER_SESSION_TTL_SECS: u64 = 10 * 60;

/// OAuth2 Token Request (application/json or application/x-www-form-urlencoded)
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,

    // For client_credentials
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub scope: Option<String>,

    // For authorization_code
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>,

    // For refresh_token
    pub refresh_token: Option<String>,
}

/// OAuth2 Authorization Request
#[derive(Debug, Deserialize)]
pub struct AuthorizeRequest {
    pub response_type: String, // Must be "code"
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: Option<String>, // Should be "S256"
}

/// Browser OAuth2 Authorization Request.
#[derive(Debug, Deserialize, Clone)]
pub struct BrowserAuthorizeRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: Option<String>,
}

/// Server-hosted consent form submission.
#[derive(Debug, Deserialize)]
pub struct ConsentRequest {
    pub action: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BrowserSessionResponse {
    pub status: &'static str,
}

/// OAuth2 Token Response (RFC 6749 §5.1)
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub scope: String,
}

/// OAuth2 Error Response (RFC 6749 §5.2)
#[derive(Debug, Serialize)]
pub struct TokenErrorResponse {
    pub error: String,
    pub error_description: String,
}

impl axum::response::IntoResponse for TokenErrorResponse {
    fn into_response(self) -> axum::response::Response {
        let status = match self.error.as_str() {
            "invalid_client" => StatusCode::UNAUTHORIZED,
            "invalid_grant" | "unsupported_grant_type" => StatusCode::BAD_REQUEST,
            "invalid_scope" => StatusCode::BAD_REQUEST,
            _ => StatusCode::BAD_REQUEST,
        };
        (status, Json(self)).into_response()
    }
}

#[derive(Debug, Clone)]
struct ValidatedAuthorizeRequest {
    app: crate::models::oauth_apps::Model,
    redirect_uri: String,
    requested_scopes: Vec<String>,
    state: Option<String>,
    code_challenge: String,
}

async fn token_handler(
    State(ctx): State<AppContext>,
    tenant_ctx: TenantContext,
    Json(req): Json<TokenRequest>,
) -> axum::response::Response {
    match req.grant_type.as_str() {
        "client_credentials" => {
            handle_client_credentials(&ctx, &tenant_ctx, &req)
                .await
                .into_response()
        }
        "authorization_code" => {
            handle_authorization_code(&ctx, &tenant_ctx, &req)
                .await
                .into_response()
        }
        "refresh_token" => {
            handle_refresh_token(&ctx, &tenant_ctx, &req)
                .await
                .into_response()
        }
        _ => TokenErrorResponse {
            error: "unsupported_grant_type".to_string(),
            error_description: format!(
                "Grant type '{}' is not supported. Supported: client_credentials, authorization_code, refresh_token",
                req.grant_type
            ),
        }
        .into_response(),
    }
}

async fn handle_client_credentials(
    ctx: &AppContext,
    tenant_ctx: &TenantContext,
    req: &TokenRequest,
) -> Result<Json<TokenResponse>, TokenErrorResponse> {
    // 1. Parse client_id
    let client_id_str = req.client_id.as_deref().ok_or_else(|| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "client_id is required".to_string(),
    })?;

    let client_id = Uuid::parse_str(client_id_str).map_err(|_| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "Invalid client_id format".to_string(),
    })?;

    // 2. Find the app
    let app = OAuthAppService::find_by_client_id(&ctx.db, client_id)
        .await
        .map_err(|_| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Internal error".to_string(),
        })?
        .ok_or_else(|| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Unknown client_id".to_string(),
        })?;

    // 3. Verify tenant
    if app.tenant_id != tenant_ctx.id {
        return Err(TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Client not registered for this tenant".to_string(),
        });
    }

    // 4. Verify the app supports client_credentials
    if !app.supports_grant_type("client_credentials") {
        return Err(TokenErrorResponse {
            error: "invalid_grant".to_string(),
            error_description: "This app does not support client_credentials grant".to_string(),
        });
    }

    // 5. Verify client_secret
    let client_secret = req
        .client_secret
        .as_deref()
        .ok_or_else(|| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "client_secret is required for client_credentials".to_string(),
        })?;

    let secret_hash = app
        .client_secret_hash
        .as_deref()
        .ok_or_else(|| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Client has no secret configured".to_string(),
        })?;

    let valid =
        OAuthAppService::verify_client_secret(client_secret, secret_hash).map_err(|_| {
            TokenErrorResponse {
                error: "invalid_client".to_string(),
                error_description: "Invalid client credentials".to_string(),
            }
        })?;

    if !valid {
        return Err(TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Invalid client credentials".to_string(),
        });
    }

    // 6. Parse requested scopes
    let requested_scopes: Vec<String> = req
        .scope
        .as_deref()
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default();

    // 7. Issue token
    let auth_config = auth_config_from_ctx(ctx).map_err(|_| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "Server configuration error".to_string(),
    })?;

    let (access_token, expires_in) =
        OAuthAppService::issue_client_credentials_token(&app, &auth_config, &requested_scopes)
            .map_err(|_| TokenErrorResponse {
                error: "invalid_client".to_string(),
                error_description: "Failed to issue token".to_string(),
            })?;

    // 8. Touch last_used_at (fire and forget)
    let db = ctx.db.clone();
    let app_id = app.id;
    tokio::spawn(async move {
        let _ = OAuthAppService::touch_last_used(&db, app_id).await;
    });

    // 9. Determine granted scopes
    let granted_scopes = if requested_scopes.is_empty() {
        app.scopes_list().join(" ")
    } else {
        requested_scopes.join(" ")
    };

    Ok(Json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in,
        refresh_token: None, // No refresh token for client_credentials
        scope: granted_scopes,
    }))
}

async fn handle_authorization_code(
    ctx: &AppContext,
    tenant_ctx: &TenantContext,
    req: &TokenRequest,
) -> Result<Json<TokenResponse>, TokenErrorResponse> {
    // 1. Verify required fields
    let client_id_str = req.client_id.as_deref().ok_or_else(|| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "client_id is required".to_string(),
    })?;
    let client_id = Uuid::parse_str(client_id_str).map_err(|_| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "Invalid client_id format".to_string(),
    })?;
    let code_verifier = req
        .code_verifier
        .as_deref()
        .ok_or_else(|| TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "code_verifier is required for PKCE".to_string(),
        })?;
    let redirect_uri = req
        .redirect_uri
        .as_deref()
        .ok_or_else(|| TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "redirect_uri is required".to_string(),
        })?;
    let code = req.code.as_deref().ok_or_else(|| TokenErrorResponse {
        error: "invalid_request".to_string(),
        error_description: "code is required".to_string(),
    })?;

    // 2. Find and check app
    let app = OAuthAppService::find_by_client_id(&ctx.db, client_id)
        .await
        .map_err(|_| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Internal error".to_string(),
        })?
        .ok_or_else(|| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Unknown client_id".to_string(),
        })?;

    if app.tenant_id != tenant_ctx.id {
        return Err(TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Client not registered for this tenant".to_string(),
        });
    }

    if !app.supports_grant_type("authorization_code") {
        return Err(TokenErrorResponse {
            error: "invalid_grant".to_string(),
            error_description: "This app does not support authorization_code grant".to_string(),
        });
    }

    // 3. Exchange code for tokens
    let auth_config = auth_config_from_ctx(ctx).map_err(|_| TokenErrorResponse {
        error: "server_error".to_string(),
        error_description: "Server configuration error".to_string(),
    })?;

    let (access_token, refresh_token_plain, expires_in) =
        OAuthAppService::exchange_authorization_code(
            &ctx.db,
            &app,
            &auth_config,
            code,
            redirect_uri,
            code_verifier,
        )
        .await
        .map_err(|e| TokenErrorResponse {
            error: "invalid_grant".to_string(),
            error_description: e.to_string(),
        })?;

    // Touch app last_used_at in background
    let db = ctx.db.clone();
    let app_id = app.id;
    tokio::spawn(async move {
        let _ = OAuthAppService::touch_last_used(&db, app_id).await;
    });

    // TODO: Determine exact granted scopes from the code execution
    // For now we assume the app's base scopes or explicitly requested ones
    Ok(Json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in,
        refresh_token: Some(refresh_token_plain),
        scope: "".to_string(), // Scope usually not echoed back unless different from requested
    }))
}

async fn handle_refresh_token(
    ctx: &AppContext,
    tenant_ctx: &TenantContext,
    req: &TokenRequest,
) -> Result<Json<TokenResponse>, TokenErrorResponse> {
    // 1. Verify fields
    let refresh_token = req
        .refresh_token
        .as_deref()
        .ok_or_else(|| TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "refresh_token is required".to_string(),
        })?;

    let client_id_str = req.client_id.as_deref().ok_or_else(|| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "client_id is required".to_string(),
    })?;

    let client_id = Uuid::parse_str(client_id_str).map_err(|_| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "Invalid client_id format".to_string(),
    })?;

    // 2. Find app
    let app = OAuthAppService::find_by_client_id(&ctx.db, client_id)
        .await
        .map_err(|_| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Internal error".to_string(),
        })?
        .ok_or_else(|| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Unknown client_id".to_string(),
        })?;

    if app.tenant_id != tenant_ctx.id {
        return Err(TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Client not registered for this tenant".to_string(),
        });
    }

    // 3. Process refresh logic
    let auth_config = auth_config_from_ctx(ctx).map_err(|_| TokenErrorResponse {
        error: "server_error".to_string(),
        error_description: "Server configuration error".to_string(),
    })?;

    let (access_token, refresh_token_plain, expires_in) =
        OAuthAppService::refresh_access_token(&ctx.db, &app, &auth_config, refresh_token)
            .await
            .map_err(|e| TokenErrorResponse {
                error: "invalid_grant".to_string(),
                error_description: e.to_string(),
            })?;

    // Touch app last_used_at in background
    let db = ctx.db.clone();
    let app_id = app.id;
    tokio::spawn(async move {
        let _ = OAuthAppService::touch_last_used(&db, app_id).await;
    });

    Ok(Json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in,
        refresh_token: Some(refresh_token_plain),
        scope: "".to_string(), // Scope usually not echoed back for refresh
    }))
}

async fn authorize_handler(
    State(ctx): State<AppContext>,
    tenant_ctx: TenantContext,
    current_user: CurrentUser,
    Json(req): Json<AuthorizeRequest>,
) -> Result<Json<serde_json::Value>, TokenErrorResponse> {
    let validated = validate_authorize_request(
        &ctx,
        tenant_ctx.id,
        req.client_id,
        req.redirect_uri,
        req.scope,
        req.state,
        req.response_type,
        req.code_challenge,
        req.code_challenge_method,
    )
    .await?;

    if validated.app.requires_user_consent() {
        let has_consent = OAuthAppService::get_active_consent(
            &ctx.db,
            validated.app.id,
            current_user.user.id,
            &validated.requested_scopes,
        )
        .await
        .map_err(|_| TokenErrorResponse {
            error: "server_error".to_string(),
            error_description: "Failed to verify consent".to_string(),
        })?;

        if !has_consent {
            return Err(TokenErrorResponse {
                error: "interaction_required".to_string(),
                error_description:
                    "User consent is required. Please prompt the user to grant access.".to_string(),
            });
        }
    }

    let code =
        issue_authorization_code(&ctx, tenant_ctx.id, &validated, current_user.user.id).await?;

    let mut response = serde_json::json!({
        "code": code,
        "redirect_uri": validated.redirect_uri,
    });

    if let Some(state) = validated.state {
        response["state"] = serde_json::json!(state);
    }

    Ok(Json(response))
}

async fn authorize_browser_handler(
    State(ctx): State<AppContext>,
    tenant_ctx: TenantContext,
    headers: HeaderMap,
    Query(req): Query<BrowserAuthorizeRequest>,
) -> axum::response::Response {
    let validated = match validate_authorize_request(
        &ctx,
        tenant_ctx.id,
        req.client_id.clone(),
        req.redirect_uri.clone(),
        req.scope.clone(),
        req.state.clone(),
        req.response_type.clone(),
        req.code_challenge.clone(),
        req.code_challenge_method.clone(),
    )
    .await
    {
        Ok(validated) => validated,
        Err(error) => return error.into_response(),
    };

    let access_token = extract_browser_access_token(&headers);
    let Some(access_token) = access_token else {
        return render_authorization_required(&validated.app.name).into_response();
    };

    let current_user =
        match resolve_current_user_from_access_token(&ctx, tenant_ctx.id, &access_token).await {
            Ok(current_user) => current_user,
            Err((status, message)) => {
                return (
                    status,
                    Html(render_error_page(
                        "Authorization required",
                        &format!("Sign in again to continue: {message}"),
                    )),
                )
                    .into_response();
            }
        };

    if !validated.app.requires_user_consent() {
        return match issue_authorization_code(&ctx, tenant_ctx.id, &validated, current_user.user.id)
            .await
        {
            Ok(code) => {
                redirect_with_code(&validated.redirect_uri, &code, validated.state.as_deref())
                    .into_response()
            }
            Err(error) => error.into_response(),
        };
    }

    let has_consent = match OAuthAppService::get_active_consent(
        &ctx.db,
        validated.app.id,
        current_user.user.id,
        &validated.requested_scopes,
    )
    .await
    {
        Ok(has_consent) => has_consent,
        Err(_) => {
            return TokenErrorResponse {
                error: "server_error".to_string(),
                error_description: "Failed to verify consent".to_string(),
            }
            .into_response();
        }
    };

    if has_consent {
        return match issue_authorization_code(&ctx, tenant_ctx.id, &validated, current_user.user.id)
            .await
        {
            Ok(code) => {
                redirect_with_code(&validated.redirect_uri, &code, validated.state.as_deref())
                    .into_response()
            }
            Err(error) => error.into_response(),
        };
    }

    Html(render_consent_page(
        &validated,
        &req,
        &current_user.user.email,
    ))
    .into_response()
}

async fn consent_handler(
    State(ctx): State<AppContext>,
    tenant_ctx: TenantContext,
    headers: HeaderMap,
    Form(req): Form<ConsentRequest>,
) -> axum::response::Response {
    let validated = match validate_authorize_request(
        &ctx,
        tenant_ctx.id,
        req.client_id.clone(),
        req.redirect_uri.clone(),
        req.scope.clone(),
        req.state.clone(),
        "code".to_string(),
        req.code_challenge.clone(),
        req.code_challenge_method.clone(),
    )
    .await
    {
        Ok(validated) => validated,
        Err(error) => return error.into_response(),
    };

    let access_token = extract_browser_access_token(&headers);
    let Some(access_token) = access_token else {
        return render_authorization_required(&validated.app.name).into_response();
    };

    let current_user =
        match resolve_current_user_from_access_token(&ctx, tenant_ctx.id, &access_token).await {
            Ok(current_user) => current_user,
            Err((status, message)) => {
                return (
                    status,
                    Html(render_error_page(
                        "Authorization required",
                        &format!("Sign in again to continue: {message}"),
                    )),
                )
                    .into_response();
            }
        };

    if req.action == "deny" {
        return redirect_with_error(
            &validated.redirect_uri,
            "access_denied",
            "The user denied the authorization request",
            validated.state.as_deref(),
        )
        .into_response();
    }

    if req.action != "approve" {
        return TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "Unknown consent action".to_string(),
        }
        .into_response();
    }

    if let Err(error) = OAuthAppService::grant_consent(
        &ctx.db,
        validated.app.id,
        current_user.user.id,
        tenant_ctx.id,
        validated.requested_scopes.clone(),
    )
    .await
    {
        return TokenErrorResponse {
            error: "server_error".to_string(),
            error_description: format!("Failed to grant consent: {error}"),
        }
        .into_response();
    }

    match issue_authorization_code(&ctx, tenant_ctx.id, &validated, current_user.user.id).await {
        Ok(code) => redirect_with_code(&validated.redirect_uri, &code, validated.state.as_deref())
            .into_response(),
        Err(error) => error.into_response(),
    }
}

async fn create_browser_session_handler(
    State(ctx): State<AppContext>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    _current_user: CurrentUser,
) -> axum::response::Response {
    let Some(access_token) = extract_bearer_token(&headers) else {
        return TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "Missing bearer token for OAuth browser session".to_string(),
        }
        .into_response();
    };

    let settings = match RustokSettings::from_settings(&ctx.config.settings) {
        Ok(settings) => settings,
        Err(_) => {
            return TokenErrorResponse {
                error: "server_error".to_string(),
                error_description: "Failed to load rustok settings".to_string(),
            }
            .into_response();
        }
    };

    (
        StatusCode::NO_CONTENT,
        [(
            SET_COOKIE,
            build_oauth_browser_session_cookie(&access_token, &headers, addr.ip(), &settings),
        )],
    )
        .into_response()
}

async fn clear_browser_session_handler(
    State(ctx): State<AppContext>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> axum::response::Response {
    let settings = match RustokSettings::from_settings(&ctx.config.settings) {
        Ok(settings) => settings,
        Err(_) => {
            return TokenErrorResponse {
                error: "server_error".to_string(),
                error_description: "Failed to load rustok settings".to_string(),
            }
            .into_response();
        }
    };
    (
        StatusCode::NO_CONTENT,
        [(
            SET_COOKIE,
            clear_oauth_browser_session_cookie(&headers, addr.ip(), &settings),
        )],
    )
        .into_response()
}

#[allow(clippy::too_many_arguments)]
async fn validate_authorize_request(
    ctx: &AppContext,
    tenant_id: Uuid,
    client_id: String,
    redirect_uri: String,
    scope: Option<String>,
    state: Option<String>,
    response_type: String,
    code_challenge: String,
    code_challenge_method: Option<String>,
) -> Result<ValidatedAuthorizeRequest, TokenErrorResponse> {
    if response_type != "code" {
        return Err(TokenErrorResponse {
            error: "unsupported_response_type".to_string(),
            error_description: "Only response_type=code is supported".to_string(),
        });
    }

    if code_challenge_method.as_deref() != Some("S256") {
        return Err(TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "code_challenge_method must be S256".to_string(),
        });
    }

    let client_id = Uuid::parse_str(&client_id).map_err(|_| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "Invalid client_id format".to_string(),
    })?;

    let app = OAuthAppService::find_by_client_id(&ctx.db, client_id)
        .await
        .map_err(|_| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Internal error".to_string(),
        })?
        .ok_or_else(|| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Unknown client_id".to_string(),
        })?;

    if app.tenant_id != tenant_id {
        return Err(TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Client not registered for this tenant".to_string(),
        });
    }

    if !app.supports_grant_type("authorization_code") {
        return Err(TokenErrorResponse {
            error: "invalid_grant".to_string(),
            error_description: "This app does not support authorization_code grant".to_string(),
        });
    }

    if !app.redirect_uris_list().contains(&redirect_uri) {
        return Err(TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "redirect_uri is not configured for this client".to_string(),
        });
    }

    let allowed_scopes = app.scopes_list();
    let requested_scopes: Vec<String> = scope
        .as_deref()
        .map(|value| value.split_whitespace().map(String::from).collect())
        .unwrap_or_else(|| allowed_scopes.clone());

    for requested_scope in &requested_scopes {
        if !crate::context::scope_matches(&allowed_scopes, requested_scope) {
            return Err(TokenErrorResponse {
                error: "invalid_scope".to_string(),
                error_description: format!(
                    "Scope '{requested_scope}' is not allowed for this client"
                ),
            });
        }
    }

    Ok(ValidatedAuthorizeRequest {
        app,
        redirect_uri,
        requested_scopes,
        state,
        code_challenge,
    })
}

async fn issue_authorization_code(
    ctx: &AppContext,
    tenant_id: Uuid,
    validated: &ValidatedAuthorizeRequest,
    user_id: Uuid,
) -> Result<String, TokenErrorResponse> {
    OAuthAppService::generate_authorization_code(
        &ctx.db,
        validated.app.id,
        user_id,
        tenant_id,
        validated.redirect_uri.clone(),
        validated.requested_scopes.clone(),
        validated.code_challenge.clone(),
    )
    .await
    .map_err(|_| TokenErrorResponse {
        error: "server_error".to_string(),
        error_description: "Failed to generate authorization code".to_string(),
    })
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn extract_browser_access_token(headers: &HeaderMap) -> Option<String> {
    extract_bearer_token(headers).or_else(|| {
        headers
            .get(COOKIE)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_oauth_browser_cookie)
    })
}

fn parse_oauth_browser_cookie(cookie_header: &str) -> Option<String> {
    cookie_header.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == OAUTH_BROWSER_SESSION_COOKIE && !value.trim().is_empty())
            .then(|| value.trim().to_string())
    })
}

fn build_oauth_browser_session_cookie(
    access_token: &str,
    headers: &HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
) -> String {
    build_oauth_browser_session_cookie_for_secure(
        access_token,
        should_use_secure_cookie(headers, Some(peer_ip), settings),
        Some(OAUTH_BROWSER_SESSION_TTL_SECS),
    )
}

fn clear_oauth_browser_session_cookie(
    headers: &HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
) -> String {
    build_oauth_browser_session_cookie_for_secure(
        "",
        should_use_secure_cookie(headers, Some(peer_ip), settings),
        Some(0),
    )
}

fn clear_oauth_browser_session_cookie_for_redirect(redirect_uri: &str) -> String {
    let secure = Url::parse(redirect_uri)
        .map(|url| url.scheme() == "https")
        .unwrap_or(false);
    build_oauth_browser_session_cookie_for_secure("", secure, Some(0))
}

fn build_oauth_browser_session_cookie_for_secure(
    access_token: &str,
    secure: bool,
    max_age: Option<u64>,
) -> String {
    let mut cookie = format!(
        "{OAUTH_BROWSER_SESSION_COOKIE}={access_token}; Path=/api/oauth; HttpOnly; SameSite=Lax"
    );
    if let Some(max_age) = max_age {
        cookie.push_str(&format!("; Max-Age={max_age}"));
    }
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

fn should_use_secure_cookie(
    headers: &HeaderMap,
    peer_ip: Option<std::net::IpAddr>,
    settings: &RustokSettings,
) -> bool {
    extract_effective_proto(headers, peer_ip, &settings.runtime.request_trust)
        .is_some_and(|value| value.eq_ignore_ascii_case("https"))
}

fn redirect_with_code(
    redirect_uri: &str,
    code: &str,
    state: Option<&str>,
) -> (StatusCode, [(axum::http::header::HeaderName, String); 2]) {
    let mut url = Url::parse(redirect_uri).expect("validated redirect URI");
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("code", code);
        if let Some(state) = state {
            query.append_pair("state", state);
        }
    }
    (
        StatusCode::FOUND,
        [
            (LOCATION, url.to_string()),
            (
                SET_COOKIE,
                clear_oauth_browser_session_cookie_for_redirect(redirect_uri),
            ),
        ],
    )
}

fn redirect_with_error(
    redirect_uri: &str,
    error: &str,
    description: &str,
    state: Option<&str>,
) -> (StatusCode, [(axum::http::header::HeaderName, String); 2]) {
    let mut url = Url::parse(redirect_uri).expect("validated redirect URI");
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("error", error);
        query.append_pair("error_description", description);
        if let Some(state) = state {
            query.append_pair("state", state);
        }
    }
    (
        StatusCode::FOUND,
        [
            (LOCATION, url.to_string()),
            (
                SET_COOKIE,
                clear_oauth_browser_session_cookie_for_redirect(redirect_uri),
            ),
        ],
    )
}

fn render_authorization_required(app_name: &str) -> (StatusCode, Html<String>) {
    (
        StatusCode::UNAUTHORIZED,
        Html(render_error_page(
            "Authorization required",
            &format!(
                "Create an OAuth browser session first, then retry authorization for '{}'. First-party frontends should POST /api/oauth/browser-session with the current bearer token before opening the browser authorization URL.",
                app_name
            ),
        )),
    )
}

fn render_error_page(title: &str, message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title><style>body{{font-family:system-ui,sans-serif;max-width:720px;margin:48px auto;padding:0 16px;color:#111827}}.card{{border:1px solid #d1d5db;border-radius:12px;padding:24px;background:#fff}}p{{line-height:1.5;color:#374151}}</style></head><body><div class=\"card\"><h1>{}</h1><p>{}</p></div></body></html>",
        escape_html(title),
        escape_html(title),
        escape_html(message)
    )
}

fn render_consent_page(
    validated: &ValidatedAuthorizeRequest,
    request: &BrowserAuthorizeRequest,
    user_email: &str,
) -> String {
    let scopes = validated
        .requested_scopes
        .iter()
        .map(|scope| format!("<li>{}</li>", escape_html(scope)))
        .collect::<Vec<_>>()
        .join("");

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Authorize {app}</title><style>body{{font-family:system-ui,sans-serif;background:#f8fafc;color:#0f172a;max-width:760px;margin:40px auto;padding:0 16px}}.card{{background:#fff;border:1px solid #dbe3f0;border-radius:16px;padding:24px;box-shadow:0 12px 30px rgba(15,23,42,0.08)}}.meta{{color:#475569}}ul{{padding-left:20px}}form{{display:flex;gap:12px;margin-top:24px}}button{{border:0;border-radius:10px;padding:12px 18px;font-weight:600;cursor:pointer}}.approve{{background:#0f766e;color:white}}.deny{{background:#e2e8f0;color:#0f172a}}</style></head><body><div class=\"card\"><h1>Authorize {app}</h1><p class=\"meta\">Signed in as {user}. This app is requesting access to:</p><ul>{scopes}</ul><p class=\"meta\">Redirect URI: {redirect}</p><form method=\"post\" action=\"/api/oauth/consent\"><input type=\"hidden\" name=\"client_id\" value=\"{client_id}\"><input type=\"hidden\" name=\"redirect_uri\" value=\"{redirect_attr}\"><input type=\"hidden\" name=\"scope\" value=\"{scope_attr}\"><input type=\"hidden\" name=\"state\" value=\"{state_attr}\"><input type=\"hidden\" name=\"code_challenge\" value=\"{challenge_attr}\"><input type=\"hidden\" name=\"code_challenge_method\" value=\"S256\"><button class=\"approve\" type=\"submit\" name=\"action\" value=\"approve\">Approve</button><button class=\"deny\" type=\"submit\" name=\"action\" value=\"deny\">Deny</button></form></div></body></html>",
        app = escape_html(&validated.app.name),
        user = escape_html(user_email),
        scopes = scopes,
        redirect = escape_html(&validated.redirect_uri),
        client_id = escape_attr(&request.client_id),
        redirect_attr = escape_attr(&request.redirect_uri),
        scope_attr = escape_attr(request.scope.as_deref().unwrap_or_default()),
        state_attr = escape_attr(request.state.as_deref().unwrap_or_default()),
        challenge_attr = escape_attr(&request.code_challenge),
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value)
}

/// OAuth2 Token Revocation Request (RFC 7009)
#[derive(Debug, Deserialize)]
pub struct RevokeRequest {
    pub token: String,
    /// Optional hint: "access_token" or "refresh_token"
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

/// Token Revocation Endpoint (RFC 7009)
/// Revokes a refresh token (access tokens are stateless JWTs and expire naturally).
async fn revoke_handler(
    State(ctx): State<AppContext>,
    tenant_ctx: TenantContext,
    Json(req): Json<RevokeRequest>,
) -> Result<axum::http::StatusCode, TokenErrorResponse> {
    // 1. Authenticate the client
    let client_id_str = req.client_id.as_deref().ok_or_else(|| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "client_id is required".to_string(),
    })?;
    let client_id = Uuid::parse_str(client_id_str).map_err(|_| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "Invalid client_id format".to_string(),
    })?;

    let app = OAuthAppService::find_by_client_id(&ctx.db, client_id)
        .await
        .map_err(|_| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Internal error".to_string(),
        })?
        .ok_or_else(|| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Unknown client_id".to_string(),
        })?;

    if app.tenant_id != tenant_ctx.id {
        return Err(TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Client not registered for this tenant".to_string(),
        });
    }

    // Verify client_secret if the app has one
    if let Some(secret_hash) = &app.client_secret_hash {
        let client_secret = req
            .client_secret
            .as_deref()
            .ok_or_else(|| TokenErrorResponse {
                error: "invalid_client".to_string(),
                error_description: "client_secret is required".to_string(),
            })?;
        let valid =
            OAuthAppService::verify_client_secret(client_secret, secret_hash).map_err(|_| {
                TokenErrorResponse {
                    error: "invalid_client".to_string(),
                    error_description: "Invalid client credentials".to_string(),
                }
            })?;
        if !valid {
            return Err(TokenErrorResponse {
                error: "invalid_client".to_string(),
                error_description: "Invalid client credentials".to_string(),
            });
        }
    }

    // 2. Hash the token and try to revoke it
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(req.token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());

    OAuthAppService::revoke_token_by_hash(&ctx.db, &token_hash, app.id)
        .await
        .map_err(|_| TokenErrorResponse {
            error: "server_error".to_string(),
            error_description: "Failed to revoke token".to_string(),
        })?;

    // RFC 7009: always return 200 OK regardless of whether token existed
    Ok(axum::http::StatusCode::OK)
}

/// OpenID Connect UserInfo Endpoint (RFC 5362)
/// Allows clients with `openid` or `profile` scopes to fetch user details.
async fn userinfo_handler(
    current_user: CurrentUser, // Automatically extracts and validates Bearer token
) -> Result<Json<serde_json::Value>, TokenErrorResponse> {
    // We already know the token is valid, active, and belongs to a user because
    // the CurrentUser extractor succeeds only if these conditions are met.

    // In a full OIDC implementation, we'd check if the token had the `openid` scope specifically.
    // We assume CurrentUser claims contain the scopes if needed, but since we rely on RBAC
    // returning the user profile here is generally safe for authenticated apps.

    let user = current_user.user;
    let inferred_role = current_user.inferred_role;

    // standard OIDC claims
    let userinfo = serde_json::json!({
        "sub": user.id.to_string(),
        "name": user.name.unwrap_or_default(),
        "email": user.email,
        "email_verified": true, // We assume true for simplicity here, adjust if rustok tracks verification
        "role": inferred_role.to_string(),
        "tenant_id": user.tenant_id.to_string(),
    });

    Ok(Json(userinfo))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/oauth")
        .add(
            "/authorize",
            get(authorize_browser_handler).post(authorize_handler),
        )
        .add(
            "/browser-session",
            post(create_browser_session_handler).delete(clear_browser_session_handler),
        )
        .add("/consent", post(consent_handler))
        .add("/token", post(token_handler))
        .add("/userinfo", get(userinfo_handler))
        .add("/revoke", post(revoke_handler))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::settings::{ForwardedHeadersMode, RequestTrustSettings};
    use axum::http::{header::HeaderValue, HeaderName};
    use chrono::Utc;
    use std::net::{IpAddr, Ipv4Addr};

    fn sample_app(app_type: &str) -> crate::models::oauth_apps::Model {
        crate::models::oauth_apps::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            name: "Example App".to_string(),
            slug: "example-app".to_string(),
            description: Some("Example".to_string()),
            app_type: app_type.to_string(),
            icon_url: None,
            client_id: Uuid::new_v4(),
            client_secret_hash: Some("hash".to_string()),
            redirect_uris: serde_json::json!(["https://client.example.com/callback"]),
            scopes: serde_json::json!(["profile", "catalog:read"]),
            grant_types: serde_json::json!(["authorization_code", "refresh_token"]),
            granted_permissions: serde_json::json!([]),
            manifest_ref: None,
            auto_created: false,
            is_active: true,
            revoked_at: None,
            last_used_at: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    fn validated_request(app_type: &str) -> ValidatedAuthorizeRequest {
        ValidatedAuthorizeRequest {
            app: sample_app(app_type),
            redirect_uri: "https://client.example.com/callback".to_string(),
            requested_scopes: vec!["profile".to_string(), "catalog:read".to_string()],
            state: Some("opaque-state".to_string()),
            code_challenge: "challenge-value".to_string(),
        }
    }

    fn trusted_settings() -> RustokSettings {
        let mut settings = RustokSettings::default();
        settings.runtime.request_trust = RequestTrustSettings {
            forwarded_headers_mode: ForwardedHeadersMode::TrustedOnly,
            trusted_proxy_cidrs: vec!["10.0.0.0/8".to_string()],
        };
        settings
    }

    #[test]
    fn browser_cookie_is_parsed_and_authorization_header_wins() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_static(
                "other=1; rustok_oauth_browser_session=session-token; another=2",
            ),
        );
        assert_eq!(
            extract_browser_access_token(&headers).as_deref(),
            Some("session-token")
        );

        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer header-token"),
        );
        assert_eq!(
            extract_browser_access_token(&headers).as_deref(),
            Some("header-token")
        );
    }

    #[test]
    fn browser_session_cookie_respects_secure_forwarding() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-forwarded-proto"),
            HeaderValue::from_static("https"),
        );
        let settings = trusted_settings();
        let peer_ip = IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3));

        let cookie = build_oauth_browser_session_cookie("token-123", &headers, peer_ip, &settings);
        assert!(cookie.contains("rustok_oauth_browser_session=token-123"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Max-Age=600"));
        assert!(cookie.contains("Secure"));

        let cleared = clear_oauth_browser_session_cookie(&headers, peer_ip, &settings);
        assert!(cleared.contains("Max-Age=0"));
        assert!(cleared.contains("Secure"));
    }

    #[test]
    fn redirect_helpers_append_query_and_clear_cookie() {
        let (status, headers) = redirect_with_code(
            "https://client.example.com/callback",
            "auth-code",
            Some("state-123"),
        );
        assert_eq!(status, StatusCode::FOUND);
        assert_eq!(
            headers[0].1,
            "https://client.example.com/callback?code=auth-code&state=state-123"
        );
        assert!(headers[1].1.contains("Max-Age=0"));
        assert!(headers[1].1.contains("Secure"));

        let (status, headers) = redirect_with_error(
            "https://client.example.com/callback",
            "access_denied",
            "The user denied access",
            Some("state-123"),
        );
        assert_eq!(status, StatusCode::FOUND);
        assert!(headers[0].1.contains("error=access_denied"));
        assert!(headers[0].1.contains("state=state-123"));
        assert!(headers[1].1.contains("Max-Age=0"));
    }

    #[test]
    fn authorization_required_page_mentions_browser_session_endpoint() {
        let (status, html) = render_authorization_required("Admin App");
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(html.0.contains("POST /api/oauth/browser-session"));
        assert!(html.0.contains("Admin App"));
    }

    #[test]
    fn consent_page_escapes_fields_and_contains_expected_form_inputs() {
        let mut validated = validated_request("third_party");
        validated.app.name = "Partner <App>".to_string();
        validated.redirect_uri = "https://client.example.com/callback?from=<unsafe>".to_string();
        validated.requested_scopes = vec!["profile".to_string(), "catalog:read".to_string()];

        let request = BrowserAuthorizeRequest {
            response_type: "code".to_string(),
            client_id: validated.app.client_id.to_string(),
            redirect_uri: validated.redirect_uri.clone(),
            scope: Some("profile catalog:read".to_string()),
            state: Some("\"quoted-state\"".to_string()),
            code_challenge: "challenge<&>".to_string(),
            code_challenge_method: Some("S256".to_string()),
        };

        let html = render_consent_page(&validated, &request, "user+test@example.com");
        assert!(html.contains("Authorize Partner &lt;App&gt;"));
        assert!(html.contains("user+test@example.com"));
        assert!(html.contains("name=\"client_id\""));
        assert!(html.contains("name=\"redirect_uri\""));
        assert!(html.contains("name=\"scope\""));
        assert!(html.contains("name=\"state\""));
        assert!(html.contains("name=\"code_challenge\""));
        assert!(html.contains("&lt;unsafe&gt;"));
        assert!(html.contains("challenge&lt;&amp;&gt;"));
        assert!(html.contains("&quot;quoted-state&quot;"));
    }

    #[test]
    fn consent_requirement_depends_on_app_type() {
        assert!(validated_request("third_party").app.requires_user_consent());
        assert!(!validated_request("first_party").app.requires_user_consent());
    }
}
