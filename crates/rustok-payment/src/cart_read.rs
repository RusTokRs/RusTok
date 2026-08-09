use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{PaymentCollectionResponse, PaymentError, PaymentService};

const PAYMENT_CART_READ_OWNER: &str = "rustok_payment";
const PAYMENT_CART_READ_BOUNDARY: &str = "payment_cart_read_port";

#[async_trait]
pub trait PaymentCartReadPort: Send + Sync {
    async fn find_reusable_collection_by_cart(
        &self,
        context: PortContext,
        request: ReusablePaymentCollectionByCartRequest,
    ) -> Result<Option<PaymentCollectionResponse>, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReusablePaymentCollectionByCartRequest {
    pub cart_id: Uuid,
}

pub struct InProcessPaymentCartReadPort {
    inner: PaymentService,
}

impl InProcessPaymentCartReadPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: PaymentService::new(db),
        }
    }
}

pub fn in_process_payment_cart_read_port(db: DatabaseConnection) -> Arc<dyn PaymentCartReadPort> {
    Arc::new(InProcessPaymentCartReadPort::new(db))
}

#[derive(Clone)]
pub struct PaymentCartReadRuntime {
    read_port: Arc<dyn PaymentCartReadPort>,
}

impl PaymentCartReadRuntime {
    pub fn new(read_port: Arc<dyn PaymentCartReadPort>) -> Self {
        Self { read_port }
    }

    pub fn in_process(db: DatabaseConnection) -> Self {
        Self::new(in_process_payment_cart_read_port(db))
    }

    pub fn read_port(&self) -> Arc<dyn PaymentCartReadPort> {
        self.read_port.clone()
    }
}

#[async_trait]
impl PaymentCartReadPort for InProcessPaymentCartReadPort {
    async fn find_reusable_collection_by_cart(
        &self,
        context: PortContext,
        request: ReusablePaymentCollectionByCartRequest,
    ) -> Result<Option<PaymentCollectionResponse>, PortError> {
        const OPERATION: &str = "find_reusable_collection_by_cart";
        context.require_policy(PortCallPolicy::read()).inspect_err(|error| {
            log_context_rejection(&context, OPERATION, "policy", error);
        })?;
        let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
            let error = PortError::validation(
                "payment.cart_read_context_invalid",
                "payment cart read context is invalid",
            );
            log_context_rejection(&context, OPERATION, "tenant_id", &error);
            error
        })?;

        self.inner
            .find_reusable_collection_by_cart(tenant_id, request.cart_id)
            .await
            .map_err(|error| map_payment_error(&context, OPERATION, request.cart_id, error))
    }
}

fn map_payment_error(
    context: &PortContext,
    operation: &'static str,
    cart_id: Uuid,
    error: PaymentError,
) -> PortError {
    let (kind, code, message, retryable, error_variant, technical) = match &error {
        PaymentError::Validation(_) => (
            PortErrorKind::Validation,
            "payment.cart_read_validation",
            "payment cart read request is invalid",
            false,
            "validation",
            false,
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            PortErrorKind::NotFound,
            "payment.cart_read_not_found",
            "payment resource was not found",
            false,
            "not_found",
            false,
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => (
            PortErrorKind::Conflict,
            "payment.cart_read_conflict",
            "payment state conflicts with the read request",
            false,
            "conflict",
            false,
        ),
        PaymentError::ProviderConfiguration { .. } => (
            PortErrorKind::Unavailable,
            "payment.cart_read_configuration",
            "payment data is temporarily unavailable",
            true,
            "provider_configuration",
            true,
        ),
        PaymentError::ProviderUnavailable { .. } | PaymentError::Database(_) => (
            PortErrorKind::Unavailable,
            "payment.cart_read_unavailable",
            "payment data is temporarily unavailable",
            true,
            "unavailable",
            true,
        ),
        PaymentError::ProviderInvalidResponse { .. }
        | PaymentError::ProviderOutcomeUnknown { .. } => (
            PortErrorKind::InvariantViolation,
            "payment.cart_read_invariant",
            "payment state could not be read safely",
            false,
            "invariant",
            true,
        ),
    };

    if technical {
        tracing::error!(
            owner = PAYMENT_CART_READ_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            cart_id_non_nil = !cart_id.is_nil(),
            error_variant,
            code,
            retryable,
            boundary = PAYMENT_CART_READ_BOUNDARY,
            "payment cart read failed"
        );
    } else {
        tracing::warn!(
            owner = PAYMENT_CART_READ_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            cart_id_non_nil = !cart_id.is_nil(),
            error_variant,
            code,
            retryable,
            boundary = PAYMENT_CART_READ_BOUNDARY,
            "payment cart read was rejected"
        );
    }

    PortError::new(kind, code, message, retryable)
}

fn log_context_rejection(
    context: &PortContext,
    operation: &'static str,
    admission: &'static str,
    error: &PortError,
) {
    tracing::warn!(
        owner = PAYMENT_CART_READ_OWNER,
        operation,
        admission,
        correlation_id = %context.correlation_id,
        tenant_id_length = context.tenant_id.chars().count(),
        actor_id_length = context.actor.id.chars().count(),
        channel_present = context.channel.is_some(),
        locale_length = context.locale.chars().count(),
        deadline_ms = ?context.deadline_ms,
        code = %error.code,
        retryable = error.retryable,
        boundary = PAYMENT_CART_READ_BOUNDARY,
        "payment cart read context was rejected"
    );
}
