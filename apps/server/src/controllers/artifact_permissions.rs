//! Host transport for RBAC-owned artifact permission grants.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::Response,
    routing::put,
};
use rustok_api::{Permission, has_effective_permission};
use rustok_rbac::{
    ArtifactPermissionAssignmentError, ArtifactRolePermissionAssignmentCommand,
    RbacArtifactPermissionAssignmentService,
};
use rustok_web::json_response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::{Error, Result, http_error},
    extractors::{auth::CurrentUser, tenant::CurrentTenant},
    services::server_runtime_context::ServerRuntimeContext,
};

/// The transport input for one exact role-to-artifact-permission operation.
#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct ArtifactRolePermissionAssignmentRequest {
    pub installation_id: Uuid,
    pub permission_key: String,
    pub idempotency_key: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ArtifactRolePermissionAssignmentResponse {
    pub applied: bool,
}

#[utoipa::path(
    put,
    path = "/api/rbac/artifact-permissions/roles/{role_id}",
    tag = "rbac",
    security(("bearer_auth" = [])),
    params(("role_id" = Uuid, Path, description = "Tenant role identifier")),
    request_body = ArtifactRolePermissionAssignmentRequest,
    responses(
        (status = 200, description = "Artifact permission granted", body = ArtifactRolePermissionAssignmentResponse),
        (status = 400, description = "Invalid command"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "modules:manage permission required"),
        (status = 404, description = "Role or registered artifact permission not found"),
        (status = 409, description = "Idempotency command conflict")
    )
)]
async fn grant_artifact_permission(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(role_id): Path<Uuid>,
    Json(input): Json<ArtifactRolePermissionAssignmentRequest>,
) -> Result<Response> {
    ensure_modules_manage(&current)?;
    assign(&ctx, tenant.id, current.user.id, role_id, input, true).await
}

#[utoipa::path(
    delete,
    path = "/api/rbac/artifact-permissions/roles/{role_id}",
    tag = "rbac",
    security(("bearer_auth" = [])),
    params(("role_id" = Uuid, Path, description = "Tenant role identifier")),
    request_body = ArtifactRolePermissionAssignmentRequest,
    responses(
        (status = 200, description = "Artifact permission revoked", body = ArtifactRolePermissionAssignmentResponse),
        (status = 400, description = "Invalid command"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "modules:manage permission required"),
        (status = 404, description = "Role or registered artifact permission not found"),
        (status = 409, description = "Idempotency command conflict")
    )
)]
async fn revoke_artifact_permission(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(role_id): Path<Uuid>,
    Json(input): Json<ArtifactRolePermissionAssignmentRequest>,
) -> Result<Response> {
    ensure_modules_manage(&current)?;
    assign(&ctx, tenant.id, current.user.id, role_id, input, false).await
}

async fn assign(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    actor_id: Uuid,
    role_id: Uuid,
    input: ArtifactRolePermissionAssignmentRequest,
    granted: bool,
) -> Result<Response> {
    let service = RbacArtifactPermissionAssignmentService::new(ctx.db_clone());
    let result = service
        .assign(ArtifactRolePermissionAssignmentCommand {
            tenant_id,
            role_id,
            installation_id: input.installation_id,
            permission_key: input.permission_key,
            actor_id,
            granted,
            idempotency_key: input.idempotency_key,
        })
        .await
        .map_err(map_assignment_error)?;
    Ok(json_response(ArtifactRolePermissionAssignmentResponse {
        applied: result.applied,
    }))
}

fn ensure_modules_manage(current: &CurrentUser) -> Result<()> {
    if has_effective_permission(&current.permissions, &Permission::MODULES_MANAGE) {
        return Ok(());
    }
    Err(http_error(rustok_web::HttpError::forbidden(
        "forbidden",
        "Permission denied: modules:manage required",
    )))
}

fn map_assignment_error(error: ArtifactPermissionAssignmentError) -> Error {
    match error {
        ArtifactPermissionAssignmentError::InvalidCommand(message) => {
            Error::BadRequest(message.to_string())
        }
        ArtifactPermissionAssignmentError::RoleNotFound
        | ArtifactPermissionAssignmentError::PermissionNotRegistered => Error::NotFound,
        ArtifactPermissionAssignmentError::IdempotencyConflict => {
            http_error(rustok_web::HttpError::new(
                StatusCode::CONFLICT,
                "idempotency_conflict",
                "Idempotency key was already used for a different artifact permission command",
            ))
        }
        ArtifactPermissionAssignmentError::Database(error) => {
            tracing::error!(%error, "artifact permission assignment failed");
            Error::InternalServerError
        }
    }
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new().route(
        "/api/rbac/artifact-permissions/roles/{role_id}",
        put(grant_artifact_permission).delete(revoke_artifact_permission),
    )
}
