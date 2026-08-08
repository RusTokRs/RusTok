use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{PaymentCollectionResponse, PaymentError, PaymentService};

const PAYMENT_ORDER_READ_OWNER: &str = "rustok_payment";
const PAYMENT_ORDER_READ_BOUNDARY: &str = "payment_order_read_port";

#[async_trait]
pub trait PaymentOrderReadPort: Send + Sync {
    async fn find_latest_collection_by_order(
        &self,
        context: PortContext,
        request: LatestPaymentCollectionByOrderRequest,
    ) -> Result<Option<PaymentCollectionResponse>, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LatestPaymentCollectionByOrderRequest {
    pub order_id: Uuid,
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

        self.inner
            .find_latest_collection_by_order(tenant_id, request.order_id)
            .await
            .map_err(|error| map_payment_error(&context, operation, request.order_id, error))
    }
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
        PaymentError::ProviderUnavailable { .. }
        | PaymentError::ProviderConfiguration { .. }
        | PaymentError::Database(_) => (
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

    PortError::new(kind, code, message, retryable)
}
