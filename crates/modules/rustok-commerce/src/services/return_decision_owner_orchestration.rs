use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_fulfillment::FulfillmentService;
use rustok_order::{
    CompleteOrderReturnInput, CompleteOrderReturnRequest, CreateOrderChangeInput,
    CreateOrderChangeRequest, CreateOrderReturnRequest, OrderPostOrderCommandPort,
    OrderReadPort, OrderReturnResponse, ReadOrderProjectionRequest,
};
use rustok_payment::{
    ListPaymentCollectionProjectionsRequest, PaymentAdminReadPort,
    ReadPaymentCollectionProjectionRequest,
    dto::{CreateRefundInput, RefundResponse},
    providers::PaymentProviderRegistry,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;
use validator::Validate;

use super::{
    PaymentOrchestrationService,
    return_refund_value::{
        resolve_return_refund_amount, return_refundable_value,
        validate_return_quantities_were_shipped,
    },
    post_order::{
        CreateReturnDecisionInput, PostOrderOrchestrationError, PostOrderOrchestrationResult,
        ReturnDecisionInput, ReturnDecisionResponse, ReturnRefundDecisionInput,
    },
};

#[derive(Debug, Error)]
pub enum ReturnDecisionOwnerOrchestrationError {
    #[error("order command owner port error: {0}")]
    OrderCommand(#[from] PortError),
    #[error("payment read owner port error: {0}")]
    PaymentRead(PortError),
    #[error(transparent)]
    PostOrder(#[from] PostOrderOrchestrationError),
}

pub type ReturnDecisionOwnerOrchestrationResult<T> =
    Result<T, ReturnDecisionOwnerOrchestrationError>;

/// Mounted return-decision orchestration with owner-local reads/writes behind host-selected ports.
///
/// Payment refund execution deliberately remains in the existing Payment orchestration path in this
/// bounded slice; only the captured-collection lookup is moved behind the Payment owner read port.
pub struct ReturnDecisionOwnerOrchestrationService {
    db: DatabaseConnection,
    order_commands: Arc<dyn OrderPostOrderCommandPort>,
    payment_reads: Arc<dyn PaymentAdminReadPort>,
    order_reads: Option<Arc<dyn OrderReadPort>>,
    payment_provider_registry: PaymentProviderRegistry,
}

impl ReturnDecisionOwnerOrchestrationService {
    pub fn new(
        db: DatabaseConnection,
        order_commands: Arc<dyn OrderPostOrderCommandPort>,
        payment_reads: Arc<dyn PaymentAdminReadPort>,
    ) -> Self {
        Self {
            db,
            order_commands,
            payment_reads,
            order_reads: None,
            payment_provider_registry: PaymentProviderRegistry::with_manual_provider(),
        }
    }

    /// Order owner read port used to value return refunds from the order's own line snapshot.
    /// Refund decisions fail closed when it is not configured.
    pub fn with_order_read_port(mut self, order_reads: Arc<dyn OrderReadPort>) -> Self {
        self.order_reads = Some(order_reads);
        self
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
        if tenant_id.is_nil() {
            return Err(ReturnDecisionOwnerOrchestrationError::PostOrder(
                PostOrderOrchestrationError::Validation("tenant_id must not be nil".to_string()),
            ));
        }
        if order_id.is_nil() {
            return Err(ReturnDecisionOwnerOrchestrationError::PostOrder(
                PostOrderOrchestrationError::Validation("order_id must not be nil".to_string()),
            ));
        }
        if base_context.tenant_id != tenant_id.to_string() {
            return Err(ReturnDecisionOwnerOrchestrationError::OrderCommand(
                PortError::validation(
                    "commerce.return_decision_owner_context_invalid",
                    "return decision owner context is invalid",
                ),
            ));
        }
        tracing::debug!(
            boundary = "commerce_return_decision_owner_orchestration",
            %tenant_id,
            %order_id,
            action = %input.decision.action,
            "creating return decision"
        );

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
                    .create_refund_for_return(
                        &base_context,
                        tenant_id,
                        order_id,
                        &order_return,
                        refund_input,
                    )
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
        base_context: &PortContext,
        tenant_id: Uuid,
        order_id: Uuid,
        order_return: &OrderReturnResponse,
        input: &ReturnRefundDecisionInput,
    ) -> ReturnDecisionOwnerOrchestrationResult<RefundResponse> {
        let collection_id = match input.payment_collection_id {
            Some(id) => {
                let read_context =
                    payment_read_context_for(base_context, "read_refund_collection", id);
                let collection = self
                    .payment_reads
                    .read_payment_collection_projection(
                        read_context,
                        ReadPaymentCollectionProjectionRequest { collection_id: id },
                    )
                    .await
                    .map_err(ReturnDecisionOwnerOrchestrationError::PaymentRead)?;
                validate_return_payment_collection_order(collection.order_id, id, order_id)?
            }
            None => {
                let read_context =
                    payment_read_context_for(base_context, "list_captured_collections", order_id);
                let page = self
                    .payment_reads
                    .list_payment_collection_projections(
                        read_context,
                        ListPaymentCollectionProjectionsRequest {
                            page: 1,
                            per_page: 1,
                            status: Some("captured".to_string()),
                            order_id: Some(order_id),
                            cart_id: None,
                            customer_id: None,
                        },
                    )
                    .await
                    .map_err(ReturnDecisionOwnerOrchestrationError::PaymentRead)?;
                page.items
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

        let order_reads = self.order_reads.as_ref().ok_or_else(|| {
            ReturnDecisionOwnerOrchestrationError::OrderCommand(PortError::validation(
                "commerce.return_decision_order_read_port_missing",
                "return refund valuation requires an order read port",
            ))
        })?;
        let order = order_reads
            .read_order_projection(
                payment_read_context_for(base_context, "read_return_order", order_id),
                ReadOrderProjectionRequest {
                    order_id,
                    tenant_default_locale: None,
                },
            )
            .await
            .map_err(ReturnDecisionOwnerOrchestrationError::OrderCommand)?;
        let fulfillments = FulfillmentService::new(self.db.clone())
            .list_by_order(tenant_id, order_id)
            .await
            .map_err(|error| {
                PostOrderOrchestrationError::OwnerPort {
                    owner: "fulfillment",
                    error: PortError::unavailable(
                        "return_decision.fulfillment_read_failed",
                        error.to_string(),
                    ),
                }
            })?;
        validate_return_quantities_were_shipped(&order, order_return, &fulfillments)?;
        let amount = resolve_return_refund_amount(
            input.amount,
            return_refundable_value(&order, order_return)?,
        )?;

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
            .map_err(PostOrderOrchestrationError::from)
            .map_err(ReturnDecisionOwnerOrchestrationError::PostOrder)
    }
}

fn validate_return_payment_collection_order(
    collection_order_id: Option<Uuid>,
    collection_id: Uuid,
    order_id: Uuid,
) -> PostOrderOrchestrationResult<Uuid> {
    if collection_order_id != Some(order_id) {
        return Err(PostOrderOrchestrationError::Validation(format!(
            "payment collection {collection_id} is not attached to order {order_id}"
        )));
    }
    Ok(collection_id)
}

fn payment_read_context_for(
    base: &PortContext,
    operation: &'static str,
    resource_id: Uuid,
) -> PortContext {
    let mut context = base.clone();
    context.correlation_id = format!("{}:{operation}:{resource_id}", base.correlation_id);
    context.idempotency_key = None;
    context
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
    let derived = {
        use sha2::{Digest, Sha256};
        let material = format!("v1:{root_idempotency_key}:{operation}:{resource_id}");
        let digest = Sha256::digest(material.as_bytes());
        format!("return-decision-{}-{operation}", hex::encode(digest))
    };
    context.idempotency_key = Some(derived);
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
    if let Some(refund) = &decision.refund {
        if let Some(payment_collection_id) = refund.payment_collection_id {
            if payment_collection_id.is_nil() {
                return Err(PostOrderOrchestrationError::Validation(
                    "payment_collection_id must not be nil".to_string(),
                ));
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_refund_collection_must_be_attached_to_target_order() {
        let collection_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();

        assert!(validate_return_payment_collection_order(None, collection_id, order_id).is_err());
        assert!(
            validate_return_payment_collection_order(Some(Uuid::new_v4()), collection_id, order_id)
                .is_err()
        );
        assert_eq!(
            validate_return_payment_collection_order(Some(order_id), collection_id, order_id)
                .unwrap(),
            collection_id
        );
    }

    #[test]
    fn validate_decision_shape_rejects_nil_payment_collection_id() {
        let decision = ReturnDecisionInput {
            action: "refund".to_string(),
            refund: Some(ReturnRefundDecisionInput {
                payment_collection_id: Some(Uuid::nil()),
                amount: None,
                reason: None,
                metadata: serde_json::json!({}),
            }),
            exchange: None,
            claim: None,
            metadata: serde_json::json!({}),
        };
        let err = validate_decision_shape("refund", &decision).unwrap_err();
        assert!(matches!(err, PostOrderOrchestrationError::Validation(msg) if msg.contains("payment_collection_id")));
    }
}
