use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
};
use rustok_web::{HttpError, HttpResult};
use tracing::info;

use crate::WorkflowService;
use rustok_tenant::{TenantError, TenantService};

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
    let tenant_service = TenantService::new(db.clone());
    let tenant = tenant_service
        .get_tenant_by_slug(&tenant_slug)
        .await
        .map_err(|err| map_tenant_lookup_error(err, &tenant_slug))?;

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
            | crate::WorkflowError::WebhookSecretNotConfigured => HttpError::unauthorized(
                "workflow_webhook_unauthorized",
                "Webhook signature verification failed".to_string(),
            ),
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

fn map_tenant_lookup_error(err: TenantError, tenant_slug: &str) -> HttpError {
    match err {
        TenantError::NotFound => HttpError::not_found(
            "workflow_tenant_not_found",
            format!("Tenant not found: {tenant_slug}"),
        ),
        other => HttpError::bad_request("workflow_operation_failed", other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn maps_tenant_not_found_to_http_404() {
        let err = map_tenant_lookup_error(TenantError::NotFound, "unknown-tenant");
        assert_eq!(err.status, StatusCode::NOT_FOUND);
        assert_eq!(err.code, "workflow_tenant_not_found");
        assert!(err.message.contains("unknown-tenant"));
    }

    #[test]
    fn maps_other_tenant_errors_to_http_400() {
        let err = map_tenant_lookup_error(
            TenantError::InvalidSettingsSchema("invalid schema".into()),
            "tenant-1",
        );
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert_eq!(err.code, "workflow_operation_failed");
    }
}
