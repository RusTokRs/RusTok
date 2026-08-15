use axum::{
    body::{Body, to_bytes},
    extract::State,
    http::{
        Method, Request, StatusCode,
        header::{self, HeaderValue},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustok_api::AuthContextExtension;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use subtle::ConstantTimeEq;

use crate::models::users;
use crate::services::marketplace_catalog::RegistryOwnerTransferRequest;
use crate::services::registry_governance::RegistryGovernanceService;
use crate::services::registry_principal::RegistryAuthority;
use crate::services::server_runtime_context::ServerRuntimeContext;

const ARTIFACT_CONTENT_TYPE: &str = "application/octet-stream";
const MAX_REGISTRY_MUTATION_BODY_BYTES: usize = 64 * 1024;

/// Enforce registry publish-request, artifact, ownership and remote-runner
/// access before the controller executes.
///
/// Every request-specific read and dry-run is authorized from the exact owner
/// projection before resolver/body business logic. Remote runner routes use the
/// host shared token with constant-time comparison.
pub async fn enforce(
    State(ctx): State<ServerRuntimeContext>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path();

    if method == Method::POST && path == "/v2/catalog/owner-transfer" {
        return validate_owner_transfer(&ctx, request, next).await;
    }

    if method == Method::POST && is_remote_runner_path(path) {
        return if runner_token_is_valid(&ctx, &request) {
            next.run(request).await
        } else {
            unauthorized("Missing or invalid registry runner token")
        };
    }

    let Some((request_id, operation)) = registry_route(path) else {
        return next.run(request).await;
    };
    if !operation.accepts_method(&method) {
        return next.run(request).await;
    }

    // Copy only the authentication context before awaiting authorization I/O.
    // Borrowing `Request<Body>` across an await makes this Axum middleware future
    // non-Send because request bodies are not Sync.
    let auth = request.extensions().get::<AuthContextExtension>().cloned();

    match operation {
        RegistryOperation::PublishStatus => {
            if let Err(response) =
                authorize_user_access(&ctx, auth.as_ref(), request_id, PublishAccess::Manage).await
            {
                return response;
            }
            next.run(request).await
        }
        RegistryOperation::ArtifactUpload => {
            if let Err(response) =
                authorize_user_access(&ctx, auth.as_ref(), request_id, PublishAccess::Manage).await
            {
                return response;
            }
            request.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(ARTIFACT_CONTENT_TYPE),
            );
            next.run(request).await
        }
        RegistryOperation::ArtifactDownload => {
            if !runner_token_is_valid(&ctx, &request)
                && let Err(response) =
                    authorize_user_access(&ctx, auth.as_ref(), request_id, PublishAccess::Manage)
                        .await
            {
                return response;
            }
            next.run(request).await
        }
        RegistryOperation::ManageMutation => {
            if let Err(response) =
                authorize_user_access(&ctx, auth.as_ref(), request_id, PublishAccess::Manage).await
            {
                return response;
            }
            next.run(request).await
        }
        RegistryOperation::ReviewMutation => {
            if let Err(response) =
                authorize_user_access(&ctx, auth.as_ref(), request_id, PublishAccess::Review).await
            {
                return response;
            }
            next.run(request).await
        }
    }
}

async fn validate_owner_transfer(
    ctx: &ServerRuntimeContext,
    request: Request<Body>,
    next: Next,
) -> Response {
    let (parts, body) = request.into_parts();
    let auth = match parts.extensions.get::<AuthContextExtension>() {
        Some(extension) => &extension.0,
        None => return unauthorized("Registry owner transfer requires authentication"),
    };
    if auth.client_id.is_some() && auth.session_id.is_nil() {
        return forbidden("Registry owner transfer requires a user session");
    }

    let bytes = match to_bytes(body, MAX_REGISTRY_MUTATION_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => return bad_request("Registry owner transfer body is invalid or too large"),
    };
    let transfer = match serde_json::from_slice::<RegistryOwnerTransferRequest>(&bytes) {
        Ok(transfer) => transfer,
        Err(_) => return bad_request("Registry owner transfer body must be valid JSON"),
    };

    let target = match users::Entity::find_by_id(transfer.new_owner_user_id)
        .filter(users::Column::TenantId.eq(auth.tenant_id))
        .one(ctx.db())
        .await
    {
        Ok(Some(user)) if user.is_active() => user,
        Ok(Some(_)) => return bad_request("Registry owner transfer target must be active"),
        Ok(None) => {
            return bad_request(
                "Registry owner transfer target does not exist in the authenticated tenant",
            );
        }
        Err(error) => {
            tracing::error!(%error, "Failed to validate registry owner transfer target");
            return internal_error("Failed to validate registry owner transfer target");
        }
    };

    tracing::debug!(
        target_user_id = %target.id,
        tenant_id = %target.tenant_id,
        dry_run = transfer.dry_run,
        "Validated registry owner transfer target"
    );
    next.run(Request::from_parts(parts, Body::from(bytes)))
        .await
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublishAccess {
    Manage,
    Review,
}

async fn authorize_user_access(
    ctx: &ServerRuntimeContext,
    auth_extension: Option<&AuthContextExtension>,
    request_id: &str,
    access: PublishAccess,
) -> Result<(), Response> {
    let auth_extension = auth_extension
        .ok_or_else(|| unauthorized("Registry publish request access requires authentication"))?;
    let auth = &auth_extension.0;
    if auth.client_id.is_some() && auth.session_id.is_nil() {
        return Err(forbidden(
            "Registry publish request access requires a user session",
        ));
    }

    let authority = RegistryAuthority::from_auth(auth_extension);
    let snapshot = RegistryGovernanceService::new(ctx.db_clone())
        .publish_request_status_snapshot_for_authority(request_id, Some(&authority))
        .await
        .map_err(|error| {
            tracing::error!(%error, request_id, "Failed to load owner publish-request authorization snapshot");
            internal_error("Failed to authorize registry publish request access")
        })?
        .ok_or_else(|| not_found("Registry publish request was not found"))?;
    let allowed = match access {
        PublishAccess::Manage => snapshot.authorization.can_manage,
        PublishAccess::Review => snapshot.authorization.can_review,
    };

    if allowed {
        Ok(())
    } else {
        Err(forbidden(match access {
            PublishAccess::Manage => {
                "Registry publish request access is restricted to the module owner, request publisher, or modules:manage"
            }
            PublishAccess::Review => {
                "Registry review operations are restricted to the module owner or modules:manage"
            }
        }))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistryOperation {
    PublishStatus,
    ArtifactUpload,
    ArtifactDownload,
    ManageMutation,
    ReviewMutation,
}

impl RegistryOperation {
    fn accepts_method(self, method: &Method) -> bool {
        match self {
            Self::PublishStatus | Self::ArtifactDownload => method == Method::GET,
            Self::ArtifactUpload => method == Method::PUT,
            Self::ManageMutation | Self::ReviewMutation => method == Method::POST,
        }
    }
}

fn registry_route(path: &str) -> Option<(&str, RegistryOperation)> {
    let segments = path.trim_matches('/').split('/').collect::<Vec<_>>();
    if segments.len() == 4
        && segments[0] == "v2"
        && segments[1] == "catalog"
        && segments[2] == "publish"
    {
        return Some((segments[3], RegistryOperation::PublishStatus));
    }
    if segments.len() == 5
        && segments[0] == "v2"
        && segments[1] == "catalog"
        && segments[2] == "publish"
    {
        let operation = match segments[4] {
            "artifact" => RegistryOperation::ArtifactUpload,
            "validate" => RegistryOperation::ManageMutation,
            "stages" | "approve" | "reject" | "request-changes" | "hold" | "resume" => {
                RegistryOperation::ReviewMutation
            }
            _ => return None,
        };
        return Some((segments[3], operation));
    }
    if segments.len() == 6
        && segments[0] == "v2"
        && segments[1] == "catalog"
        && segments[2] == "publish"
        && segments[4] == "artifact"
        && segments[5] == "download"
    {
        return Some((segments[3], RegistryOperation::ArtifactDownload));
    }
    None
}

fn is_remote_runner_path(path: &str) -> bool {
    let segments = path.trim_matches('/').split('/').collect::<Vec<_>>();
    if segments.as_slice() == ["v2", "catalog", "runner", "claim"] {
        return true;
    }
    segments.len() == 5
        && segments[0] == "v2"
        && segments[1] == "catalog"
        && segments[2] == "runner"
        && !segments[3].is_empty()
        && matches!(segments[4], "heartbeat" | "complete" | "fail")
}

fn runner_token_is_valid(ctx: &ServerRuntimeContext, request: &Request<Body>) -> bool {
    let settings = &ctx.settings().registry.remote_executor;
    if !settings.enabled {
        return false;
    }
    let Some(expected) = settings.shared_token.as_deref() else {
        return false;
    };
    let Some(provided) = request
        .headers()
        .get("x-rustok-runner-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    bool::from(provided.as_bytes().ct_eq(expected.as_bytes()))
}

fn response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        axum::Json(serde_json::json!({
            "error": code,
            "message": message,
        })),
    )
        .into_response()
}

fn bad_request(message: &str) -> Response {
    response(StatusCode::BAD_REQUEST, "bad_request", message)
}

fn unauthorized(message: &str) -> Response {
    response(StatusCode::UNAUTHORIZED, "unauthorized", message)
}

fn forbidden(message: &str) -> Response {
    response(StatusCode::FORBIDDEN, "forbidden", message)
}

fn not_found(message: &str) -> Response {
    response(StatusCode::NOT_FOUND, "not_found", message)
}

fn internal_error(message: &str) -> Response {
    response(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
}

#[cfg(test)]
mod tests {
    use super::{RegistryOperation, is_remote_runner_path, registry_route};

    #[test]
    fn classifies_sensitive_registry_publish_routes() {
        assert_eq!(
            registry_route("/v2/catalog/publish/rpr_1"),
            Some(("rpr_1", RegistryOperation::PublishStatus))
        );
        assert_eq!(
            registry_route("/v2/catalog/publish/rpr_1/artifact"),
            Some(("rpr_1", RegistryOperation::ArtifactUpload))
        );
        assert_eq!(
            registry_route("/v2/catalog/publish/rpr_1/artifact/download"),
            Some(("rpr_1", RegistryOperation::ArtifactDownload))
        );
        assert_eq!(
            registry_route("/v2/catalog/publish/rpr_1/validate"),
            Some(("rpr_1", RegistryOperation::ManageMutation))
        );
        for action in [
            "stages",
            "approve",
            "reject",
            "request-changes",
            "hold",
            "resume",
        ] {
            assert_eq!(
                registry_route(&format!("/v2/catalog/publish/rpr_1/{action}")),
                Some(("rpr_1", RegistryOperation::ReviewMutation))
            );
        }
        assert_eq!(registry_route("/catalog/rpr_1/artifact"), None);
        assert_eq!(registry_route("/v2/catalog/yank"), None);
    }

    #[test]
    fn matches_only_remote_runner_mutation_routes() {
        assert!(is_remote_runner_path("/v2/catalog/runner/claim"));
        assert!(is_remote_runner_path(
            "/v2/catalog/runner/claim_1/heartbeat"
        ));
        assert!(is_remote_runner_path("/v2/catalog/runner/claim_1/complete"));
        assert!(is_remote_runner_path("/v2/catalog/runner/claim_1/fail"));
        assert!(!is_remote_runner_path("/v2/catalog/runner/claim_1"));
        assert!(!is_remote_runner_path("/v2/catalog/publish/rpr_1"));
    }
}
