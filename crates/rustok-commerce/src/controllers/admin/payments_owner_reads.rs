use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use rustok_api::{
    AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext,
};
use rustok_payment::{
    AuthorizeAdminPaymentCollectionRequest, CancelAdminPaymentCollectionRequest,
    CancelAdminRefundRequest, CaptureAdminPaymentCollectionRequest, CompleteAdminRefundRequest,
    CreateAdminRefundRequest, ListPaymentCollectionProjectionsRequest, ListRefundProjectionsRequest,
    ReadPaymentCollectionProjectionRequest, ReadRefundProjectionRequest,
};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

pub use super::payments_legacy::*;
use super::{
    super::CommerceHttpRuntime,
    super::common::{PaginatedResponse, ensure_permissions},
    ListPaymentCollectionsParams, ListRefundsParams,
};
use crate::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
    CompleteRefundInput, CreateRefundInput, PaymentCollectionResponse, RefundResponse,
};

const MAX_REFUND_CREATION_KEY_LENGTH: usize = 191;
const ADMIN_PAYMENT_READ_OWNER: &str = "rustok_payment.admin_read";
const ADMIN_PAYMENT_READ_BOUNDARY: &str = "commerce_admin_payment_read_http";
const ADMIN_PAYMENT_COMMAND_OWNER: &str = "rustok_payment.admin_collection_command";
const ADMIN_PAYMENT_COMMAND_BOUNDARY: &str = "commerce_admin_payment_collection_command_http";
const ADMIN_REFUND_COMMAND_OWNER: &str = "rustok_payment.admin_refund_command";
const ADMIN_REFUND_COMMAND_BOUNDARY: &str = "commerce_admin_refund_command_http";

type AdminPaymentReadHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);
type AdminPaymentCommandHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

fn admin_payment_read_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    resource_id: Option<Uuid>,
    operation: &'static str,
) -> PortContext {
    let scope = resource_id.unwrap_or(tenant.id);
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-payment-read:{operation}:{scope}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn admin_payment_collection_command_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    collection_id: Uuid,
    operation: &'static str,
) -> PortContext {
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-payment-command:{operation}:{collection_id}"),
    )
    .with_idempotency_key(format!(
        "admin-payment-collection:{collection_id}:{operation}"
    ))
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn admin_refund_create_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    collection_id: Uuid,
    creation_key: &str,
) -> PortContext {
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-refund-command:create:{collection_id}"),
    )
    .with_idempotency_key(creation_key.to_string())
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn admin_refund_transition_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    refund_id: Uuid,
    operation: &'static str,
) -> PortContext {
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-refund-command:{operation}:{refund_id}"),
    )
    .with_idempotency_key(format!("admin-refund:{refund_id}:{operation}"))
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn payment_read_error_policy(error: &PortError) -> AdminPaymentReadHttpPolicy {
    match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_payment_invalid",
            "Payment request is invalid",
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
            "commerce_admin_payment_state_conflict",
            "Payment operation conflicts with the current state",
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
            "commerce_admin_payment_storage_unavailable",
            "Payment storage is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_payment_failed",
            "Payment operation could not be completed safely",
            "invariant_violation",
        ),
    }
}

fn payment_command_error_policy(error: &PortError) -> AdminPaymentCommandHttpPolicy {
    match error.code.as_str() {
        "payment.refund_reserved_reconciliation_required" => (
            StatusCode::CONFLICT,
            "commerce_admin_refund_reconciliation_required",
            "Refund remains reserved while the provider outcome is reconciled",
            "refund_reconciliation_required",
        ),
        "payment.refund_reserved_provider_unavailable" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_refund_provider_unavailable",
            "Refund remains reserved and the provider operation may be retried safely",
            "refund_provider_unavailable",
        ),
        "payment.provider_unavailable" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_provider_unavailable",
            "Payment provider is temporarily unavailable",
            "provider_unavailable",
        ),
        "payment.provider_invalid_response" => (
            StatusCode::BAD_GATEWAY,
            "commerce_admin_payment_provider_invalid_response",
            "Payment provider returned an invalid response; reconciliation may be required",
            "provider_invalid_response",
        ),
        "payment.provider_outcome_unknown" => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_reconciliation_required",
            "Payment provider outcome is unknown and requires reconciliation",
            "provider_outcome_unknown",
        ),
        "payment.provider_not_configured" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_provider_not_configured",
            "Payment provider is not configured for this tenant",
            "provider_configuration",
        ),
        "payment.provider_rejected" | "payment.invalid_transition" => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_state_conflict",
            "Payment operation conflicts with the current state",
            "state_conflict",
        ),
        "payment.database_unavailable" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_storage_unavailable",
            "Payment storage is temporarily unavailable",
            "database",
        ),
        _ => match &error.kind {
            PortErrorKind::Validation => (
                StatusCode::BAD_REQUEST,
                "commerce_admin_payment_invalid",
                "Payment request is invalid",
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
                "commerce_admin_payment_state_conflict",
                "Payment operation conflicts with the current state",
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
                "commerce_admin_payment_storage_unavailable",
                "Payment storage is temporarily unavailable",
                "unavailable",
            ),
            PortErrorKind::InvariantViolation => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "commerce_admin_payment_failed",
                "Payment operation could not be completed safely",
                "invariant_violation",
            ),
        },
    }
}

