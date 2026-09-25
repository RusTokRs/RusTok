use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{
    AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext,
};
use rustok_fulfillment::FulfillmentError;
use rustok_order::error::OrderError;
use rustok_order::{ListOrderProjectionsRequest, OrderService, ReadOrderProjectionRequest};
use rustok_payment::PaymentError;
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime,
    super::common::{PaginatedResponse, ensure_permissions},
    AdminOrderDetailResponse, ListOrdersParams,
};
use crate::dto::{
    CancelOrderInput, DeliverOrderInput, MarkPaidOrderInput, OrderResponse, ShipOrderInput,
};

const ADMIN_ORDER_OWNER: &str = "rustok_order.admin_orders";
const ADMIN_ORDER_BOUNDARY: &str = "commerce_admin_order_http";
const ADMIN_ORDER_DETAIL_PAYMENT_OWNER: &str = "rustok_payment.admin_order_detail";
const ADMIN_ORDER_DETAIL_PAYMENT_OPERATION: &str = "find_latest_payment_collection_by_order";
const ADMIN_ORDER_DETAIL_FULFILLMENT_OWNER: &str = "rustok_fulfillment.admin_order_detail";
const ADMIN_ORDER_DETAIL_FULFILLMENT_OPERATION: &str = "find_fulfillment_by_order";

type AdminOrderHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

struct AdminOrderDetailOwnerErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

fn payment_detail_error_facts(error: &PaymentError) -> AdminOrderDetailOwnerErrorFacts {
    match error {
        PaymentError::PaymentCollectionNotFound(id)
        | PaymentError::PaymentNotFound(id)
        | PaymentError::RefundNotFound(id) => AdminOrderDetailOwnerErrorFacts {
            error_variant: match error {
                PaymentError::PaymentCollectionNotFound(_) => "payment_collection_not_found",
                PaymentError::PaymentNotFound(_) => "payment_not_found",
                PaymentError::RefundNotFound(_) => "refund_not_found",
                _ => unreachable!(),
            },
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 1,
            uuid_non_nil_count: if id.is_nil() { 0 } else { 1 },
            opaque_payload_present: false,
        },
        PaymentError::Validation(message) => AdminOrderDetailOwnerErrorFacts {
            error_variant: "validation",
            text_field_count: 1,
            text_total_length: message.chars().count(),
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        },
        PaymentError::InvalidTransition { from, to } => AdminOrderDetailOwnerErrorFacts {
            error_variant: "state_conflict",
            text_field_count: 2,
            text_total_length: from.chars().count() + to.chars().count(),
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        },
        PaymentError::ProviderRejected { .. } => AdminOrderDetailOwnerErrorFacts {
            error_variant: "provider_rejected",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: true,
        },
        PaymentError::ProviderUnavailable { .. } => AdminOrderDetailOwnerErrorFacts {
            error_variant: "provider_unavailable",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: true,
        },
        PaymentError::ProviderInvalidResponse { .. } => AdminOrderDetailOwnerErrorFacts {
            error_variant: "provider_invalid_response",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: true,
        },
        PaymentError::ProviderOutcomeUnknown { .. } => AdminOrderDetailOwnerErrorFacts {
            error_variant: "provider_outcome_unknown",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: true,
        },
        PaymentError::ProviderConfiguration { .. } => AdminOrderDetailOwnerErrorFacts {
            error_variant: "provider_configuration",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: true,
        },
        PaymentError::Database(_) => AdminOrderDetailOwnerErrorFacts {
            error_variant: "database",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: true,
        },
    }
}

fn fulfillment_detail_error_facts(
    error: &FulfillmentError,
) -> AdminOrderDetailOwnerErrorFacts {
    match error {
        FulfillmentError::Validation(message) => AdminOrderDetailOwnerErrorFacts {
            error_variant: "validation",
            text_field_count: 1,
            text_total_length: message.chars().count(),
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        },
        FulfillmentError::ShippingOptionNotFound(id)
        | FulfillmentError::FulfillmentNotFound(id) => AdminOrderDetailOwnerErrorFacts {
            error_variant: match error {
                FulfillmentError::ShippingOptionNotFound(_) => "shipping_option_not_found",
                FulfillmentError::FulfillmentNotFound(_) => "fulfillment_not_found",
                _ => unreachable!(),
            },
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 1,
            uuid_non_nil_count: if id.is_nil() { 0 } else { 1 },
            opaque_payload_present: false,
        },
        FulfillmentError::InvalidTransition { from, to } => AdminOrderDetailOwnerErrorFacts {
            error_variant: "state_conflict",
            text_field_count: 2,
            text_total_length: from.chars().count() + to.chars().count(),
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        },
        FulfillmentError::Database(_) => AdminOrderDetailOwnerErrorFacts {
            error_variant: "database",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: true,
        },
    }
}

