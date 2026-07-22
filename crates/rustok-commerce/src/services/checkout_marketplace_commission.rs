use std::{collections::HashSet, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_marketplace_commission::{
    AssessMarketplaceOrderCommissionsInput, AssessMarketplaceOrderCommissionsResponse,
    MarketplaceCommissionCommandPort,
};
use rustok_order::OrderResponse;
use thiserror::Error;
use uuid::Uuid;

use super::CheckoutOrderPlanPayload;

const COMMISSION_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Debug, Error)]
pub enum CheckoutMarketplaceCommissionError {
    #[error("marketplace commission result is invalid: {0}")]
    Validation(String),
    #[error("marketplace commission boundary `{code}` failed: {message}")]
    Boundary {
        code: String,
        message: String,
        retryable: bool,
    },
}

pub type CheckoutMarketplaceCommissionResult<T> = Result<T, CheckoutMarketplaceCommissionError>;

pub struct CheckoutMarketplaceCommissionStage {
    commission_port: Arc<dyn MarketplaceCommissionCommandPort>,
}

impl CheckoutMarketplaceCommissionStage {
    pub fn new(commission_port: Arc<dyn MarketplaceCommissionCommandPort>) -> Self {
        Self { commission_port }
    }

    pub async fn assess_if_present(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        plan: &CheckoutOrderPlanPayload,
        order: &OrderResponse,
    ) -> CheckoutMarketplaceCommissionResult<Option<AssessMarketplaceOrderCommissionsResponse>>
    {
        if plan.marketplace_lines.is_empty() {
            return Ok(None);
        }

        let mut context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(actor_id.to_string()),
            plan.context.locale.clone(),
            format!("checkout-marketplace-commission-{operation_id}"),
        )
        .with_deadline(COMMISSION_DEADLINE)
        .with_idempotency_key(format!("checkout:{operation_id}:marketplace-commission:v1"));
        if let Some(channel) = plan.channel_slug.clone() {
            context = context.with_channel(channel);
        }

        let response = self
            .commission_port
            .assess_order(
                context,
                AssessMarketplaceOrderCommissionsInput {
                    order_id: order.id,
                    // Stable across retries. Using wall-clock time here would change the
                    // owner request hash for the same checkout idempotency key.
                    assessed_at: order.created_at.fixed_offset(),
                },
            )
            .await
            .map_err(map_port_error)?;
        validate_response(plan, order, &response)?;
        Ok(Some(response))
    }
}

fn validate_response(
    plan: &CheckoutOrderPlanPayload,
    order: &OrderResponse,
    response: &AssessMarketplaceOrderCommissionsResponse,
) -> CheckoutMarketplaceCommissionResult<()> {
    if response.order_id != order.id {
        return Err(CheckoutMarketplaceCommissionError::Validation(format!(
            "commission response order {} does not match checkout order {}",
            response.order_id, order.id
        )));
    }
    if response.assessments.len() != plan.marketplace_lines.len() {
        return Err(CheckoutMarketplaceCommissionError::Validation(format!(
            "commission response for order {} contains {} assessments, expected {}",
            order.id,
            response.assessments.len(),
            plan.marketplace_lines.len()
        )));
    }

    let expected_order_lines = plan
        .marketplace_lines
        .iter()
        .map(|planned| {
            order
                .line_items
                .get(planned.order_line_index)
                .map(|line| line.id)
                .ok_or_else(|| {
                    CheckoutMarketplaceCommissionError::Validation(format!(
                        "marketplace plan references missing order line index {}",
                        planned.order_line_index
                    ))
                })
        })
        .collect::<CheckoutMarketplaceCommissionResult<HashSet<_>>>()?;
    let mut actual_order_lines = HashSet::with_capacity(response.assessments.len());
    let mut allocation_ids = HashSet::with_capacity(response.assessments.len());
    for assessment in &response.assessments {
        if assessment.order_id != order.id
            || !assessment
                .currency_code
                .eq_ignore_ascii_case(&order.currency_code)
            || assessment.commission_amount < 0
            || assessment.seller_proceeds_amount < 0
        {
            return Err(CheckoutMarketplaceCommissionError::Validation(format!(
                "commission assessment {} does not match checkout order economics",
                assessment.id
            )));
        }
        if !actual_order_lines.insert(assessment.order_line_item_id) {
            return Err(CheckoutMarketplaceCommissionError::Validation(format!(
                "commission response contains duplicate order line {}",
                assessment.order_line_item_id
            )));
        }
        if !allocation_ids.insert(assessment.allocation_id) {
            return Err(CheckoutMarketplaceCommissionError::Validation(format!(
                "commission response contains duplicate allocation {}",
                assessment.allocation_id
            )));
        }
    }
    if actual_order_lines != expected_order_lines {
        return Err(CheckoutMarketplaceCommissionError::Validation(format!(
            "commission response for order {} does not cover the immutable marketplace line set",
            order.id
        )));
    }
    Ok(())
}

fn map_port_error(error: PortError) -> CheckoutMarketplaceCommissionError {
    let message = match error.kind {
        PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict => {
            error.message
        }
        PortErrorKind::Forbidden => "marketplace commission permission denied".to_string(),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            "marketplace commission owner is temporarily unavailable".to_string()
        }
        PortErrorKind::InvariantViolation => {
            "marketplace commission receipt requires operator review".to_string()
        }
    };
    CheckoutMarketplaceCommissionError::Boundary {
        code: error.code,
        message,
        retryable: error.retryable,
    }
}
