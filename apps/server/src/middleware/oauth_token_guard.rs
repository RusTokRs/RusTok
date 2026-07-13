use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use rustok_auth::{TokenErrorResponse, TokenRequest};

use crate::context::TenantContextExt;
use crate::services::oauth_token_service::{OAuthTokenProtocolError, OAuthTokenService};
use crate::services::server_runtime_context::ServerAuthRuntime;

const TOKEN_PATH: &str = "/api/oauth/token";
const MAX_TOKEN_REQUEST_BYTES: usize = 64 * 1024;

/// Route the OAuth token endpoint through the hardened service boundary.
///
/// The legacy controller remains mounted for compatibility, but token requests
/// are completed here and never reach it. This keeps HTTP parsing in the
/// middleware while all client, grant, scope, PKCE, consent, replay and token
/// issuance rules live in `OAuthTokenService`.
pub async fn validate(
    State(runtime): State<ServerAuthRuntime>,
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
            return protocol_error(OAuthTokenProtocolError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                error: "server_error",
                description: "Tenant context is unavailable".to_string(),
            })
        }
    };
    let bytes = match to_bytes(body, MAX_TOKEN_REQUEST_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return protocol_error(OAuthTokenProtocolError {
                status: StatusCode::BAD_REQUEST,
                error: "invalid_request",
                description: "Token request body is invalid or too large".to_string(),
            })
        }
    };
    let token_request = match serde_json::from_slice::<TokenRequest>(&bytes) {
        Ok(request) => request,
        Err(_) => {
            return protocol_error(OAuthTokenProtocolError {
                status: StatusCode::BAD_REQUEST,
                error: "invalid_request",
                description: "Token request must be valid JSON".to_string(),
            })
        }
    };

    match OAuthTokenService::exchange(&runtime, tenant_id, &token_request).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => protocol_error(error),
    }
}

fn protocol_error(error: OAuthTokenProtocolError) -> Response {
    (
        error.status,
        Json(TokenErrorResponse {
            error: error.error.to_string(),
            error_description: error.description,
        }),
    )
        .into_response()
}
