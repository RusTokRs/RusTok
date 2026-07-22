use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
};
use rustok_api::{
    AuthContext, HostRuntimeContext, Permission, TenantContext, has_any_effective_permission,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    PaymentDomainEventApplier, PaymentError, PaymentProviderEventRecoveryFailure,
    PaymentProviderEventRecoveryReport, PaymentProviderEventRecoveryService,
};

#[derive(Clone)]
pub struct PaymentProviderEventRecoveryHttpRuntime {
    db: sea_orm::DatabaseConnection,
}

impl PaymentProviderEventRecoveryHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> Self {
        Self {
            db: runtime.db_clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, IntoParams)]
pub struct PaymentProviderEventRecoveryQuery {
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct PaymentProviderEventRecoveryFailureResponse {
    pub event_id: Uuid,
    pub status: String,
    pub error_code: Option<String>,
}

impl From<PaymentProviderEventRecoveryFailure> for PaymentProviderEventRecoveryFailureResponse {
    fn from(value: PaymentProviderEventRecoveryFailure) -> Self {
        Self {
            event_id: value.event_id,
            status: value.status,
            error_code: value.error_code,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct PaymentProviderEventRecoveryResponse {
    pub scanned: usize,
    pub processed: usize,
    pub retryable: usize,
    pub dead_letter: usize,
    pub in_progress: usize,
    pub errors: usize,
    pub failures: Vec<PaymentProviderEventRecoveryFailureResponse>,
}

impl From<PaymentProviderEventRecoveryReport> for PaymentProviderEventRecoveryResponse {
    fn from(value: PaymentProviderEventRecoveryReport) -> Self {
        Self {
            scanned: value.scanned,
            processed: value.processed,
            retryable: value.retryable,
            dead_letter: value.dead_letter,
            in_progress: value.in_progress,
            errors: value.errors,
            failures: value.failures.into_iter().map(Into::into).collect(),
        }
    }
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<Router> {
    Ok(Router::new()
        .route(
            "/api/payment/provider-events/recovery/run",
            axum::routing::post(run_provider_event_recovery),
        )
        .with_state(PaymentProviderEventRecoveryHttpRuntime::from_host(runtime)))
}

#[utoipa::path(
    post,
    path = "/api/payment/provider-events/recovery/run",
    tag = "payment-provider-events",
    params(PaymentProviderEventRecoveryQuery),
    responses(
        (status = 200, description = "Bounded recovery sweep completed; per-event failures are reported safely", body = PaymentProviderEventRecoveryResponse),
        (status = 403, description = "payments:manage is required"),
        (status = 503, description = "Initial provider event recovery query failed")
    )
)]
pub async fn run_provider_event_recovery(
    State(runtime): State<PaymentProviderEventRecoveryHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<PaymentProviderEventRecoveryQuery>,
) -> Result<Json<PaymentProviderEventRecoveryResponse>, (StatusCode, Json<Value>)> {
    if !has_any_effective_permission(&auth.permissions, &[Permission::PAYMENTS_MANAGE]) {
        return Err(safe_error(
            StatusCode::FORBIDDEN,
            "payment_permission_denied",
            "payments:manage required",
        ));
    }

    let service = PaymentProviderEventRecoveryService::new(
        runtime.db.clone(),
        Arc::new(PaymentDomainEventApplier::new(runtime.db)),
    );
    let report = service
        .run(
            tenant.id,
            "payment-provider-event-http-recovery",
            query.limit,
        )
        .await
        .map_err(map_recovery_error)?;
    Ok(Json(report.into()))
}

fn map_recovery_error(error: PaymentError) -> (StatusCode, Json<Value>) {
    match error {
        PaymentError::Database(_)
        | PaymentError::ProviderUnavailable { .. }
        | PaymentError::ProviderConfiguration { .. } => safe_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_provider_event_recovery_unavailable",
            "Payment provider event recovery is unavailable",
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderOutcomeUnknown { .. } => {
            safe_error(
                StatusCode::CONFLICT,
                "payment_provider_event_state_conflict",
                "Payment provider event requires reconciliation",
            )
        }
        PaymentError::Validation(_)
        | PaymentError::ProviderRejected { .. }
        | PaymentError::ProviderInvalidResponse { .. }
        | PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => safe_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "payment_provider_event_recovery_failed",
            "Payment provider event recovery could not be completed",
        ),
    }
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
