use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ListPaymentCollectionsInput, ListRefundsInput, PaymentCollectionResponse, PaymentError,
    PaymentService, RefundResponse,
};

const PAYMENT_ADMIN_READ_OWNER: &str = "rustok_payment";
const PAYMENT_ADMIN_READ_BOUNDARY: &str = "payment_admin_read_port";

#[async_trait]
pub trait PaymentAdminReadPort: Send + Sync {
    async fn list_payment_collection_projections(
        &self,
        context: PortContext,
        request: ListPaymentCollectionProjectionsRequest,
    ) -> Result<PaymentCollectionProjectionPage, PortError>;

    async fn read_payment_collection_projection(
        &self,
        context: PortContext,
        request: ReadPaymentCollectionProjectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError>;

    async fn list_refund_projections(
        &self,
        context: PortContext,
        request: ListRefundProjectionsRequest,
    ) -> Result<RefundProjectionPage, PortError>;

    async fn read_refund_projection(
        &self,
        context: PortContext,
        request: ReadRefundProjectionRequest,
    ) -> Result<RefundResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListPaymentCollectionProjectionsRequest {
    pub page: u64,
    pub per_page: u64,
    pub status: Option<String>,
    pub order_id: Option<Uuid>,
    pub cart_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadPaymentCollectionProjectionRequest {
    pub collection_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentCollectionProjectionPage {
    pub items: Vec<PaymentCollectionResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListRefundProjectionsRequest {
    pub page: u64,
    pub per_page: u64,
    pub payment_collection_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadRefundProjectionRequest {
    pub refund_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundProjectionPage {
    pub items: Vec<RefundResponse>,
    pub total: u64,
}

pub struct InProcessPaymentAdminReadPort {
    inner: PaymentService,
}

impl InProcessPaymentAdminReadPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: PaymentService::new(db),
        }
    }
}

pub fn in_process_payment_admin_read_port(db: DatabaseConnection) -> Arc<dyn PaymentAdminReadPort> {
    Arc::new(InProcessPaymentAdminReadPort::new(db))
}

#[derive(Clone)]
pub struct PaymentAdminReadRuntime {
    read_port: Arc<dyn PaymentAdminReadPort>,
}

impl PaymentAdminReadRuntime {
    pub fn new(read_port: Arc<dyn PaymentAdminReadPort>) -> Self {
        Self { read_port }
    }

    pub fn in_process(db: DatabaseConnection) -> Self {
        Self::new(in_process_payment_admin_read_port(db))
    }

    pub fn read_port(&self) -> Arc<dyn PaymentAdminReadPort> {
        self.read_port.clone()
    }
}

#[async_trait]
impl PaymentAdminReadPort for InProcessPaymentAdminReadPort {
    async fn list_payment_collection_projections(
        &self,
        context: PortContext,
        request: ListPaymentCollectionProjectionsRequest,
    ) -> Result<PaymentCollectionProjectionPage, PortError> {
        let operation = "list_payment_collection_projections";
        let tenant_id = require_admin_read_context(&context, operation)?;
        let (items, total) = self
            .inner
            .list_collections(
                tenant_id,
                ListPaymentCollectionsInput {
                    page: request.page,
                    per_page: request.per_page,
                    status: request.status,
                    order_id: request.order_id,
                    cart_id: request.cart_id,
                    customer_id: request.customer_id,
                },
            )
            .await
            .map_err(|error| map_payment_error(&context, operation, error))?;
        Ok(PaymentCollectionProjectionPage { items, total })
    }

    async fn read_payment_collection_projection(
        &self,
        context: PortContext,
        request: ReadPaymentCollectionProjectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        let operation = "read_payment_collection_projection";
        let tenant_id = require_admin_read_context(&context, operation)?;
        self.inner
            .get_collection(tenant_id, request.collection_id)
            .await
            .map_err(|error| map_payment_error(&context, operation, error))
    }

    async fn list_refund_projections(
        &self,
        context: PortContext,
        request: ListRefundProjectionsRequest,
    ) -> Result<RefundProjectionPage, PortError> {
        let operation = "list_refund_projections";
        let tenant_id = require_admin_read_context(&context, operation)?;
        let (items, total) = self
            .inner
            .list_refunds(
                tenant_id,
                ListRefundsInput {
                    page: request.page,
                    per_page: request.per_page,
                    payment_collection_id: request.payment_collection_id,
                    order_id: request.order_id,
                    status: request.status,
                },
            )
            .await
            .map_err(|error| map_payment_error(&context, operation, error))?;
        Ok(RefundProjectionPage { items, total })
    }

    async fn read_refund_projection(
        &self,
        context: PortContext,
        request: ReadRefundProjectionRequest,
    ) -> Result<RefundResponse, PortError> {
        let operation = "read_refund_projection";
        let tenant_id = require_admin_read_context(&context, operation)?;
        self.inner
            .get_refund(tenant_id, request.refund_id)
            .await
            .map_err(|error| map_payment_error(&context, operation, error))
    }
}

fn require_admin_read_context(
    context: &PortContext,
    operation: &'static str,
) -> Result<Uuid, PortError> {
    context
        .require_policy(PortCallPolicy::read())
        .inspect_err(|error| {
            log_context_rejection(context, operation, "policy", error);
        })?;
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let error = PortError::validation(
            "payment.admin_read_context_invalid",
            "payment admin read context is invalid",
        );
        log_context_rejection(context, operation, "tenant_id", &error);
        error
    })
}

fn map_payment_error(
    context: &PortContext,
    operation: &'static str,
    error: PaymentError,
) -> PortError {
    let (kind, code, message, retryable, error_variant) = match &error {
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            PortErrorKind::NotFound,
            "payment.admin_read_not_found",
            "payment resource was not found",
            false,
            "not_found",
        ),
        PaymentError::Validation(_) => (
            PortErrorKind::Validation,
            "payment.admin_read_validation",
            "payment read request is invalid",
            false,
            "validation",
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => (
            PortErrorKind::Conflict,
            "payment.admin_read_conflict",
            "payment state conflicts with the read request",
            false,
            "conflict",
        ),
        PaymentError::ProviderConfiguration { .. } => (
            PortErrorKind::Unavailable,
            "payment.admin_read_configuration",
            "payment read capability is temporarily unavailable",
            true,
            "provider_configuration",
        ),
        PaymentError::ProviderUnavailable { .. } | PaymentError::Database(_) => (
            PortErrorKind::Unavailable,
            "payment.admin_read_unavailable",
            "payment read capability is temporarily unavailable",
            true,
            "unavailable",
        ),
        PaymentError::ProviderInvalidResponse { .. }
        | PaymentError::ProviderOutcomeUnknown { .. } => (
            PortErrorKind::InvariantViolation,
            "payment.admin_read_invariant",
            "payment state could not be read safely",
            false,
            "invariant_violation",
        ),
    };
    let technical_failure = matches!(
        &kind,
        PortErrorKind::Unavailable | PortErrorKind::InvariantViolation
    );
    if technical_failure {
        tracing::error!(
            owner = PAYMENT_ADMIN_READ_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            actor_kind = actor_kind(&context.actor.kind),
            actor_id_length = context.actor.id.chars().count(),
            channel_present = context.channel.is_some(),
            locale_length = context.locale.chars().count(),
            deadline_ms = ?context.deadline_ms,
            error_variant,
            boundary = PAYMENT_ADMIN_READ_BOUNDARY,
            "payment admin read owner operation failed"
        );
    } else {
        tracing::warn!(
            owner = PAYMENT_ADMIN_READ_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            actor_kind = actor_kind(&context.actor.kind),
            actor_id_length = context.actor.id.chars().count(),
            channel_present = context.channel.is_some(),
            locale_length = context.locale.chars().count(),
            deadline_ms = ?context.deadline_ms,
            error_variant,
            boundary = PAYMENT_ADMIN_READ_BOUNDARY,
            "payment admin read owner operation was rejected"
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
        owner = PAYMENT_ADMIN_READ_OWNER,
        operation,
        admission,
        correlation_id = %context.correlation_id,
        tenant_id_length = context.tenant_id.chars().count(),
        actor_kind = actor_kind(&context.actor.kind),
        actor_id_length = context.actor.id.chars().count(),
        channel_present = context.channel.is_some(),
        locale_length = context.locale.chars().count(),
        deadline_ms = ?context.deadline_ms,
        code = %error.code,
        retryable = error.retryable,
        boundary = PAYMENT_ADMIN_READ_BOUNDARY,
        "payment admin read context was rejected"
    );
}

fn actor_kind(kind: &PortActorKind) -> &'static str {
    match kind {
        PortActorKind::User => "user",
        PortActorKind::Service => "service",
        PortActorKind::System => "system",
    }
}
