use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use rustok_api::{HostRuntimeContext, TenantContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::providers::{PaymentProviderRegistry, PaymentProviderWebhookRequest};
use crate::{
    PaymentDomainEventApplier, PaymentProviderEventIngressError,
    PaymentProviderEventIngressService,
};

const DELIVERY_ID_HEADERS: [&str; 3] = [
    "x-rustok-provider-delivery-id",
    "x-webhook-id",
    "idempotency-key",
];
const IDEMPOTENCY_KEY_HEADERS: [&str; 2] = ["idempotency-key", "x-rustok-provider-delivery-id"];
const SIGNATURE_HEADERS: [&str; 3] = [
    "x-provider-signature",
    "stripe-signature",
    "x-webhook-signature",
];
const MAX_EVENT_KEY_LENGTH: usize = 191;

#[derive(Clone)]
pub struct PaymentWebhookHttpRuntime {
    db: sea_orm::DatabaseConnection,
    provider_registry: PaymentProviderRegistry,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentWebhookIngressResponse {
    pub event_id: Uuid,
    pub status: String,
    pub replayed: bool,
}

pub fn axum_router(runtime: &HostRuntimeContext) -> Router {
    let state = PaymentWebhookHttpRuntime {
        db: runtime.db_clone(),
        provider_registry: runtime
            .shared_get::<PaymentProviderRegistry>()
            .unwrap_or_else(PaymentProviderRegistry::with_manual_provider),
    };
    Router::new()
        .route("/webhooks/{provider_id}", axum::routing::post(ingest_provider_webhook))
        .with_state(state)
}

#[utoipa::path(
    post,
    path = "/payment/webhooks/{provider_id}",
    tag = "payment-webhooks",
    params(("provider_id" = String, Path, description = "Payment provider identifier")),
    responses(
        (status = 200, description = "Provider delivery processed or replayed", body = PaymentWebhookIngressResponse),
        (status = 202, description = "Provider delivery is already processing", body = PaymentWebhookIngressResponse),
        (status = 400, description = "Webhook request is invalid"),
        (status = 401, description = "Webhook signature is invalid"),
        (status = 422, description = "Normalized provider event is unsupported"),
        (status = 503, description = "Provider delivery will be retried")
    )
)]
pub async fn ingest_provider_webhook(
    State(runtime): State<PaymentWebhookHttpRuntime>,
    tenant: TenantContext,
    Path(provider_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<PaymentWebhookIngressResponse>), (StatusCode, Json<Value>)> {
    let provider_id = normalize_required(provider_id, "provider_id", 100)?;
    let delivery_id = required_header(&headers, &DELIVERY_ID_HEADERS, "provider delivery id")?;
    let idempotency_key = required_header(
        &headers,
        &IDEMPOTENCY_KEY_HEADERS,
        "provider idempotency key",
    )?;
    let signature = optional_header(&headers, &SIGNATURE_HEADERS);
    let provider_headers = header_map(&headers);
    let service = PaymentProviderEventIngressService::new(
        runtime.db.clone(),
        runtime.provider_registry,
        Arc::new(PaymentDomainEventApplier::new(runtime.db)),
    );
    let lease_owner = format!(
        "payment-webhook:{provider_id}:{delivery_id}:{}",
        Uuid::new_v4()
    );
    let result = service
        .ingest(
            PaymentProviderWebhookRequest {
                tenant_id: tenant.id,
                provider_id,
                delivery_id: delivery_id.clone(),
                idempotency_key,
                raw_payload: body.to_vec(),
                signature,
                headers: provider_headers,
                metadata: serde_json::json!({
                    "transport": "http",
                    "delivery_id": delivery_id,
                }),
            },
            lease_owner,
        )
        .await;

    match result {
        Ok(execution) => Ok((
            StatusCode::OK,
            Json(PaymentWebhookIngressResponse {
                event_id: execution.inbox_event.id,
                status: execution.inbox_event.status,
                replayed: execution.replayed,
            }),
        )),
        Err(PaymentProviderEventIngressError::InProgress(event_id)) => Ok((
            StatusCode::ACCEPTED,
            Json(PaymentWebhookIngressResponse {
                event_id,
                status: "processing".to_string(),
                replayed: true,
            }),
        )),
        Err(PaymentProviderEventIngressError::DeadLetter(event_id)) => Err(safe_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "payment_webhook_dead_letter",
            format!("Payment provider event {event_id} requires operator review"),
        )),
        Err(PaymentProviderEventIngressError::Provider(_)) => Err(safe_error(
            if headers_contain_signature(&headers) {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::UNAUTHORIZED
            },
            "payment_webhook_invalid",
            "Payment provider webhook could not be verified or parsed",
        )),
        Err(PaymentProviderEventIngressError::Apply(error)) if error.retryable => Err(safe_error(
            StatusCode::SERVICE_UNAVAILABLE,
            error.code,
            "Payment provider event will be retried",
        )),
        Err(PaymentProviderEventIngressError::Apply(error)) => Err(safe_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            error.code,
            "Payment provider event is not applicable",
        )),
        Err(PaymentProviderEventIngressError::Payment(_))
        | Err(PaymentProviderEventIngressError::ApplyAndJournal { .. }) => Err(safe_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_webhook_storage_unavailable",
            "Payment provider event will be retried",
        )),
    }
}

fn required_header(
    headers: &HeaderMap,
    names: &[&str],
    label: &str,
) -> Result<String, (StatusCode, Json<Value>)> {
    optional_header(headers, names)
        .map(|value| normalize_required(value, label, MAX_EVENT_KEY_LENGTH))
        .transpose()?
        .ok_or_else(|| {
            safe_error(
                StatusCode::BAD_REQUEST,
                "payment_webhook_identity_required",
                format!("{label} header is required"),
            )
        })
}

fn optional_header(headers: &HeaderMap, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        headers
            .get(*name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn header_map(headers: &HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect()
}

fn headers_contain_signature(headers: &HeaderMap) -> bool {
    optional_header(headers, &SIGNATURE_HEADERS).is_some()
}

fn normalize_required(
    value: String,
    label: &str,
    max_length: usize,
) -> Result<String, (StatusCode, Json<Value>)> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > max_length {
        return Err(safe_error(
            StatusCode::BAD_REQUEST,
            "payment_webhook_identity_invalid",
            format!("{label} must contain 1 to {max_length} bytes"),
        ));
    }
    Ok(value)
}

fn safe_error(
    status: StatusCode,
    code: impl Into<String>,
    message: impl Into<String>,
) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(serde_json::json!({
            "error": {
                "code": code.into(),
                "message": message.into(),
            }
        })),
    )
}
