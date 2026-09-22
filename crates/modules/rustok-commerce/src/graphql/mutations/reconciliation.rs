use async_graphql::{Context, ErrorExtensions, Object, Result};
use rustok_api::{Permission, graphql::require_module_enabled};
use rustok_payment::error::PaymentError;
use uuid::Uuid;

use crate::PaymentOrchestrationError;
use crate::graphql_runtime::refund_reconciliation_from_context;

use super::super::{MODULE_SLUG, require_commerce_permission, types::GqlRefund};

fn public_reconciliation_graphql_error(
    message: &'static str,
    code: &'static str,
    retryable: bool,
) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
    })
}

fn payment_error_envelope(error: &PaymentError) -> (&'static str, &'static str, bool) {
    match error {
        PaymentError::Validation(_) => (
            "Payment reconciliation request is invalid",
            "PAYMENT_RECONCILIATION_REQUEST_INVALID",
            false,
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            "Payment resource was not found",
            "PAYMENT_RESOURCE_NOT_FOUND",
            false,
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => (
            "Payment reconciliation conflicts with the current state",
            "PAYMENT_RECONCILIATION_STATE_CONFLICT",
            false,
        ),
        PaymentError::ProviderUnavailable { .. } | PaymentError::Database(_) => (
            "Payment reconciliation is temporarily unavailable",
            "PAYMENT_RECONCILIATION_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        PaymentError::ProviderInvalidResponse { .. }
        | PaymentError::ProviderOutcomeUnknown { .. } => (
            "Payment operation requires reconciliation",
            "PAYMENT_RECONCILIATION_REQUIRED",
            false,
        ),
        PaymentError::ProviderConfiguration { .. } => (
            "Payment reconciliation is not configured",
            "PAYMENT_CONFIGURATION_ERROR",
            false,
        ),
    }
}

fn reconciliation_graphql_error(
    tenant_id: Uuid,
    refund_id: Uuid,
    operation: &'static str,
    error: PaymentOrchestrationError,
) -> async_graphql::Error {
    tracing::error!(
        error = ?error,
        tenant_id = %tenant_id,
        refund_id = %refund_id,
        operation,
        "commerce GraphQL refund reconciliation failed"
    );

    let (message, code, retryable) = match &error {
        PaymentOrchestrationError::Provider(source)
        | PaymentOrchestrationError::Payment(source) => payment_error_envelope(source),
        PaymentOrchestrationError::ProviderAfterRefundReservation { .. } => (
            "Payment operation requires reconciliation",
            "PAYMENT_RECONCILIATION_REQUIRED",
            false,
        ),
    };
    public_reconciliation_graphql_error(message, code, retryable)
}

#[derive(Default)]
pub struct CommerceReconciliationMutation;

#[Object]
impl CommerceReconciliationMutation {
    /// Resume a previously journaled refund provider operation using its original
    /// persisted request and idempotency key.
    async fn retry_refund_provider(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        refund_id: Uuid,
    ) -> Result<GqlRefund> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let refund = refund_reconciliation_from_context(ctx, db.clone())
            .retry_refund_provider(tenant_id, refund_id)
            .await
            .map_err(|error| {
                reconciliation_graphql_error(tenant_id, refund_id, "retry_refund_provider", error)
            })?;

        Ok(refund.into())
    }
}
