use std::sync::Arc;

use rust_decimal::Decimal;
use rustok_api::{PortContext, PortError};
use rustok_order::{
    CompleteOrderReturnInput, CompleteOrderReturnRequest, CreateOrderChangeInput,
    CreateOrderChangeRequest, CreateOrderReturnRequest, OrderPostOrderCommandPort,
    OrderReturnResponse,
};
use rustok_payment::{
    PaymentService,
    dto::{CreateRefundInput, ListPaymentCollectionsInput, RefundResponse},
    providers::PaymentProviderRegistry,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;
use validator::Validate;

use super::{
    PaymentOrchestrationService,
    post_order::{
        CreateReturnDecisionInput, PostOrderOrchestrationError, PostOrderOrchestrationResult,
        ReturnDecisionInput, ReturnDecisionResponse, ReturnRefundDecisionInput,
    },
};

#[derive(Debug, Error)]
pub enum ReturnDecisionOwnerOrchestrationError {
    #[error("order command owner port error: {0}")]
    OrderCommand(#[from] PortError),
    #[error(transparent)]
    PostOrder(#[from] PostOrderOrchestrationError),
}

pub type ReturnDecisionOwnerOrchestrationResult<T> =
    Result<T, ReturnDecisionOwnerOrchestrationError>;

/// Mounted return-decision orchestration with Order-owned writes behind a host-selected port.
///
/// Payment lookup/refund execution deliberately retains the existing Payment compatibility path in
/// this bounded slice. That dependency remains a separate topology gap.
pub struct ReturnDecisionOwnerOrchestrationService {
    db: DatabaseConnection,
    order_commands: Arc<dyn OrderPostOrderCommandPort>,
    payment_provider_registry: PaymentProviderRegistry,
}

impl ReturnDecisionOwnerOrchestrationService {
    pub fn new(
        db: DatabaseConnection,
        order_commands: Arc<dyn OrderPostOrderCommandPort>,
    ) -> Self {
        Self {
            db,
            order_commands,
            payment_provider_registry: PaymentProviderRegistry::with_manual_provider(),
        }
    }

    pub fn with_payment_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.payment_provider_registry = payment_provider_registry;
        self
    }

    pub async fn create_return_decision(
        &self,
        base_context: PortContext,
        tenant_id: Uuid,
        order_id: Uuid,
        input: CreateReturnDecisionInput,
    ) -> ReturnDecisionOwnerOrchestrationResult<ReturnDecisionResponse> {
        if base_context.tenant_id != tenant_id.to_string() {
            return Err(ReturnDecisionOwnerOrchestrationError::OrderCommand(
                PortError::validation(
                    "commerce.return_decision_owner_context_invalid",
                    "return decision owner context is invalid",
                ),
            ));
        }

        input
            .validate()
            .map_err(|error| PostOrderOrchestrationError::Validation(error.to_string()))?;
        let action = normalize_decision_action(&input.decision.action)?;
        validate_decision_shape(&action, &input.decision)?;

        let decision_metadata = input.decision.metadata.clone();
        let create_context = command_context_for(&base_context, "create_return", order_id)?;
        let order_return = self
            .order_commands
            .create_return(
                create_context,
                CreateOrderReturnRequest {
                    order_id,
                    input: input.return_request,
                },
            )
            .await
            .map_err(ReturnDecisionOwnerOrchestrationError::OrderCommand)?;

        let (order_return, refund, order_change) = match action.as_str() {
            "return_only" => {
                let order_return = self
                    .complete_return_decision(
                        &base_context,
                        order_return.id,
                        None,
                        None,
                        None,
                        decision_metadata.clone(),
                    )
                    .await?;
                (order_return, None, None)
            }
            "refund" => {
                let refund_input = input.decision.refund.as_ref().ok_or_else(|| {
                    PostOrderOrchestrationError::Validation(
                        "refund decision requires refund details".to_string(),
                    )
                })?;
                let refund = self
                    .create_refund_for_return(tenant_id, order_id, &order_return, refund_input)
                    .await?;
                let order_return = self
                    .complete_return_decision(
                        &base_context,
                        order_return.id,
                        Some("refund"),
                        Some(refund.id),
                        None,
                        decision_metadata.clone(),
                    )
                    .await?;
                (order_return, Some(refund), None)
            }
            "exchange" => {
                let exchange_input = input.decision.exchange.as_ref().ok_or_else(|| {
                    PostOrderOrchestrationError::Validation(
                        "exchange decision requires exchange details".to_string(),
                    )
                })?;
                let change_context = command_context_for(&base_context, "create_change", order_id)?;
                let order_change = self
                    .order_commands
                    .create_change(
                        change_context,
                        CreateOrderChangeRequest {
                            order_id,
                            input: build_return_order_change_input(
                                "exchange",
                                exchange_input.description.clone(),
                                exchange_input.preview.clone(),
                                exchange_input.metadata.clone(),
                                order_return.id,
                            )?,
                        },
                    )
                    .await
                    .map_err(ReturnDecisionOwnerOrchestrationError::OrderCommand)?;
                let order_return = self
                    .complete_return_decision(
                        &base_context,
                        order_return.id,
                        Some("exchange"),
                        None,
                        Some(order_change.id),
                        decision_metadata.clone(),
                    )
                    .await?;
                (order_return, None, Some(order_change))
            }
            "claim" => {
                let claim_input = input.decision.claim.as_ref().ok_or_else(|| {
                    PostOrderOrchestrationError::Validation(
                        "claim decision requires claim details".to_string(),
                    )
                })?;
                let change_context = command_context_for(&base_context, "create_change", order_id)?;
                let order_change = self
                    .order_commands
                    .create_change(
                        change_context,
                        CreateOrderChangeRequest {
                            order_id,
                            input: build_return_order_change_input(
                                "claim",
                                claim_input.description.clone(),
                                claim_input.preview.clone(),
                                claim_input.metadata.clone(),
                                order_return.id,
                            )?,
                        },
                    )
                    .await
                    .map_err(ReturnDecisionOwnerOrchestrationError::OrderCommand)?;
                let order_return = self
                    .complete_return_decision(
                        &base_context,
                        order_return.id,
                        Some("claim"),
                        None,
                        Some(order_change.id),
                        decision_metadata.clone(),
                    )
                    .await?;
                (order_return, None, Some(order_change))
            }
            _ => unreachable!("validated action"),
        };

        Ok(ReturnDecisionResponse {
            action,
            order_return,
            refund,
            order_change,
            metadata: normalize_object_or_empty(decision_metadata, "decision.metadata")?,
        })
    }

    async fn complete_return_decision(
        &self,
        base_context: &PortContext,
        return_id: Uuid,
        resolution_type: Option<&str>,
        refund_id: Option<Uuid>,
        order_change_id: Option<Uuid>,
        metadata: Value,
    ) -> ReturnDecisionOwnerOrchestrationResult<OrderReturnResponse> {
        let context = command_context_for(base_context, "complete_return", return_id)?;
        self.order_commands
            .complete_return(
                context,
                CompleteOrderReturnRequest {
                    return_id,
                    input: CompleteOrderReturnInput {
                        resolution_type: resolution_type.map(str::to_string),
                        refund_id,
                        order_change_id,
                        metadata: normalize_object_or_empty(metadata, "decision.metadata")?,
                    },
                },
            )
            .await
            .map_err(ReturnDecisionOwnerOrchestrationError::OrderCommand)
    }

    async fn create_refund_for_return(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
        order_return: &OrderReturnResponse,
        input: &ReturnRefundDecisionInput,
    ) -> PostOrderOrchestrationResult<RefundResponse> {
        let payment_service = PaymentService::new(self.db.clone());
        let collection_id = match input.payment_collection_id {
            Some(id) => id,
            None => {
                let (collections, _) = payment_service
                    .list_collections(
                        tenant_id,
                        ListPaymentCollectionsInput {
                            page: 1,
                            per_page: 1,
                            status: Some("captured".to_string()),
                            order_id: Some(order_id),
                            cart_id: None,
                            customer_id: None,
                        },
                    )
                    .await?;
                collections
                    .into_iter()
                    .next()
                    .map(|collection| collection.id)
                    .ok_or_else(|| {
                        PostOrderOrchestrationError::Validation(format!(
                            "order {order_id} has no captured payment collection for return refund"
                        ))
                    })?
            }
        };

        let amount = match input.amount {
            Some(amount) => amount,
            None => return_items_amount(order_return)?,
        };
        if amount <= Decimal::ZERO {
            return Err(PostOrderOrchestrationError::Validation(
                "refund decision requires a positive amount or priced return items".to_string(),
            ));
        }

        PaymentOrchestrationService::new(self.db.clone())
            .with_provider_registry(self.payment_provider_registry.clone())
            .create_refund(
                tenant_id,
                collection_id,
                CreateRefundInput {
                    amount,
                    reason: input.reason.clone().or_else(|| order_return.reason.clone()),
                    metadata: attach_return_context(input.metadata.clone(), order_return.id)?,
                },
            )
            .await
            .map_err(Into::into)
    }
}

fn command_context_for(
    base: &PortContext,
    operation: &'static str,
    resource_id: Uuid,
) -> Result<PortContext, PortError> {
    let Some(root_idempotency_key) = base
        .idempotency_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Err(PortError::validation(
            "commerce.return_decision_owner_context_invalid",
            "return decision owner context is invalid",
        ));
    };
    let mut context = base.clone();
    context.correlation_id = format!("{}:{operation}:{resource_id}", base.correlation_id);
    context.idempotency_key = Some(format!(
        "{root_idempotency_key}:{operation}:{resource_id}"
    ));
    Ok(context)
}

fn normalize_decision_action(action: &str) -> PostOrderOrchestrationResult<String> {
    let normalized = action.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "none" | "return" | "return_only" => Ok("return_only".to_string()),
        "refund" => Ok("refund".to_string()),
        "exchange" => Ok("exchange".to_string()),
        "claim" => Ok("claim".to_string()),
        _ => Err(PostOrderOrchestrationError::Validation(
            "return decision action must be one of return_only, refund, exchange, claim"
                .to_string(),
        )),
    }
}

