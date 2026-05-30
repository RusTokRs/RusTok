use rust_decimal::Decimal;
use rustok_order::dto::{
    CompleteOrderReturnInput, CreateOrderChangeInput, CreateOrderReturnInput, OrderChangeResponse,
    OrderReturnResponse,
};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::dto::{CreateRefundInput, ListPaymentCollectionsInput, RefundResponse};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{OrderService, PaymentService};

#[derive(Debug, Error)]
pub enum PostOrderOrchestrationError {
    #[error("order error: {0}")]
    Order(#[from] rustok_order::error::OrderError),
    #[error("payment error: {0}")]
    Payment(#[from] rustok_payment::error::PaymentError),
    #[error("validation error: {0}")]
    Validation(String),
}

pub type PostOrderOrchestrationResult<T> = Result<T, PostOrderOrchestrationError>;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateReturnDecisionInput {
    #[serde(flatten)]
    pub return_request: CreateOrderReturnInput,
    pub decision: ReturnDecisionInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ReturnDecisionInput {
    #[validate(length(min = 1, max = 32))]
    pub action: String,
    pub refund: Option<ReturnRefundDecisionInput>,
    pub exchange: Option<ReturnExchangeDecisionInput>,
    pub claim: Option<ReturnClaimDecisionInput>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ReturnRefundDecisionInput {
    pub payment_collection_id: Option<Uuid>,
    pub amount: Option<Decimal>,
    #[validate(length(max = 255))]
    pub reason: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ReturnExchangeDecisionInput {
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    pub preview: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ReturnClaimDecisionInput {
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    pub preview: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReturnDecisionResponse {
    pub action: String,
    pub order_return: OrderReturnResponse,
    pub refund: Option<RefundResponse>,
    pub order_change: Option<OrderChangeResponse>,
    pub metadata: Value,
}

pub struct PostOrderOrchestrationService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl PostOrderOrchestrationService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    pub async fn create_return_decision(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
        input: CreateReturnDecisionInput,
    ) -> PostOrderOrchestrationResult<ReturnDecisionResponse> {
        input
            .validate()
            .map_err(|error| PostOrderOrchestrationError::Validation(error.to_string()))?;

        let action = normalize_decision_action(&input.decision.action)?;
        validate_decision_shape(&action, &input.decision)?;

        let decision_metadata = input.decision.metadata.clone();
        let order_service = OrderService::new(self.db.clone(), self.event_bus.clone());
        let order_return = order_service
            .create_return(tenant_id, order_id, input.return_request)
            .await?;

        let (order_return, refund, order_change) = match action.as_str() {
            "return_only" => {
                let order_return = complete_return_decision(
                    &order_service,
                    tenant_id,
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
                let order_return = complete_return_decision(
                    &order_service,
                    tenant_id,
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
                let order_change = order_service
                    .create_order_change(
                        tenant_id,
                        actor_id,
                        order_id,
                        build_return_order_change_input(
                            "exchange",
                            exchange_input.description.clone(),
                            exchange_input.preview.clone(),
                            exchange_input.metadata.clone(),
                            order_return.id,
                        )?,
                    )
                    .await?;
                let order_return = complete_return_decision(
                    &order_service,
                    tenant_id,
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
                let order_change = order_service
                    .create_order_change(
                        tenant_id,
                        actor_id,
                        order_id,
                        build_return_order_change_input(
                            "claim",
                            claim_input.description.clone(),
                            claim_input.preview.clone(),
                            claim_input.metadata.clone(),
                            order_return.id,
                        )?,
                    )
                    .await?;
                let order_return = complete_return_decision(
                    &order_service,
                    tenant_id,
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

        let amount = input
            .amount
            .unwrap_or_else(|| return_items_amount(order_return));
        if amount <= Decimal::ZERO {
            return Err(PostOrderOrchestrationError::Validation(
                "refund decision requires a positive amount or priced return items".to_string(),
            ));
        }

        payment_service
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

async fn complete_return_decision(
    order_service: &OrderService,
    tenant_id: Uuid,
    return_id: Uuid,
    resolution_type: Option<&str>,
    refund_id: Option<Uuid>,
    order_change_id: Option<Uuid>,
    metadata: Value,
) -> PostOrderOrchestrationResult<OrderReturnResponse> {
    order_service
        .complete_return(
            tenant_id,
            return_id,
            CompleteOrderReturnInput {
                resolution_type: resolution_type.map(str::to_string),
                refund_id,
                order_change_id,
                metadata: normalize_object_or_empty(metadata, "decision.metadata")?,
            },
        )
        .await
        .map_err(Into::into)
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
        preview: attach_return_context(preview, return_id)?,
        metadata: attach_return_context(metadata, return_id)?,
    })
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

fn return_items_amount(order_return: &OrderReturnResponse) -> Decimal {
    order_return
        .items
        .iter()
        .filter_map(|item| {
            item.metadata
                .get("refund_amount")
                .and_then(|value| value.as_str())
                .and_then(|value| value.parse::<Decimal>().ok())
        })
        .fold(Decimal::ZERO, |acc, amount| acc + amount)
}
