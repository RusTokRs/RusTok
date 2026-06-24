use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::{AuthContext, TenantContext};
use rustok_core::Permission;
use uuid::Uuid;

use crate::{
    dto::{
        AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
        CompleteRefundInput, CreateRefundInput, ListPaymentCollectionsInput, ListRefundsInput,
        PaymentCollectionResponse, RefundResponse,
    },
    PaymentService,
};
use super::{
    super::common::{ensure_permissions, PaginatedResponse},
    ListPaymentCollectionsParams, ListRefundsParams,
};

/// List admin payment collections
#[utoipa::path(
    get,
    path = "/admin/payment-collections",
    tag = "admin",
    params(ListPaymentCollectionsParams),
    responses(
        (status = 200, description = "Payment collections", body = PaginatedResponse<PaymentCollectionResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_payment_collections(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListPaymentCollectionsParams>,
) -> Result<Json<PaginatedResponse<PaymentCollectionResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (collections, total) = PaymentService::new(ctx.db.clone())
        .list_collections(
            tenant.id,
            ListPaymentCollectionsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status,
                order_id: params.order_id,
                cart_id: params.cart_id,
                customer_id: params.customer_id,
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(PaginatedResponse {
        data: collections,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Show admin payment collection
#[utoipa::path(
    get,
    path = "/admin/payment-collections/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    responses(
        (status = 200, description = "Payment collection details", body = PaymentCollectionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Payment collection not found")
    )
)]
pub async fn show_payment_collection(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;

    let collection = PaymentService::new(ctx.db.clone())
        .get_collection(tenant.id, id)
        .await
        .map_err(super::map_payment_error)?;

    Ok(Json(collection))
}

/// Authorize admin payment collection
#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/authorize",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    request_body = AuthorizePaymentInput,
    responses(
        (status = 200, description = "Payment collection authorized", body = PaymentCollectionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Payment collection not found")
    )
)]
pub async fn authorize_payment_collection(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<AuthorizePaymentInput>,
) -> Result<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let collection = PaymentService::new(ctx.db.clone())
        .authorize_collection(tenant.id, id, input)
        .await
        .map_err(super::map_payment_error)?;

    Ok(Json(collection))
}

/// Capture admin payment collection
#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/capture",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    request_body = CapturePaymentInput,
    responses(
        (status = 200, description = "Payment collection captured", body = PaymentCollectionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Payment collection not found")
    )
)]
pub async fn capture_payment_collection(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CapturePaymentInput>,
) -> Result<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let collection = PaymentService::new(ctx.db.clone())
        .capture_collection(tenant.id, id, input)
        .await
        .map_err(super::map_payment_error)?;

    Ok(Json(collection))
}

/// Cancel admin payment collection
#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    request_body = CancelPaymentInput,
    responses(
        (status = 200, description = "Payment collection cancelled", body = PaymentCollectionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Payment collection not found")
    )
)]
pub async fn cancel_payment_collection(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelPaymentInput>,
) -> Result<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let collection = crate::PaymentOrchestrationService::new(ctx.db.clone())
        .cancel_collection(tenant.id, id, input)
        .await
        .map_err(super::map_payment_orchestration_error)?;

    Ok(Json(collection))
}

/// Create admin refund
#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/refunds",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    request_body = CreateRefundInput,
    responses(
        (status = 201, description = "Refund created", body = RefundResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Payment collection not found")
    )
)]
pub async fn create_refund(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateRefundInput>,
) -> Result<(StatusCode, Json<RefundResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let refund = crate::PaymentOrchestrationService::new(ctx.db.clone())
        .create_refund(tenant.id, id, input)
        .await
        .map_err(super::map_payment_orchestration_error)?;

    Ok((StatusCode::CREATED, Json(refund)))
}

/// List admin refunds
#[utoipa::path(
    get,
    path = "/admin/refunds",
    tag = "admin",
    params(ListRefundsParams),
    responses(
        (status = 200, description = "Refunds", body = PaginatedResponse<RefundResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_refunds(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListRefundsParams>,
) -> Result<Json<PaginatedResponse<RefundResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (refunds, total) = PaymentService::new(ctx.db.clone())
        .list_refunds(
            tenant.id,
            ListRefundsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                payment_collection_id: params.payment_collection_id,
                order_id: params.order_id,
                status: params.status,
            },
        )
        .await
        .map_err(super::map_payment_error)?;

    Ok(Json(PaginatedResponse {
        data: refunds,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Show admin refund
#[utoipa::path(
    get,
    path = "/admin/refunds/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Refund ID")),
    responses(
        (status = 200, description = "Refund details", body = RefundResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Refund not found")
    )
)]
pub async fn show_refund(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;

    let refund = PaymentService::new(ctx.db.clone())
        .get_refund(tenant.id, id)
        .await
        .map_err(super::map_payment_error)?;

    Ok(Json(refund))
}

/// Complete admin refund
#[utoipa::path(
    post,
    path = "/admin/refunds/{id}/complete",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Refund ID")),
    request_body = CompleteRefundInput,
    responses(
        (status = 200, description = "Refund completed", body = RefundResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Refund not found")
    )
)]
pub async fn complete_refund(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CompleteRefundInput>,
) -> Result<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let refund = PaymentService::new(ctx.db.clone())
        .complete_refund(tenant.id, id, input)
        .await
        .map_err(super::map_payment_error)?;

    Ok(Json(refund))
}

/// Cancel admin refund
#[utoipa::path(
    post,
    path = "/admin/refunds/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Refund ID")),
    request_body = CancelRefundInput,
    responses(
        (status = 200, description = "Refund cancelled", body = RefundResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Refund not found")
    )
)]
pub async fn cancel_refund(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelRefundInput>,
) -> Result<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let refund = PaymentService::new(ctx.db.clone())
        .cancel_refund(tenant.id, id, input)
        .await
        .map_err(super::map_payment_error)?;

    Ok(Json(refund))
}
