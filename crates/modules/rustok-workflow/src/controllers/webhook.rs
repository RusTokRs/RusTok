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
            other => {
                tracing::error!(
                    tenant_slug = %tenant_slug,
                    webhook_slug = %webhook_slug,
                    error = %other,
                    "Workflow webhook execution failed"
                );
                HttpError::new(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "workflow_webhook_failed",
                    "Workflow webhook execution failed".to_string(),
                )
            }
        })?;

    info!(
        tenant_slug = %tenant_slug,
        webhook_slug = %webhook_slug,
        executions = executions.len(),
        "Workflow webhook triggered execution(s)"
    );

    Ok(Json(WebhookResponse { executions }))
}