fn validate_decision_shape(
    action: &str,
    decision: &ReturnDecisionInput,
) -> PostOrderOrchestrationResult<()> {
    if action != "refund" && decision.refund.is_some() {
        return Err(PostOrderOrchestrationError::Validation(
            "refund details are only allowed for refund decisions".to_string(),
        ));
    }
    if action != "exchange" && decision.exchange.is_some() {
        return Err(PostOrderOrchestrationError::Validation(
            "exchange details are only allowed for exchange decisions".to_string(),
        ));
    }
    if action != "claim" && decision.claim.is_some() {
        return Err(PostOrderOrchestrationError::Validation(
            "claim details are only allowed for claim decisions".to_string(),
        ));
    }
    Ok(())
}

fn build_return_order_change_input(
    change_type: &str,
    description: Option<String>,
    preview: Value,
    metadata: Value,
    return_id: Uuid,
) -> PostOrderOrchestrationResult<CreateOrderChangeInput> {
    Ok(CreateOrderChangeInput {
        change_type: change_type.to_string(),
        description,
        preview: attach_return_order_change_context(preview, return_id, change_type)?,
        metadata: attach_return_order_change_context(metadata, return_id, change_type)?,
    })
}

fn attach_return_order_change_context(
    value: Value,
    return_id: Uuid,
    change_type: &str,
) -> PostOrderOrchestrationResult<Value> {
    let mut object = match attach_return_context(value, return_id)? {
        Value::Object(object) => object,
        _ => unreachable!("attach_return_context returns object"),
    };
    object.insert(
        "return_decision_action".to_string(),
        Value::String(change_type.to_string()),
    );
    object.insert(
        "return_decision_source".to_string(),
        Value::String("rustok-commerce".to_string()),
    );
    Ok(Value::Object(object))
}