struct AdminOrderErrorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
    order_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminOrderErrorContext {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            actor_id,
            order_id,
            customer_id,
            operation,
        }
    }
}

struct AdminOrderReadDiagnosticContext {
    tenant_id: &'static str,
    actor_id: &'static str,
    order_id: &'static str,
    customer_id: &'static str,
    operation: &'static str,
}

impl From<&AdminOrderErrorContext> for AdminOrderReadDiagnosticContext {
    fn from(context: &AdminOrderErrorContext) -> Self {
        Self {
            tenant_id: uuid_shape(context.tenant_id),
            actor_id: uuid_shape(context.actor_id),
            order_id: optional_uuid_shape(context.order_id),
            customer_id: optional_uuid_shape(context.customer_id),
            operation: context.operation,
        }
    }
}

struct AdminOrderReadPortDiagnosticContext {
    correlation_id: &'static str,
    actor: &'static str,
    channel: &'static str,
    locale: usize,
    deadline_ms: Option<u64>,
}

impl From<&PortContext> for AdminOrderReadPortDiagnosticContext {
    fn from(context: &PortContext) -> Self {
        Self {
            correlation_id: text_presence_shape(context.correlation_id.as_str()),
            actor: text_presence_shape(context.actor.id.as_str()),
            channel: optional_text_presence_shape(context.channel.as_deref()),
            locale: context.locale.len(),
            deadline_ms: context.deadline_ms,
        }
    }
}

struct AdminOrderReadPortDiagnosticError<'a> {
    code: &'a str,
    retryable: bool,
}

impl std::fmt::Debug for AdminOrderReadPortDiagnosticError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

struct AdminOrderMutationDiagnosticContext {
    tenant_id: &'static str,
    actor_id: &'static str,
    order_id: &'static str,
    customer_id: &'static str,
    operation: &'static str,
}

impl From<&AdminOrderErrorContext> for AdminOrderMutationDiagnosticContext {
    fn from(context: &AdminOrderErrorContext) -> Self {
        Self {
            tenant_id: uuid_shape(context.tenant_id),
            actor_id: uuid_shape(context.actor_id),
            order_id: optional_uuid_shape(context.order_id),
            customer_id: optional_uuid_shape(context.customer_id),
            operation: context.operation,
        }
    }
}

struct AdminOrderMutationDiagnosticError;

impl std::fmt::Debug for AdminOrderMutationDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

fn uuid_shape(value: Uuid) -> &'static str {
    if value.is_nil() { "nil" } else { "non_nil" }
}

fn optional_uuid_shape(value: Option<Uuid>) -> &'static str {
    match value {
        None => "absent",
        Some(value) if value.is_nil() => "present_nil",
        Some(_) => "present_non_nil",
    }
}

fn text_presence_shape(value: &str) -> &'static str {
    if value.is_empty() {
        "empty"
    } else {
        "present_non_empty"
    }
}

fn optional_text_presence_shape(value: Option<&str>) -> &'static str {
    match value {
        None => "absent",
        Some("") => "present_empty",
        Some(_) => "present_non_empty",
    }
}

fn admin_order_read_port_context(
    tenant_id: Uuid,
    auth: &AuthContext,
    request_context: &RequestContext,
    order_id: Option<Uuid>,
    operation: &'static str,
) -> PortContext {
    let resource_id = order_id.unwrap_or(tenant_id);
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-order:{operation}:{resource_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn map_admin_order_port_error(
    context: AdminOrderErrorContext,
    port_context: &PortContext,
    owner_operation: &'static str,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_order_invalid",
            "Order request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_admin_order_state_conflict",
            "Order operation conflicts with the current state",
            "state_conflict",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::UNAUTHORIZED,
            "commerce_permission_denied",
            "Permission denied",
            "forbidden",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_order_storage_unavailable",
            "Order storage is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_order_failed",
            "Order operation could not be completed safely",
            "invariant_violation",
        ),
    };
    let context = AdminOrderReadDiagnosticContext::from(&context);
    let port_context = AdminOrderReadPortDiagnosticContext::from(port_context);
    let error = AdminOrderReadPortDiagnosticError {
        code: error.code.as_str(),
        retryable: error.retryable,
    };
    tracing::error!(
        error = ?error,
        owner = ADMIN_ORDER_OWNER,
        owner_operation,
        correlation_id = %port_context.correlation_id,
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        order_id = ?context.order_id,
        customer_id = ?context.customer_id,
        operation = %context.operation,
        actor = ?port_context.actor,
        channel = ?port_context.channel,
        locale = %port_context.locale,
        deadline_ms = ?port_context.deadline_ms,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_BOUNDARY,
        "commerce admin order owner read failed"
    );
    HttpError::new(status, code, message)
}

