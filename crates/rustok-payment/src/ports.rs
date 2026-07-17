use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::PaymentCollectionResponse;

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
                .map_err(payment_error_to_port_error)?
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
                        .map_err(payment_error_to_port_error)?
                    {
                        return Ok(collection);
                    }
                }
                Err(payment_error_to_port_error(create_error))
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
            .map_err(payment_error_to_port_error)?;
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

fn payment_error_to_port_error(error: crate::PaymentError) -> PortError {
    match error {
        crate::PaymentError::Validation(message) => {
            PortError::validation("payment.validation", message)
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
        crate::PaymentError::InvalidTransition { from, to } => PortError::conflict(
            "payment.invalid_transition",
            format!("invalid payment transition from `{from}` to `{to}`"),
        ),
        crate::PaymentError::ProviderUnavailable {
            provider_id,
            operation,
        } => PortError::unavailable(
            "payment.provider_unavailable",
            format!("payment provider `{provider_id}` is unavailable for `{operation}`"),
        ),
        crate::PaymentError::ProviderRejected {
            provider_id,
            operation,
        } => PortError::conflict(
            "payment.provider_rejected",
            format!("payment provider `{provider_id}` rejected `{operation}`"),
        ),
        crate::PaymentError::ProviderInvalidResponse {
            provider_id,
            operation,
        } => PortError::new(
            PortErrorKind::InvariantViolation,
            "payment.provider_invalid_response",
            format!(
                "payment provider `{provider_id}` returned an invalid response for `{operation}`"
            ),
            false,
        ),
        crate::PaymentError::ProviderOutcomeUnknown {
            provider_id,
            operation,
        } => PortError::new(
            PortErrorKind::Conflict,
            "payment.provider_outcome_unknown",
            format!("payment provider `{provider_id}` outcome is unknown for `{operation}`"),
            false,
        ),
        crate::PaymentError::ProviderConfiguration { provider_id } => PortError::new(
            PortErrorKind::InvariantViolation,
            "payment.provider_not_configured",
            format!("payment provider `{provider_id}` is not configured"),
            false,
        ),
        crate::PaymentError::Database(_) => PortError::unavailable(
            "payment.database_unavailable",
            "payment storage is unavailable",
        ),
    }
}
