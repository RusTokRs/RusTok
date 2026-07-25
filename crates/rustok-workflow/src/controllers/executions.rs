use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{WorkflowError, WorkflowExecutionResponse, WorkflowService};

fn map_workflow_execution_error(
    error: WorkflowError,
    operation: &'static str,
    tenant_id: Uuid,
    workflow_id: Option<Uuid>,
    execution_id: Option<Uuid>,
) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        WorkflowError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            "workflow_not_found",
            "Workflow was not found",
            "workflow_not_found",
        ),
        WorkflowError::StepNotFound(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "workflow_execution_failed",
            "Workflow execution could not be loaded safely",
            "unexpected_step_not_found",
        ),
        WorkflowError::ExecutionNotFound(_) => (
            StatusCode::NOT_FOUND,
            "workflow_execution_not_found",
            "Workflow execution was not found",
            "execution_not_found",
        ),
        WorkflowError::NotActive(_) => (
            StatusCode::CONFLICT,
            "workflow_state_conflict",
            "Workflow operation conflicts with the current state",
            "state_conflict",
        ),
        WorkflowError::UnknownStepType(_)
        | WorkflowError::InvalidTriggerConfig(_)
        | WorkflowError::InvalidStepConfig(_) => (
            StatusCode::BAD_REQUEST,
            "workflow_execution_invalid",
            "Workflow execution request is invalid",
            "validation",
        ),
        WorkflowError::StepFailed(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "workflow_execution_failed",
            "Workflow execution could not be loaded safely",
            "step_failed",
        ),
        WorkflowError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "workflow_storage_unavailable",
            "Workflow storage is temporarily unavailable",
            "database",
        ),
        WorkflowError::Serialization(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "workflow_execution_failed",
            "Workflow execution could not be loaded safely",
            "serialization",
        ),
    };
    tracing::error!(
        error = ?error,
        owner = "rustok_workflow.workflow_service",
        operation,
        tenant_id = %tenant_id,
        workflow_id = ?workflow_id,
        execution_id = ?execution_id,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "workflow_execution_http",
        "workflow execution operation failed"
    );
    HttpError::new(status, code, message)
}

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
        .map_err(|error| {
            map_workflow_execution_error(
                error,
                "list_executions",
                tenant.id,
                Some(workflow_id),
                None,
            )
        })?;
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
        .map_err(|error| {
            map_workflow_execution_error(
                error,
                "get_execution",
                tenant.id,
                None,
                Some(execution_id),
            )
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