fn admin_order_error_policy(error: &OrderError) -> AdminOrderHttpPolicy {
    match error {
        OrderError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_order_invalid",
            "Order request is invalid",
            "validation",
        ),
        OrderError::OrderNotFound(_)
        | OrderError::OrderReturnNotFound(_)
        | OrderError::OrderChangeNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        OrderError::InvalidTransition { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_order_state_conflict",
            "Order operation conflicts with the current state",
            "state_conflict",
        ),
        OrderError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_order_storage_unavailable",
            "Order storage is temporarily unavailable",
            "database",
        ),
        OrderError::Core(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_order_failed",
            "Order operation could not be completed safely",
            "core",
        ),
    }
}

fn map_admin_order_error(mut context: AdminOrderErrorContext, error: OrderError) -> HttpError {
    if let OrderError::OrderNotFound(id) = &error {
        context.order_id = Some(*id);
    }
    let (status, code, message, error_kind) = admin_order_error_policy(&error);
    let context = AdminOrderMutationDiagnosticContext::from(&context);
    let error = AdminOrderMutationDiagnosticError;
    tracing::error!(
        error = ?error,
        owner = ADMIN_ORDER_OWNER,
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        order_id = ?context.order_id,
        customer_id = ?context.customer_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_BOUNDARY,
        "commerce admin order operation failed"
    );
    HttpError::new(status, code, message)
}

/// Show admin ecommerce order
#[utoipa::path(
    get,
    path = "/admin/orders",
    tag = "admin",
    params(ListOrdersParams),
    responses(
        (status = 200, description = "Orders", body = PaginatedResponse<OrderResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_orders(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<ListOrdersParams>,
) -> HttpResult<Json<PaginatedResponse<OrderResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_LIST],
        "Permission denied: orders:list required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let customer_id = params.customer_id;
    let read_context =
        admin_order_read_port_context(tenant.id, &auth, &request_context, None, "list_orders");
    let page = runtime
        .order_read_port()
        .list_order_projections(
            read_context.clone(),
            ListOrderProjectionsRequest {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status,
                customer_id,
                tenant_default_locale: Some(tenant.default_locale.clone()),
            },
        )
        .await
        .map_err(|error| {
            map_admin_order_port_error(
                AdminOrderErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    None,
                    customer_id,
                    "list_orders",
                ),
                &read_context,
                "list_order_projections",
                error,
            )
        })?;

    Ok(Json(PaginatedResponse {
        data: page.items,
        meta: super::super::common::PaginationMeta::new(
            pagination.page,
            pagination.limit(),
            page.total,
        ),
    }))
}

#[utoipa::path(
    get,
    path = "/admin/orders/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    responses(
        (status = 200, description = "Order details", body = AdminOrderDetailResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn show_order(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<AdminOrderDetailResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;

    let read_context =
        admin_order_read_port_context(tenant.id, &auth, &request_context, Some(id), "get_order");
    let order = runtime
        .order_read_port()
        .read_order_projection(
            read_context.clone(),
            ReadOrderProjectionRequest {
                order_id: id,
                tenant_default_locale: Some(tenant.default_locale.clone()),
            },
        )
        .await
        .map_err(|error| {
            map_admin_order_port_error(
                AdminOrderErrorContext::new(tenant.id, auth.user_id, Some(id), None, "get_order"),
                &read_context,
                "read_order_projection",
                error,
            )
        })?;
    let payment_context =
        admin_order_read_port_context(tenant.id, &auth, &request_context, Some(id), "get_order_payment");
    let payment_collection = runtime
        .payment_order_read_port()
        .find_latest_collection_by_order(
            payment_context.clone(),
            rustok_payment::LatestPaymentCollectionByOrderRequest { order_id: id },
        )
        .await
        .map_err(|error| map_order_detail_payment_port_error(id, error))?;

    let fulfillment_context = admin_order_read_port_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "get_order_fulfillment",
    );
    let fulfillment = runtime
        .fulfillment_read_port()
        .find_latest_fulfillment_by_order_projection(
            fulfillment_context.clone(),
            rustok_fulfillment::FindLatestFulfillmentByOrderProjectionRequest { order_id: id },
        )
        .await
        .map_err(|error| map_order_detail_fulfillment_port_error(id, error))?;

    Ok(Json(AdminOrderDetailResponse {
        order,
        payment_collection,
        fulfillment,
    }))
}

