use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
};
use rustok_web::{HttpError, HttpResult};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::Value;
use tracing::{info, warn};

use crate::WorkflowService;

#[derive(serde::Serialize)]
pub struct WebhookResponse {
    pub executions: Vec<uuid::Uuid>,
}

pub async fn receive(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    Path((tenant_slug, webhook_slug)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> HttpResult<Json<WebhookResponse>> {
    let db = runtime.db_clone();
    let tenant = rustok_tenant::entities::tenant::Entity::find()
        .filter(rustok_tenant::entities::tenant::Column::Slug.eq(&tenant_slug))
        .one(&db)
        .await
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?
        .ok_or_else(|| {
            HttpError::bad_request(
                "workflow_operation_failed",
                format!("Tenant not found: {tenant_slug}"),
            )
        })?;

    let payload: Value = serde_json::from_slice(&body)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into_owned()));

    if let Some(signature) = headers.get("x-webhook-signature") {
        info!(
            tenant_slug = %tenant_slug,
            webhook_slug = %webhook_slug,
            signature = ?signature,
            "Workflow webhook received with signature"
        );
    } else {
        warn!(
            tenant_slug = %tenant_slug,
            webhook_slug = %webhook_slug,
            "Workflow webhook received without X-Webhook-Signature"
        );
    }

    let service = WorkflowService::new(db);
    let executions = service
        .trigger_by_webhook(tenant.id, &webhook_slug, payload)
        .await
        .map_err(|err| HttpError::bad_request("workflow_operation_failed", err.to_string()))?;

    info!(
        tenant_slug = %tenant_slug,
        webhook_slug = %webhook_slug,
        executions = executions.len(),
        "Workflow webhook triggered execution(s)"
    );

    Ok(Json(WebhookResponse { executions }))
}
