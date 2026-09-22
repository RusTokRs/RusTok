use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
};
use rustok_web::{HttpError, HttpResult};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tracing::info;

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

    let signature = headers
        .get("x-webhook-signature")
        .and_then(|value| value.to_str().ok());

    let service = WorkflowService::new(db);
    let executions = service
        .trigger_by_webhook(tenant.id, &webhook_slug, &body, signature)
        .await
        .map_err(|err| match err {
            crate::WorkflowError::WebhookSignatureMissing
            | crate::WorkflowError::WebhookSignatureInvalid
            | crate::WorkflowError::WebhookSecretNotConfigured => {
                HttpError::unauthorized(
                    "workflow_webhook_unauthorized",
                    "Webhook signature verification failed".to_string(),
                )
            }
            other => HttpError::bad_request("workflow_operation_failed", other.to_string()),
        })?;

    info!(
        tenant_slug = %tenant_slug,
        webhook_slug = %webhook_slug,
        executions = executions.len(),
        "Workflow webhook triggered execution(s)"
    );

    Ok(Json(WebhookResponse { executions }))
}
