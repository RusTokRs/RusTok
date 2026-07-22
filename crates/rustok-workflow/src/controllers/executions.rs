use axum::{
    Json,
    extract::{Path, State},
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{WorkflowExecutionResponse, WorkflowService};

pub async fn list_executions(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(workflow_id): Path<Uuid>,
) -> HttpResult<Json<Vec<WorkflowExecutionResponse>>> {
    ensure_execution_permission(
        &auth,
        &[Permission::WORKFLOW_EXECUTIONS_LIST],
        "Permission denied: workflow_executions:list required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    let executions = service
        .list_executions(tenant.id, workflow_id)
        .await
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
    Ok(Json(executions))
}

pub async fn get_execution(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(execution_id): Path<Uuid>,
) -> HttpResult<Json<WorkflowExecutionResponse>> {
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
            crate::WorkflowError::ExecutionNotFound(_) => HttpError::not_found(
                "workflow_execution_not_found",
                "Workflow execution not found",
            ),
            other => HttpError::bad_request("workflow_operation_failed", other.to_string()),
        })?;
    Ok(Json(execution))
}

fn ensure_execution_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> HttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(HttpError::unauthorized(
            "workflow_permission_denied",
            message.to_string(),
        ));
    }

    Ok(())
}
