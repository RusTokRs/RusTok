//! Host transport for RBAC-owned artifact permission grants.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::Response,
    routing::put,
};
use rustok_api::{AuthContext, AuthPrincipalContext, Permission, has_effective_permission};
use rustok_rbac::{
    ArtifactPermissionAssignmentError, ArtifactRolePermissionAssignmentCommand,
    RbacArtifactPermissionAssignmentService, RbacControlPlanePrincipal,
    require_direct_control_plane_user,
};
use rustok_web::json_response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::{Error, Result, http_error},
    extractors::tenant::CurrentTenant,
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
        (status = 403, description = "Direct session user with modules:manage required"),
        (status = 404, description = "Role or registered artifact permission not found"),
        (status = 409, description = "Idempotency command conflict")
    )
)]
async fn grant_artifact_permission(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    auth: AuthContext,
    principal_context: AuthPrincipalContext,
    Path(role_id): Path<Uuid>,
    Json(input): Json<ArtifactRolePermissionAssignmentRequest>,
) -> Result<Response> {
    ensure_artifact_permission_control_plane(&auth, principal_context, tenant.id)?;
    assign(&ctx, tenant.id, auth.user_id, role_id, input, true).await
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
        (status = 403, description = "Direct session user with modules:manage required"),
        (status = 404, description = "Role or registered artifact permission not found"),
        (status = 409, description = "Idempotency command conflict")
    )
)]
async fn revoke_artifact_permission(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    auth: AuthContext,
    principal_context: AuthPrincipalContext,
    Path(role_id): Path<Uuid>,
    Json(input): Json<ArtifactRolePermissionAssignmentRequest>,
) -> Result<Response> {
    ensure_artifact_permission_control_plane(&auth, principal_context, tenant.id)?;
    assign(&ctx, tenant.id, auth.user_id, role_id, input, false).await
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

fn ensure_artifact_permission_control_plane(
    auth: &AuthContext,
    principal_context: AuthPrincipalContext,
    tenant_id: Uuid,
) -> Result<()> {
    let principal = RbacControlPlanePrincipal {
        tenant_id: auth.tenant_id,
        principal_kind: principal_context.kind,
    };
    require_direct_control_plane_user(principal, tenant_id).map_err(|error| {
        http_error(rustok_web::HttpError::forbidden(
            "forbidden",
            error.to_string(),
        ))
    })?;
    ensure_modules_manage(&auth.permissions)
}

fn ensure_modules_manage(permissions: &[Permission]) -> Result<()> {
    if has_effective_permission(permissions, &Permission::MODULES_MANAGE) {
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

#[cfg(test)]
mod tests {
    use super::ensure_artifact_permission_control_plane;
    use rustok_api::{AuthContext, AuthPrincipalContext, AuthPrincipalKind, Permission};
    use uuid::Uuid;

    fn auth_context(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    #[test]
    fn direct_session_manager_is_admitted() {
        let tenant_id = Uuid::new_v4();
        let auth = auth_context(tenant_id, vec![Permission::MODULES_MANAGE]);

        assert!(
            ensure_artifact_permission_control_plane(
                &auth,
                AuthPrincipalContext::new(AuthPrincipalKind::DirectUser),
                tenant_id,
            )
            .is_ok()
        );
    }

    #[test]
    fn delegated_and_service_principals_are_denied_even_with_modules_manage() {
        for principal_kind in [AuthPrincipalKind::DelegatedUser, AuthPrincipalKind::Service] {
            let tenant_id = Uuid::new_v4();
            let auth = auth_context(tenant_id, vec![Permission::MODULES_MANAGE]);

            assert!(
                ensure_artifact_permission_control_plane(
                    &auth,
                    AuthPrincipalContext::new(principal_kind),
                    tenant_id,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn direct_user_from_another_tenant_is_denied() {
        let auth = auth_context(Uuid::new_v4(), vec![Permission::MODULES_MANAGE]);

        assert!(
            ensure_artifact_permission_control_plane(
                &auth,
                AuthPrincipalContext::new(AuthPrincipalKind::DirectUser),
                Uuid::new_v4(),
            )
            .is_err()
        );
    }

    #[test]
    fn direct_user_without_modules_manage_is_denied() {
        let tenant_id = Uuid::new_v4();
        let auth = auth_context(tenant_id, vec![Permission::MODULES_READ]);

        assert!(
            ensure_artifact_permission_control_plane(
                &auth,
                AuthPrincipalContext::new(AuthPrincipalKind::DirectUser),
                tenant_id,
            )
            .is_err()
        );
    }
}