fn map_order_detail_payment_port_error(order_id: Uuid, error: PortError) -> HttpError {
    let (status, code, message, error_kind) = match error.kind {
        PortErrorKind::Validation => (
            axum::http::StatusCode::BAD_REQUEST,
            "commerce_admin_payment_invalid",
            "Payment request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            axum::http::StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PortErrorKind::Conflict => (
            axum::http::StatusCode::CONFLICT,
            "commerce_admin_payment_state_conflict",
            "Payment operation conflicts with the current state",
            "state_conflict",
        ),
        PortErrorKind::Forbidden => (
            axum::http::StatusCode::UNAUTHORIZED,
            "commerce_permission_denied",
            "Permission denied",
            "forbidden",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_storage_unavailable",
            "Payment storage is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_payment_failed",
            "Payment data could not be read safely",
            "invariant_violation",
        ),
    };
    tracing::error!(
        owner = ADMIN_ORDER_DETAIL_PAYMENT_OWNER,
        order_id = uuid_shape(order_id),
        operation = ADMIN_ORDER_DETAIL_PAYMENT_OPERATION,
        error_kind,
        internal_code_length = error.code.chars().count(),
        retryable = error.retryable,
        public_code = code,
        status = %status,
        boundary = "commerce_admin_order_detail_http",
        "commerce admin order detail payment owner-port lookup failed with bounded diagnostics"
    );
    HttpError::new(status, code, message)
}

fn map_order_detail_fulfillment_port_error(
    order_id: Uuid,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = match error.kind {
        PortErrorKind::Validation => (
            axum::http::StatusCode::BAD_REQUEST,
            "commerce_admin_fulfillment_invalid",
            "Fulfillment request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            axum::http::StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PortErrorKind::Conflict => (
            axum::http::StatusCode::CONFLICT,
            "commerce_admin_fulfillment_state_conflict",
            "Fulfillment operation conflicts with the current state",
            "state_conflict",
        ),
        PortErrorKind::Forbidden => (
            axum::http::StatusCode::UNAUTHORIZED,
            "commerce_permission_denied",
            "Permission denied",
            "forbidden",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_fulfillment_storage_unavailable",
            "Fulfillment storage is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_fulfillment_failed",
            "Fulfillment data could not be read safely",
            "invariant_violation",
        ),
    };
    tracing::error!(
        owner = ADMIN_ORDER_DETAIL_FULFILLMENT_OWNER,
        order_id = uuid_shape(order_id),
        operation = ADMIN_ORDER_DETAIL_FULFILLMENT_OPERATION,
        error_kind,
        internal_code_length = error.code.chars().count(),
        retryable = error.retryable,
        public_code = code,
        status = %status,
        boundary = "commerce_admin_order_detail_http",
        "commerce admin order detail fulfillment owner-port lookup failed with bounded diagnostics"
    );
    HttpError::new(status, code, message)
}





/// Mark admin ecommerce order as paid
#[utoipa::path(
    post,
    path = "/admin/orders/{id}/mark-paid",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = MarkPaidOrderInput,
    responses(
        (status = 200, description = "Order marked paid", body = OrderResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn mark_order_paid(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<MarkPaidOrderInput>,
) -> HttpResult<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .mark_paid(
            tenant.id,
            auth.user_id,
            id,
            input.payment_id,
            input.payment_method,
        )
        .await
        .map_err(|error| {
            map_admin_order_error(
                AdminOrderErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(id),
                    None,
                    "mark_order_paid",
                ),
                error,
            )
        })?;

    Ok(Json(order))
}

/// Ship admin ecommerce order
#[utoipa::path(
    post,
    path = "/admin/orders/{id}/ship",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = ShipOrderInput,
    responses(
        (status = 200, description = "Order shipped", body = OrderResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn ship_order(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ShipOrderInput>,
) -> HttpResult<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .ship_order(
            tenant.id,
            auth.user_id,
            id,
            input.tracking_number,
            input.carrier,
        )
        .await
        .map_err(|error| {
            map_admin_order_error(
                AdminOrderErrorContext::new(tenant.id, auth.user_id, Some(id), None, "ship_order"),
                error,
            )
        })?;

    Ok(Json(order))
}

/// Deliver admin ecommerce order
#[utoipa::path(
    post,
    path = "/admin/orders/{id}/deliver",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = DeliverOrderInput,
    responses(
        (status = 200, description = "Order delivered", body = OrderResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn deliver_order(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<DeliverOrderInput>,
) -> HttpResult<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .deliver_order(tenant.id, auth.user_id, id, input.delivered_signature)
        .await
        .map_err(|error| {
            map_admin_order_error(
                AdminOrderErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(id),
                    None,
                    "deliver_order",
                ),
                error,
            )
        })?;

    Ok(Json(order))
}

/// Cancel admin ecommerce order
#[utoipa::path(
    post,
    path = "/admin/orders/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = CancelOrderInput,
    responses(
        (status = 200, description = "Order cancelled", body = OrderResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn cancel_order(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelOrderInput>,
) -> HttpResult<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .cancel_order(tenant.id, auth.user_id, id, input.reason)
        .await
        .map_err(|error| {
            map_admin_order_error(
                AdminOrderErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(id),
                    None,
                    "cancel_order",
                ),
                error,
            )
        })?;

    Ok(Json(order))
}
