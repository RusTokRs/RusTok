use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::{DateTime, FixedOffset};
use rust_decimal::Decimal;
use rustok_api::{AuthContext, Permission, PortActor, PortContext, RequestContext, TenantContext};
use rustok_cart::in_process_cart_checkout_port;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DbErr;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{CommerceHttpRuntime, common::ensure_permissions};
use crate::{
    CheckoutCompensationError, CheckoutInventoryReservationError, CheckoutOperationError,
    CheckoutReconciliationAction, CheckoutReconciliationActionRequest, CheckoutReconciliationError,
    CheckoutReconciliationOutcome, CheckoutReconciliationService,
};

const ADMIN_CHECKOUT_OPERATION_OWNER: &str = "rustok_commerce.admin_checkout_operation";
const ADMIN_CHECKOUT_OPERATION_BOUNDARY: &str = "commerce_admin_checkout_operation_http";

type AdminCheckoutOperationHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

struct AdminCheckoutOperationErrorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
    checkout_operation_id: Option<Uuid>,
    reservation_id: Option<Uuid>,
    payment_collection_id: Option<Uuid>,
    payment_id: Option<Uuid>,
    refund_id: Option<Uuid>,
    order_id: Option<Uuid>,
    order_return_id: Option<Uuid>,
    order_change_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminCheckoutOperationErrorContext {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        checkout_operation_id: Option<Uuid>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            actor_id,
            checkout_operation_id,
            reservation_id: None,
            payment_collection_id: None,
            payment_id: None,
            refund_id: None,
            order_id: None,
            order_return_id: None,
            order_change_id: None,
            operation,
        }
    }
}

struct AdminCheckoutOperationDiagnosticContext {
    tenant_state: &'static str,
    actor_state: &'static str,
    checkout_operation_state: &'static str,
    reservation_state: &'static str,
    payment_collection_state: &'static str,
    payment_state: &'static str,
    refund_state: &'static str,
    order_state: &'static str,
    order_return_state: &'static str,
    order_change_state: &'static str,
    operation: &'static str,
}