fn map_payment_read_error(
    tenant_id: Uuid,
    actor_id: Uuid,
    resource_id: Option<Uuid>,
    operation: &'static str,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = payment_read_error_policy(&error);
    tracing::error!(
        owner = ADMIN_PAYMENT_READ_OWNER,
        tenant_id_non_nil = !tenant_id.is_nil(),
        actor_id_non_nil = !actor_id.is_nil(),
        resource_id_present = resource_id.is_some(),
        resource_id_non_nil = resource_id.map(|value| !value.is_nil()).unwrap_or(false),
        operation,
        correlation_id = %context.correlation_id,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_PAYMENT_READ_BOUNDARY,
        "commerce admin payment owner read failed"
    );
    HttpError::new(status, code, message)
}

fn map_payment_command_error(
    tenant_id: Uuid,
    actor_id: Uuid,
    collection_id: Uuid,
    operation: &'static str,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = payment_command_error_policy(&error);
    tracing::error!(
        owner = ADMIN_PAYMENT_COMMAND_OWNER,
        tenant_id_non_nil = !tenant_id.is_nil(),
        actor_id_non_nil = !actor_id.is_nil(),
        collection_id_non_nil = !collection_id.is_nil(),
        operation,
        correlation_id = %context.correlation_id,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_PAYMENT_COMMAND_BOUNDARY,
        "commerce admin payment owner command failed"
    );
    HttpError::new(status, code, message)
}

fn map_refund_command_error(
    tenant_id: Uuid,
    actor_id: Uuid,
    collection_id: Option<Uuid>,
    refund_id: Option<Uuid>,
    operation: &'static str,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = payment_command_error_policy(&error);
    tracing::error!(
        owner = ADMIN_REFUND_COMMAND_OWNER,
        tenant_id_non_nil = !tenant_id.is_nil(),
        actor_id_non_nil = !actor_id.is_nil(),
        collection_id_present = collection_id.is_some(),
        collection_id_non_nil = collection_id.map(|value| !value.is_nil()).unwrap_or(false),
        refund_id_present = refund_id.is_some(),
        refund_id_non_nil = refund_id.map(|value| !value.is_nil()).unwrap_or(false),
        operation,
        correlation_id = %context.correlation_id,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_REFUND_COMMAND_BOUNDARY,
        "commerce admin refund owner command failed"
    );
    HttpError::new(status, code, message)
}

