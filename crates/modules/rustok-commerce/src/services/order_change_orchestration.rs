use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_order::{
    ApplyOrderChangeInput, ApplyOrderChangeRequest, OrderPostOrderCommandPort, OrderReadPort,
    OrderService, ReadOrderChangeProjectionRequest, in_process_order_post_order_command_port,
    in_process_order_read_port,
};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::providers::PaymentProviderRegistry;
use rustok_payment::{CreateRefundInput, ListPaymentCollectionsInput, PaymentService};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use super::payment_orchestration::PaymentOrchestrationService;
use super::post_order::{
    ApplyOrderChangeResult, ExchangeDifferenceRefundInput, PostOrderOrchestrationError,
    PostOrderOrchestrationResult, PostOrderOrchestrationService,
};

#[derive(Debug, Error)]
pub enum OrderChangeOrchestrationError {
    #[error("order read owner port error: {0}")]
    OrderRead(PortError),
    #[error("order command owner port error: {0}")]
    OrderCommand(PortError),
    #[error(transparent)]
    PostOrder(#[from] PostOrderOrchestrationError),
}

pub type OrderChangeOrchestrationResult<T> = Result<T, OrderChangeOrchestrationError>;

fn with_order_change_apply_action(
    metadata: Value,
    action: &'static str,
) -> PostOrderOrchestrationResult<Value> {
    let mut object = match metadata {
        Value::Null => serde_json::Map::new(),
        Value::Object(object) => object,
        _ => {
            return Err(PostOrderOrchestrationError::Validation(
                "metadata must be a JSON object".to_string(),
            ));
        }
    };
    object.insert(
        "apply_action".to_string(),
        Value::String(action.to_string()),
    );
    Ok(Value::Object(object))
}

fn decimal_from_exchange_value(
    value: &Value,
    field: &str,
) -> PostOrderOrchestrationResult<rust_decimal::Decimal> {
    let text = match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        _ => {
            return Err(PostOrderOrchestrationError::Validation(format!(
                "{field} must be a decimal string or JSON number"
            )));
        }
    };
    text.parse::<rust_decimal::Decimal>().map_err(|error| {
        PostOrderOrchestrationError::Validation(format!(
            "{field} contains an invalid decimal value: {error}"
        ))
    })
}

fn difference_refund_from_applied_order_change(
    order_change: &rustok_order::OrderChangeResponse,
) -> PostOrderOrchestrationResult<Option<ExchangeDifferenceRefundInput>> {
    let amount_value = order_change
        .preview
        .get("difference_refund_amount")
        .or_else(|| order_change.metadata.get("difference_refund_amount"))
        .or_else(|| order_change.preview.get("refund_amount"))
        .or_else(|| order_change.metadata.get("refund_amount"));
    let Some(amount_value) = amount_value else {
        return Ok(None);
    };

    let amount = decimal_from_exchange_value(amount_value, "difference_refund_amount")?;
    let reason = order_change
        .preview
        .get("difference_refund_reason")
        .or_else(|| order_change.metadata.get("difference_refund_reason"))
        .or_else(|| order_change.preview.get("refund_reason"))
        .or_else(|| order_change.metadata.get("refund_reason"))
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(Some(ExchangeDifferenceRefundInput {
        amount,
        reason,
        metadata: Value::Null,
    }))
}

fn with_exchange_refund_context(
    metadata: Value,
    change_id: Uuid,
) -> PostOrderOrchestrationResult<Value> {
    let mut object = match metadata {
        Value::Null => serde_json::Map::new(),
        Value::Object(object) => object,
        _ => {
            return Err(PostOrderOrchestrationError::Validation(
                "metadata must be a JSON object".to_string(),
            ));
        }
    };
    object.insert(
        "order_change_id".to_string(),
        Value::String(change_id.to_string()),
    );
    object.insert(
        "apply_action".to_string(),
        Value::String("exchange".to_string()),
    );
    Ok(Value::Object(object))
}

/// Explicit compatibility seam for the still-open Payment half of exchange application.
///
/// Mounted transports no longer re-enter a concrete Order service here. Payment collection
/// lookup/provider execution intentionally remains isolated in this helper until the next
/// topology slice injects host-selected Payment owner ports without changing refund semantics.
async fn create_exchange_difference_refund_compat(
    db: &DatabaseConnection,
    payment_provider_registry: &PaymentProviderRegistry,
    tenant_id: Uuid,
    order_id: Uuid,
    change_id: Uuid,
    order_change: &rustok_order::OrderChangeResponse,
    difference_refund: Option<ExchangeDifferenceRefundInput>,
) -> PostOrderOrchestrationResult<Option<rustok_payment::RefundResponse>> {
    let refund_input = match difference_refund {
        Some(input) => Some(input),
        None => difference_refund_from_applied_order_change(order_change)?,
    };
    let Some(refund_input) = refund_input else {
        return Ok(None);
    };
    if refund_input.amount <= rust_decimal::Decimal::ZERO {
        return Ok(None);
    }

    let payment_service = PaymentService::new(db.clone());
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
    let collection_id = collections
        .into_iter()
        .next()
        .map(|collection| collection.id)
        .ok_or_else(|| {
            PostOrderOrchestrationError::Validation(format!(
                "order {order_id} has no captured payment collection"
            ))
        })?;

    let refund = PaymentOrchestrationService::new(db.clone())
        .with_provider_registry(payment_provider_registry.clone())
        .create_refund(
            tenant_id,
            collection_id,
            CreateRefundInput {
                amount: refund_input.amount,
                reason: refund_input
                    .reason
                    .or_else(|| Some("exchange_difference".to_string())),
                metadata: with_exchange_refund_context(refund_input.metadata, change_id)?,
            },
        )
        .await?;
    Ok(Some(refund))
}