impl From<&AdminCheckoutOperationErrorContext> for AdminCheckoutOperationDiagnosticContext {
    fn from(context: &AdminCheckoutOperationErrorContext) -> Self {
        Self {
            tenant_state: uuid_shape(context.tenant_id),
            actor_state: uuid_shape(context.actor_id),
            checkout_operation_state: optional_uuid_shape(context.checkout_operation_id),
            reservation_state: optional_uuid_shape(context.reservation_id),
            payment_collection_state: optional_uuid_shape(context.payment_collection_id),
            payment_state: optional_uuid_shape(context.payment_id),
            refund_state: optional_uuid_shape(context.refund_id),
            order_state: optional_uuid_shape(context.order_id),
            order_return_state: optional_uuid_shape(context.order_return_id),
            order_change_state: optional_uuid_shape(context.order_change_id),
            operation: context.operation,
        }
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

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AdminCheckoutOperationResponse {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub status: String,
    /// Provider execution admission level (`open` / `settling` / `closed`) the
    /// checkout journal owns for this operation.
    pub execution_admission: String,
    /// Monotonic admission generation a provider operation must carry to be
    /// admitted for execution.
    pub admission_epoch: i64,
    pub stage: String,
    pub order_id: Option<Uuid>,
    pub payment_collection_id: Option<Uuid>,
    pub attempt_count: i32,
    pub lease_expires_at: Option<DateTime<FixedOffset>>,
    pub last_error_code: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub completed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
pub struct AdminCheckoutCompensationSweepInput {
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AdminCheckoutCompensationSweepFailure {
    pub operation_id: Uuid,
    pub manual_reconciliation: bool,
    pub error_code: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AdminCheckoutCompensationSweepResponse {
    pub scanned: usize,
    pub compensated: usize,
    pub retryable: usize,
    pub manual_reconciliation: usize,
    /// Operations this sweep parked because their compensation attempts were
    /// exhausted; every one of them is now visible in the reconciliation list.
    pub exhausted: usize,
    pub failures: Vec<AdminCheckoutCompensationSweepFailure>,
}

/// One operator decision on a parked checkout operation.
///
/// Which fields are mandatory depends on `action`, and the rules live next to
/// the action in `CheckoutReconciliationAction`: `attest_external` needs
/// `outcome` + `evidence_ref`, `write_off` needs `second_approver_id`,
/// `refund_partial` needs `amount`, and every other field is rejected when it
/// does not belong to the action.
#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct AdminCheckoutReconciliationActionInput {
    pub action: CheckoutReconciliationAction,
    /// Operator justification, recorded on the operation row and in the journal.
    pub reason: String,
    /// Terminal outcome for `attest_external`.
    pub outcome: Option<CheckoutReconciliationOutcome>,
    /// Out-of-band artefact reference for `attest_external`.
    pub evidence_ref: Option<String>,
    /// Second approver for `write_off`; must differ from the calling operator.
    pub second_approver_id: Option<Uuid>,
    /// Refund amount for `refund_partial`.
    pub amount: Option<Decimal>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AdminCheckoutReconciliationActionResponse {
    pub id: Uuid,
    pub checkout_operation_id: Uuid,
    pub action: String,
    pub result_status: String,
    pub amount: Option<Decimal>,
    pub currency_code: Option<String>,
    pub reason: String,
    pub evidence_ref: Option<String>,
    pub operator_id: Uuid,
    pub approver_id: Option<Uuid>,
    pub refund_id: Option<Uuid>,
    pub refund_status: Option<String>,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema, IntoParams)]
pub struct AdminCheckoutReconciliationActionListQuery {
    /// Page size, clamped to the journal's list limit.
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema, IntoParams)]
pub struct AdminCheckoutOperationListQuery {
    /// Exact status filter, for example `reconciliation_required`.
    pub status: Option<String>,
    /// Page size, clamped to the journal's list limit.
    pub limit: Option<u64>,
}

pub fn axum_router() -> axum::Router<CommerceHttpRuntime> {
    axum::Router::new()
        .route("/", axum::routing::get(list_checkout_operations))
        .route(
            "/compensation-sweep",
            axum::routing::post(sweep_checkout_compensations),
        )
        .route("/{id}", axum::routing::get(show_checkout_operation))
        .route(
            "/{id}/compensate",
            axum::routing::post(compensate_checkout_operation),
        )
        .route(
            "/{id}/actions",
            axum::routing::post(execute_checkout_reconciliation_action)
                .get(list_checkout_reconciliation_actions),
        )
}

#[utoipa::path(
    get,
    path = "/admin/checkout-operations/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Checkout operation ID")),
    responses(
        (status = 200, description = "Checkout operation", body = AdminCheckoutOperationResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"),
        (status = 404, description = "Checkout operation not found")
    )
)]
pub async fn show_checkout_operation(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<AdminCheckoutOperationResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;
    let operation = crate::CheckoutOperationJournal::new(runtime.db_clone(), runtime.event_bus())
        .get(tenant.id, id)
        .await
        .map_err(|error| {
            map_operation_error(
                AdminCheckoutOperationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(id),
                    "show_checkout_operation",
                ),
                error,
            )
        })?;
    Ok(Json(map_operation(operation)))
}

#[utoipa::path(
    post,
    path = "/admin/checkout-operations/{id}/compensate",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Checkout operation ID")),
    responses(
        (status = 200, description = "Checkout operation compensated", body = AdminCheckoutOperationResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"),
        (status = 404, description = "Checkout operation not found"),
        (status = 409, description = "Compensation requires retry or manual reconciliation")
    )
)]
pub async fn compensate_checkout_operation(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<AdminCheckoutOperationResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_MANAGE],
        "Permission denied: orders:manage required",
    )?;
    let service = crate::CheckoutCompensationService::new(
        runtime.db_clone(),
        runtime.event_bus(),
        rustok_inventory::in_process_inventory_reservation_identity_port(runtime.db_clone()),
        in_process_cart_checkout_port(runtime.db_clone()),
    )
    .with_payment_provider_registry(runtime.payment_provider_registry());
    let operation = service
        .compensate(
            tenant.id,
            auth.user_id,
            id,
            format!(
                "admin-checkout-compensation:{}:{}",
                auth.user_id,
                Uuid::new_v4()
            ),
        )
        .await
        .map_err(|error| {
            map_compensation_error(
                AdminCheckoutOperationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(id),
                    "compensate_checkout_operation",
                ),
                error,
            )
        })?;
    Ok(Json(map_operation(operation)))
}

