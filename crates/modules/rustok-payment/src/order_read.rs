use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ListRefundsInput, PaymentCollectionResponse, PaymentError, PaymentService, RefundResponse,
};

const PAYMENT_ORDER_READ_OWNER: &str = "rustok_payment";
const PAYMENT_ORDER_READ_BOUNDARY: &str = "payment_order_read_port";

#[async_trait]
pub trait PaymentOrderReadPort: Send + Sync {
    async fn find_latest_collection_by_order(
        &self,
        context: PortContext,
        request: LatestPaymentCollectionByOrderRequest,
    ) -> Result<Option<PaymentCollectionResponse>, PortError>;

    async fn list_refunds_by_order(
        &self,
        context: PortContext,
        _request: ListRefundsByOrderRequest,
    ) -> Result<PaymentOrderRefundPage, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        Err(PortError::unavailable(
            "payment.order_refund_read_unavailable",
            "payment order refund read capability is unavailable",
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LatestPaymentCollectionByOrderRequest {
    pub order_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListRefundsByOrderRequest {
    pub order_id: Uuid,
    pub page: u64,
    pub per_page: u64,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentOrderRefundPage {
    pub items: Vec<RefundResponse>,
    pub total: u64,
}

pub struct InProcessPaymentOrderReadPort {
    inner: PaymentService,
}

impl InProcessPaymentOrderReadPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: PaymentService::new(db),
        }
    }
}

pub fn in_process_payment_order_read_port(db: DatabaseConnection) -> Arc<dyn PaymentOrderReadPort> {
    Arc::new(InProcessPaymentOrderReadPort::new(db))
}

#[derive(Clone)]
pub struct PaymentOrderReadRuntime {
    read_port: Arc<dyn PaymentOrderReadPort>,
}

impl PaymentOrderReadRuntime {
    pub fn new(read_port: Arc<dyn PaymentOrderReadPort>) -> Self {
        Self { read_port }
    }

    pub fn in_process(db: DatabaseConnection) -> Self {
        Self::new(in_process_payment_order_read_port(db))
    }

    pub fn read_port(&self) -> Arc<dyn PaymentOrderReadPort> {
        self.read_port.clone()
    }
}

#[async_trait]
impl PaymentOrderReadPort for InProcessPaymentOrderReadPort {
    async fn find_latest_collection_by_order(
        &self,
        context: PortContext,
        request: LatestPaymentCollectionByOrderRequest,
    ) -> Result<Option<PaymentCollectionResponse>, PortError> {
        let operation = "find_latest_collection_by_order";
        let tenant_id = require_order_read_context(&context, operation)?;

        tracing::debug!(
            boundary = PAYMENT_ORDER_READ_BOUNDARY,
            operation,
            correlation_id = %context.correlation_id,
            order_id = %request.order_id,
            "payment find_latest_collection_by_order entered"
        );
        if request.order_id.is_nil() {
            return Err(PortError::validation(
                "payment.order_id_nil",
                "order_id must not be a nil UUID",
            ));
        }

        self.inner
            .find_latest_collection_by_order(tenant_id, request.order_id)
            .await
            .map_err(|error| map_payment_error(&context, operation, request.order_id, error))
    }

    async fn list_refunds_by_order(
        &self,
        context: PortContext,
        request: ListRefundsByOrderRequest,
    ) -> Result<PaymentOrderRefundPage, PortError> {
        let operation = "list_refunds_by_order";
        let tenant_id = require_order_read_context(&context, operation)?;

        tracing::debug!(
            boundary = PAYMENT_ORDER_READ_BOUNDARY,
            operation,
            correlation_id = %context.correlation_id,
            order_id = %request.order_id,
            page = request.page,
            per_page = request.per_page,
            "payment list_refunds_by_order entered"
        );
        if request.order_id.is_nil() {
            return Err(PortError::validation(
                "payment.order_id_nil",
                "order_id must not be a nil UUID",
            ));
        }
        if request.page == 0 || request.per_page == 0 {
            return Err(PortError::validation(
                "payment.pagination_invalid",
                "page and per_page must be greater than zero",
            ));
        }

        let (items, total) = self
            .inner
            .list_refunds(
                tenant_id,
                ListRefundsInput {
                    page: request.page,
                    per_page: request.per_page,
                    payment_collection_id: None,
                    order_id: Some(request.order_id),
                    status: request.status,
                },
            )
            .await
            .map_err(|error| map_refund_read_error(&context, request.order_id, error))?;
        Ok(PaymentOrderRefundPage { items, total })
    }
}

fn require_order_read_context(
    context: &PortContext,
    operation: &'static str,
) -> Result<Uuid, PortError> {
    context.require_policy(PortCallPolicy::read())?;
    let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        tracing::warn!(
            owner = PAYMENT_ORDER_READ_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            boundary = PAYMENT_ORDER_READ_BOUNDARY,
            "payment order read context was rejected"
        );
        PortError::validation(
            "payment.order_read_context_invalid",
            "payment request context is invalid",
        )
    })?;
    if tenant_id.is_nil() {
        let error = PortError::validation(
            "payment.tenant_id_nil",
            "PortContext.tenant_id must not be a nil UUID",
        );
        tracing::warn!(
            owner = PAYMENT_ORDER_READ_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            boundary = PAYMENT_ORDER_READ_BOUNDARY,
            "payment order read tenant context was nil"
        );
        return Err(error);
    }
    Ok(tenant_id)
}

