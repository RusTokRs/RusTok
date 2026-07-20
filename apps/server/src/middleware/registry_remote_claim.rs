use axum::{
    Json,
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

use crate::services::marketplace_catalog::{
    REGISTRY_MUTATION_SCHEMA_VERSION, RegistryRunnerClaimPayload, RegistryRunnerClaimRequest,
    RegistryRunnerClaimResponse, RegistryRunnerCompletionRequest, RegistryRunnerHeartbeatRequest,
    RegistryRunnerMutationResponse,
};
use crate::services::registry_remote_runner::claim_remote_validation_stage_atomic;
use crate::services::registry_remote_transitions::{
    RegistryRemoteTransitionError, RemoteTerminalOutcome, finish_remote_validation_stage_atomic,
    heartbeat_remote_validation_stage_atomic,
};
use crate::services::server_runtime_context::ServerRuntimeContext;

const CLAIM_PATH: &str = "/v2/catalog/runner/claim";
const PUBLISH_PATH: &str = "/v2/catalog/publish";
const MAX_RUNNER_BODY_BYTES: usize = 64 * 1024;
type RegistryClaimValidationResult<T> = Result<T, Box<Response>>;

/// Replace legacy read-check-update runner routes with database CAS operations.
/// Claim, heartbeat, complete and fail are all serialized by stage status,
/// claim id, runner id and lease expiry before the old controller can execute.
/// The same globally installed boundary delegates first-party publish requests
/// to the registry metadata policy before persistence.
pub async fn claim_atomic(
    State(ctx): State<ServerRuntimeContext>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.method() == Method::POST && request.uri().path() == PUBLISH_PATH {
        return crate::middleware::registry_publish_policy::enforce(request, next).await;
    }
    if request.method() != Method::POST {
        return next.run(request).await;
    }
    let path = request.uri().path().to_string();
    let Some(route) = runner_route(&path) else {
        return next.run(request).await;
    };

    let executor = &ctx.settings().registry.remote_executor;
    if !executor.enabled {
        return response(
            StatusCode::NOT_FOUND,
            "not_found",
            "Remote executor is disabled",
        );
    }
    let Some(expected) = executor.shared_token.as_deref() else {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            "configuration_error",
            "Remote executor shared token is not configured",
        );
    };
    let supplied = request
        .headers()
        .get("x-rustok-runner-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if !supplied.is_some_and(|value| bool::from(value.as_bytes().ct_eq(expected.as_bytes()))) {
        return response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Missing or invalid registry runner token",
        );
    }

    let (_parts, body) = request.into_parts();
    let bytes = match to_bytes(body, MAX_RUNNER_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return response(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Runner request body is invalid or too large",
            );
        }
    };

    match route {
        RunnerRoute::Claim => handle_claim(&ctx, executor.lease_ttl_ms, &bytes).await,
        RunnerRoute::Heartbeat { claim_id } => {
            handle_heartbeat(&ctx, executor.lease_ttl_ms, &claim_id, &bytes).await
        }
        RunnerRoute::Complete { claim_id } => {
            handle_terminal(&ctx, &claim_id, &bytes, RemoteTerminalOutcome::Passed).await
        }
        RunnerRoute::Fail { claim_id } => {
            handle_terminal(&ctx, &claim_id, &bytes, RemoteTerminalOutcome::Failed).await
        }
    }
}

async fn handle_claim(ctx: &ServerRuntimeContext, lease_ttl_ms: u64, bytes: &[u8]) -> Response {
    let input = match serde_json::from_slice::<RegistryRunnerClaimRequest>(bytes) {
        Ok(input) => input,
        Err(_) => {
            return response(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Runner claim body must be valid JSON",
            );
        }
    };
    if let Err(response) = validate_schema_version(input.schema_version) {
        return *response;
    }
    if input.runner_id.trim().is_empty() {
        return response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Runner claim requires a non-empty runner_id",
        );
    }
    if input.supported_stages.is_empty() {
        return response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Runner claim requires at least one supported stage",
        );
    }

    let claim = match claim_remote_validation_stage_atomic(
        ctx.db(),
        &input.runner_id,
        &input.supported_stages,
        lease_ttl_ms,
    )
    .await
    {
        Ok(claim) => claim,
        Err(error)
            if error
                .to_string()
                .starts_with("Unsupported validation stage") =>
        {
            return response(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                &error.to_string(),
            );
        }
        Err(error) => {
            tracing::error!(%error, runner_id = %input.runner_id, "Atomic registry runner claim failed");
            return response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to claim registry validation stage",
            );
        }
    };

    Json(RegistryRunnerClaimResponse {
        accepted: true,
        claim: claim.map(|claim| RegistryRunnerClaimPayload {
            claim_id: claim.claim_id,
            request_id: claim.request_id,
            slug: claim.slug,
            version: claim.version,
            stage_key: claim.stage_key,
            execution_mode: claim.execution_mode,
            runnable: claim.runnable,
            requires_manual_confirmation: claim.requires_manual_confirmation,
            allowed_terminal_reason_codes: claim.allowed_terminal_reason_codes,
            suggested_pass_reason_code: claim.suggested_pass_reason_code,
            suggested_failure_reason_code: claim.suggested_failure_reason_code,
            suggested_blocked_reason_code: claim.suggested_blocked_reason_code,
            artifact_download_url: claim.artifact_download_url,
            artifact_checksum_sha256: claim.artifact_checksum_sha256,
            crate_name: claim.crate_name,
        }),
    })
    .into_response()
}