#[utoipa::path(
    get,
    path = "/admin/checkout-operations",
    tag = "admin",
    params(AdminCheckoutOperationListQuery),
    responses(
        (status = 200, description = "Checkout operations", body = Vec<AdminCheckoutOperationResponse>),
        (status = 400, description = "Invalid status filter"),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn list_checkout_operations(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<AdminCheckoutOperationListQuery>,
) -> HttpResult<Json<Vec<AdminCheckoutOperationResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;
    let operations = crate::CheckoutOperationJournal::new(runtime.db_clone(), runtime.event_bus())
        .list_by_status(tenant.id, query.status.as_deref(), query.limit.unwrap_or(50))
        .await
        .map_err(|error| {
            map_operation_error(
                AdminCheckoutOperationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    None,
                    "list_checkout_operations",
                ),
                error,
            )
        })?;
    Ok(Json(operations.into_iter().map(map_operation).collect()))
}

#[utoipa::path(
    post,
    path = "/admin/checkout-operations/{id}/actions",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Checkout operation ID")),
    request_body = AdminCheckoutReconciliationActionInput,
    responses(
        (status = 201, description = "Reconciliation action recorded", body = AdminCheckoutReconciliationActionResponse),
        (status = 400, description = "Invalid request"), (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"), (status = 404, description = "Checkout operation not found"),
        (status = 409, description = "Checkout operation is not parked in reconciliation_required"),
        (status = 502, description = "Payment owner rejected the reconciliation step")
    )
)]
pub async fn execute_checkout_reconciliation_action(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<AdminCheckoutReconciliationActionInput>,
) -> HttpResult<(StatusCode, Json<AdminCheckoutReconciliationActionResponse>)> {
    let action = input.action;
    // The permission set is derived from the action itself, so a money moving
    // action can never be reachable through the safe-action permission path.
    ensure_permissions(
        &auth,
        action.required_permissions(),
        action_permission_message(action),
    )?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let context = reconciliation_port_context(
        &tenant,
        &auth,
        &request_context,
        id,
        idempotency_key.as_str(),
    );
    let service = CheckoutReconciliationService::new(
        runtime.db_clone(),
        runtime.event_bus(),
        runtime.payment_admin_read_port(),
        runtime.payment_admin_refund_command_port(),
        runtime.payment_admin_collection_command_port(),
    );
    let record = service
        .execute(
            context,
            CheckoutReconciliationActionRequest {
                tenant_id: tenant.id,
                operation_id: id,
                action,
                operator_id: auth.user_id,
                idempotency_key,
                reason: input.reason,
                outcome: input.outcome,
                evidence_ref: input.evidence_ref,
                second_approver_id: input.second_approver_id,
                amount: input.amount,
            },
        )
        .await
        .map_err(|error| {
            map_reconciliation_error(
                AdminCheckoutOperationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(id),
                    "execute_checkout_reconciliation_action",
                ),
                error,
            )
        })?;
    tracing::info!(
        owner = ADMIN_CHECKOUT_OPERATION_OWNER,
        tenant_id = %tenant.id,
        operator_id = %auth.user_id,
        checkout_operation_id = %record.checkout_operation_id,
        reconciliation_action = record.action.as_str(),
        result_status = record.result_status.as_str(),
        "checkout reconciliation action recorded"
    );
    Ok((StatusCode::CREATED, Json(map_reconciliation_action(record))))
}

