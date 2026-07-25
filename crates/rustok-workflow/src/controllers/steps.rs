use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{
    CreateWorkflowStepInput, UpdateWorkflowStepInput, WorkflowError, WorkflowService,
};

fn map_workflow_step_error(
    error: WorkflowError,
    operation: &'static str,
    tenant_id: Uuid,
    workflow_id: Uuid,
    step_id: Option<Uuid>,
) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        WorkflowError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            "workflow_not_found",
            "Workflow was not found",
            "workflow_not_found",
        ),
        WorkflowError::StepNotFound(_) => (
            StatusCode::NOT_FOUND,
            "workflow_step_not_found",
            "Workflow step was not found",
            "step_not_found",
        ),
        WorkflowError::ExecutionNotFound(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "workflow_step_failed",
            "Workflow step operation could not be completed safely",
            "unexpected_execution_not_found",
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
            "workflow_step_invalid",
            "Workflow step request is invalid",
            "validation",
        ),
        WorkflowError::StepFailed(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "workflow_step_failed",
            "Workflow step operation could not be completed safely",
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
            "workflow_step_failed",
            "Workflow step operation could not be completed safely",
            "serialization",
        ),
    };
    tracing::error!(
        error = ?error,
        owner = "rustok_workflow.workflow_service",
        operation,
        tenant_id = %tenant_id,
        workflow_id = %workflow_id,
        step_id = ?step_id,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "workflow_step_http",
        "workflow step operation failed"
    );
    HttpError::new(status, code, message)
}

pub async fn add_step(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateWorkflowStepInput>,
) -> HttpResult<Json<serde_json::Value>> {
    ensure_workflow_permission(&auth)?;

    let service = WorkflowService::new(runtime.db_clone());
    let step_id = service
        .add_step(tenant.id, id, input)
        .await
        .map_err(|error| map_workflow_step_error(error, "add_step", tenant.id, id, None))?;
    Ok(Json(serde_json::json!({ "id": step_id })))
}

pub async fn update_step(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path((id, step_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateWorkflowStepInput>,
) -> HttpResult<Json<serde_json::Value>> {
    ensure_workflow_permission(&auth)?;

    let service = WorkflowService::new(runtime.db_clone());
    service
        .update_step(tenant.id, id, step_id, input)
        .await
        .map_err(|error| {
            map_workflow_step_error(error, "update_step", tenant.id, id, Some(step_id))
        })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn delete_step(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path((id, step_id)): Path<(Uuid, Uuid)>,
) -> HttpResult<Json<serde_json::Value>> {
    ensure_workflow_permission(&auth)?;

    let service = WorkflowService::new(runtime.db_clone());
    service
        .delete_step(tenant.id, id, step_id)
        .await
        .map_err(|error| {
            map_workflow_step_error(error, "delete_step", tenant.id, id, Some(step_id))
        })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn ensure_workflow_permission(auth: &AuthContext) -> HttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, &[Permission::WORKFLOWS_UPDATE]) {
        return Err(HttpError::unauthorized(
            "workflow_permission_denied",
            "Permission denied: workflows:update required".to_string(),
        ));
    }

    Ok(())
}
