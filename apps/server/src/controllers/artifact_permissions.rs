//! Host transport for RBAC-owned artifact permission grants.

use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::Response,
    routing::put,
};
use rustok_api::{AuthContext, AuthPrincipalContext, Permission, has_effective_permission};
use rustok_events::RbacArtifactPermissionEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_rbac::{
    ArtifactPermissionAssignmentError, ArtifactPermissionAssignmentScope,
    ArtifactPermissionEventPublisher, ArtifactRolePermissionAssignmentCommand,
    RbacArtifactPermissionAssignmentService, RbacControlPlanePrincipal,
    require_direct_control_plane_user,
};
use rustok_web::json_response;
use sea_orm::DatabaseTransaction;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::{Error, Result, http_error},
    extractors::tenant::CurrentTenant,
    services::{
        event_bus::transactional_event_bus_from_context,
        server_runtime_context::ServerRuntimeContext,
    },
};

#[derive(Clone)]
struct TransactionalOutboxArtifactPermissionEventPublisher {
    event_bus: TransactionalEventBus,
}

impl TransactionalOutboxArtifactPermissionEventPublisher {
    fn new(event_bus: TransactionalEventBus) -> Self {
        Self { event_bus }
    }
}

#[async_trait]
impl ArtifactPermissionEventPublisher for TransactionalOutboxArtifactPermissionEventPublisher {
    async fn publish_assignment_changed(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        event: RbacArtifactPermissionEvent,
    ) -> std::result::Result<(), ArtifactPermissionAssignmentError> {
        self.event_bus
            .publish_contract_in_tx(transaction, tenant_id, Some(actor_id), event)
            .await
            .map_err(|error| ArtifactPermissionAssignmentError::Database(error.to_string()))
    }
}

/// Explicit admitted scope. Tenant scope always uses the authenticated routed tenant.
#[derive(Clone, Copy, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ArtifactPermissionAssignmentScopeRequest {
    Platform,
    Tenant,
}

impl From<ArtifactPermissionAssignmentScopeRequest> for ArtifactPermissionAssignmentScope {
    fn from(value: ArtifactPermissionAssignmentScopeRequest) -> Self {
        match value {
            ArtifactPermissionAssignmentScopeRequest::Platform => Self::Platform,
            ArtifactPermissionAssignmentScopeRequest::Tenant => Self::Tenant,
        }
    }
}

/// The transport input for one exact role-to-artifact-permission operation.
#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct ArtifactRolePermissionAssignmentRequest {
    pub scope: ArtifactPermissionAssignmentScopeRequest,
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
        (status = 404, description = "Role or permission in the requested explicit scope not found"),
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
        (status = 404, description = "Role or permission in the requested explicit scope not found"),
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
    let event_publisher = Arc::new(TransactionalOutboxArtifactPermissionEventPublisher::new(
        transactional_event_bus_from_context(ctx),
    ));
    let service = RbacArtifactPermissionAssignmentService::new(ctx.db_clone(), event_publisher);
    let result = service
        .assign(ArtifactRolePermissionAssignmentCommand {
            tenant_id,
            role_id,
            scope: input.scope.into(),
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
    use super::{
        ArtifactPermissionAssignmentScopeRequest,
        TransactionalOutboxArtifactPermissionEventPublisher,
        ensure_artifact_permission_control_plane,
    };
    use rustok_api::{AuthContext, AuthPrincipalContext, AuthPrincipalKind, Permission};
    use rustok_events::RbacArtifactPermissionEvent;
    use rustok_outbox::{OutboxTransport, SysEvents, SysEventsMigration, TransactionalEventBus};
    use rustok_rbac::{ArtifactPermissionAssignmentScope, ArtifactPermissionEventPublisher};
    use sea_orm::{Database, EntityTrait, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use std::sync::Arc;
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
    fn request_scope_maps_without_accepting_a_tenant_identifier() {
        assert_eq!(
            ArtifactPermissionAssignmentScope::from(
                ArtifactPermissionAssignmentScopeRequest::Platform,
            ),
            ArtifactPermissionAssignmentScope::Platform
        );
        assert_eq!(
            ArtifactPermissionAssignmentScope::from(
                ArtifactPermissionAssignmentScopeRequest::Tenant,
            ),
            ArtifactPermissionAssignmentScope::Tenant
        );
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

    #[tokio::test]
    async fn transactional_outbox_adapter_writes_typed_event() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite");
        SysEventsMigration
            .up(&SchemaManager::new(&db))
            .await
            .expect("create outbox table");

        let adapter = TransactionalOutboxArtifactPermissionEventPublisher::new(
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
        );
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let transaction = db.begin().await.expect("begin owner transaction");
        adapter
            .publish_assignment_changed(
                &transaction,
                tenant_id,
                actor_id,
                RbacArtifactPermissionEvent::AssignmentChanged {
                    operation_id: Uuid::new_v4(),
                    artifact_permission_id: Uuid::new_v4(),
                    role_id: Uuid::new_v4(),
                    installation_id: Uuid::new_v4(),
                    permission_key: "sample.events.handle".to_string(),
                    granted: true,
                },
            )
            .await
            .expect("publish typed event");
        transaction
            .commit()
            .await
            .expect("commit owner transaction");

        let events = SysEvents::find().all(&db).await.expect("load outbox");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].event_type,
            "rbac.artifact_role_permission.assignment_changed"
        );
        assert_eq!(events[0].schema_version, 1);
        assert_eq!(events[0].payload["tenant_id"], serde_json::json!(tenant_id));
        assert_eq!(events[0].payload["actor_id"], serde_json::json!(actor_id));
    }
}
