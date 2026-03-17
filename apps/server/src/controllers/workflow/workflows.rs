use axum::{
    extract::{Path, State},
    Json,
};
use loco_rs::app::AppContext;
use rustok_workflow::{
    CreateWorkflowInput, UpdateWorkflowInput, WorkflowResponse, WorkflowService, WorkflowSummary,
};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::context::TenantContext;
use crate::error::{Error, Result};
use crate::extractors::rbac::{
    RequireWorkflowsCreate, RequireWorkflowsDelete, RequireWorkflowsExecute,
    RequireWorkflowsList, RequireWorkflowsRead, RequireWorkflowsUpdate,
};

/// List all workflows for the current tenant
pub async fn list(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _auth: RequireWorkflowsList,
) -> Result<Json<Vec<WorkflowSummary>>> {
    let service = WorkflowService::new(ctx.db.clone());
    let workflows = service
        .list(tenant.id)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(workflows))
}

/// Get a single workflow with its steps
pub async fn get(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _auth: RequireWorkflowsRead,
    Path(id): Path<Uuid>,
) -> Result<Json<WorkflowResponse>> {
    let service = WorkflowService::new(ctx.db.clone());
    let workflow = service
        .get(tenant.id, id)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(workflow))
}

/// Create a new workflow (starts as Draft)
pub async fn create(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: RequireWorkflowsCreate,
    Json(input): Json<CreateWorkflowInput>,
) -> Result<Json<serde_json::Value>> {
    let service = WorkflowService::new(ctx.db.clone());
    let id = service
        .create(tenant.id, Some(auth.0.id), input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(serde_json::json!({ "id": id })))
}

/// Update a workflow's name / description / trigger_config / status
pub async fn update(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: RequireWorkflowsUpdate,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateWorkflowInput>,
) -> Result<Json<serde_json::Value>> {
    let service = WorkflowService::new(ctx.db.clone());
    service
        .update(tenant.id, id, Some(auth.0.id), input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Delete a workflow (cascade-deletes steps and executions)
pub async fn delete_workflow(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _auth: RequireWorkflowsDelete,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let service = WorkflowService::new(ctx.db.clone());
    service
        .delete(tenant.id, id)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Activate a workflow (set status = Active)
pub async fn activate(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _auth: RequireWorkflowsUpdate,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let service = WorkflowService::new(ctx.db.clone());
    service
        .update(
            tenant.id,
            id,
            UpdateWorkflowInput {
                status: Some(rustok_workflow::entities::WorkflowStatus::Active),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Pause a workflow (set status = Paused)
pub async fn pause(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _auth: RequireWorkflowsUpdate,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let service = WorkflowService::new(ctx.db.clone());
    service
        .update(
            tenant.id,
            id,
            UpdateWorkflowInput {
                status: Some(rustok_workflow::entities::WorkflowStatus::Paused),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct TriggerManualInput {
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub force: bool,
}

/// Manually trigger a workflow execution
pub async fn trigger_manual(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: RequireWorkflowsExecute,
    Path(id): Path<Uuid>,
    Json(input): Json<TriggerManualInput>,
) -> Result<Json<serde_json::Value>> {
    let service = WorkflowService::new(ctx.db.clone());
    let execution_id = service
        .trigger_manual(tenant.id, id, Some(auth.0.id), input.payload, input.force)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(serde_json::json!({ "execution_id": execution_id })))
}
