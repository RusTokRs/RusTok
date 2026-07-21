use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use rustok_api::{
    has_any_effective_permission, AuthContext, HostRuntimeContext, Permission, TenantContext,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::entities::provider_event;
use crate::providers::{PaymentProviderRegistry, PaymentProviderWebhookRequest};
use crate::{
    PaymentError, PaymentObservedDomainEventApplier, PaymentProviderEventIngressError,
    PaymentProviderEventIngressService, PaymentProviderEventJournal,
    PaymentProviderEventObservers,
};

const DELIVERY_ID_HEADERS: [&str; 3] = [
    "x-rustok-provider-delivery-id",
    "x-webhook-id",
    "idempotency-key",
];
const REPLAY_KEY_HEADERS: [&str; 2] = ["idempotency-key", "x-rustok-provider-delivery-id"];
const SIGNATURE_HEADERS: [&str; 3] = [
    "x-provider-signature",
    "stripe-signature",
    "x-webhook-signature",
];
const MAX_EVENT_KEY_LENGTH: usize = 191;
const MAX_SIGNATURE_LENGTH: usize = 4096;
const MAX_RAW_PAYLOAD_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct PaymentHttpRuntime {
    db: sea_orm::DatabaseConnection,
    provider_registry: PaymentProviderRegistry,
    event_observers: PaymentProviderEventObservers,
}

impl PaymentHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> Self {
        Self {
            db: runtime.db_clone(),
            provider_registry: runtime
                .shared_get::<PaymentProviderRegistry>()
                .unwrap_or_else(PaymentProviderRegistry::with_manual_provider),
            event_observers: runtime
                .shared_get::<PaymentProviderEventObservers>()
                .unwrap_or_default(),
        }
    }

    fn ingress_service(&self) -> PaymentProviderEventIngressService {
        PaymentProviderEventIngressService::new(
            self.db.clone(),
            self.provider_registry.clone(),
            Arc::new(PaymentObservedDomainEventApplier::new(
                self.db.clone(),
                self.event_observers.clone(),
            )),
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentWebhookIngressResponse {
    pub event_id: Uuid,
    pub status: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct PaymentProviderEventAdminResponse {
    pub event_id: Uuid,
    pub provider_id: String,
    pub delivery_id: String,
    pub status: String,
    pub event_type: Option<String>,
    pub external_reference: Option<String>,
    pub attempt_count: i32,
    pub error_code: Option<String>,
    pub received_at: String,
    pub updated_at: String,
    pub processed_at: Option<String>,
}

impl From<provider_event::Model> for PaymentProviderEventAdminResponse {
    fn from(value: provider_event::Model) -> Self {
        Self {
            event_id: value.id,
            provider_id: value.provider_id,
            delivery_id: value.delivery_id,
            status: value.status,
            event_type: value.event_type,
            external_reference: value.external_reference,
            attempt_count: value.attempt_count,
            error_code: value.error_code,
            received_at: value.received_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            processed_at: value.processed_at.map(|timestamp| timestamp.to_rfc3339()),
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DeadLetterQuery {
    pub limit: Option<u64>,
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<Router> {
    let state = PaymentHttpRuntime::from_host(runtime);
    Ok(Router::new()
        .route(
            "/api/payment/provider-events/dead-letter",
            axum::routing::get(list_dead_letters),
        )
        .route(
            "/api/payment/provider-events/{event_id}",
            axum::routing::get(get_provider_event),
        )
        .route(
            "/api/payment/provider-events/{event_id}/replay",
            axum::routing::post(replay_dead_letter),
        )
        .with_state(state))
}

pub fn axum_webhook_router(runtime: &HostRuntimeContext) -> anyhow::Result<Router> {
    let state = PaymentHttpRuntime::from_host(runtime);
    Ok(Router::new()
        .route(
            "/payment/webhooks/{provider_id}",
            axum::routing::post(ingest_provider_webhook),
        )
        .layer(DefaultBodyLimit::max(MAX_RAW_PAYLOAD_BYTES))
        .with_state(state))
}

#[utoipa::path(
    post,
    path = "/payment/webhooks/{provider_id}",
    tag = "payment-webhooks",
    params(
        ("provider_id" = String, Path, description = "Registered payment provider identifier"),
        ("x-rustok-provider-delivery-id" = Option<String>, Header, description = "Optional untrusted delivery identity hint; verified provider output is authoritative"),
        ("idempotency-key" = Option<String>, Header, description = "Optional untrusted replay identity hint; verified provider output is authoritative"),
        ("x-provider-signature" = String, Header, description = "Provider-specific signature; adapters may also accept a documented provider header")
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream", description = "Raw signed provider payload, maximum 1 MiB"),
    responses(
        (status = 200, description = "Provider event processed or replayed", body = PaymentWebhookIngressResponse),
        (status = 202, description = "Provider event is already processing", body = PaymentWebhookIngressResponse),
        (status = 400, description = "Webhook hint or payload is invalid"),
        (status = 401, description = "Provider signature header is missing or invalid"),
        (status = 409, description = "Provider event conflicts with current owner state"),
        (status = 413, description = "Payload exceeds 1 MiB"),
        (status = 422, description = "Provider event requires operator review"),
        (status = 503, description = "Storage, configuration, or provider is temporarily unavailable")
    )
)]
pub async fn ingest_provider_webhook(
    State(runtime): State<PaymentHttpRuntime>,
    tenant: TenantContext,
    Path(provider_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<PaymentWebhookIngressResponse>), (StatusCode, Json<Value>)> {
    let provider_id = normalize_required(provider_id, "provider_id", 100)?;
    let delivery_id = optional_normalized_header(
        &headers,
        &DELIVERY_ID_HEADERS,
        "provider delivery id",
        MAX_EVENT_KEY_LENGTH,
    )?;
    let idempotency_key = optional_normalized_header(
        &headers,
        &REPLAY_KEY_HEADERS,
        "provider replay key",
        MAX_EVENT_KEY_LENGTH,
    )?;
    let signature = required_signature(&headers)?;
    if body.is_empty() || body.len() > MAX_RAW_PAYLOAD_BYTES {
        return Err(safe_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payment_webhook_payload_invalid",
            format!("Webhook payload must contain 1 to {MAX_RAW_PAYLOAD_BYTES} bytes"),
        ));
    }

    let lease_owner = format!("payment-webhook:{provider_id}:{}", Uuid::new_v4());
    let result = runtime
        .ingress_service()
        .ingest(
            PaymentProviderWebhookRequest {
                tenant_id: tenant.id,
                provider_id,
                delivery_id,
                idempotency_key,
                signature: Some(signature),
                raw_payload: body.to_vec(),
            },
            lease_owner,
        )
        .await;
    map_ingress_result(result)
}

#[utoipa::path(
    get,
    path = "/api/payment/provider-events/{event_id}",
    tag = "payment-provider-events",
    params(("event_id" = Uuid, Path, description = "Provider inbox event ID")),
    responses(
        (status = 200, description = "Safe provider event projection", body = PaymentProviderEventAdminResponse),
        (status = 403, description = "payments:read or payments:manage is required"),
        (status = 404, description = "Provider event not found"),
        (status = 503, description = "Provider event storage unavailable")
    )
)]
pub async fn get_provider_event(
    State(runtime): State<PaymentHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(event_id): Path<Uuid>,
) -> Result<Json<PaymentProviderEventAdminResponse>, (StatusCode, Json<Value>)> {
    ensure_payment_permission(
        &auth,
        &[Permission::PAYMENTS_READ, Permission::PAYMENTS_MANAGE],
        "payments:read or payments:manage required",
    )?;
    let event = PaymentProviderEventJournal::new(runtime.db)
        .get(tenant.id, event_id)
        .await
        .map_err(map_admin_payment_error)?;
    Ok(Json(event.into()))
}

#[utoipa::path(
    get,
    path = "/api/payment/provider-events/dead-letter",
    tag = "payment-provider-events",
    params(DeadLetterQuery),
    responses(
        (status = 200, description = "Newest dead-letter events", body = [PaymentProviderEventAdminResponse]),
        (status = 403, description = "payments:read or payments:manage is required"),
        (status = 503, description = "Provider event storage unavailable")
    )
)]
pub async fn list_dead_letters(
    State(runtime): State<PaymentHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<DeadLetterQuery>,
) -> Result<Json<Vec<PaymentProviderEventAdminResponse>>, (StatusCode, Json<Value>)> {
    ensure_payment_permission(
        &auth,
        &[Permission::PAYMENTS_READ, Permission::PAYMENTS_MANAGE],
        "payments:read or payments:manage required",
    )?;
    let events = PaymentProviderEventJournal::new(runtime.db)
        .list_dead_letters(tenant.id, query.limit.unwrap_or(50))
        .await
        .map_err(map_admin_payment_error)?;
    Ok(Json(events.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    post,
    path = "/api/payment/provider-events/{event_id}/replay",
    tag = "payment-provider-events",
    params(("event_id" = Uuid, Path, description = "Dead-letter provider event ID")),
    responses(
        (status = 200, description = "Dead-letter event replayed", body = PaymentWebhookIngressResponse),
        (status = 403, description = "payments:manage is required"),
        (status = 409, description = "Event is already processing"),
        (status = 422, description = "Event cannot be replayed or still conflicts with owner state"),
        (status = 503, description = "Replay state could not be recorded")
    )
)]
pub async fn replay_dead_letter(
    State(runtime): State<PaymentHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(event_id): Path<Uuid>,
) -> Result<Json<PaymentWebhookIngressResponse>, (StatusCode, Json<Value>)> {
    ensure_payment_permission(
        &auth,
        &[Permission::PAYMENTS_MANAGE],
        "payments:manage required",
    )?;
    let execution = runtime
        .ingress_service()
        .replay_dead_letter(
            tenant.id,
            event_id,
            format!("payment-webhook-replay:{event_id}:{}", Uuid::new_v4()),
        )
        .await
        .map_err(map_replay_error)?;
    Ok(Json(PaymentWebhookIngressResponse {
        event_id: execution.inbox_event.id,
        status: execution.inbox_event.status,
        replayed: true,
    }))
}

fn map_ingress_result(
    result: Result<crate::PaymentProviderEventExecution, PaymentProviderEventIngressError>,
) -> Result<(StatusCode, Json<PaymentWebhookIngressResponse>), (StatusCode, Json<Value>)> {
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
        Err(PaymentProviderEventIngressError::Payment(error)) => {
            Err(map_webhook_payment_error(error))
        }
        Err(PaymentProviderEventIngressError::ApplyAndJournal { .. }) => Err(safe_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_webhook_storage_unavailable",
            "Payment provider event will be retried",
        )),
    }
}

fn map_replay_error(error: PaymentProviderEventIngressError) -> (StatusCode, Json<Value>) {
    match error {
        PaymentProviderEventIngressError::InProgress(event_id) => safe_error(
            StatusCode::CONFLICT,
            "payment_webhook_replay_in_progress",
            format!("Payment provider event {event_id} is already processing"),
        ),
        PaymentProviderEventIngressError::DeadLetter(event_id) => safe_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "payment_webhook_replay_unavailable",
            format!("Payment provider event {event_id} cannot be replayed"),
        ),
        PaymentProviderEventIngressError::Apply(error) => safe_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            error.code,
            "Payment provider event replay failed and remains in dead-letter",
        ),
        PaymentProviderEventIngressError::Payment(error) => map_admin_payment_error(error),
        PaymentProviderEventIngressError::ApplyAndJournal { .. } => safe_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_webhook_storage_unavailable",
            "Payment provider event replay could not be recorded",
        ),
    }
}

