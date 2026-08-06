use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext};
use rustok_payment::PaymentService;
use rustok_payment::error::PaymentError;
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime,
    super::common::{PaginatedResponse, ensure_permissions},
    ListPaymentCollectionsParams, ListRefundsParams,
};
use crate::PaymentOrchestrationError;
use crate::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
    CompleteRefundInput, CreateRefundInput, ListPaymentCollectionsInput, ListRefundsInput,
    PaymentCollectionResponse, RefundResponse,
};

const MAX_REFUND_CREATION_KEY_LENGTH: usize = 191;
const ADMIN_PAYMENT_OWNER: &str = "rustok_payment.admin_payments";
const ADMIN_PAYMENT_BOUNDARY: &str = "commerce_admin_payment_http";

type AdminPaymentHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

#[derive(Clone, Copy)]
struct AdminPaymentErrorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
    payment_collection_id: Option<Uuid>,
    refund_id: Option<Uuid>,
    order_id: Option<Uuid>,
    cart_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminPaymentErrorContext {
    fn new(tenant_id: Uuid, actor_id: Uuid, operation: &'static str) -> Self {
        Self {
            tenant_id,
            actor_id,
            payment_collection_id: None,
            refund_id: None,
            order_id: None,
            cart_id: None,
            customer_id: None,
            operation,
        }
    }

    fn with_payment_collection_id(mut self, payment_collection_id: Option<Uuid>) -> Self {
        self.payment_collection_id = payment_collection_id;
        self
    }

    fn with_refund_id(mut self, refund_id: Option<Uuid>) -> Self {
        self.refund_id = refund_id;
        self
    }

    fn with_filters(
        mut self,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        customer_id: Option<Uuid>,
    ) -> Self {
        self.order_id = order_id;
        self.cart_id = cart_id;
        self.customer_id = customer_id;
        self
    }
}

struct AdminPaymentDiagnosticContext {
    tenant_id: &'static str,
    actor_id: &'static str,
    payment_collection_id: &'static str,
    refund_id: &'static str,
    order_id: &'static str,
    cart_id: &'static str,
    customer_id: &'static str,
    operation: &'static str,
}

impl From<&AdminPaymentErrorContext> for AdminPaymentDiagnosticContext {
    fn from(context: &AdminPaymentErrorContext) -> Self {
        Self {
            tenant_id: uuid_shape(context.tenant_id),
            actor_id: uuid_shape(context.actor_id),
            payment_collection_id: optional_uuid_shape(context.payment_collection_id),
            refund_id: optional_uuid_shape(context.refund_id),
            order_id: optional_uuid_shape(context.order_id),
            cart_id: optional_uuid_shape(context.cart_id),
            customer_id: optional_uuid_shape(context.customer_id),
            operation: context.operation,
        }
    }
}

struct AdminPaymentDiagnosticError;