async fn handle_heartbeat(
    ctx: &ServerRuntimeContext,
    lease_ttl_ms: u64,
    claim_id: &str,
    bytes: &[u8],
) -> Response {
    let input = match serde_json::from_slice::<RegistryRunnerHeartbeatRequest>(bytes) {
        Ok(input) => input,
        Err(_) => {
            return response(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Runner heartbeat body must be valid JSON",
            );
        }
    };
    if let Err(response) = validate_schema_version(input.schema_version) {
        return *response;
    }

    match heartbeat_remote_validation_stage_atomic(
        ctx.db(),
        claim_id,
        &input.runner_id,
        lease_ttl_ms,
    )
    .await
    {
        Ok(_) => mutation_response(claim_id, "running"),
        Err(error) => transition_error_response(error),
    }
}

async fn handle_terminal(
    ctx: &ServerRuntimeContext,
    claim_id: &str,
    bytes: &[u8],
    outcome: RemoteTerminalOutcome,
) -> Response {
    let input = match serde_json::from_slice::<RegistryRunnerCompletionRequest>(bytes) {
        Ok(input) => input,
        Err(_) => {
            return response(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Runner terminal body must be valid JSON",
            );
        }
    };
    if let Err(response) = validate_schema_version(input.schema_version) {
        return *response;
    }

    match finish_remote_validation_stage_atomic(
        ctx.db(),
        claim_id,
        &input.runner_id,
        outcome,
        input.detail.as_deref(),
        input.reason_code.as_deref(),
    )
    .await
    {
        Ok(_) => mutation_response(
            claim_id,
            match outcome {
                RemoteTerminalOutcome::Passed => "passed",
                RemoteTerminalOutcome::Failed => "failed",
            },
        ),
        Err(error) => transition_error_response(error),
    }
}

fn validate_schema_version(schema_version: u32) -> RegistryClaimValidationResult<()> {
    if schema_version == REGISTRY_MUTATION_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(Box::new(response(
            StatusCode::BAD_REQUEST,
            "invalid_schema_version",
            "Runner request schema_version is not supported",
        )))
    }
}

fn mutation_response(claim_id: &str, status: &str) -> Response {
    Json(RegistryRunnerMutationResponse {
        accepted: true,
        claim_id: claim_id.to_string(),
        status: status.to_string(),
        warnings: Vec::new(),
    })
    .into_response()
}

fn transition_error_response(error: RegistryRemoteTransitionError) -> Response {
    match error {
        RegistryRemoteTransitionError::Invalid(message) => {
            response(StatusCode::BAD_REQUEST, "invalid_request", &message)
        }
        RegistryRemoteTransitionError::Forbidden(message) => {
            response(StatusCode::FORBIDDEN, "forbidden", &message)
        }
        RegistryRemoteTransitionError::NotFound(message) => {
            response(StatusCode::NOT_FOUND, "not_found", &message)
        }
        RegistryRemoteTransitionError::Conflict(message) => {
            response(StatusCode::CONFLICT, "conflict", &message)
        }
        RegistryRemoteTransitionError::Internal(message) => {
            tracing::error!(error = %message, "Atomic registry runner transition failed");
            response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to update registry validation stage",
            )
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RunnerRoute {
    Claim,
    Heartbeat { claim_id: String },
    Complete { claim_id: String },
    Fail { claim_id: String },
}

fn runner_route(path: &str) -> Option<RunnerRoute> {
    if path == CLAIM_PATH {
        return Some(RunnerRoute::Claim);
    }
    let segments = path.trim_matches('/').split('/').collect::<Vec<_>>();
    if segments.len() != 5
        || segments[0] != "v2"
        || segments[1] != "catalog"
        || segments[2] != "runner"
        || segments[3].is_empty()
    {
        return None;
    }
    let claim_id = segments[3].to_string();
    match segments[4] {
        "heartbeat" => Some(RunnerRoute::Heartbeat { claim_id }),
        "complete" => Some(RunnerRoute::Complete { claim_id }),
        "fail" => Some(RunnerRoute::Fail { claim_id }),
        _ => None,
    }
}

fn response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({
            "error": code,
            "message": message,
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::{CLAIM_PATH, MAX_RUNNER_BODY_BYTES, PUBLISH_PATH, RunnerRoute, runner_route};

    #[test]
    fn routes_all_atomic_runner_transitions() {
        assert_eq!(runner_route(CLAIM_PATH), Some(RunnerRoute::Claim));
        assert_eq!(
            runner_route("/v2/catalog/runner/rvc_1/heartbeat"),
            Some(RunnerRoute::Heartbeat {
                claim_id: "rvc_1".to_string()
            })
        );
        assert_eq!(
            runner_route("/v2/catalog/runner/rvc_1/complete"),
            Some(RunnerRoute::Complete {
                claim_id: "rvc_1".to_string()
            })
        );
        assert_eq!(
            runner_route("/v2/catalog/runner/rvc_1/fail"),
            Some(RunnerRoute::Fail {
                claim_id: "rvc_1".to_string()
            })
        );
        assert_eq!(MAX_RUNNER_BODY_BYTES, 64 * 1024);
        assert_eq!(PUBLISH_PATH, "/v2/catalog/publish");
    }
}
