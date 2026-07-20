use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext};
use rustok_order::OrderService;
use rustok_web::HttpResult;
use uuid::Uuid;

use super::{
    super::{
        CommerceHttpRuntime,
        common::{PaginatedResponse, ensure_permissions},
    },
    AdminCompleteOrderReturnInput, ListOrderReturnsParams,
};
use crate::{
    CompleteReturnClaimInput, CompleteReturnExchangeInput, CompleteReturnRefundInput,
    CompleteReturnResolutionInput, CreateReturnDecisionInput, PostOrderOrchestrationService,
    ReturnCompletionOrchestrationService, ReturnDecisionResponse,
    dto::{
        CancelOrderReturnInput, CreateOrderReturnInput, ListOrderReturnsInput, OrderReturnResponse,
    },
};

#[utoipa::path(
    post,
    path = "/admin/orders/{id}/returns",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = CreateOrderReturnInput,
    responses(
        (status = 201, description = "Return created", body = OrderReturnResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn create_order_return(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateOrderReturnInput>,
) -> HttpResult<(StatusCode, Json<OrderReturnResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;
    let created = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .create_return(tenant.id, id, input)
        .await
        .map_err(super::map_order_error)?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    post,
    path = "/admin/orders/{id}/returns/decision",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = CreateReturnDecisionInput,
    responses(
        (status = 201, description = "Return decision created", body = ReturnDecisionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn create_order_return_decision(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateReturnDecisionInput>,
) -> HttpResult<(StatusCode, Json<ReturnDecisionResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;
    if super::decision_requires_payments_update(
        input.decision.action.as_str(),
        input.decision.refund.is_some(),
    ) {
        ensure_permissions(
            &auth,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;
    }
    let service = PostOrderOrchestrationService::new(runtime.db_clone(), runtime.event_bus())
        .with_payment_provider_registry(runtime.payment_provider_registry());
    let decision = service
        .create_return_decision(tenant.id, auth.user_id, id, input)
        .await
        .map_err(super::map_post_order_orchestration_error)?;
    Ok((StatusCode::CREATED, Json(decision)))
}

#[utoipa::path(
    get,
    path = "/admin/returns",
    tag = "admin",
    params(ListOrderReturnsParams),
    responses(
        (status = 200, description = "Returns", body = PaginatedResponse<OrderReturnResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_order_returns(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListOrderReturnsParams>,
) -> HttpResult<Json<PaginatedResponse<OrderReturnResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;
    let pagination = params.pagination.unwrap_or_default();
    let (items, total) = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .list_returns(
            tenant.id,
            ListOrderReturnsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                order_id: params.order_id,
                status: params.status,
            },
        )
        .await
        .map_err(super::map_order_error)?;
    Ok(Json(PaginatedResponse {
        data: items,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

#[utoipa::path(
    get,
    path = "/admin/returns/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Return ID")),
    responses(
        (status = 200, description = "Return details", body = OrderReturnResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Return not found")
    )
)]
pub async fn show_order_return(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<OrderReturnResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;
    let item = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .get_return(tenant.id, id)
        .await
        .map_err(super::map_order_error)?;
    Ok(Json(item))
}

#[utoipa::path(
    post,
    path = "/admin/returns/{id}/complete",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Return ID")),
    request_body = AdminCompleteOrderReturnInput,
    responses(
        (status = 200, description = "Return completed", body = OrderReturnResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Return not found")
    )
)]
pub async fn complete_order_return(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<AdminCompleteOrderReturnInput>,
) -> HttpResult<Json<OrderReturnResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;
    if input.refund.is_some() {
        ensure_permissions(
            &auth,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;
    }

    let command = CompleteReturnResolutionInput {
        resolution_type: input.resolution_type,
        refund_id: input.refund_id,
        order_change_id: input.order_change_id,
        refund: input.refund.map(|refund| CompleteReturnRefundInput {
            payment_collection_id: refund.payment_collection_id,
            amount: refund.amount,
            reason: refund.reason,
            metadata: refund.metadata,
            complete: refund.complete,
        }),
        exchange: input.exchange.map(|exchange| CompleteReturnExchangeInput {
            description: exchange.description,
            preview: exchange.preview,
            metadata: exchange.metadata,
        }),
        claim: input.claim.map(|claim| CompleteReturnClaimInput {
            description: claim.description,
            preview: claim.preview,
            metadata: claim.metadata,
        }),
        metadata: input.metadata,
    };
    let item = ReturnCompletionOrchestrationService::new(runtime.db_clone(), runtime.event_bus())
        .with_payment_provider_registry(runtime.payment_provider_registry())
        .complete_return(tenant.id, auth.user_id, id, command)
        .await
        .map_err(super::map_post_order_orchestration_error)?;

    Ok(Json(item))
}

#[utoipa::path(
    post,
    path = "/admin/returns/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Return ID")),
    request_body = CancelOrderReturnInput,
    responses(
        (status = 200, description = "Return cancelled", body = OrderReturnResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Return not found")
    )
)]
pub async fn cancel_order_return(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelOrderReturnInput>,
) -> HttpResult<Json<OrderReturnResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;
    let item = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .cancel_return(tenant.id, id, input)
        .await
        .map_err(super::map_order_error)?;
    Ok(Json(item))
}