#[utoipa::path(
    get,
    path = "/admin/checkout-operations/{id}/actions",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Checkout operation ID"), AdminCheckoutReconciliationActionListQuery),
    responses(
        (status = 200, description = "Reconciliation actions", body = Vec<AdminCheckoutReconciliationActionResponse>),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn list_checkout_reconciliation_actions(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Query(query): Query<AdminCheckoutReconciliationActionListQuery>,
) -> HttpResult<Json<Vec<AdminCheckoutReconciliationActionResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;
    let actions = CheckoutReconciliationService::new(
        runtime.db_clone(),
        runtime.event_bus(),
        runtime.payment_admin_read_port(),
        runtime.payment_admin_refund_command_port(),
        runtime.payment_admin_collection_command_port(),
    )
    .list_actions(tenant.id, id, query.limit.unwrap_or(50))
    .await
    .map_err(|error| {
        map_reconciliation_error(
            AdminCheckoutOperationErrorContext::new(
                tenant.id,
                auth.user_id,
                Some(id),
                "list_checkout_reconciliation_actions",
            ),
            error,
        )
    })?;
    Ok(Json(
        actions.into_iter().map(map_reconciliation_action).collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/admin/checkout-operations/compensation-sweep",
    tag = "admin",
    request_body = AdminCheckoutCompensationSweepInput,
    responses(
        (status = 200, description = "Checkout compensation sweep report", body = AdminCheckoutCompensationSweepResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn sweep_checkout_compensations(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<AdminCheckoutCompensationSweepInput>,
) -> HttpResult<Json<AdminCheckoutCompensationSweepResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_MANAGE],
        "Permission denied: orders:manage required",
    )?;
    let report = crate::CheckoutCompensationSweepService::new(
        runtime.db_clone(),
        runtime.event_bus(),
        rustok_inventory::in_process_inventory_reservation_identity_port(runtime.db_clone()),
        in_process_cart_checkout_port(runtime.db_clone()),
    )
    .with_payment_provider_registry(runtime.payment_provider_registry())
    .run(
        tenant.id,
        auth.user_id,
        format!("admin:{}", auth.user_id),
        input.limit,
    )
    .await
    .map_err(|error| {
        map_sweep_error(
            AdminCheckoutOperationErrorContext::new(
                tenant.id,
                auth.user_id,
                None,
                "sweep_checkout_compensations",
            ),
            error,
        )
    })?;

    Ok(Json(AdminCheckoutCompensationSweepResponse {
        scanned: report.scanned,
        compensated: report.compensated,
        retryable: report.retryable,
        manual_reconciliation: report.manual_reconciliation,
        exhausted: report.exhausted,
        failures: report
            .failures
            .into_iter()
            .map(|failure| AdminCheckoutCompensationSweepFailure {
                operation_id: failure.operation_id,
                manual_reconciliation: failure.manual_reconciliation,
                error_code: failure.error_code,
            })
            .collect(),
    }))
}

fn map_operation(
    operation: crate::entities::checkout_operation::Model,
) -> AdminCheckoutOperationResponse {
    AdminCheckoutOperationResponse {
        id: operation.id,
        cart_id: operation.cart_id,
        status: operation.status,
        execution_admission: operation.execution_admission,
        admission_epoch: operation.admission_epoch,
        stage: operation.stage,
        order_id: operation.order_id,
        payment_collection_id: operation.payment_collection_id,
        attempt_count: operation.attempt_count,
        lease_expires_at: operation.lease_expires_at,
        last_error_code: operation.last_error_code,
        created_at: operation.created_at,
        updated_at: operation.updated_at,
        completed_at: operation.completed_at,
    }
}

fn map_reconciliation_action(
    action: crate::entities::checkout_reconciliation_action::Model,
) -> AdminCheckoutReconciliationActionResponse {
    AdminCheckoutReconciliationActionResponse {
        id: action.id,
        checkout_operation_id: action.checkout_operation_id,
        action: action.action,
        result_status: action.result_status,
        amount: action.amount,
        currency_code: action.currency_code,
        reason: action.reason,
        evidence_ref: action.evidence_ref,
        operator_id: action.operator_id,
        approver_id: action.approver_id,
        refund_id: action.refund_id,
        refund_status: action.refund_status,
        created_at: action.created_at,
    }
}

fn action_permission_message(action: CheckoutReconciliationAction) -> &'static str {
    if action.moves_money() {
        "Permission denied: orders:manage and payments:update required"
    } else {
        "Permission denied: orders:manage required"
    }
}

fn require_idempotency_key(headers: &HeaderMap) -> HttpResult<String> {
    let value = headers
        .get("Idempotency-Key")
        .ok_or_else(|| {
            HttpError::new(
                StatusCode::BAD_REQUEST,
                "commerce_admin_idempotency_key_required",
                "Idempotency-Key header is required for reconciliation actions",
            )
        })?
        .to_str()
        .map_err(|_| {
            HttpError::new(
                StatusCode::BAD_REQUEST,
                "commerce_admin_idempotency_key_invalid",
                "Idempotency-Key header is invalid",
            )
        })?
        .trim()
        .to_string();
    if value.is_empty() || value.chars().count() > 191 {
        return Err(HttpError::new(
            StatusCode::BAD_REQUEST,
            "commerce_admin_idempotency_key_invalid",
            "Idempotency-Key header must contain 1 to 191 characters",
        ));
    }
    Ok(value)
}

fn reconciliation_port_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    operation_id: Uuid,
    idempotency_key: &str,
) -> PortContext {
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-checkout-reconciliation:{operation_id}"),
    )
    .with_idempotency_key(idempotency_key.to_string())
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn map_reconciliation_error(
    context: AdminCheckoutOperationErrorContext,
    error: CheckoutReconciliationError,
) -> HttpError {
    let (policy, source_owner) = reconciliation_error_policy(&error);
    admin_checkout_operation_http_error(
        &context,
        source_owner,
        policy,
        "commerce admin checkout reconciliation action failed",
    )
}

