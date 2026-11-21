use axum::{
    Json,
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustok_modules::{ModuleGovernanceError, ModuleGovernanceErrorCategory};

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
    if executor.shared_token.as_deref().is_none() {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            "configuration_error",
            "Remote executor shared token is not configured",
        );
    };
    if !super::registry_artifact_access::runner_token_is_valid(&ctx, &request) {
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
    let runner_id = match normalize_runner_id(&input.runner_id) {
        Ok(runner_id) => runner_id,
        Err(response) => return *response,
    };
    if input.supported_stages.is_empty() {
        return response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Runner claim requires at least one supported stage",
        );
    }

    let claim = match claim_remote_validation_stage_atomic(
        ctx.db(),
        &runner_id,
        &input.supported_stages,
        lease_ttl_ms,
    )
    .await
    {
        Ok(claim) => claim,
        Err(error) => return claim_error_response(error),
    };

    Json(RegistryRunnerClaimResponse {
        accepted: true,
        claim: claim.map(|claim| RegistryRunnerClaimPayload {
            claim_id: claim.claim_id,
            request_id: claim.request_id,
            request_revision: claim.request_revision,
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
    let runner_id = match normalize_runner_id(&input.runner_id) {
        Ok(runner_id) => runner_id,
        Err(response) => return *response,
    };

    match heartbeat_remote_validation_stage_atomic(ctx.db(), claim_id, &runner_id, lease_ttl_ms)
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
    let runner_id = match normalize_runner_id(&input.runner_id) {
        Ok(runner_id) => runner_id,
        Err(response) => return *response,
    };

    match finish_remote_validation_stage_atomic(
        ctx.db(),
        claim_id,
        &runner_id,
        input.expected_request_revision,
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

fn normalize_runner_id(runner_id: &str) -> RegistryClaimValidationResult<String> {
    let runner_id = runner_id.trim();
    if runner_id.is_empty() {
        return Err(Box::new(response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Runner request requires a non-empty runner_id",
        )));
    }
    Ok(runner_id.to_string())
}

fn claim_error_response(error: ModuleGovernanceError) -> Response {
    let category = error.category();
    let code = error.code();

    match category {
        ModuleGovernanceErrorCategory::InvalidInput => {
            tracing::warn!(error_code = code, "Rejected invalid registry runner claim");
            response(
                StatusCode::BAD_REQUEST,
                code,
                "Runner claim request is invalid",
            )
        }
        ModuleGovernanceErrorCategory::PermissionDenied => {
            tracing::warn!(error_code = code, "Registry runner claim was denied");
            response(
                StatusCode::FORBIDDEN,
                code,
                "Runner claim is not authorized",
            )
        }
        ModuleGovernanceErrorCategory::NotFound => response(
            StatusCode::NOT_FOUND,
            code,
            "Registry runner claim target was not found",
        ),
        ModuleGovernanceErrorCategory::Conflict => response(
            StatusCode::CONFLICT,
            code,
            "Registry runner claim conflicts with the current state",
        ),
        ModuleGovernanceErrorCategory::Internal => {
            tracing::error!(error_code = code, "Atomic registry runner claim failed");
            response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to claim registry validation stage",
            )
        }
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
    let RegistryRemoteTransitionError {
        category,
        code,
        detail,
    } = error;
    match category {
        ModuleGovernanceErrorCategory::InvalidInput => {
            response(StatusCode::BAD_REQUEST, code, &detail)
        }
        ModuleGovernanceErrorCategory::PermissionDenied => {
            response(StatusCode::FORBIDDEN, code, &detail)
        }
        ModuleGovernanceErrorCategory::NotFound => {
            response(StatusCode::NOT_FOUND, code, "Not found")
        }
        ModuleGovernanceErrorCategory::Conflict => response(StatusCode::CONFLICT, code, &detail),
        ModuleGovernanceErrorCategory::Internal => {
            tracing::error!(error = %detail, "Atomic registry runner transition failed");
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
    let segments = path.split('/').collect::<Vec<_>>();
    if segments.len() != 6
        || !segments[0].is_empty()
        || segments[1] != "v2"
        || segments[2] != "catalog"
        || segments[3] != "runner"
        || segments[4].is_empty()
    {
        return None;
    }
    let claim_id = segments[4].to_string();
    match segments[5] {
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
    use axum::{
        body::to_bytes,
        http::StatusCode,
    };
    use rustok_modules::ModuleGovernanceError;

    use super::{
        CLAIM_PATH, MAX_RUNNER_BODY_BYTES, PUBLISH_PATH, RunnerRoute, claim_error_response,
        runner_route,
    };

    #[tokio::test]
    async fn invalid_claim_stage_is_a_redacted_bad_request() {
        let untrusted_stage = "unsupported-stage-".repeat(256);
        let response = claim_error_response(
            ModuleGovernanceError::InvalidRemoteValidationClaimStage(untrusted_stage.clone()),
        );

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 1024)
            .await
            .expect("claim error response should fit its bounded envelope");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("claim error response should be valid JSON");
        assert_eq!(payload["error"], "module_governance_invalid_input");
        assert_eq!(payload["message"], "Runner claim request is invalid");
        assert_ne!(payload["message"].as_str(), Some(untrusted_stage.as_str()));
    }

    #[tokio::test]
    async fn internal_claim_failure_does_not_expose_storage_details() {
        let response = super::claim_error_response(ModuleGovernanceError::Store(
            "database password and SQL detail".to_string(),
        ));

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = to_bytes(response.into_body(), 1024)
            .await
            .expect("claim error response should fit its bounded envelope");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("claim error response should be valid JSON");
        assert_eq!(payload["error"], "internal_error");
        assert_eq!(
            payload["message"],
            "Failed to claim registry validation stage"
        );
    }

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
        assert_eq!(runner_route("/v2/catalog/runner/rvc_1/heartbeat/"), None);
        assert_eq!(runner_route("//v2/catalog/runner/rvc_1/heartbeat"), None);
    }

    #[test]
    fn runner_id_normalization_matches_controller_contract() {
        let normalized =
            super::normalize_runner_id("  runner-1  ").expect("runner id should be normalized");
        assert_eq!(normalized, "runner-1");
        assert!(super::normalize_runner_id("   ").is_err());
    }
}
