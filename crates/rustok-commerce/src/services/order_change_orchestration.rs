use rustok_order::OrderService;
use rustok_order::dto::ApplyOrderChangeInput;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::providers::PaymentProviderRegistry;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use super::post_order::{
    ApplyOrderChangeResult, ExchangeDifferenceRefundInput, PostOrderOrchestrationResult,
    PostOrderOrchestrationService,
};

/// Routes order-change application through the correct post-order workflow.
///
/// Transport layers must not inspect `change_type` and duplicate exchange/claim
/// branching. The order owner still persists every lifecycle transition, while
/// commerce coordinates any cross-domain refund work.
pub struct OrderChangeOrchestrationService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    payment_provider_registry: PaymentProviderRegistry,
}

impl OrderChangeOrchestrationService {
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
}