fn reconciliation_error_policy(
    error: &CheckoutReconciliationError,
) -> (AdminCheckoutOperationHttpPolicy, &'static str) {
    match error {
        CheckoutReconciliationError::Validation(_) => (
            (
                StatusCode::BAD_REQUEST,
                "checkout_reconciliation_invalid_request",
                "Checkout reconciliation request is invalid",
                "validation",
            ),
            "rustok_commerce.checkout_reconciliation",
        ),
        CheckoutReconciliationError::NotFound(_) => (
            (
                StatusCode::NOT_FOUND,
                "checkout_operation_not_found",
                "Checkout operation not found",
                "not_found",
            ),
            "rustok_commerce.checkout_operation",
        ),
        CheckoutReconciliationError::JournalConflict(_) => (
            (
                StatusCode::CONFLICT,
                "checkout_reconciliation_idempotency_conflict",
                "Idempotency key already used for a different reconciliation action",
                "journal_conflict",
            ),
            "rustok_commerce.checkout_reconciliation",
        ),
        CheckoutReconciliationError::Conflict(_) => (
            (
                StatusCode::CONFLICT,
                "checkout_reconciliation_conflict",
                "Checkout reconciliation cannot proceed from the current state",
                "conflict",
            ),
            "rustok_commerce.checkout_reconciliation",
        ),
        CheckoutReconciliationError::PaymentOwner { retryable: true, .. } => (
            (
                StatusCode::CONFLICT,
                "checkout_reconciliation_pending",
                "Checkout reconciliation will be retried",
                "retryable_payment_owner",
            ),
            "rustok_payment.admin_refund_command",
        ),
        CheckoutReconciliationError::PaymentOwner { .. } => (
            (
                StatusCode::BAD_GATEWAY,
                "checkout_reconciliation_payment_failed",
                "Payment owner rejected the reconciliation step",
                "payment_owner_failed",
            ),
            "rustok_payment.admin_refund_command",
        ),
        CheckoutReconciliationError::CloseAfterMoneyMoved { .. } => (
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Checkout reconciliation requires operator follow-up",
                "close_after_money_moved",
            ),
            "rustok_commerce.checkout_reconciliation",
        ),
        CheckoutReconciliationError::Operation(source) => (
            checkout_operation_error_policy(source),
            "rustok_commerce.checkout_operation",
        ),
        CheckoutReconciliationError::Database(_) => (
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Checkout reconciliation storage is unavailable",
                "database",
            ),
            "rustok_commerce.checkout_reconciliation",
        ),
    }
}

fn checkout_operation_error_policy(
    error: &CheckoutOperationError,
) -> AdminCheckoutOperationHttpPolicy {
    match error {
        CheckoutOperationError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            "checkout_operation_not_found",
            "Checkout operation not found",
            "not_found",
        ),
        CheckoutOperationError::Conflict(_) => (
            StatusCode::CONFLICT,
            "checkout_operation_conflict",
            "Checkout operation conflicts with the current state",
            "conflict",
        ),
        CheckoutOperationError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "checkout_operation_invalid",
            "Checkout operation request is invalid",
            "validation",
        ),
        CheckoutOperationError::Database(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Checkout operation storage is unavailable",
            "database",
        ),
    }
}

fn adopt_operation_error_identity(
    context: &mut AdminCheckoutOperationErrorContext,
    error: &CheckoutOperationError,
) {
    if let CheckoutOperationError::NotFound(id) = error {
        context.checkout_operation_id = Some(*id);
    }
}

fn adopt_reservation_error_identity(
    context: &mut AdminCheckoutOperationErrorContext,
    error: &CheckoutInventoryReservationError,
) {
    if let CheckoutInventoryReservationError::NotFound(id) = error {
        context.reservation_id = Some(*id);
    }
}

