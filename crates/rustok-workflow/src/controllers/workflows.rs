use axum::{
    extract::{Path, State},
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::Permission;
use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    entities::WorkflowStatus, CreateWorkflowInput, UpdateWorkflowInput, WorkflowResponse,
    WorkflowService, WorkflowSummary,
};

pub async fn list(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
) -> Result<Json<Vec<WorkflowSummary>>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_LIST],
        "Permission denied: workflows:list required",
    )?;

    let service = WorkflowService::new(ctx.db.clone());
    let workflows = service
        .list(tenant.id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(workflows))
}

pub async fn get(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<WorkflowResponse>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_READ],
        "Permission denied: workflows:read required",
    )?;

    let service = WorkflowService::new(ctx.db.clone());
    let workflow = service
        .get(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(workflow))
}

pub async fn create(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateWorkflowInput>,
) -> Result<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_CREATE],
        "Permission denied: workflows:create required",
    )?;

    let service = WorkflowService::new(ctx.db.clone());
    let id = service
        .create(tenant.id, Some(auth.user_id), input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn update(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateWorkflowInput>,
) -> Result<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_UPDATE],
        "Permission denied: workflows:update required",
    )?;

    let service = WorkflowService::new(ctx.db.clone());
    service
        .update(tenant.id, id, Some(auth.user_id), input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn delete_workflow(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_DELETE],
        "Permission denied: workflows:delete required",
    )?;

    let service = WorkflowService::new(ctx.db.clone());
    service
        .delete(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn activate(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_UPDATE],
        "Permission denied: workflows:update required",
    )?;

    let service = WorkflowService::new(ctx.db.clone());
    service
        .update(
            tenant.id,
            id,
            Some(auth.user_id),
            UpdateWorkflowInput {
                status: Some(WorkflowStatus::Active),
                ..Default::default()
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn pause(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_UPDATE],
        "Permission denied: workflows:update required",
    )?;

    let service = WorkflowService::new(ctx.db.clone());
    service
        .update(
            tenant.id,
            id,
            Some(auth.user_id),
            UpdateWorkflowInput {
                status: Some(WorkflowStatus::Paused),
                ..Default::default()
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<TriggerManualInput>,
) -> Result<Json<serde_json::Value>> {
    ensure_workflow_permission(
        &auth,
        &[Permission::WORKFLOWS_EXECUTE],
        "Permission denied: workflows:execute required",
    )?;

    let service = WorkflowService::new(ctx.db.clone());
    let execution_id = service
        .trigger_manual(
            tenant.id,
            id,
            Some(auth.user_id),
            input.payload,
            input.force,
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(serde_json::json!({ "execution_id": execution_id })))
}

fn ensure_workflow_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> Result<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(Error::Unauthorized(message.to_string()));
    }

    Ok(())
}
