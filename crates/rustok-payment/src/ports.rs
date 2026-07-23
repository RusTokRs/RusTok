use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{PaymentCollectionResponse, PaymentCollectionStatusKind};

/// Transport-neutral owner boundary for payment collection create/reuse flows.
#[async_trait]
pub trait PaymentCollectionPort: Send + Sync {
    async fn create_or_reuse_collection(
        &self,
        context: PortContext,
        request: PaymentCollectionCreateOrReuseRequest,
    ) -> Result<PaymentCollectionResponse, PortError>;

    async fn read_collection_status(
        &self,
        context: PortContext,
        request: PaymentCollectionStatusRequest,
    ) -> Result<PaymentCollectionStatusSnapshot, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentCollectionCreateOrReuseRequest {
    pub cart_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub currency_code: String,
    pub amount: Decimal,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentCollectionStatusRequest {
    pub collection_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentCollectionStatusSnapshot {
    pub collection_id: Uuid,
    pub status: String,
    pub currency_code: String,
    pub amount: Decimal,
    pub authorized_amount: Decimal,
    pub captured_amount: Decimal,
    pub provider_id: Option<String>,
}

impl PaymentCollectionStatusSnapshot {
    pub fn from_response(response: &PaymentCollectionResponse) -> Self {
        Self {
            collection_id: response.id,
            status: response.status.clone(),
            currency_code: response.currency_code.clone(),
            amount: response.amount,
            authorized_amount: response.authorized_amount,
            captured_amount: response.captured_amount,
            provider_id: response.provider_id.clone(),
        }
    }

    pub fn status_kind(&self) -> PaymentCollectionStatusKind {
        PaymentCollectionStatusKind::from_raw(self.status.as_str())
    }
}

#[async_trait]
impl PaymentCollectionPort for crate::PaymentService {
    async fn create_or_reuse_collection(
        &self,
        context: PortContext,
        request: PaymentCollectionCreateOrReuseRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context)?;

        if let Some(cart_id) = request.cart_id {
            if let Some(collection) = self
                .find_reusable_collection_by_cart(tenant_id, cart_id)
                .await
                .map_err(|error| {
                    payment_error_to_port_error(
                        &context,
                        "create_or_reuse_collection.read_existing",
                        error,
                    )
                })?
            {
                return Ok(collection);
            }
        }

        let cart_id = request.cart_id;
        let create_result = self
            .create_collection(
                tenant_id,
                crate::CreatePaymentCollectionInput {
                    cart_id,
                    order_id: request.order_id,
                    customer_id: request.customer_id,
                    currency_code: request.currency_code,
                    amount: request.amount,
                    metadata: request.metadata,
                },
            )
            .await;

        match create_result {
            Ok(collection) => Ok(collection),
            Err(create_error) => {
                if let Some(cart_id) = cart_id {
                    if let Some(collection) = self
                        .find_reusable_collection_by_cart(tenant_id, cart_id)
                        .await
                        .map_err(|error| {
                            payment_error_to_port_error(
                                &context,
                                "create_or_reuse_collection.adopt_race",
                                error,
                            )
                        })?
                    {
                        return Ok(collection);
                    }
                }
                Err(payment_error_to_port_error(
                    &context,
                    "create_or_reuse_collection.create",
                    create_error,
                ))
            }
        }
    }

    async fn read_collection_status(
        &self,
        context: PortContext,
        request: PaymentCollectionStatusRequest,
    ) -> Result<PaymentCollectionStatusSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let response = self
            .get_collection(tenant_id, request.collection_id)
            .await
            .map_err(|error| {
                payment_error_to_port_error(&context, "read_collection_status", error)
            })?;
        Ok(PaymentCollectionStatusSnapshot::from_response(&response))
    }
}

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "payment.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for payment ports",
        )
    })
}

fn payment_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: crate::PaymentError,
) -> PortError {
    match error {
        crate::PaymentError::Validation(message) => {
            tracing::warn!(
                cause = %message,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "payment.validation",
                "payment owner rejected a collection request"
            );
            PortError::validation("payment.validation", "payment request is invalid")
        }
        crate::PaymentError::PaymentCollectionNotFound(id) => PortError::not_found(
            "payment.collection_not_found",
            format!("payment collection {id} not found"),
        ),
        crate::PaymentError::PaymentNotFound(id) => PortError::not_found(
            "payment.payment_not_found",
            format!("payment for collection {id} not found"),
        ),
        crate::PaymentError::RefundNotFound(id) => {
            PortError::not_found("payment.refund_not_found", format!("refund {id} not found"))
        }
        crate::PaymentError::InvalidTransition { from, to } => {
            tracing::warn!(
                from = %from,
                to = %to,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "payment.invalid_transition",
                "payment owner rejected a lifecycle transition"
            );
            PortError::conflict(
                "payment.invalid_transition",
                "payment lifecycle conflicts with the requested operation",
            )
        }
        crate::PaymentError::ProviderUnavailable {
            provider_id,
            operation,
        } => {
            tracing::error!(
                provider_id = %provider_id,
                provider_operation = %operation,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "payment.provider_unavailable",
                "payment provider is unavailable"
            );
            PortError::unavailable(
                "payment.provider_unavailable",
                "payment provider is temporarily unavailable",
            )
        }
        crate::PaymentError::ProviderRejected {
            provider_id,
            operation,
        } => {
            tracing::warn!(
                provider_id = %provider_id,
                provider_operation = %operation,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "payment.provider_rejected",
                "payment provider rejected an owner operation"
            );
            PortError::conflict(
                "payment.provider_rejected",
                "payment provider rejected the requested operation",
            )
        }
        crate::PaymentError::ProviderInvalidResponse {
            provider_id,
            operation,
        } => {
            tracing::error!(
                provider_id = %provider_id,
                provider_operation = %operation,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "payment.provider_invalid_response",
                "payment provider returned invalid normalized facts"
            );
            PortError::new(
                PortErrorKind::InvariantViolation,
                "payment.provider_invalid_response",
                "payment provider response could not be applied safely",
                false,
            )
        }
        crate::PaymentError::ProviderOutcomeUnknown {
            provider_id,
            operation,
        } => {
            tracing::error!(
                provider_id = %provider_id,
                provider_operation = %operation,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "payment.provider_outcome_unknown",
                "payment provider outcome requires reconciliation"
            );
            PortError::new(
                PortErrorKind::Conflict,
                "payment.provider_outcome_unknown",
                "payment provider outcome requires reconciliation",
                false,
            )
        }
        crate::PaymentError::ProviderConfiguration { provider_id } => {
            tracing::error!(
                provider_id = %provider_id,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "payment.provider_not_configured",
                "payment provider configuration is unavailable"
            );
            PortError::new(
                PortErrorKind::InvariantViolation,
                "payment.provider_not_configured",
                "payment provider is not configured for the requested operation",
                false,
            )
        }
        crate::PaymentError::Database(error) => {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "payment.database_unavailable",
                "payment owner storage operation failed"
            );
            PortError::unavailable(
                "payment.database_unavailable",
                "payment storage is temporarily unavailable",
            )
        }
    }
}
