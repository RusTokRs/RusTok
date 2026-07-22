use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, FixedOffset};
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_web::{HttpError, HttpResult};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{CommerceHttpRuntime, common::ensure_permissions};

#[derive(Clone, Debug, Default, Deserialize, IntoParams, ToSchema)]
pub struct MarketplaceReversalOperatorListQuery {
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
pub struct MarketplaceReversalSweepInput {
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MarketplaceReversalEventResponse {
    pub id: Uuid,
    pub provider_event_id: Uuid,
    pub event_source: String,
    pub event_id: String,
    pub reversal_kind: String,
    pub source_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub occurred_at: DateTime<FixedOffset>,
    pub currency_code: String,
    pub currency_exponent: i16,
    pub total_amount: i64,
    pub line_count: usize,
    pub status: String,
    pub attempt_count: i32,
    pub reversal_id: Option<Uuid>,
    pub ledger_transaction_id: Option<Uuid>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub processed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MarketplaceReversalAdaptationFailureResponse {
    pub id: Uuid,
    pub provider_event_id: Uuid,
    pub event_source: String,
    pub event_id: String,
    pub event_type: String,
    pub status: String,
    pub retryable: bool,
    pub attempt_count: i32,
    pub last_error_code: String,
    pub last_error_message: String,
    pub next_retry_at: Option<DateTime<FixedOffset>>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub resolved_at: Option<DateTime<FixedOffset>>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MarketplaceReversalSweepFailureResponse {
    pub inbox_id: Uuid,
    pub retryable: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MarketplaceReversalSweepResponse {
    pub selected: usize,
    pub processed: usize,
    pub retryable_failures: usize,
    pub operator_review_failures: usize,
    pub failures: Vec<MarketplaceReversalSweepFailureResponse>,
}

pub fn axum_router() -> axum::Router<CommerceHttpRuntime> {
    axum::Router::new()
        .route(
            "/reversal-events/operator-review",
            axum::routing::get(list_operator_review),
        )
        .route("/reversal-events/{id}", axum::routing::get(show_event))
        .route(
            "/reversal-events/{id}/retry",
            axum::routing::post(retry_event),
        )
        .route(
            "/reversal-events/recovery-sweep",
            axum::routing::post(run_recovery_sweep),
        )
        .route(
            "/reversal-adaptation-failures/operator-review",
            axum::routing::get(list_adaptation_failures_operator_review),
        )
        .route(
            "/reversal-adaptation-failures/{id}",
            axum::routing::get(show_adaptation_failure),
        )
        .route(
            "/reversal-adaptation-failures/{id}/retry",
            axum::routing::post(retry_adaptation_failure),
        )
}

#[utoipa::path(
    get,
    path = "/admin/marketplace-financial/reversal-events/operator-review",
    tag = "admin-marketplace-financial",
    params(MarketplaceReversalOperatorListQuery),
    responses(
        (status = 200, description = "Marketplace reversal events requiring operator review", body = [MarketplaceReversalEventResponse]),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_operator_review(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<MarketplaceReversalOperatorListQuery>,
) -> HttpResult<Json<Vec<MarketplaceReversalEventResponse>>> {
    ensure_read_permission(&auth)?;
    let items = runtime
        .marketplace_reversal_operator_service()
        .list_operator_review(tenant.id, query.limit.unwrap_or(50))
        .await
        .map_err(map_operator_error)?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/admin/marketplace-financial/reversal-events/{id}",
    tag = "admin-marketplace-financial",
    params(("id" = Uuid, Path, description = "Marketplace reversal inbox ID")),
    responses(
        (status = 200, description = "Marketplace reversal event", body = MarketplaceReversalEventResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Reversal event not found")
    )
)]
pub async fn show_event(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<MarketplaceReversalEventResponse>> {
    ensure_read_permission(&auth)?;
    runtime
        .marketplace_reversal_operator_service()
        .get_event(tenant.id, id)
        .await
        .map(Into::into)
        .map(Json)
        .map_err(map_operator_error)
}

#[utoipa::path(
    post,
    path = "/admin/marketplace-financial/reversal-events/{id}/retry",
    tag = "admin-marketplace-financial",
    params(("id" = Uuid, Path, description = "Marketplace reversal inbox ID")),
    responses(
        (status = 200, description = "Marketplace reversal event processed after explicit safe retry", body = MarketplaceReversalEventResponse),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Reversal event is not safely retryable")
    )
)]
pub async fn retry_event(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<MarketplaceReversalEventResponse>> {
    ensure_manage_permission(&auth)?;
    runtime
        .marketplace_reversal_operator_service()
        .retry_event(tenant.id, id)
        .await
        .map(Into::into)
        .map(Json)
        .map_err(map_operator_error)
}

#[utoipa::path(
    post,
    path = "/admin/marketplace-financial/reversal-events/recovery-sweep",
    tag = "admin-marketplace-financial",
    request_body = MarketplaceReversalSweepInput,
    responses(
        (status = 200, description = "Bounded tenant-scoped marketplace reversal recovery sweep", body = MarketplaceReversalSweepResponse),
        (status = 401, description = "Unauthorized"),
        (status = 503, description = "Recovery storage unavailable")
    )
)]
pub async fn run_recovery_sweep(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<MarketplaceReversalSweepInput>,
) -> HttpResult<Json<MarketplaceReversalSweepResponse>> {
    ensure_manage_permission(&auth)?;
    let report = runtime
        .marketplace_reversal_operator_service()
        .sweep_tenant(tenant.id, input.limit.unwrap_or(100))
        .await
        .map_err(map_operator_error)?;
    Ok(Json(MarketplaceReversalSweepResponse {
        selected: report.selected,
        processed: report.processed,
        retryable_failures: report.retryable_failures,
        operator_review_failures: report.operator_review_failures,
        failures: report
            .failures
            .into_iter()
            .map(|failure| MarketplaceReversalSweepFailureResponse {
                inbox_id: failure.inbox_id,
                retryable: failure.retryable,
            })
            .collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/admin/marketplace-financial/reversal-adaptation-failures/operator-review",
    tag = "admin-marketplace-financial",
    params(MarketplaceReversalOperatorListQuery),
    responses(
        (status = 200, description = "Historical reversal adaptation failures requiring operator review", body = [MarketplaceReversalAdaptationFailureResponse]),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_adaptation_failures_operator_review(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<MarketplaceReversalOperatorListQuery>,
) -> HttpResult<Json<Vec<MarketplaceReversalAdaptationFailureResponse>>> {
    ensure_read_permission(&auth)?;
    let items = runtime
        .marketplace_reversal_operator_service()
        .list_adaptation_failures_operator_review(tenant.id, query.limit.unwrap_or(50))
        .await
        .map_err(map_operator_error)?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/admin/marketplace-financial/reversal-adaptation-failures/{id}",
    tag = "admin-marketplace-financial",
    params(("id" = Uuid, Path, description = "Marketplace reversal adaptation failure ID")),
    responses(
        (status = 200, description = "Marketplace reversal adaptation failure", body = MarketplaceReversalAdaptationFailureResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Adaptation failure not found")
    )
)]
pub async fn show_adaptation_failure(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<MarketplaceReversalAdaptationFailureResponse>> {
    ensure_read_permission(&auth)?;
    runtime
        .marketplace_reversal_operator_service()
        .get_adaptation_failure(tenant.id, id)
        .await
        .map(Into::into)
        .map(Json)
        .map_err(map_operator_error)
}

#[utoipa::path(
    post,
    path = "/admin/marketplace-financial/reversal-adaptation-failures/{id}/retry",
    tag = "admin-marketplace-financial",
    params(("id" = Uuid, Path, description = "Marketplace reversal adaptation failure ID")),
    responses(
        (status = 200, description = "Adaptation failure resolved after explicit retry", body = MarketplaceReversalAdaptationFailureResponse),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Adaptation remains invalid or is not retryable"),
        (status = 503, description = "Adaptation dependencies are temporarily unavailable")
    )
)]
pub async fn retry_adaptation_failure(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<MarketplaceReversalAdaptationFailureResponse>> {
    ensure_manage_permission(&auth)?;
    runtime
        .marketplace_reversal_operator_service()
        .retry_adaptation_failure(tenant.id, id)
        .await
        .map(Into::into)
        .map(Json)
        .map_err(map_operator_error)
}

fn ensure_read_permission(auth: &AuthContext) -> HttpResult<()> {
    ensure_permissions(
        auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )
}

fn ensure_manage_permission(auth: &AuthContext) -> HttpResult<()> {
    ensure_permissions(
        auth,
        &[Permission::PAYMENTS_MANAGE],
        "Permission denied: payments:manage required",
    )
}

fn map_operator_error(error: crate::MarketplaceReversalOperatorError) -> HttpError {
    match error {
        crate::MarketplaceReversalOperatorError::Validation(_) => HttpError::bad_request(
            "marketplace_reversal_operator_invalid",
            "Marketplace reversal operator request is invalid",
        ),
        crate::MarketplaceReversalOperatorError::Conflict(message)
            if message.contains("was not found") =>
        {
            HttpError::not_found(
                "marketplace_reversal_operator_not_found",
                "Marketplace reversal event or adaptation failure was not found",
            )
        }
        crate::MarketplaceReversalOperatorError::Conflict(_) => HttpError::new(
            StatusCode::CONFLICT,
            "marketplace_reversal_operator_conflict",
            "Marketplace reversal recovery requires reconciliation or is not safely retryable",
        ),
        crate::MarketplaceReversalOperatorError::Database(_) => {
            HttpError::internal("Marketplace reversal operator storage is unavailable")
        }
        crate::MarketplaceReversalOperatorError::Inbox(error) => {
            if error.retryable() {
                HttpError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "marketplace_reversal_recovery_unavailable",
                    "Marketplace reversal recovery is temporarily unavailable",
                )
            } else {
                HttpError::new(
                    StatusCode::CONFLICT,
                    "marketplace_reversal_reconciliation_required",
                    "Marketplace reversal recovery requires operator review",
                )
            }
        }
        crate::MarketplaceReversalOperatorError::AdaptationFailure(error) => match error {
            crate::MarketplaceReversalAdaptationFailureError::Validation(_) => {
                HttpError::bad_request(
                    "marketplace_reversal_adaptation_invalid",
                    "Marketplace reversal adaptation request is invalid",
                )
            }
            crate::MarketplaceReversalAdaptationFailureError::Conflict(message)
                if message.contains("was not found") =>
            {
                HttpError::not_found(
                    "marketplace_reversal_adaptation_not_found",
                    "Marketplace reversal adaptation failure was not found",
                )
            }
            crate::MarketplaceReversalAdaptationFailureError::Conflict(_) => HttpError::new(
                StatusCode::CONFLICT,
                "marketplace_reversal_adaptation_conflict",
                "Marketplace reversal adaptation failure is not safely retryable",
            ),
            crate::MarketplaceReversalAdaptationFailureError::Database(_) => {
                HttpError::internal("Marketplace reversal adaptation storage is unavailable")
            }
        },
        crate::MarketplaceReversalOperatorError::Adapter(error) => {
            if error.retryable() {
                HttpError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "marketplace_reversal_adaptation_unavailable",
                    "Marketplace reversal adaptation is temporarily unavailable",
                )
            } else {
                HttpError::new(
                    StatusCode::CONFLICT,
                    "marketplace_reversal_adaptation_requires_review",
                    "Marketplace reversal adaptation still requires operator review",
                )
            }
        }
    }
}

impl From<crate::MarketplaceReversalEventOperatorView> for MarketplaceReversalEventResponse {
    fn from(value: crate::MarketplaceReversalEventOperatorView) -> Self {
        Self {
            id: value.id,
            provider_event_id: value.provider_event_id,
            event_source: value.event_source,
            event_id: value.event_id,
            reversal_kind: value.reversal_kind,
            source_id: value.source_id,
            order_id: value.order_id,
            payment_collection_id: value.payment_collection_id,
            occurred_at: value.occurred_at,
            currency_code: value.currency_code,
            currency_exponent: value.currency_exponent,
            total_amount: value.total_amount,
            line_count: value.line_count,
            status: value.status,
            attempt_count: value.attempt_count,
            reversal_id: value.reversal_id,
            ledger_transaction_id: value.ledger_transaction_id,
            last_error_code: value.last_error_code,
            last_error_message: value.last_error_message,
            created_at: value.created_at,
            updated_at: value.updated_at,
            processed_at: value.processed_at,
        }
    }
}

impl From<crate::services::MarketplaceReversalAdaptationFailureOperatorView>
    for MarketplaceReversalAdaptationFailureResponse
{
    fn from(value: crate::services::MarketplaceReversalAdaptationFailureOperatorView) -> Self {
        Self {
            id: value.id,
            provider_event_id: value.provider_event_id,
            event_source: value.event_source,
            event_id: value.event_id,
            event_type: value.event_type,
            status: value.status,
            retryable: value.retryable,
            attempt_count: value.attempt_count,
            last_error_code: value.last_error_code,
            last_error_message: value.last_error_message,
            next_retry_at: value.next_retry_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
            resolved_at: value.resolved_at,
        }
    }
}