fn attach_return_context(value: Value, return_id: Uuid) -> PostOrderOrchestrationResult<Value> {
    let mut object = match normalize_object_or_empty(value, "metadata")? {
        Value::Object(object) => object,
        _ => unreachable!("normalize returns object"),
    };
    object.insert(
        "order_return_id".to_string(),
        Value::String(return_id.to_string()),
    );
    Ok(Value::Object(object))
}

fn normalize_object_or_empty(value: Value, field: &str) -> PostOrderOrchestrationResult<Value> {
    match value {
        Value::Null => Ok(serde_json::json!({})),
        Value::Object(_) => Ok(value),
        _ => Err(PostOrderOrchestrationError::Validation(format!(
            "{field} must be a JSON object"
        ))),
    }
}

fn return_items_amount(order_return: &OrderReturnResponse) -> PostOrderOrchestrationResult<Decimal> {
    order_return
        .items
        .iter()
        .filter_map(|item| item.metadata.get("refund_amount"))
        .try_fold(Decimal::ZERO, |total, value| {
            let amount = decimal_from_json_value(value, "refund_amount")?;
            if amount < Decimal::ZERO {
                return Err(PostOrderOrchestrationError::Validation(
                    "refund_amount must not be negative".to_string(),
                ));
            }
            total.checked_add(amount).ok_or_else(|| {
                PostOrderOrchestrationError::Validation(
                    "return item refund amount total overflowed Decimal".to_string(),
                )
            })
        })
}

fn decimal_from_json_value(value: &Value, field: &str) -> PostOrderOrchestrationResult<Decimal> {
    let text = match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        _ => {
            return Err(PostOrderOrchestrationError::Validation(format!(
                "{field} must be a decimal string or JSON number"
            )));
        }
    };
    text.parse::<Decimal>().map_err(|error| {
        PostOrderOrchestrationError::Validation(format!(
            "{field} contains an invalid decimal value: {error}"
        ))
    })
}