fn admin_checkout_operation_http_error(
    context: &AdminCheckoutOperationErrorContext,
    source_owner: &'static str,
    policy: AdminCheckoutOperationHttpPolicy,
    log_message: &'static str,
) -> HttpError {
    let context = AdminCheckoutOperationDiagnosticContext::from(context);
    let (status, code, message, error_kind) = policy;
    tracing::error!(
        owner = ADMIN_CHECKOUT_OPERATION_OWNER,
        source_owner,
        tenant_state = context.tenant_state,
        actor_state = context.actor_state,
        checkout_operation_state = context.checkout_operation_state,
        reservation_state = context.reservation_state,
        payment_collection_state = context.payment_collection_state,
        payment_state = context.payment_state,
        refund_state = context.refund_state,
        order_state = context.order_state,
        order_return_state = context.order_return_state,
        order_change_state = context.order_change_state,
        operation = context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_CHECKOUT_OPERATION_BOUNDARY,
        "{log_message}"
    );
    HttpError::new(status, code, message)
}

fn map_operation_error(
    mut context: AdminCheckoutOperationErrorContext,
    error: CheckoutOperationError,
) -> HttpError {
    adopt_operation_error_identity(&mut context, &error);
    let policy = checkout_operation_error_policy(&error);
    admin_checkout_operation_http_error(
        &context,
        "rustok_commerce.checkout_operation",
        policy,
        "commerce admin checkout operation lookup failed",
    )
}

fn map_compensation_error(
    mut context: AdminCheckoutOperationErrorContext,
    error: CheckoutCompensationError,
) -> HttpError {
    let (policy, source_owner) = match &error {
        CheckoutCompensationError::Operation(source) => {
            adopt_operation_error_identity(&mut context, source);
            (
                checkout_operation_error_policy(source),
                "rustok_commerce.checkout_operation",
            )
        }
        CheckoutCompensationError::ReservationJournal(source) => {
            adopt_reservation_error_identity(&mut context, source);
            (
                (
                    StatusCode::CONFLICT,
                    "checkout_compensation_pending",
                    "Checkout compensation will be retried",
                    "reservation_journal",
                ),
                "rustok_commerce.checkout_inventory_reservation",
            )
        }
        CheckoutCompensationError::ManualReconciliation(_) => (
            (
                StatusCode::CONFLICT,
                "checkout_reconciliation_required",
                "Checkout requires manual reconciliation",
                "manual_reconciliation",
            ),
            "rustok_commerce.checkout_compensation",
        ),
        CheckoutCompensationError::Conflict(_) => (
            (
                StatusCode::CONFLICT,
                "checkout_compensation_conflict",
                "Checkout compensation cannot proceed from the current state",
                "conflict",
            ),
            "rustok_commerce.checkout_compensation",
        ),
        CheckoutCompensationError::Boundary {
            stage,
            retryable: true,
            ..
        } => (
            (
                StatusCode::CONFLICT,
                "checkout_compensation_pending",
                "Checkout compensation will be retried",
                "retryable_boundary",
            ),
            compensation_boundary_owner(stage),
        ),
        CheckoutCompensationError::Boundary { stage, .. } => (
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Checkout compensation is unavailable",
                "compensation_failed",
            ),
            compensation_boundary_owner(stage),
        ),
        CheckoutCompensationError::CompensationAndJournal { .. } => (
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Checkout compensation is unavailable",
                "compensation_failed",
            ),
            "rustok_commerce.checkout_compensation",
        ),
    };

    admin_checkout_operation_http_error(
        &context,
        source_owner,
        policy,
        "commerce admin checkout compensation failed",
    )
}

fn compensation_boundary_owner(stage: &str) -> &'static str {
    match stage {
        "compensate_payment" => "rustok_payment",
        "compensate_order" => "rustok_order",
        "release_inventory" => "rustok_inventory",
        "read_cart" | "release_cart" => "rustok_cart",
        _ => "rustok_commerce.checkout_compensation",
    }
}

fn map_sweep_error(context: AdminCheckoutOperationErrorContext, _error: DbErr) -> HttpError {
    admin_checkout_operation_http_error(
        &context,
        "rustok_commerce.checkout_compensation_sweep",
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Checkout compensation storage is unavailable",
            "database",
        ),
        "commerce admin checkout compensation sweep failed",
    )
}
