use rust_decimal::Decimal;
use rustok_order::dto::{
    CompleteOrderReturnInput, CreateOrderChangeInput, OrderReturnResponse,
};
use rustok_order::OrderService;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::dto::{CompleteRefundInput, CreateRefundInput};
use rustok_payment::providers::PaymentProviderRegistry;
use rustok_payment::PaymentService;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use super::payment_orchestration::PaymentOrchestrationService;
use super::post_order::{PostOrderOrchestrationError, PostOrderOrchestrationResult};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteReturnResolutionInput {
    pub resolution_type: Option<String>,
    pub refund_id: Option<Uuid>,
    pub order_change_id: Option<Uuid>,
    pub refund: Option<CompleteReturnRefundInput>,
    pub exchange: Option<CompleteReturnExchangeInput>,
    pub claim: Option<CompleteReturnClaimInput>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteReturnRefundInput {
    pub payment_collection_id: Option<Uuid>,
    pub amount: Decimal,
    pub reason: Option<String>,
    pub metadata: Value,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteReturnExchangeInput {
    pub description: Option<String>,
    pub preview: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteReturnClaimInput {
    pub description: Option<String>,
    pub preview: Value,
    pub metadata: Value,
}

/// Coordinates completion of an existing return with an optional refund,
/// exchange, or claim resolution.
///
/// The complete command is validated before any provider or owner side effect.
/// Transports must only parse their wire formats and delegate this workflow.
pub struct ReturnCompletionOrchestrationService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    payment_provider_registry: PaymentProviderRegistry,
}

impl ReturnCompletionOrchestrationService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            db,
            event_bus,
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

    pub async fn complete_return(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        return_id: Uuid,
        input: CompleteReturnResolutionInput,
    ) -> PostOrderOrchestrationResult<OrderReturnResponse> {
        validate_completion_shape(&input)?;

        let CompleteReturnResolutionInput {
            resolution_type,
            refund_id,
            order_change_id,
            refund,
            exchange,
            claim,
            metadata,
        } = input;
        let order_service = OrderService::new(self.db.clone(), self.event_bus.clone());
        let mut owner_input = CompleteOrderReturnInput {
            resolution_type,
            refund_id,
            order_change_id,
            metadata: normalize_object_or_empty(metadata, "metadata")?,
        };

        if let Some(refund_input) = refund {
            let existing_return = order_service.get_return(tenant_id, return_id).await?;
            let collection_id = self
                .resolve_payment_collection(
                    tenant_id,
                    existing_return.order_id,
                    refund_input.payment_collection_id,
                )
                .await?;
            let payment_orchestration = PaymentOrchestrationService::new(self.db.clone())
                .with_provider_registry(self.payment_provider_registry.clone());
            let refund = payment_orchestration
                .create_refund_idempotent(
                    tenant_id,
                    collection_id,
                    format!("order_return:{return_id}:refund"),
                    CreateRefundInput {
                        amount: refund_input.amount,
                        reason: refund_input.reason,
                        metadata: normalize_object_or_empty(
                            refund_input.metadata,
                            "refund.metadata",
                        )?,
                    },
                )
                .await?;
            let refund = if refund_input.complete {
                payment_orchestration
                    .complete_refund(
                        tenant_id,
                        refund.id,
                        CompleteRefundInput {
                            metadata: serde_json::json!({
                                "source": "order_return_completion",
                                "return_id": return_id,
                            }),
                        },
                    )
                    .await?
            } else {
                refund
            };
            owner_input.resolution_type = Some("refund".to_string());
            owner_input.refund_id = Some(refund.id);
        } else if let Some(exchange_input) = exchange {
            let existing_return = order_service.get_return(tenant_id, return_id).await?;
            let order_change = order_service
                .create_order_change(
                    tenant_id,
                    actor_id,
                    existing_return.order_id,
                    build_resolution_order_change(
                        "exchange",
                        exchange_input.description,
                        exchange_input.preview,
                        exchange_input.metadata,
                        return_id,
                    )?,
                )
                .await?;
            owner_input.resolution_type = Some("exchange".to_string());
            owner_input.order_change_id = Some(order_change.id);
        } else if let Some(claim_input) = claim {
            let existing_return = order_service.get_return(tenant_id, return_id).await?;
            let order_change = order_service
                .create_order_change(
                    tenant_id,
                    actor_id,
                    existing_return.order_id,
                    build_resolution_order_change(
                        "claim",
                        claim_input.description,
                        claim_input.preview,
                        claim_input.metadata,
                        return_id,
                    )?,
                )
                .await?;
            owner_input.resolution_type = Some("claim".to_string());
            owner_input.order_change_id = Some(order_change.id);
        }

        order_service
            .complete_return(tenant_id, return_id, owner_input)
            .await
            .map_err(Into::into)
    }

    async fn resolve_payment_collection(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
        explicit_collection_id: Option<Uuid>,
    ) -> PostOrderOrchestrationResult<Uuid> {
        let payment_service = PaymentService::new(self.db.clone());
        if let Some(collection_id) = explicit_collection_id {
            let collection = payment_service
                .get_collection(tenant_id, collection_id)
                .await?;
            if collection.order_id != Some(order_id) {
                return Err(PostOrderOrchestrationError::Validation(format!(
                    "payment collection {collection_id} is not attached to order {order_id}"
                )));
            }
            return Ok(collection_id);
        }

        payment_service
            .find_latest_collection_by_order(tenant_id, order_id)
            .await?
            .map(|collection| collection.id)
            .ok_or_else(|| {
                PostOrderOrchestrationError::Validation(format!(
                    "order {order_id} has no payment collection for return refund"
                ))
            })
    }
}

fn validate_completion_shape(
    input: &CompleteReturnResolutionInput,
) -> PostOrderOrchestrationResult<()> {
    let helpers = usize::from(input.refund.is_some())
        + usize::from(input.exchange.is_some())
        + usize::from(input.claim.is_some());
    if helpers > 1 {
        return Err(PostOrderOrchestrationError::Validation(
            "refund, exchange, and claim helpers are mutually exclusive".to_string(),
        ));
    }
    if helpers > 0 && (input.refund_id.is_some() || input.order_change_id.is_some()) {
        return Err(PostOrderOrchestrationError::Validation(
            "resolution helpers cannot be combined with explicit refund_id or order_change_id"
                .to_string(),
        ));
    }

    let expected = if input.refund.is_some() {
        Some("refund")
    } else if input.exchange.is_some() {
        Some("exchange")
    } else if input.claim.is_some() {
        Some("claim")
    } else {
        None
    };
    if let (Some(expected), Some(actual)) = (expected, input.resolution_type.as_deref()) {
        if !actual.trim().eq_ignore_ascii_case(expected) {
            return Err(PostOrderOrchestrationError::Validation(format!(
                "{expected} helper requires resolution_type to be omitted or `{expected}`"
            )));
        }
    }
    Ok(())
}

fn build_resolution_order_change(
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
    let mut object = match normalize_object_or_empty(value, "metadata")? {
        Value::Object(object) => object,
        _ => unreachable!("normalize_object_or_empty returns an object"),
    };
    object.insert(
        "order_return_id".to_string(),
        Value::String(return_id.to_string()),
    );
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

fn normalize_object_or_empty(
    value: Value,
    field: &str,
) -> PostOrderOrchestrationResult<Value> {
    match value {
        Value::Null => Ok(serde_json::json!({})),
        Value::Object(_) => Ok(value),
        _ => Err(PostOrderOrchestrationError::Validation(format!(
            "{field} must be a JSON object"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> CompleteReturnResolutionInput {
        CompleteReturnResolutionInput {
            resolution_type: None,
            refund_id: None,
            order_change_id: None,
            refund: None,
            exchange: None,
            claim: None,
            metadata: Value::Null,
        }
    }

    #[test]
    fn conflicting_helpers_are_rejected_before_execution() {
        let mut value = input();
        value.refund = Some(CompleteReturnRefundInput {
            payment_collection_id: None,
            amount: Decimal::ONE,
            reason: None,
            metadata: Value::Null,
            complete: false,
        });
        value.exchange = Some(CompleteReturnExchangeInput {
            description: None,
            preview: Value::Null,
            metadata: Value::Null,
        });
        assert!(validate_completion_shape(&value).is_err());
    }

    #[test]
    fn helper_resolution_type_must_match() {
        let mut value = input();
        value.resolution_type = Some("claim".to_string());
        value.refund = Some(CompleteReturnRefundInput {
            payment_collection_id: None,
            amount: Decimal::ONE,
            reason: None,
            metadata: Value::Null,
            complete: false,
        });
        assert!(validate_completion_shape(&value).is_err());
    }
}
