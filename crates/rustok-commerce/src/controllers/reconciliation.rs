use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use chrono::{Duration, Utc};
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_fulfillment::providers::FulfillmentProviderOperationResult;
use rustok_fulfillment::{FulfillmentProviderOperationRecovery, entities::provider_operation};
use rustok_web::{HttpError, HttpResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{CommerceHttpRuntime, admin::map_fulfillment_orchestration_error};
use crate::{FulfillmentCreateLabelRecoveryService, FulfillmentReconciliationService};

#[derive(Debug, Clone, Deserialize)]
pub struct ListReconciliationParams {
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuarantineStaleInput {
    pub stale_after_seconds: u64,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolveUnknownFailedInput {
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolveUnknownSucceededInput {
    pub provider_result: FulfillmentProviderOperationResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuarantineStaleResponse {
    pub quarantined: u64,
}

pub fn axum_router() -> Router<CommerceHttpRuntime> {
    Router::new()
        .route("/reconciliation", get(list_reconciliation_required))
        .route("/quarantine-stale", post(quarantine_stale_executing))
        .route("/{id}/resolve-failed", post(resolve_unknown_as_failed))
        .route(
            "/{id}/resolve-succeeded",
            post(resolve_unknown_as_succeeded),
        )
        .route("/{id}/retry-local", post(retry_local_persistence))
        .route("/{id}/retry-create-label", post(retry_create_label))
}

async fn list_reconciliation_required(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListReconciliationParams>,
) -> HttpResult<Json<Vec<provider_operation::Model>>> {
    require_manage_permission(&auth)?;
    let operations = FulfillmentProviderOperationRecovery::new(runtime.db_clone())
        .list_reconciliation_required(tenant.id, params.limit.unwrap_or(100))
        .await
        .map_err(super::admin::map_fulfillment_error)?;
    Ok(Json(operations))
}

async fn quarantine_stale_executing(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<QuarantineStaleInput>,
) -> HttpResult<Json<QuarantineStaleResponse>> {
    require_manage_permission(&auth)?;
    let stale_after_seconds = input.stale_after_seconds.clamp(60, 7 * 24 * 60 * 60);
    let stale_before = Utc::now() - Duration::seconds(stale_after_seconds as i64);
    let quarantined = FulfillmentProviderOperationRecovery::new(runtime.db_clone())
        .quarantine_stale_executing(tenant.id, stale_before, input.limit.unwrap_or(100))
        .await
        .map_err(super::admin::map_fulfillment_error)?;
    Ok(Json(QuarantineStaleResponse { quarantined }))
}

async fn resolve_unknown_as_failed(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(operation_id): Path<Uuid>,
    Json(input): Json<ResolveUnknownFailedInput>,
) -> HttpResult<Json<provider_operation::Model>> {
    require_manage_permission(&auth)?;
    let operation = FulfillmentProviderOperationRecovery::new(runtime.db_clone())
        .resolve_unknown_as_failed(tenant.id, operation_id, input.reason)
        .await
        .map_err(super::admin::map_fulfillment_error)?;
    Ok(Json(operation))
}

async fn resolve_unknown_as_succeeded(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(operation_id): Path<Uuid>,
    Json(input): Json<ResolveUnknownSucceededInput>,
) -> HttpResult<Json<provider_operation::Model>> {
    require_manage_permission(&auth)?;
    let provider_reference = input.provider_result.external_reference.clone();
    let provider_result = serde_json::to_value(input.provider_result).map_err(|error| {
        HttpError::bad_request(
            "commerce_admin_invalid",
            format!("failed to serialize provider result: {error}"),
        )
    })?;
    let operation = FulfillmentProviderOperationRecovery::new(runtime.db_clone())
        .resolve_unknown_as_succeeded(tenant.id, operation_id, provider_reference, provider_result)
        .await
        .map_err(super::admin::map_fulfillment_error)?;
    Ok(Json(operation))
}

async fn retry_local_persistence(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(operation_id): Path<Uuid>,
) -> HttpResult<Json<crate::dto::FulfillmentResponse>> {
    require_manage_permission(&auth)?;
    let fulfillment = FulfillmentReconciliationService::new(runtime.db_clone())
        .retry_local_persistence(tenant.id, operation_id)
        .await
        .map_err(map_fulfillment_orchestration_error)?;
    Ok(Json(fulfillment))
}

async fn retry_create_label(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(operation_id): Path<Uuid>,
) -> HttpResult<Json<crate::dto::FulfillmentResponse>> {
    require_manage_permission(&auth)?;
    let fulfillment = FulfillmentCreateLabelRecoveryService::new(runtime.db_clone())
        .with_provider_registry(runtime.fulfillment_provider_registry())
        .retry(tenant.id, operation_id)
        .await
        .map_err(map_fulfillment_orchestration_error)?;
    Ok(Json(fulfillment))
}

fn require_manage_permission(auth: &AuthContext) -> HttpResult<()> {
    super::common::ensure_permissions(
        auth,
        &[Permission::FULFILLMENTS_MANAGE],
        "Permission denied: fulfillments:manage required",
    )
}
