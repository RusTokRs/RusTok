use axum::{
    extract::{Path, State},
    Json,
};
use loco_rs::{Error, Result};
use rustok_api::Permission;
use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};
use uuid::Uuid;

use crate::{WorkflowExecutionResponse, WorkflowService};

pub async fn list_executions(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(workflow_id): Path<Uuid>,
) -> Result<Json<Vec<WorkflowExecutionResponse>>> {
    ensure_execution_permission(
        &auth,
        &[Permission::WORKFLOW_EXECUTIONS_LIST],
        "Permission denied: workflow_executions:list required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    let executions = service
        .list_executions(tenant.id, workflow_id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(executions))
}

pub async fn get_execution(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(execution_id): Path<Uuid>,
) -> Result<Json<WorkflowExecutionResponse>> {
    ensure_execution_permission(
        &auth,
        &[Permission::WORKFLOW_EXECUTIONS_READ],
        "Permission denied: workflow_executions:read required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    let execution = service
        .get_execution(tenant.id, execution_id)
        .await
        .map_err(|err| match err {
            crate::WorkflowError::ExecutionNotFound(_) => Error::NotFound,
            other => Error::BadRequest(other.to_string()),
        })?;
    Ok(Json(execution))
}

fn ensure_execution_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> Result<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(Error::Unauthorized(message.to_string()));
    }

    Ok(())
}