fn map_webhook_payment_error(error: PaymentError) -> (StatusCode, Json<Value>) {
    match error {
        PaymentError::Database(_) | PaymentError::ProviderUnavailable { .. } => safe_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_webhook_temporarily_unavailable",
            "Payment provider event will be retried",
        ),
        PaymentError::ProviderConfiguration { .. } => safe_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_webhook_provider_not_configured",
            "Payment provider webhook configuration is unavailable",
        ),
        PaymentError::Validation(_)
        | PaymentError::ProviderRejected { .. }
        | PaymentError::ProviderInvalidResponse { .. } => safe_error(
            StatusCode::BAD_REQUEST,
            "payment_webhook_invalid",
            "Payment provider webhook could not be verified or parsed",
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => safe_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "payment_webhook_outcome_unknown",
            "Payment provider event requires operator review",
        ),
        PaymentError::InvalidTransition { .. } => safe_error(
            StatusCode::CONFLICT,
            "payment_webhook_state_conflict",
            "Payment provider event conflicts with current state",
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => safe_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_webhook_owner_unavailable",
            "Payment owner record is not available yet",
        ),
    }
}

fn map_admin_payment_error(error: PaymentError) -> (StatusCode, Json<Value>) {
    match error {
        PaymentError::Database(_)
        | PaymentError::ProviderUnavailable { .. }
        | PaymentError::ProviderConfiguration { .. } => safe_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_provider_event_unavailable",
            "Payment provider event service is unavailable",
        ),
        PaymentError::Validation(_) => safe_error(
            StatusCode::NOT_FOUND,
            "payment_provider_event_not_found",
            "Payment provider event was not found",
        ),
        PaymentError::InvalidTransition { .. } => safe_error(
            StatusCode::CONFLICT,
            "payment_provider_event_state_conflict",
            "Payment provider event state changed concurrently",
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => safe_error(
            StatusCode::CONFLICT,
            "payment_provider_outcome_unknown",
            "Payment provider operation requires reconciliation",
        ),
        PaymentError::ProviderRejected { .. }
        | PaymentError::ProviderInvalidResponse { .. } => safe_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "payment_provider_event_invalid",
            "Payment provider event cannot be applied",
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => safe_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "payment_provider_event_owner_missing",
            "Payment provider event owner record is missing",
        ),
    }
}

fn ensure_payment_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> Result<(), (StatusCode, Json<Value>)> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(safe_error(
            StatusCode::FORBIDDEN,
            "payment_permission_denied",
            message,
        ));
    }
    Ok(())
}

fn optional_normalized_header(
    headers: &HeaderMap,
    names: &[&str],
    label: &str,
    max_length: usize,
) -> Result<Option<String>, (StatusCode, Json<Value>)> {
    optional_header(headers, names)
        .map(|value| normalize_required(value, label, max_length))
        .transpose()
}

fn required_signature(headers: &HeaderMap) -> Result<String, (StatusCode, Json<Value>)> {
    optional_header(headers, &SIGNATURE_HEADERS)
        .map(|value| normalize_required(value, "provider signature", MAX_SIGNATURE_LENGTH))
        .transpose()?
        .ok_or_else(|| {
            safe_error(
                StatusCode::UNAUTHORIZED,
                "payment_webhook_signature_required",
                "Provider signature header is required",
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
