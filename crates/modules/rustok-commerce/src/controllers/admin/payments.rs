//! Legacy admin payment controller, retained as explicit source debt.
//!
//! The mounted admin payment surface is `payments_owner_reads.rs` (host-selected owner ports).
//! This module keeps the pre-cutover concrete-service controller verbatim: it is re-exported
//! through `pub use super::payments_legacy::*;` for OpenAPI/compatibility surface and stays off
//! the router, so the cutover remains reversible and auditable.
#![allow(dead_code)]
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use rustok_api::{AuthContext, Permission, RequestContext, TenantContext};
use rustok_payment::PaymentService;
use rustok_payment::error::PaymentError;
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime,
    super::common::{PaginatedResponse, PaginationMeta, ensure_permissions},
    ListPaymentCollectionsParams, ListRefundsParams,
};
use crate::PaymentOrchestrationError;
use crate::PaymentOrchestrationService;
use crate::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
    CompleteRefundInput, CreateRefundInput, ListPaymentCollectionsInput, ListRefundsInput,
    PaymentCollectionResponse, RefundResponse,
};

const ADMIN_PAYMENT_OWNER: &str = "rustok_payment.admin_payments";
const ADMIN_PAYMENT_BOUNDARY: &str = "commerce_admin_payment_http";
const MAX_REFUND_CREATION_KEY_LENGTH: usize = 191;

type AdminPaymentHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