/// Routes order-change application through the correct post-order workflow.
///
/// Transport layers must not inspect `change_type` and duplicate exchange/claim
/// branching. Mounted REST and GraphQL inject host-selected owner ports through
/// `from_order_ports`; directly embedded compatibility callers retain the legacy
/// entrypoint until their separate compatibility cleanup is complete.
pub struct OrderChangeOrchestrationService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    order_reads: Arc<dyn OrderReadPort>,
    order_commands: Arc<dyn OrderPostOrderCommandPort>,
    payment_provider_registry: PaymentProviderRegistry,
}

impl OrderChangeOrchestrationService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        let order_reads = in_process_order_read_port(db.clone(), event_bus.clone());
        let order_commands =
            in_process_order_post_order_command_port(db.clone(), event_bus.clone());
        Self::from_order_ports(db, event_bus, order_reads, order_commands)
    }

    pub fn from_order_ports(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        order_reads: Arc<dyn OrderReadPort>,
        order_commands: Arc<dyn OrderPostOrderCommandPort>,
    ) -> Self {
        Self {
            db,
            event_bus,
            order_reads,
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

    /// Legacy compatibility entry point retained for directly embedded callers.
    pub async fn apply_order_change(
        &self,
        tenant_id: Uuid,
        change_id: Uuid,
        difference_refund: Option<ExchangeDifferenceRefundInput>,
        metadata: Value,
    ) -> PostOrderOrchestrationResult<ApplyOrderChangeResult> {
        let order_service = OrderService::new(self.db.clone(), self.event_bus.clone());
        let order_change = order_service.get_order_change(tenant_id, change_id).await?;

        let post_order =
            PostOrderOrchestrationService::new(self.db.clone(), self.event_bus.clone())
                .with_payment_provider_registry(self.payment_provider_registry.clone());

        match order_change.change_type.as_str() {
            "exchange" => {
                post_order
                    .apply_exchange_order_change(
                        tenant_id,
                        order_change.order_id,
                        change_id,
                        difference_refund,
                        metadata,
                    )
                    .await
            }
            "claim" => {
                post_order
                    .apply_claim_order_change(tenant_id, change_id, metadata)
                    .await
            }
            _ => {
                let order_change = order_service
                    .apply_order_change(tenant_id, change_id, ApplyOrderChangeInput { metadata })
                    .await?;
                Ok(ApplyOrderChangeResult {
                    order_change,
                    refund: None,
                })
            }
        }
    }

    /// Host-composed mounted transport entry point for order-owned read/apply operations.
    pub async fn apply_order_change_with_owner_ports(
        &self,
        tenant_id: Uuid,
        change_id: Uuid,
        order_read_context: PortContext,
        order_command_context: PortContext,
        difference_refund: Option<ExchangeDifferenceRefundInput>,
        metadata: Value,
    ) -> OrderChangeOrchestrationResult<ApplyOrderChangeResult> {
        let tenant_identity = tenant_id.to_string();
        if order_read_context.tenant_id != tenant_identity {
            return Err(OrderChangeOrchestrationError::OrderRead(
                PortError::validation(
                    "commerce.order_change_owner_context_invalid",
                    "order change owner context is invalid",
                ),
            ));
        }
        if order_command_context.tenant_id != tenant_identity {
            return Err(OrderChangeOrchestrationError::OrderCommand(
                PortError::validation(
                    "commerce.order_change_owner_context_invalid",
                    "order change owner context is invalid",
                ),
            ));
        }

        let order_change = self
            .order_reads
            .read_order_change_projection(
                order_read_context,
                ReadOrderChangeProjectionRequest { change_id },
            )
            .await
            .map_err(OrderChangeOrchestrationError::OrderRead)?;

        match order_change.change_type.as_str() {
            "exchange" => {
                let metadata = with_order_change_apply_action(metadata, "exchange")?;
                let order_change = self
                    .order_commands
                    .apply_change(
                        order_command_context,
                        ApplyOrderChangeRequest {
                            change_id,
                            input: ApplyOrderChangeInput { metadata },
                        },
                    )
                    .await
                    .map_err(OrderChangeOrchestrationError::OrderCommand)?;
                let refund = create_exchange_difference_refund_compat(
                    &self.db,
                    &self.payment_provider_registry,
                    tenant_id,
                    order_change.order_id,
                    change_id,
                    &order_change,
                    difference_refund,
                )
                .await?;
                Ok(ApplyOrderChangeResult {
                    order_change,
                    refund,
                })
            }
            "claim" => {
                let metadata = with_order_change_apply_action(metadata, "claim")?;
                let order_change = self
                    .order_commands
                    .apply_change(
                        order_command_context,
                        ApplyOrderChangeRequest {
                            change_id,
                            input: ApplyOrderChangeInput { metadata },
                        },
                    )
                    .await
                    .map_err(OrderChangeOrchestrationError::OrderCommand)?;
                Ok(ApplyOrderChangeResult {
                    order_change,
                    refund: None,
                })
            }
            _ => {
                let order_change = self
                    .order_commands
                    .apply_change(
                        order_command_context,
                        ApplyOrderChangeRequest {
                            change_id,
                            input: ApplyOrderChangeInput { metadata },
                        },
                    )
                    .await
                    .map_err(OrderChangeOrchestrationError::OrderCommand)?;
                Ok(ApplyOrderChangeResult {
                    order_change,
                    refund: None,
                })
            }
        }
    }
}
