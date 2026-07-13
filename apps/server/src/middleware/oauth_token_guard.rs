use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use rustok_api::context::scope_matches;
use rustok_auth::{TokenErrorResponse, TokenRequest};
use uuid::Uuid;

use crate::context::TenantContextExt;
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
            // The existing exchange service performs redirect URI, PKCE and
            // authorization-code binding checks. Client authentication is
            // repeated here so every confidential token exchange has one
            // consistent precondition.
            authenticate_confidential_client(&app, request)?;
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
            if request
                .refresh_token
                .as_deref()
                .is_none_or(|token| token.trim().is_empty())
            {
                return Err(token_error(
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "refresh_token is required",
                ));
            }
        }
        _ => {}
    }

    Ok(())
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
        .collect::<Vec<_>>();

    if requested
        .iter()
        .all(|scope| scope_matches(allowed_scopes, scope))
    {
        return Ok(());
    }

    Err(token_error(
        StatusCode::BAD_REQUEST,
        "invalid_scope",
        "One or more requested scopes are not allowed for this client",
    ))
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
    use super::validate_requested_scopes;

    #[test]
    fn requested_scope_must_be_subset_of_app_scopes() {
        let allowed = vec!["catalog:*".to_string(), "profile".to_string()];
        assert!(validate_requested_scopes(&allowed, Some("catalog:read profile")).is_ok());
        assert!(validate_requested_scopes(&allowed, Some("catalog:read admin:users")).is_err());
    }

    #[test]
    fn omitted_scope_uses_server_default_policy() {
        assert!(validate_requested_scopes(&["catalog:read".to_string()], None).is_ok());
    }
}