impl std::fmt::Debug for AdminPaymentDiagnosticError {
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
    Query(params): Query<ListPaymentCollectionsParams>,
) -> HttpResult<Json<PaginatedResponse<PaymentCollectionResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    let pagination = params.pagination.unwrap_or_default();
    let order_id = params.order_id;
    let cart_id = params.cart_id;
    let customer_id = params.customer_id;
    let (collections, total) = PaymentService::new(runtime.db_clone())
        .list_collections(
            tenant.id,
            ListPaymentCollectionsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status,
                order_id,
                cart_id,
                customer_id,
            },
        )
        .await
        .map_err(|error| {
            map_admin_payment_error(
                AdminPaymentErrorContext::new(tenant.id, auth.user_id, "list_payment_collections")
                    .with_filters(order_id, cart_id, customer_id),
                error,
            )
        })?;
    Ok(Json(PaginatedResponse {
        data: collections,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
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
    Path(id): Path<Uuid>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    let collection = PaymentService::new(runtime.db_clone())
        .get_collection(tenant.id, id)
        .await
        .map_err(|error| {
            map_admin_payment_error(
                AdminPaymentErrorContext::new(tenant.id, auth.user_id, "show_payment_collection")
                    .with_payment_collection_id(Some(id)),
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
    Path(id): Path<Uuid>,
    Json(input): Json<AuthorizePaymentInput>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    let collection = crate::PaymentOrchestrationService::new(runtime.db_clone())
        .with_provider_registry(runtime.payment_provider_registry())
        .authorize_collection(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_payment_orchestration_error(
                AdminPaymentErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    "authorize_payment_collection",
                )
                .with_payment_collection_id(Some(id)),
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
    Path(id): Path<Uuid>,
    Json(input): Json<CapturePaymentInput>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    let collection = crate::PaymentOrchestrationService::new(runtime.db_clone())
        .with_provider_registry(runtime.payment_provider_registry())
        .capture_collection(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_payment_orchestration_error(
                AdminPaymentErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    "capture_payment_collection",
                )
                .with_payment_collection_id(Some(id)),
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
    Path(id): Path<Uuid>,
    Json(input): Json<CancelPaymentInput>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    let collection = crate::PaymentOrchestrationService::new(runtime.db_clone())
        .with_provider_registry(runtime.payment_provider_registry())
        .cancel_collection(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_payment_orchestration_error(
                AdminPaymentErrorContext::new(tenant.id, auth.user_id, "cancel_payment_collection")
                    .with_payment_collection_id(Some(id)),
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
    let refund = crate::PaymentOrchestrationService::new(runtime.db_clone())
        .with_provider_registry(runtime.payment_provider_registry())
        .create_refund_idempotent(tenant.id, id, creation_key, input)
        .await
        .map_err(|error| {
            map_admin_payment_orchestration_error(
                AdminPaymentErrorContext::new(tenant.id, auth.user_id, "create_refund")
                    .with_payment_collection_id(Some(id)),
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
    Query(params): Query<ListRefundsParams>,
) -> HttpResult<Json<PaginatedResponse<RefundResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    let pagination = params.pagination.unwrap_or_default();
    let payment_collection_id = params.payment_collection_id;
    let order_id = params.order_id;
    let (refunds, total) = PaymentService::new(runtime.db_clone())
        .list_refunds(
            tenant.id,
            ListRefundsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                payment_collection_id,
                order_id,
                status: params.status,
            },
        )
        .await
        .map_err(|error| {
            map_admin_payment_error(
                AdminPaymentErrorContext::new(tenant.id, auth.user_id, "list_refunds")
                    .with_payment_collection_id(payment_collection_id)
                    .with_filters(order_id, None, None),
                error,
            )
        })?;
    Ok(Json(PaginatedResponse {
        data: refunds,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
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
    Path(id): Path<Uuid>,
) -> HttpResult<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    let refund = PaymentService::new(runtime.db_clone())
        .get_refund(tenant.id, id)
        .await
        .map_err(|error| {
            map_admin_payment_error(
                AdminPaymentErrorContext::new(tenant.id, auth.user_id, "show_refund")
                    .with_refund_id(Some(id)),
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
    Path(id): Path<Uuid>,
    Json(input): Json<CompleteRefundInput>,
) -> HttpResult<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    let refund = crate::PaymentOrchestrationService::new(runtime.db_clone())
        .with_provider_registry(runtime.payment_provider_registry())
        .complete_refund(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_payment_orchestration_error(
                AdminPaymentErrorContext::new(tenant.id, auth.user_id, "complete_refund")
                    .with_refund_id(Some(id)),
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
    Path(id): Path<Uuid>,
    Json(input): Json<CancelRefundInput>,
) -> HttpResult<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    let refund = crate::PaymentOrchestrationService::new(runtime.db_clone())
        .with_provider_registry(runtime.payment_provider_registry())
        .cancel_refund(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_payment_orchestration_error(
                AdminPaymentErrorContext::new(tenant.id, auth.user_id, "cancel_refund")
                    .with_refund_id(Some(id)),
                error,
            )
        })?;
    Ok(Json(refund))
}

fn payment_error_policy(error: &PaymentError) -> AdminPaymentHttpPolicy {
    match error {
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PaymentError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_payment_invalid",
            "Payment request is invalid",
            "validation",
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_state_conflict",
            "Payment operation conflicts with the current state",
            "state_conflict",
        ),
        PaymentError::ProviderUnavailable { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_provider_unavailable",
            "Payment provider is temporarily unavailable",
            "provider_unavailable",
        ),
        PaymentError::ProviderInvalidResponse { .. } => (
            StatusCode::BAD_GATEWAY,
            "commerce_admin_payment_provider_invalid_response",
            "Payment provider returned an invalid response; reconciliation may be required",
            "provider_invalid_response",
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_reconciliation_required",
            "Payment provider outcome is unknown and requires reconciliation",
            "provider_outcome_unknown",
        ),
        PaymentError::ProviderConfiguration { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_provider_not_configured",
            "Payment provider is not configured for this tenant",
            "provider_configuration",
        ),
        PaymentError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_storage_unavailable",
            "Payment storage is temporarily unavailable",
            "database",
        ),
    }
}

fn reserved_refund_error_policy(error: &PaymentError) -> AdminPaymentHttpPolicy {
    match error {
        PaymentError::ProviderOutcomeUnknown { .. }
        | PaymentError::ProviderInvalidResponse { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_refund_reconciliation_required",
            "Refund remains reserved while the provider outcome is reconciled",
            "refund_reconciliation_required",
        ),
        PaymentError::ProviderUnavailable { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_refund_provider_unavailable",
            "Refund remains reserved and the provider operation may be retried safely",
            "refund_provider_unavailable",
        ),
        error => payment_error_policy(error),
    }
}

fn adopt_payment_error_identity(context: &mut AdminPaymentErrorContext, error: &PaymentError) {
    match error {
        PaymentError::PaymentCollectionNotFound(id) | PaymentError::PaymentNotFound(id) => {
            context.payment_collection_id = Some(*id);
        }
        PaymentError::RefundNotFound(id) => context.refund_id = Some(*id),
        _ => {}
    }
}

fn admin_payment_http_error<E>(
    context: &AdminPaymentErrorContext,
    _error: &E,
    source_owner: &'static str,
    policy: AdminPaymentHttpPolicy,
) -> HttpError
where
    E: std::fmt::Debug,
{
    let (status, code, message, error_kind) = policy;
    let context = AdminPaymentDiagnosticContext::from(context);
    let error = AdminPaymentDiagnosticError;
    tracing::error!(
        error = ?error,
        owner = ADMIN_PAYMENT_OWNER,
        source_owner,
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        payment_collection_id = ?context.payment_collection_id,
        refund_id = ?context.refund_id,
        order_id = ?context.order_id,
        cart_id = ?context.cart_id,
        customer_id = ?context.customer_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_PAYMENT_BOUNDARY,
        "commerce admin payment operation failed"
    );
    HttpError::new(status, code, message)
}

fn map_admin_payment_error(
    mut context: AdminPaymentErrorContext,
    error: PaymentError,
) -> HttpError {
    adopt_payment_error_identity(&mut context, &error);
    let policy = payment_error_policy(&error);
    admin_payment_http_error(&context, &error, "rustok_payment", policy)
}

fn map_admin_payment_orchestration_error(
    mut context: AdminPaymentErrorContext,
    error: PaymentOrchestrationError,
) -> HttpError {
    let policy = match &error {
        PaymentOrchestrationError::Payment(source)
        | PaymentOrchestrationError::Provider(source) => {
            adopt_payment_error_identity(&mut context, source);
            payment_error_policy(source)
        }
        PaymentOrchestrationError::ProviderAfterRefundReservation { refund_id, source } => {
            context.refund_id = Some(*refund_id);
            reserved_refund_error_policy(source)
        }
    };
    admin_payment_http_error(&context, &error, "rustok_payment", policy)
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