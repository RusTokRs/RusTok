use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext};
use rustok_order::OrderService;
use rustok_outbox::loco::transactional_event_bus_from_context;
use rustok_payment::PaymentService;
use uuid::Uuid;

use super::{
    super::common::{ensure_permissions, PaginatedResponse},
    AdminCompleteOrderReturnInput, ListOrderReturnsParams,
};
use crate::{
    dto::{
        CancelOrderReturnInput, CompleteRefundInput, CreateOrderReturnInput, CreateRefundInput,
        ListOrderReturnsInput, OrderReturnResponse,
    },
    CreateReturnDecisionInput, PostOrderOrchestrationService, ReturnDecisionResponse,
};

/// Create admin order return
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateOrderReturnInput>,
) -> Result<(StatusCode, Json<OrderReturnResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let created = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .create_return(tenant.id, id, input)
        .await
        .map_err(super::map_order_error)?;

    Ok((StatusCode::CREATED, Json(created)))
}

/// Create admin order return and apply decision tree orchestration
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateReturnDecisionInput>,
) -> Result<(StatusCode, Json<ReturnDecisionResponse>)> {
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

    let service = PostOrderOrchestrationService::new(
        ctx.db.clone(),
        transactional_event_bus_from_context(&ctx),
    );
    let decision = service
        .create_return_decision(tenant.id, auth.user_id, id, input)
        .await
        .map_err(super::map_post_order_orchestration_error)?;

    Ok((StatusCode::CREATED, Json(decision)))
}

/// List admin order returns
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListOrderReturnsParams>,
) -> Result<Json<PaginatedResponse<OrderReturnResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (items, total) =
        OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
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

/// Show admin order return
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<OrderReturnResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;

    let item = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .get_return(tenant.id, id)
        .await
        .map_err(super::map_order_error)?;

    Ok(Json(item))
}

fn attach_return_order_change_context(
    value: serde_json::Value,
    return_id: Uuid,
    change_type: &str,
) -> Result<serde_json::Value> {
    let mut object = match value {
        serde_json::Value::Null => serde_json::Map::new(),
        serde_json::Value::Object(obj) => obj,
        _ => return Err(Error::BadRequest("Value must be a JSON object".to_string())),
    };
    object.insert(
        "order_return_id".to_string(),
        serde_json::Value::String(return_id.to_string()),
    );
    object.insert(
        "return_decision_action".to_string(),
        serde_json::Value::String(change_type.to_string()),
    );
    object.insert(
        "return_decision_source".to_string(),
        serde_json::Value::String("rustok-commerce".to_string()),
    );
    Ok(serde_json::Value::Object(object))
}