fn map_payment_error(
    context: &PortContext,
    operation: &'static str,
    order_id: Uuid,
    error: PaymentError,
) -> PortError {
    let (kind, code, message, retryable, error_variant, technical) = match &error {
        PaymentError::Validation(_) => (
            PortErrorKind::Validation,
            "payment.order_read_validation",
            "payment request is invalid",
            false,
            "validation",
            false,
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            PortErrorKind::NotFound,
            "payment.order_read_not_found",
            "payment resource was not found",
            false,
            "not_found",
            false,
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => (
            PortErrorKind::Conflict,
            "payment.order_read_conflict",
            "payment state conflicts with the request",
            false,
            "conflict",
            false,
        ),
        PaymentError::ProviderConfiguration { .. } => (
            PortErrorKind::Unavailable,
            "payment.order_read_configuration",
            "payment storage is temporarily unavailable",
            true,
            "provider_configuration",
            true,
        ),
        PaymentError::ProviderUnavailable { .. } | PaymentError::Database(_) => (
            PortErrorKind::Unavailable,
            "payment.order_read_unavailable",
            "payment storage is temporarily unavailable",
            true,
            "unavailable",
            true,
        ),
        PaymentError::ProviderInvalidResponse { .. }
        | PaymentError::ProviderOutcomeUnknown { .. } => (
            PortErrorKind::InvariantViolation,
            "payment.order_read_invariant",
            "payment state could not be read safely",
            false,
            "invariant",
            true,
        ),
    };

    log_payment_read_error(
        context,
        operation,
        order_id,
        error_variant,
        code,
        retryable,
        technical,
    );
    PortError::new(kind, code, message, retryable)
}

fn map_refund_read_error(context: &PortContext, order_id: Uuid, error: PaymentError) -> PortError {
    let operation = "list_refunds_by_order";
    let (kind, code, message, retryable, error_variant, technical) = match &error {
        PaymentError::Validation(_) => (
            PortErrorKind::Validation,
            "payment.order_refund_read_validation",
            "payment refund request is invalid",
            false,
            "validation",
            false,
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            PortErrorKind::NotFound,
            "payment.order_refund_read_not_found",
            "payment refund resource was not found",
            false,
            "not_found",
            false,
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => (
            PortErrorKind::Conflict,
            "payment.order_refund_read_state_conflict",
            "payment refund state conflicts with the request",
            false,
            "state_conflict",
            false,
        ),
        PaymentError::ProviderUnavailable { .. } => (
            PortErrorKind::Unavailable,
            "payment.order_refund_provider_unavailable",
            "payment provider is temporarily unavailable",
            true,
            "provider_unavailable",
            true,
        ),
        PaymentError::ProviderInvalidResponse { .. } => (
            PortErrorKind::InvariantViolation,
            "payment.order_refund_provider_invalid_response",
            "payment provider response could not be read safely",
            false,
            "provider_invalid_response",
            true,
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => (
            PortErrorKind::Conflict,
            "payment.order_refund_reconciliation_required",
            "payment state requires reconciliation",
            false,
            "provider_outcome_unknown",
            true,
        ),
        PaymentError::ProviderConfiguration { .. } => (
            PortErrorKind::Unavailable,
            "payment.order_refund_provider_not_configured",
            "payment provider is not configured",
            true,
            "provider_configuration",
            true,
        ),
        PaymentError::Database(_) => (
            PortErrorKind::Unavailable,
            "payment.order_refund_read_unavailable",
            "payment refund storage is temporarily unavailable",
            true,
            "database",
            true,
        ),
    };

    log_payment_read_error(
        context,
        operation,
        order_id,
        error_variant,
        code,
        retryable,
        technical,
    );
    PortError::new(kind, code, message, retryable)
}

fn log_payment_read_error(
    context: &PortContext,
    operation: &'static str,
    order_id: Uuid,
    error_variant: &'static str,
    code: &'static str,
    retryable: bool,
    technical: bool,
) {
    if technical {
        tracing::error!(
            owner = PAYMENT_ORDER_READ_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            order_id_non_nil = !order_id.is_nil(),
            error_variant,
            code,
            retryable,
            boundary = PAYMENT_ORDER_READ_BOUNDARY,
            "payment order read failed"
        );
    } else {
        tracing::warn!(
            owner = PAYMENT_ORDER_READ_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            order_id_non_nil = !order_id.is_nil(),
            error_variant,
            code,
            retryable,
            boundary = PAYMENT_ORDER_READ_BOUNDARY,
            "payment order read was rejected"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{PortActor, PortContext};

    #[test]
    fn require_order_read_context_accepts_valid_and_rejects_nil_or_invalid() {
        let valid_tenant = Uuid::new_v4();
        let ctx = PortContext::new(
            valid_tenant.to_string(),
            PortActor::system(),
            "en-US",
            Uuid::new_v4(),
        )
        .with_deadline(std::time::Duration::from_millis(5000));
        assert_eq!(
            require_order_read_context(&ctx, "test_op").unwrap(),
            valid_tenant
        );

        let nil_ctx = PortContext::new(
            Uuid::nil().to_string(),
            PortActor::system(),
            "en-US",
            Uuid::new_v4(),
        )
        .with_deadline(std::time::Duration::from_millis(5000));
        let err = require_order_read_context(&nil_ctx, "test_op").unwrap_err();
        assert_eq!(err.code, "payment.tenant_id_nil");
        assert_eq!(err.kind, PortErrorKind::Validation);

        let invalid_ctx =
            PortContext::new("not-a-uuid", PortActor::system(), "en-US", Uuid::new_v4())
                .with_deadline(std::time::Duration::from_millis(5000));
        let err = require_order_read_context(&invalid_ctx, "test_op").unwrap_err();
        assert_eq!(err.code, "payment.order_read_context_invalid");
        assert_eq!(err.kind, PortErrorKind::Validation);
    }
}
