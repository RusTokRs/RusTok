use axum::{
    Json,
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustok_auth::{AcceptInviteParams, InviteAcceptResponse};
use rustok_web::json_response;

use crate::context::TenantContextExt;
use crate::error::Error;
use crate::services::auth_lifecycle::AuthLifecycleService;
use crate::services::server_runtime_context::ServerAuthRuntime;

const INVITE_ACCEPT_PATH: &str = "/api/auth/invite/accept";
const MAX_INVITE_REQUEST_BYTES: usize = 64 * 1024;

/// Route guard that keeps the legacy REST surface while delegating acceptance
/// to the same transactionally one-shot service used by GraphQL/server ports.
pub async fn consume_once(
    State(ctx): State<ServerAuthRuntime>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.method() != Method::POST || request.uri().path() != INVITE_ACCEPT_PATH {
        return next.run(request).await;
    }

    let (parts, body) = request.into_parts();
    let tenant_id = match parts.tenant_context() {
        Some(tenant) => tenant.id,
        None => return Error::InternalServerError.into_response(),
    };
    let bytes = match to_bytes(body, MAX_INVITE_REQUEST_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_request",
                    "message": "Invite request body is invalid or too large"
                })),
            )
                .into_response();
        }
    };
    let params = match serde_json::from_slice::<AcceptInviteParams>(&bytes) {
        Ok(params) => params,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_request",
                    "message": "Invite request must be valid JSON"
                })),
            )
                .into_response();
        }
    };
    let config = match ctx.auth_config() {
        Some(config) => config,
        None => return Error::InternalServerError.into_response(),
    };

    let accepted = match AuthLifecycleService::accept_invite_once_runtime(
        ctx.runtime_ctx(),
        config,
        tenant_id,
        &params.token,
        &params.password,
        params.name,
    )
    .await
    {
        Ok(accepted) => accepted,
        Err(error) => return Error::from(error).into_response(),
    };

    json_response(InviteAcceptResponse {
        status: "ok",
        email: accepted.email,
        role: accepted.role,
    })
}
