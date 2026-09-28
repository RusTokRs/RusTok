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
        .map_err(|err| {
            tracing::error!(
                tenant_slug = %tenant_slug,
                error = %err,
                "Workflow webhook tenant lookup failed"
            );
            HttpError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "workflow_webhook_unavailable",
                "Workflow webhook is temporarily unavailable".to_string(),
            )
        })?
        .filter(|tenant| tenant.is_active)
        .ok_or_else(|| {
            HttpError::not_found(
                "workflow_webhook_not_found",
                "Workflow webhook endpoint was not found".to_string(),
            )
        })?;
    runtime.ensure_module_enabled(tenant.id).await?;

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
