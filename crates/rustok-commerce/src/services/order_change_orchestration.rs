use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_order::{
    ApplyOrderChangeInput, ApplyOrderChangeRequest, OrderPostOrderCommandPort, OrderReadPort,
    OrderService, ReadOrderChangeProjectionRequest, in_process_order_post_order_command_port,
    in_process_order_read_port,
};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::providers::PaymentProviderRegistry;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

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

/// Routes order-change application through the correct post-order workflow.
///
/// Transport layers must not inspect `change_type` and duplicate exchange/claim
/// branching. Mounted REST can inject host-selected owner ports through
/// `from_order_ports`; directly embedded and GraphQL compatibility callers retain
/// the legacy method until their runtime-composition cutover is handled separately.
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

    /// Legacy compatibility entry point retained for GraphQL/runtime callers in this slice.
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

    /// Host-composed REST entry point for order-owned read/default-apply operations.
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

        let post_order =
            PostOrderOrchestrationService::new(self.db.clone(), self.event_bus.clone())
                .with_payment_provider_registry(self.payment_provider_registry.clone());

        match order_change.change_type.as_str() {
            "exchange" => post_order
                .apply_exchange_order_change(
                    tenant_id,
                    order_change.order_id,
                    change_id,
                    difference_refund,
                    metadata,
                )
                .await
                .map_err(OrderChangeOrchestrationError::from),
            "claim" => post_order
                .apply_claim_order_change(tenant_id, change_id, metadata)
                .await
                .map_err(OrderChangeOrchestrationError::from),
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
