use axum::{
    Json,
    extract::{Path, State},
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    CreateWorkflowInput, UpdateWorkflowInput, WorkflowResponse, WorkflowService, WorkflowSummary,
    entities::WorkflowStatus,
};

pub async fn list(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
) -> HttpResult<Json<Vec<WorkflowSummary>>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_LIST],
        "Permission denied: workflows:list required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    let workflows = service
        .list(tenant.id)
        .await
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
    Ok(Json(workflows))
}

pub async fn get(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<WorkflowResponse>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_READ],
        "Permission denied: workflows:read required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    let workflow = service
        .get(tenant.id, id)
        .await
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
    Ok(Json(workflow))
}

pub async fn create(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateWorkflowInput>,
) -> HttpResult<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_CREATE],
        "Permission denied: workflows:create required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    let id = service
        .create(tenant.id, auth.human_user_id(), input)
        .await
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn update(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateWorkflowInput>,
) -> HttpResult<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_UPDATE],
        "Permission denied: workflows:update required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    service
        .update(tenant.id, id, auth.human_user_id(), input)
        .await
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn delete_workflow(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_DELETE],
        "Permission denied: workflows:delete required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    service
        .delete(tenant.id, id)
        .await
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn activate(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_UPDATE],
        "Permission denied: workflows:update required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    service
        .update(
            tenant.id,
            id,
            auth.human_user_id(),
            UpdateWorkflowInput {
                status: Some(WorkflowStatus::Active),
                ..Default::default()
            },
        )
        .await
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn pause(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_UPDATE],
        "Permission denied: workflows:update required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    service
        .update(
            tenant.id,
            id,
            auth.human_user_id(),
            UpdateWorkflowInput {
                status: Some(WorkflowStatus::Paused),
                ..Default::default()
            },
        )
        .await
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct TriggerManualInput {
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub force: bool,
}

pub async fn trigger_manual(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<TriggerManualInput>,
) -> HttpResult<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_EXECUTE],
        "Permission denied: workflows:execute required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    let execution_id = service
        .trigger_manual(
            tenant.id,
            id,
            auth.human_user_id(),
            input.payload,
            input.force,
        )
        .await
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;
    Ok(Json(serde_json::json!({ "execution_id": execution_id })))
}

fn ensure_workflow_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> HttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(HttpError::forbidden(
            "workflow_permission_denied",
            message.to_string(),
        ));
    }

    Ok(())
}
