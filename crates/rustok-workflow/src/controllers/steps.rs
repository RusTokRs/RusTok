use axum::{
    Json,
    extract::{Path, State},
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{CreateWorkflowStepInput, UpdateWorkflowStepInput, WorkflowService};

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
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
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
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
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
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
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
