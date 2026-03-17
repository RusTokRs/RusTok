/// Public webhook ingress for workflow triggers.
///
/// POST /webhooks/:tenant_slug/:webhook_slug
///
/// No authentication required — the caller identifies itself via the
/// `X-Webhook-Signature` header (HMAC-SHA256 of the raw body with the
/// workflow's `webhook_secret`). Signature verification is advisory for now
/// (logged but not enforced) to ease initial integration.
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use loco_rs::app::AppContext;
use serde_json::Value;
use tracing::{info, warn};

use crate::error::Result;
use crate::models::tenants;
use rustok_workflow::WorkflowService;

#[derive(serde::Serialize)]
pub struct WebhookResponse {
    pub executions: Vec<uuid::Uuid>,
}

/// Receive an incoming webhook and trigger matching active workflows.
pub async fn receive(
    State(ctx): State<AppContext>,
    Path((tenant_slug, webhook_slug)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WebhookResponse>> {
    // Resolve tenant by slug
    let tenant = tenants::Model::find_by_slug(&ctx.db, &tenant_slug)
        .await
        .map_err(|e| crate::error::Error::BadRequest(e.to_string()))?
        .ok_or_else(|| {
            crate::error::Error::BadRequest(format!("Tenant not found: {tenant_slug}"))
        })?;

    // Parse body as JSON; fall back to wrapping raw bytes as a string value
    let payload: Value = serde_json::from_slice(&body).unwrap_or_else(|_| {
        Value::String(String::from_utf8_lossy(&body).into_owned())
    });

    // Log signature header (verification can be enforced later)
    if let Some(sig) = headers.get("x-webhook-signature") {
        info!(
            tenant_slug = %tenant_slug,
            webhook_slug = %webhook_slug,
            signature = ?sig,
            "Webhook received with signature"
        );
    } else {
        warn!(
            tenant_slug = %tenant_slug,
            webhook_slug = %webhook_slug,
            "Webhook received without X-Webhook-Signature"
        );
    }

    let service = WorkflowService::new(ctx.db.clone());
    let executions = service
        .trigger_by_webhook(tenant.id, &webhook_slug, payload)
        .await
        .map_err(|e| crate::error::Error::BadRequest(e.to_string()))?;

    info!(
        tenant_slug = %tenant_slug,
        webhook_slug = %webhook_slug,
        executions = executions.len(),
        "Webhook triggered {} execution(s)",
        executions.len()
    );

    Ok(Json(WebhookResponse { executions }))
}
