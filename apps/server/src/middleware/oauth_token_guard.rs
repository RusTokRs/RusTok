use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rustok_api::context::scope_matches;
use rustok_auth::{TokenErrorResponse, TokenRequest};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::auth::hash_refresh_token;
use crate::context::TenantContextExt;
use crate::models::{
    oauth_authorization_codes as oauth_codes,
    oauth_consents::Entity as OAuthConsents,
    oauth_tokens,
    users::{self, Entity as Users},
};
use crate::services::oauth_app::OAuthAppService;
use crate::services::server_runtime_context::ServerRuntimeContext;

const TOKEN_PATH: &str = "/api/oauth/token";
const MAX_TOKEN_REQUEST_BYTES: usize = 64 * 1024;

pub async fn validate(
    State(ctx): State<ServerRuntimeContext>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.method() != Method::POST || request.uri().path() != TOKEN_PATH {
        return next.run(request).await;
    }

    let (parts, body) = request.into_parts();
    let tenant_id = match parts.tenant_context() {
        Some(tenant) => tenant.id,
        None => {
            return token_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                "Tenant context is unavailable",
            )
        }
    };
    let bytes = match to_bytes(body, MAX_TOKEN_REQUEST_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return token_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Token request body is invalid or too large",
            )
        }
    };
    let token_request = match serde_json::from_slice::<TokenRequest>(&bytes) {
        Ok(request) => request,
        Err(_) => {
            return token_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Token request must be valid JSON",
            )
        }
    };

    if let Err(response) = validate_request(&ctx, tenant_id, &token_request).await {
        return response;
    }

    next.run(Request::from_parts(parts, Body::from(bytes))).await
}

async fn validate_request(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    request: &TokenRequest,
) -> Result<(), Response> {
    match request.grant_type.as_str() {
        "authorization_code" => {
            let app = resolve_client(ctx, tenant_id, request).await?;
            require_grant(&app, "authorization_code")?;
            authenticate_confidential_client(&app, request)?;
            validate_authorization_code(ctx, tenant_id, &app, request).await?;
        }
        "client_credentials" => {
            let app = resolve_client(ctx, tenant_id, request).await?;
            require_grant(&app, "client_credentials")?;
            authenticate_confidential_client(&app, request)?;
            validate_requested_scopes(&app.scopes_list(), request.scope.as_deref())?;
        }
        "refresh_token" => {
            let app = resolve_client(ctx, tenant_id, request).await?;
            require_grant(&app, "refresh_token")?;
            authenticate_confidential_client(&app, request)?;
            validate_refresh_token(ctx, tenant_id, &app, request).await?;
        }
        _ => {}
    }

    Ok(())
}

async fn validate_authorization_code(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    app: &crate::models::oauth_apps::Model,
    request: &TokenRequest,
) -> Result<(), Response> {
    let code = required_value(request.code.as_deref(), "code is required")?;
    let redirect_uri = required_value(
        request.redirect_uri.as_deref(),
        "redirect_uri is required",
    )?;
    let verifier = required_value(
        request.code_verifier.as_deref(),
        "code_verifier is required",
    )?;

    let code_hash = hex::encode(Sha256::digest(code.as_bytes()));
    let code_model = oauth_codes::Entity::find()
        .filter(oauth_codes::Column::CodeHash.eq(code_hash))
        .filter(oauth_codes::Column::UsedAt.is_null())
        .filter(oauth_codes::Column::ExpiresAt.gt(chrono::Utc::now()))
        .one(ctx.db())
        .await
        .map_err(|_| {
            token_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                "Failed to validate authorization code",
            )
        })?
        .ok_or_else(|| {
            token_error(
                StatusCode::BAD_REQUEST,
                "invalid_grant",
                "Authorization code is invalid, expired, or already used",
            )
        })?;

    if code_model.app_id != app.id || code_model.tenant_id != tenant_id {
        return Err(token_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "Authorization code is not bound to this client or tenant",
        ));
    }
    if code_model.redirect_uri != redirect_uri {
        return Err(token_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "redirect_uri does not match the authorization request",
        ));
    }
    if code_model.code_challenge_method != "S256" {
        return Err(token_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "Unsupported PKCE challenge method",
        ));
    }

    let expected_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    if !bool::from(
        expected_challenge
            .as_bytes()
            .ct_eq(code_model.code_challenge.as_bytes()),
    ) {
        return Err(token_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "PKCE verification failed",
        ));
    }

    let scopes = code_model.scopes_list();
    validate_scope_subset(&app.scopes_list(), &scopes)?;
    validate_active_consent(ctx, app, tenant_id, code_model.user_id, &scopes).await?;

    let user = Users::find_by_id(code_model.user_id)
        .filter(users::Column::TenantId.eq(tenant_id))
        .one(ctx.db())
        .await
        .map_err(|_| {
            token_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                "Failed to validate authorization subject",
            )
        })?
        .filter(|user| user.is_active())
        .ok_or_else(|| {
            token_error(
                StatusCode::BAD_REQUEST,
                "invalid_grant",
                "Authorization subject is missing or inactive",
            )
        })?;

    if user.id != code_model.user_id {
        return Err(token_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "Authorization subject mismatch",
        ));
    }

    Ok(())
}