/// Complete admin order return
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<AdminCompleteOrderReturnInput>,
) -> Result<Json<OrderReturnResponse>> {
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

    let event_bus = transactional_event_bus_from_context(&ctx);
    let order_service = OrderService::new(ctx.db.clone(), event_bus);
    let mut complete_input = rustok_order::dto::CompleteOrderReturnInput {
        resolution_type: input.resolution_type,
        refund_id: input.refund_id,
        order_change_id: input.order_change_id,
        metadata: input.metadata,
    };

    let has_refund_helper = input.refund.is_some();
    let has_exchange_helper = input.exchange.is_some();
    let has_claim_helper = input.claim.is_some();

    if let Some(refund_input) = input.refund {
        if complete_input.refund_id.is_some() || complete_input.order_change_id.is_some() {
            return Err(Error::BadRequest(
                "refund helper cannot be combined with explicit refund_id or order_change_id"
                    .to_string(),
            ));
        }
        if complete_input
            .resolution_type
            .as_deref()
            .map(|value| value.trim().eq_ignore_ascii_case("refund"))
            == Some(false)
        {
            return Err(Error::BadRequest(
                "refund helper requires resolution_type to be omitted or `refund`".to_string(),
            ));
        }

        let existing_return = order_service
            .get_return(tenant.id, id)
            .await
            .map_err(super::map_order_error)?;
        let payment_service = PaymentService::new(ctx.db.clone());
        let collection_id = super::resolve_return_refund_collection_id(
            &payment_service,
            tenant.id,
            existing_return.order_id,
            refund_input.payment_collection_id,
        )
        .await?;
        let refund = payment_service
            .create_refund(
                tenant.id,
                collection_id,
                CreateRefundInput {
                    amount: refund_input.amount,
                    reason: refund_input.reason,
                    metadata: refund_input.metadata,
                },
            )
            .await
            .map_err(super::map_payment_error)?;
        let refund = if refund_input.complete {
            payment_service
                .complete_refund(
                    tenant.id,
                    refund.id,
                    CompleteRefundInput {
                        metadata: serde_json::json!({
                            "source": "order_return_completion",
                            "return_id": id,
                        }),
                    },
                )
                .await
                .map_err(super::map_payment_error)?
        } else {
            refund
        };

        complete_input.resolution_type = Some("refund".to_string());
        complete_input.refund_id = Some(refund.id);
    }

    if let Some(exchange_input) = input.exchange {
        if complete_input.refund_id.is_some()
            || complete_input.order_change_id.is_some()
            || has_refund_helper
            || has_claim_helper
        {
            return Err(Error::BadRequest(
                "exchange helper cannot be combined with explicit refund_id, order_change_id, refund helper, or claim helper"
                    .to_string(),
            ));
        }
        if complete_input
            .resolution_type
            .as_deref()
            .map(|value| value.trim().eq_ignore_ascii_case("exchange"))
            == Some(false)
        {
            return Err(Error::BadRequest(
                "exchange helper requires resolution_type to be omitted or `exchange`".to_string(),
            ));
        }

        let existing_return = order_service
            .get_return(tenant.id, id)
            .await
            .map_err(super::map_order_error)?;

        let preview = attach_return_order_change_context(exchange_input.preview, id, "exchange")?;
        let metadata = attach_return_order_change_context(exchange_input.metadata, id, "exchange")?;

        let order_change = order_service
            .create_order_change(
                tenant.id,
                auth.user_id,
                existing_return.order_id,
                rustok_order::dto::CreateOrderChangeInput {
                    change_type: "exchange".to_string(),
                    description: exchange_input.description,
                    preview,
                    metadata,
                },
            )
            .await
            .map_err(super::map_order_error)?;

        complete_input.resolution_type = Some("exchange".to_string());
        complete_input.order_change_id = Some(order_change.id);
    }

    if let Some(claim_input) = input.claim {
        if complete_input.refund_id.is_some()
            || complete_input.order_change_id.is_some()
            || has_refund_helper
            || has_exchange_helper
        {
            return Err(Error::BadRequest(
                "claim helper cannot be combined with explicit refund_id, order_change_id, refund helper, or exchange helper"
                    .to_string(),
            ));
        }
        if complete_input
            .resolution_type
            .as_deref()
            .map(|value| value.trim().eq_ignore_ascii_case("claim"))
            == Some(false)
        {
            return Err(Error::BadRequest(
                "claim helper requires resolution_type to be omitted or `claim`".to_string(),
            ));
        }

        let existing_return = order_service
            .get_return(tenant.id, id)
            .await
            .map_err(super::map_order_error)?;

        let preview = attach_return_order_change_context(claim_input.preview, id, "claim")?;
        let metadata = attach_return_order_change_context(claim_input.metadata, id, "claim")?;

        let order_change = order_service
            .create_order_change(
                tenant.id,
                auth.user_id,
                existing_return.order_id,
                rustok_order::dto::CreateOrderChangeInput {
                    change_type: "claim".to_string(),
                    description: claim_input.description,
                    preview,
                    metadata,
                },
            )
            .await
            .map_err(super::map_order_error)?;

        complete_input.resolution_type = Some("claim".to_string());
        complete_input.order_change_id = Some(order_change.id);
    }

    let item = order_service
        .complete_return(tenant.id, id, complete_input)
        .await
        .map_err(super::map_order_error)?;

    Ok(Json(item))
}

/// Cancel admin order return
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelOrderReturnInput>,
) -> Result<Json<OrderReturnResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let item = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .cancel_return(tenant.id, id, input)
        .await
        .map_err(super::map_order_error)?;

    Ok(Json(item))
}
