//! Compatibility composition for the selected Fulfillment-owned local-recovery capability.
//! The owner, not Commerce, reads the retained journal, preserves its literal key and commits
//! local progress/native stock plus the provider checkpoint on one transaction.
use super::fulfillment_orchestration::FulfillmentOrchestrationResult;
use super::journaled_fulfillment_orchestration::JournaledFulfillmentOrchestrationService;
use crate::dto::FulfillmentResponse;
use rustok_api::PortContext;
use rustok_fulfillment::{FulfillmentAdminCommandPort, ShipmentStockPort};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct FulfillmentReconciliationService {
    db: DatabaseConnection,
    bridge: JournaledFulfillmentOrchestrationService,
}
impl FulfillmentReconciliationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            bridge: JournaledFulfillmentOrchestrationService::new(db.clone()),
            db,
        }
    }
    pub fn with_order_read_port(mut self, reads: Arc<dyn rustok_order::OrderReadPort>) -> Self {
        self.bridge = self.bridge.with_shipment_stock_port(
            super::shipment_stock::inventory_shipment_stock_port(self.db.clone(), reads),
        );
        self
    }
    pub fn with_shipment_stock_port(mut self, port: Arc<dyn ShipmentStockPort>) -> Self {
        self.bridge = self.bridge.with_shipment_stock_port(port);
        self
    }
    pub fn with_fulfillment_admin_command_port(
        mut self,
        port: Arc<dyn FulfillmentAdminCommandPort>,
    ) -> Self {
        self.bridge = self.bridge.with_fulfillment_admin_command_port(port);
        self
    }
    /// Never call the carrier; unsupported selected recovery fails rather than using native data.
    pub async fn retry_local_persistence(
        &self,
        context: PortContext,
        operation_id: Uuid,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.bridge
            .retry_local_persistence(context, operation_id)
            .await
    }
}

#[cfg(test)]
mod tests;