async fn validate_refresh_token(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    app: &crate::models::oauth_apps::Model,
    request: &TokenRequest,
) -> Result<(), Response> {
    let refresh_token = required_value(
        request.refresh_token.as_deref(),
        "refresh_token is required",
    )?;
    let token_hash = hash_refresh_token(refresh_token);
    let token = oauth_tokens::Entity::find()
        .filter(oauth_tokens::Column::TokenHash.eq(token_hash))
        .filter(oauth_tokens::Column::AppId.eq(app.id))
        .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
        .filter(oauth_tokens::Column::TokenType.eq("refresh"))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .filter(oauth_tokens::Column::ExpiresAt.gt(chrono::Utc::now()))
        .one(ctx.db())
        .await
        .map_err(|_| {
            token_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                "Failed to validate refresh token",
            )
        })?
        .ok_or_else(|| {
            token_error(
                StatusCode::BAD_REQUEST,
                "invalid_grant",
                "Refresh token is invalid, expired, or already used",
            )
        })?;

    let scopes = token.scopes_list();
    validate_scope_subset(&app.scopes_list(), &scopes)?;

    if let Some(user_id) = token.user_id {
        let user = Users::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .one(ctx.db())
            .await
            .map_err(|_| {
                token_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "server_error",
                    "Failed to validate refresh token subject",
                )
            })?
            .filter(|user| user.is_active())
            .ok_or_else(|| {
                token_error(
                    StatusCode::BAD_REQUEST,
                    "invalid_grant",
                    "Refresh token subject is missing or inactive",
                )
            })?;

        validate_active_consent(ctx, app, tenant_id, user.id, &scopes).await?;
    }

    Ok(())
}

async fn validate_active_consent(
    ctx: &ServerRuntimeContext,
    app: &crate::models::oauth_apps::Model,
    tenant_id: Uuid,
    user_id: Uuid,
    scopes: &[String],
) -> Result<(), Response> {
    if !app.requires_user_consent() {
        return Ok(());
    }

    let consent = OAuthConsents::find_active_consent(ctx.db(), app.id, user_id)
        .await
        .map_err(|_| {
            token_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                "Failed to validate OAuth consent",
            )
        })?
        .filter(|consent| consent.tenant_id == tenant_id)
        .ok_or_else(|| {
            token_error(
                StatusCode::BAD_REQUEST,
                "invalid_grant",
                "OAuth consent is missing or revoked",
            )
        })?;

    validate_scope_subset(&consent.scopes_list(), scopes)
}