#[utoipa::path(
    get,
    path = "/admin/payment-collections",
    tag = "admin",
    params(ListPaymentCollectionsParams),
    responses((status = 200, description = "Payment collections", body = PaginatedResponse<PaymentCollectionResponse>), (status = 401, description = "Unauthorized"))
)]
pub async fn list_payment_collections(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<ListPaymentCollectionsParams>,
) -> HttpResult<Json<PaginatedResponse<PaymentCollectionResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    let pagination = params.pagination.unwrap_or_default();
    let context = admin_payment_read_context(
        &tenant,
        &auth,
        &request_context,
        None,
        "list_payment_collections",
    );
    let page = runtime
        .payment_admin_read_port()
        .list_payment_collection_projections(
            context.clone(),
            ListPaymentCollectionProjectionsRequest {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status,
                order_id: params.order_id,
                cart_id: params.cart_id,
                customer_id: params.customer_id,
            },
        )
        .await
        .map_err(|error| {
            map_payment_read_error(
                tenant.id,
                auth.user_id,
                None,
                "list_payment_collections",
                &context,
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
    path = "/admin/payment-collections/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    responses((status = 200, description = "Payment collection details", body = PaymentCollectionResponse), (status = 401, description = "Unauthorized"), (status = 404, description = "Payment collection not found"))
)]
pub async fn show_payment_collection(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    let context = admin_payment_read_context(
        &tenant,
        &auth,
        &request_context,
        Some(id),
        "show_payment_collection",
    );
    let collection = runtime
        .payment_admin_read_port()
        .read_payment_collection_projection(
            context.clone(),
            ReadPaymentCollectionProjectionRequest { collection_id: id },
        )
        .await
        .map_err(|error| {
            map_payment_read_error(
                tenant.id,
                auth.user_id,
                Some(id),
                "show_payment_collection",
                &context,
                error,
            )
        })?;
    Ok(Json(collection))
}

#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/authorize",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    request_body = AuthorizePaymentInput,
    responses((status = 200, description = "Payment collection authorized", body = PaymentCollectionResponse), (status = 401, description = "Unauthorized"), (status = 404, description = "Payment collection not found"))
)]
pub async fn authorize_payment_collection(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<AuthorizePaymentInput>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    let context = admin_payment_collection_command_context(
        &tenant,
        &auth,
        &request_context,
        id,
        "authorize",
    );
    let collection = runtime
        .payment_admin_collection_command_port()
        .authorize_payment_collection(
            context.clone(),
            AuthorizeAdminPaymentCollectionRequest {
                collection_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_payment_command_error(
                tenant.id,
                auth.user_id,
                id,
                "authorize_payment_collection",
                &context,
                error,
            )
        })?;
    Ok(Json(collection))
}

#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/capture",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    request_body = CapturePaymentInput,
    responses((status = 200, description = "Payment collection captured", body = PaymentCollectionResponse), (status = 401, description = "Unauthorized"), (status = 404, description = "Payment collection not found"))
)]
pub async fn capture_payment_collection(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CapturePaymentInput>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    let context = admin_payment_collection_command_context(
        &tenant,
        &auth,
        &request_context,
        id,
        "capture",
    );
    let collection = runtime
        .payment_admin_collection_command_port()
        .capture_payment_collection(
            context.clone(),
            CaptureAdminPaymentCollectionRequest {
                collection_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_payment_command_error(
                tenant.id,
                auth.user_id,
                id,
                "capture_payment_collection",
                &context,
                error,
            )
        })?;
    Ok(Json(collection))
}

#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    request_body = CancelPaymentInput,
    responses((status = 200, description = "Payment collection cancelled", body = PaymentCollectionResponse), (status = 401, description = "Unauthorized"), (status = 404, description = "Payment collection not found"))
)]
pub async fn cancel_payment_collection(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelPaymentInput>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    let context = admin_payment_collection_command_context(
        &tenant,
        &auth,
        &request_context,
        id,
        "cancel",
    );
    let collection = runtime
        .payment_admin_collection_command_port()
        .cancel_payment_collection(
            context.clone(),
            CancelAdminPaymentCollectionRequest {
                collection_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_payment_command_error(
                tenant.id,
                auth.user_id,
                id,
                "cancel_payment_collection",
                &context,
                error,
            )
        })?;
    Ok(Json(collection))
}

#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/refunds",
    tag = "admin",
    params(
        ("id" = Uuid, Path, description = "Payment collection ID"),
        ("Idempotency-Key" = String, Header, description = "Stable refund creation identity, maximum 191 bytes")
    ),
    request_body = CreateRefundInput,
    responses(
        (status = 201, description = "Refund created or replayed", body = RefundResponse),
        (status = 400, description = "Missing, invalid, or conflicting idempotency key"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Payment collection not found")
    )
)]
pub async fn create_refund(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateRefundInput>,
) -> HttpResult<(StatusCode, Json<RefundResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    let creation_key = refund_creation_key(&headers)?;
    let context = admin_refund_create_context(
        &tenant,
        &auth,
        &request_context,
        id,
        creation_key.as_str(),
    );
    let refund = runtime
        .payment_admin_refund_command_port()
        .create_refund(
            context.clone(),
            CreateAdminRefundRequest {
                collection_id: id,
                creation_key,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_refund_command_error(
                tenant.id,
                auth.user_id,
                Some(id),
                None,
                "create_refund",
                &context,
                error,
            )
        })?;
    Ok((StatusCode::CREATED, Json(refund)))
}

