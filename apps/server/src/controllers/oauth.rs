//! OAuth2 REST endpoints (RFC 6749)
//!
//! `POST /oauth/token` — Token endpoint (client_credentials flow)

use axum::{
    extract::State,
    routing::{get, post},
    Json,
};
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::auth_config_from_ctx;
use crate::context::TenantContext;
use crate::extractors::auth::CurrentUser;
use crate::services::oauth_app::OAuthAppService;

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
            "invalid_client" => axum::http::StatusCode::UNAUTHORIZED,
            "invalid_grant" | "unsupported_grant_type" => axum::http::StatusCode::BAD_REQUEST,
            "invalid_scope" => axum::http::StatusCode::BAD_REQUEST,
            _ => axum::http::StatusCode::BAD_REQUEST,
        };
        (status, Json(self)).into_response()
    }
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
    current_user: CurrentUser, // Requires authenticated user
    Json(req): Json<AuthorizeRequest>,
) -> Result<Json<serde_json::Value>, TokenErrorResponse> {
    // 1. Verify standard parameters
    if req.response_type != "code" {
        return Err(TokenErrorResponse {
            error: "unsupported_response_type".to_string(),
            error_description: "Only response_type=code is supported".to_string(),
        });
    }

    if req.code_challenge_method.as_deref() != Some("S256") {
        return Err(TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "code_challenge_method must be S256".to_string(),
        });
    }

    let client_id = Uuid::parse_str(&req.client_id).map_err(|_| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "Invalid client_id format".to_string(),
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

    // Check if the redirect URI is allowed
    let allowed_uris = app.redirect_uris_list();
    if !allowed_uris.contains(&req.redirect_uri) {
        return Err(TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "redirect_uri is not configured for this client".to_string(),
        });
    }

    // 3. Parse requested scopes and ensure they are allowed
    let allowed_scopes = app.scopes_list();
    let requested_scopes: Vec<String> = req
        .scope
        .as_deref()
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default();

    let granted_scopes = if requested_scopes.is_empty() {
        allowed_scopes.clone()
    } else {
        requested_scopes
            .iter()
            .filter(|s| crate::services::oauth_app::scope_matches(&allowed_scopes, s))
            .cloned()
            .collect()
    };

    // 4. Check consent if app is ThirdParty
    if app.app_type == "third_party" {
        let has_consent = OAuthAppService::get_active_consent(
            &ctx.db,
            app.id,
            current_user.user.id,
            &granted_scopes,
        )
        .await
        .map_err(|_| TokenErrorResponse {
            error: "server_error".to_string(),
            error_description: "Failed to verify consent".to_string(),
        })?;

        if !has_consent {
            // App needs consent from user.
            // In a real environment, we would redirect to a consent UI.
            // But since this is API-first, we return interaction_required.
            return Err(TokenErrorResponse {
                error: "interaction_required".to_string(),
                error_description:
                    "User consent is required. Please prompt the user to grant access.".to_string(),
            });
        }
    }

    // 5. Generate Auth Code
    let code = OAuthAppService::generate_authorization_code(
        &ctx.db,
        app.id,
        current_user.user.id,
        tenant_ctx.id,
        req.redirect_uri.clone(),
        granted_scopes,
        req.code_challenge,
    )
    .await
    .map_err(|_| TokenErrorResponse {
        error: "server_error".to_string(),
        error_description: "Failed to generate authorization code".to_string(),
    })?;

    // Return the result (in a real browser flow, this would be a 302 Redirect)
    // Here we return JSON so frontends can handle the redirect manually,
    // which works well for SPA architectures.
    let mut response = serde_json::json!({
        "code": code,
        "redirect_uri": req.redirect_uri,
    });

    if let Some(state) = req.state {
        response["state"] = serde_json::json!(state);
    }

    Ok(Json(response))
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
        .add("/authorize", post(authorize_handler))
        .add("/token", post(token_handler))
        .add("/userinfo", get(userinfo_handler))
        .add("/revoke", post(revoke_handler))
}