async fn resolve_client(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    request: &TokenRequest,
) -> Result<crate::models::oauth_apps::Model, Response> {
    let client_id = request
        .client_id
        .as_deref()
        .ok_or_else(|| {
            token_error(
                StatusCode::UNAUTHORIZED,
                "invalid_client",
                "client_id is required",
            )
        })
        .and_then(|value| {
            Uuid::parse_str(value).map_err(|_| {
                token_error(
                    StatusCode::UNAUTHORIZED,
                    "invalid_client",
                    "Invalid client_id format",
                )
            })
        })?;

    let app = OAuthAppService::find_by_client_id(ctx.db(), client_id)
        .await
        .map_err(|_| {
            token_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                "Failed to resolve OAuth client",
            )
        })?
        .ok_or_else(|| {
            token_error(
                StatusCode::UNAUTHORIZED,
                "invalid_client",
                "Unknown or inactive client",
            )
        })?;

    if app.tenant_id != tenant_id {
        return Err(token_error(
            StatusCode::UNAUTHORIZED,
            "invalid_client",
            "Client is not registered for this tenant",
        ));
    }

    Ok(app)
}

fn require_grant(
    app: &crate::models::oauth_apps::Model,
    grant_type: &'static str,
) -> Result<(), Response> {
    if app.supports_grant_type(grant_type) {
        Ok(())
    } else {
        Err(token_error(
            StatusCode::BAD_REQUEST,
            "unauthorized_client",
            "The client is not allowed to use this grant type",
        ))
    }
}

fn authenticate_confidential_client(
    app: &crate::models::oauth_apps::Model,
    request: &TokenRequest,
) -> Result<(), Response> {
    let Some(secret_hash) = app.client_secret_hash.as_deref() else {
        return Ok(());
    };
    let secret = request
        .client_secret
        .as_deref()
        .filter(|secret| !secret.is_empty())
        .ok_or_else(|| {
            token_error(
                StatusCode::UNAUTHORIZED,
                "invalid_client",
                "client_secret is required",
            )
        })?;
    let valid = OAuthAppService::verify_client_secret(secret, secret_hash).map_err(|_| {
        token_error(
            StatusCode::UNAUTHORIZED,
            "invalid_client",
            "Invalid client credentials",
        )
    })?;

    if valid {
        Ok(())
    } else {
        Err(token_error(
            StatusCode::UNAUTHORIZED,
            "invalid_client",
            "Invalid client credentials",
        ))
    }
}

fn validate_requested_scopes(
    allowed_scopes: &[String],
    requested_scope: Option<&str>,
) -> Result<(), Response> {
    let requested = requested_scope
        .unwrap_or_default()
        .split_whitespace()
        .filter(|scope| !scope.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    validate_scope_subset(allowed_scopes, &requested)
}

fn validate_scope_subset(allowed_scopes: &[String], requested: &[String]) -> Result<(), Response> {
    if requested
        .iter()
        .all(|scope| scope_matches(allowed_scopes, scope))
    {
        return Ok(());
    }

    Err(token_error(
        StatusCode::BAD_REQUEST,
        "invalid_scope",
        "One or more requested scopes are not allowed",
    ))
}

fn required_value<'a>(value: Option<&'a str>, description: &'static str) -> Result<&'a str, Response> {
    value.filter(|value| !value.trim().is_empty()).ok_or_else(|| {
        token_error(StatusCode::BAD_REQUEST, "invalid_request", description)
    })
}

fn token_error(status: StatusCode, error: &str, description: &str) -> Response {
    (
        status,
        Json(TokenErrorResponse {
            error: error.to_string(),
            error_description: description.to_string(),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::{validate_requested_scopes, validate_scope_subset};

    #[test]
    fn requested_scope_must_be_subset_of_app_scopes() {
        let allowed = vec!["catalog:*".to_string(), "profile".to_string()];
        assert!(validate_requested_scopes(&allowed, Some("catalog:read profile")).is_ok());
        assert!(validate_requested_scopes(&allowed, Some("catalog:read admin:users")).is_err());
    }

    #[test]
    fn code_and_refresh_scopes_use_the_same_subset_policy() {
        let allowed = vec!["forum:*".to_string()];
        assert!(validate_scope_subset(&allowed, &["forum:read".to_string()]).is_ok());
        assert!(validate_scope_subset(&allowed, &["admin:users".to_string()]).is_err());
    }

    #[test]
    fn omitted_scope_uses_server_default_policy() {
        assert!(validate_requested_scopes(&["catalog:read".to_string()], None).is_ok());
    }
}