/// Typed admin payment error context: truthful identities, no raw payloads.
pub struct AdminPaymentErrorContext {
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
    pub fn new(tenant_id: Uuid, actor_id: Uuid, operation: &'static str) -> Self {
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

    pub fn with_payment_collection_id(mut self, payment_collection_id: Option<Uuid>) -> Self {
        self.payment_collection_id = payment_collection_id;
        self
    }

    pub fn with_refund_id(mut self, refund_id: Option<Uuid>) -> Self {
        self.refund_id = refund_id;
        self
    }

    pub fn with_filters(
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

/// Bounded diagnostic projection: shape only, never raw identity values.
pub struct AdminPaymentDiagnosticContext {
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

fn payment_error_policy(error: &PaymentError) -> AdminPaymentHttpPolicy {
    match error {
        PaymentError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_payment_invalid",
            "Payment request is invalid",
            "validation",
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PaymentError::InvalidTransition { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_state_conflict",
            "Payment operation conflicts with the current state",
            "state_conflict",
        ),
        PaymentError::ProviderRejected { .. } => (
            StatusCode::BAD_GATEWAY,
            "commerce_admin_payment_provider_rejected",
            "Payment provider rejected the operation",
            "provider_rejected",
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
            "Payment provider returned an invalid response",
            "provider_invalid_response",
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_reconciliation_required",
            "Payment provider outcome requires reconciliation",
            "reconciliation_required",
        ),
        PaymentError::ProviderConfiguration { .. } => (
            StatusCode::BAD_GATEWAY,
            "commerce_admin_payment_provider_not_configured",
            "Payment provider is not configured",
            "provider_not_configured",
        ),
        PaymentError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_storage_unavailable",
            "Payment storage is temporarily unavailable",
            "database",
        ),
    }
}

/// A provider failure raised after a refund was reserved keeps the refund reserved, so the
/// caller learns that fact instead of a generic provider failure.
fn reserved_refund_error_policy(error: &PaymentError) -> AdminPaymentHttpPolicy {
    match error {
        PaymentError::ProviderOutcomeUnknown { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_refund_reconciliation_required",
            "Refund remains reserved while the provider outcome is reconciled",
            "reconciliation_required",
        ),
        PaymentError::ProviderInvalidResponse { .. } | PaymentError::ProviderUnavailable { .. } => {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "commerce_admin_refund_provider_unavailable",
                "Refund remains reserved and the provider operation may be retried safely",
                "refund_reserved_retryable",
            )
        }
        error => payment_error_policy(error),
    }
}

fn adopt_payment_error_identity(context: &mut AdminPaymentErrorContext, error: &PaymentError) {
    match error {
        PaymentError::PaymentCollectionNotFound(id) | PaymentError::PaymentNotFound(id) => {
            context.payment_collection_id = Some(*id);
        }
        PaymentError::RefundNotFound(id) => {
            context.refund_id = Some(*id);
        }
        _ => {}
    }
}

/// One shared bounded diagnostic event for every admin payment failure.
fn admin_payment_http_error<E>(
    context: &AdminPaymentErrorContext,
    _error: &E,
    source_owner: &str,
    policy: AdminPaymentHttpPolicy,
) -> HttpError {
    let context = AdminPaymentDiagnosticContext::from(context);
    let error = AdminPaymentDiagnosticError;
    let (status, code, message, error_kind) = policy;
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
        "commerce admin payment owner call failed with bounded diagnostics"
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
    match &error {
        PaymentOrchestrationError::Payment(source)
        | PaymentOrchestrationError::Provider(source) => {
            adopt_payment_error_identity(&mut context, source);
            let policy = payment_error_policy(source);
            admin_payment_http_error(&context, &error, "rustok_payment", policy)
        }
        PaymentOrchestrationError::ProviderAfterRefundReservation { refund_id, source } => {
            context.refund_id = Some(*refund_id);
            let policy = reserved_refund_error_policy(source);
            admin_payment_http_error(&context, &error, "rustok_payment", policy)
        }
    }
}

/// Refund creation is idempotent by a caller-owned key, validated before any owner call.
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

#[utoipa::path(
    get,
    path = "/admin/payment-collections",
    tag = "admin",
    params(ListPaymentCollectionsParams),
    responses((status = 200, description = "Payment collections", body = PaginatedResponse<PaymentCollectionResponse>), (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"))
)]
pub async fn list_payment_collections(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<ListPaymentCollectionsParams>,
) -> HttpResult<Json<PaginatedResponse<PaymentCollectionResponse>>> {
    let _ = &request_context;
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    let pagination = params.pagination.unwrap_or_default();
    let order_id = params.order_id;
    let cart_id = params.cart_id;
    let customer_id = params.customer_id;
    let (items, total) = PaymentService::new(runtime.db_clone())
        .list_collections(
            tenant.id,
            ListPaymentCollectionsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status.clone(),
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
        data: items,
        meta: PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

#[utoipa::path(
    get,
    path = "/admin/payment-collections/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    responses((status = 200, description = "Payment collection details", body = PaymentCollectionResponse), (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"), (status = 404, description = "Payment collection not found"))
)]
pub async fn show_payment_collection(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    let _ = &request_context;
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_payment_collection_id",
            "Payment collection ID must not be nil",
        ));
    }
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
    params(("id" = Uuid, Path, description = "Payment collection ID"), ("Idempotency-Key" = String, Header, description = "Stable write operation identity, maximum 191 bytes")),
    request_body = AuthorizePaymentInput,
    responses((status = 200, description = "Payment collection authorized", body = PaymentCollectionResponse), (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"), (status = 404, description = "Payment collection not found"))
)]
pub async fn authorize_payment_collection(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<AuthorizePaymentInput>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    let _ = &request_context;
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    let collection = PaymentOrchestrationService::new(runtime.db_clone())
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
    params(("id" = Uuid, Path, description = "Payment collection ID"), ("Idempotency-Key" = String, Header, description = "Stable write operation identity, maximum 191 bytes")),
    request_body = CapturePaymentInput,
    responses((status = 200, description = "Payment collection captured", body = PaymentCollectionResponse), (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"), (status = 404, description = "Payment collection not found"))
)]
pub async fn capture_payment_collection(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CapturePaymentInput>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    let _ = &request_context;
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    let collection = PaymentOrchestrationService::new(runtime.db_clone())
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
    params(("id" = Uuid, Path, description = "Payment collection ID"), ("Idempotency-Key" = String, Header, description = "Stable write operation identity, maximum 191 bytes")),
    request_body = CancelPaymentInput,
    responses((status = 200, description = "Payment collection cancelled", body = PaymentCollectionResponse), (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"), (status = 404, description = "Payment collection not found"))
)]
pub async fn cancel_payment_collection(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelPaymentInput>,
) -> HttpResult<Json<PaymentCollectionResponse>> {
    let _ = &request_context;
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    let collection = PaymentOrchestrationService::new(runtime.db_clone())
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
    params(("id" = Uuid, Path, description = "Payment collection ID"), ("Idempotency-Key" = String, Header, description = "Caller-owned refund creation key, maximum 191 bytes")),
    request_body = CreateRefundInput,
    responses((status = 201, description = "Refund created", body = RefundResponse), (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"), (status = 404, description = "Payment collection not found"))
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
    let _ = &request_context;
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    let creation_key = refund_creation_key(&headers)?;
    let refund = PaymentOrchestrationService::new(runtime.db_clone())
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
    responses((status = 200, description = "Refunds", body = PaginatedResponse<RefundResponse>), (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"))
)]
pub async fn list_refunds(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<ListRefundsParams>,
) -> HttpResult<Json<PaginatedResponse<RefundResponse>>> {
    let _ = &request_context;
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    let pagination = params.pagination.unwrap_or_default();
    let payment_collection_id = params.payment_collection_id;
    let order_id = params.order_id;
    let (items, total) = PaymentService::new(runtime.db_clone())
        .list_refunds(
            tenant.id,
            ListRefundsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status.clone(),
                payment_collection_id,
                order_id,
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
        data: items,
        meta: PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

#[utoipa::path(
    get,
    path = "/admin/refunds/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Refund ID")),
    responses((status = 200, description = "Refund details", body = RefundResponse), (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"), (status = 404, description = "Refund not found"))
)]
pub async fn show_refund(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<RefundResponse>> {
    let _ = &request_context;
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
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
    params(("id" = Uuid, Path, description = "Refund ID"), ("Idempotency-Key" = String, Header, description = "Stable write operation identity, maximum 191 bytes")),
    request_body = CompleteRefundInput,
    responses((status = 200, description = "Refund completed", body = RefundResponse), (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"), (status = 404, description = "Refund not found"))
)]
pub async fn complete_refund(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CompleteRefundInput>,
) -> HttpResult<Json<RefundResponse>> {
    let _ = &request_context;
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    let refund = PaymentOrchestrationService::new(runtime.db_clone())
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
    params(("id" = Uuid, Path, description = "Refund ID"), ("Idempotency-Key" = String, Header, description = "Stable write operation identity, maximum 191 bytes")),
    request_body = CancelRefundInput,
    responses((status = 200, description = "Refund cancelled", body = RefundResponse), (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"), (status = 404, description = "Refund not found"))
)]
pub async fn cancel_refund(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelRefundInput>,
) -> HttpResult<Json<RefundResponse>> {
    let _ = &request_context;
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    let refund = PaymentOrchestrationService::new(runtime.db_clone())
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