#[utoipa::path(
    get,
    path = "/admin/refunds",
    tag = "admin",
    params(ListRefundsParams),
    responses((status = 200, description = "Refunds", body = PaginatedResponse<RefundResponse>), (status = 401, description = "Unauthorized"))
)]
pub async fn list_refunds(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<ListRefundsParams>,
) -> HttpResult<Json<PaginatedResponse<RefundResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    let pagination = params.pagination.unwrap_or_default();
    let context = admin_payment_read_context(&tenant, &auth, &request_context, None, "list_refunds");
    let page = runtime
        .payment_admin_read_port()
        .list_refund_projections(
            context.clone(),
            ListRefundProjectionsRequest {
                page: pagination.page,
                per_page: pagination.limit(),
                payment_collection_id: params.payment_collection_id,
                order_id: params.order_id,
                status: params.status,
            },
        )
        .await
        .map_err(|error| {
            map_payment_read_error(
                tenant.id,
                auth.user_id,
                None,
                "list_refunds",
                &context,
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
    path = "/admin/refunds/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Refund ID")),
    responses((status = 200, description = "Refund details", body = RefundResponse), (status = 401, description = "Unauthorized"), (status = 404, description = "Refund not found"))
)]
pub async fn show_refund(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    let context = admin_payment_read_context(&tenant, &auth, &request_context, Some(id), "show_refund");
    let refund = runtime
        .payment_admin_read_port()
        .read_refund_projection(
            context.clone(),
            ReadRefundProjectionRequest { refund_id: id },
        )
        .await
        .map_err(|error| {
            map_payment_read_error(
                tenant.id,
                auth.user_id,
                Some(id),
                "show_refund",
                &context,
                error,
            )
        })?;
    Ok(Json(refund))
}

#[utoipa::path(
    post,
    path = "/admin/refunds/{id}/complete",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Refund ID")),
    request_body = CompleteRefundInput,
    responses((status = 200, description = "Refund completed", body = RefundResponse), (status = 401, description = "Unauthorized"), (status = 404, description = "Refund not found"))
)]
pub async fn complete_refund(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CompleteRefundInput>,
) -> HttpResult<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    let context = admin_refund_transition_context(
        &tenant,
        &auth,
        &request_context,
        id,
        "complete",
    );
    let refund = runtime
        .payment_admin_refund_command_port()
        .complete_refund(
            context.clone(),
            CompleteAdminRefundRequest {
                refund_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_refund_command_error(
                tenant.id,
                auth.user_id,
                None,
                Some(id),
                "complete_refund",
                &context,
                error,
            )
        })?;
    Ok(Json(refund))
}

#[utoipa::path(
    post,
    path = "/admin/refunds/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Refund ID")),
    request_body = CancelRefundInput,
    responses((status = 200, description = "Refund cancelled", body = RefundResponse), (status = 401, description = "Unauthorized"), (status = 404, description = "Refund not found"))
)]
pub async fn cancel_refund(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelRefundInput>,
) -> HttpResult<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    let context = admin_refund_transition_context(
        &tenant,
        &auth,
        &request_context,
        id,
        "cancel",
    );
    let refund = runtime
        .payment_admin_refund_command_port()
        .cancel_refund(
            context.clone(),
            CancelAdminRefundRequest {
                refund_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_refund_command_error(
                tenant.id,
                auth.user_id,
                None,
                Some(id),
                "cancel_refund",
                &context,
                error,
            )
        })?;
    Ok(Json(refund))
}

fn refund_creation_key(headers: &HeaderMap) -> HttpResult<String> {
    let value = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            HttpError::bad_request(
                "refund_idempotency_key_required",
                "Idempotency-Key header is required",
            )
        })?;
    if value.len() > MAX_REFUND_CREATION_KEY_LENGTH {
        return Err(HttpError::bad_request(
            "refund_idempotency_key_invalid",
            format!("Idempotency-Key must contain at most {MAX_REFUND_CREATION_KEY_LENGTH} bytes"),
        ));
    }
    Ok(value.to_string())
}