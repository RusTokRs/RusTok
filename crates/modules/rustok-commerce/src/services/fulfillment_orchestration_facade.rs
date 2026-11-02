use rustok_api::PortContext;
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_fulfillment::{FulfillmentAdminCommandPort, FulfillmentService, ShipmentStockPort};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, DeliverFulfillmentInput, FulfillmentResponse,
    ReopenFulfillmentInput, ReshipFulfillmentInput, ShipFulfillmentInput,
};

use super::fulfillment_orchestration::{
    FulfillmentOrchestrationError, FulfillmentOrchestrationResult,
    FulfillmentOrchestrationService as LegacyFulfillmentOrchestrationService,
};
use super::journaled_create_label_provider::wrap_create_label_providers;
use super::journaled_fulfillment_orchestration::JournaledFulfillmentOrchestrationService;

/// Compatibility facade that preserves the existing transport API while routing
/// fulfillment commands through one commerce-owned orchestration boundary.
pub struct FulfillmentOrchestrationService {
    db: DatabaseConnection,
    legacy: LegacyFulfillmentOrchestrationService,
    journaled: JournaledFulfillmentOrchestrationService,
    create_label_registry_error: Option<String>,
}

impl FulfillmentOrchestrationService {
    pub fn new(db: DatabaseConnection) -> Self {
        let registry = FulfillmentProviderRegistry::with_manual_provider();
        let (legacy, create_label_registry_error) =
            legacy_with_journaled_labels(db.clone(), registry.clone());
        Self {
            db: db.clone(),
            legacy,
            journaled: JournaledFulfillmentOrchestrationService::new(db)
                .with_provider_registry(registry),
            create_label_registry_error,
        }
    }

    pub fn with_provider_registry(
        mut self,
        fulfillment_provider_registry: FulfillmentProviderRegistry,
    ) -> Self {
        let (legacy, create_label_registry_error) =
            legacy_with_journaled_labels(self.db.clone(), fulfillment_provider_registry.clone());
        self.legacy = legacy;
        self.journaled = self
            .journaled
            .with_provider_registry(fulfillment_provider_registry);
        self.create_label_registry_error = create_label_registry_error;
        self
    }

    pub async fn create_manual_fulfillment(
        &self,
        tenant_id: Uuid,
        input: CreateFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        if tenant_id.is_nil() {
            return Err(FulfillmentOrchestrationError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        tracing::debug!(
            boundary = "commerce_fulfillment_orchestration_facade",
            %tenant_id,
            order_id = %input.order_id,
            "facade create_manual_fulfillment"
        );
        if let Some(error) = &self.create_label_registry_error {
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "failed to compose journaled create-label providers: {error}"
            )));
        }
        self.legacy
            .create_manual_fulfillment(tenant_id, input)
            .await
    }

    pub fn with_shipment_stock_port(mut self, port: Arc<dyn ShipmentStockPort>) -> Self {
        self.journaled = self.journaled.with_shipment_stock_port(port);
        self
    }
    pub fn with_order_read_port(self, port: Arc<dyn rustok_order::OrderReadPort>) -> Self {
        let stock = super::shipment_stock::inventory_shipment_stock_port(self.db.clone(), port);
        self.with_shipment_stock_port(stock)
    }
    pub fn with_fulfillment_admin_command_port(
        mut self,
        port: Arc<dyn FulfillmentAdminCommandPort>,
    ) -> Self {
        self.journaled = self.journaled.with_fulfillment_admin_command_port(port);
        self
    }
    pub async fn ship_fulfillment(
        &self,
        context: PortContext,
        fulfillment_id: Uuid,
        input: ShipFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.journaled
            .ship_fulfillment(context, fulfillment_id, input)
            .await
    }

    pub async fn deliver_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: DeliverFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        if tenant_id.is_nil() {
            return Err(FulfillmentOrchestrationError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        if fulfillment_id.is_nil() {
            return Err(FulfillmentOrchestrationError::Validation(
                "fulfillment_id must not be nil".to_string(),
            ));
        }
        tracing::debug!(
            boundary = "commerce_fulfillment_orchestration_facade",
            %tenant_id,
            %fulfillment_id,
            "facade deliver_fulfillment"
        );
        FulfillmentService::new(self.db.clone())
            .deliver_fulfillment(tenant_id, fulfillment_id, input)
            .await
            .map_err(Into::into)
    }

    pub async fn reopen_fulfillment(
        &self,
        context: PortContext,
        fulfillment_id: Uuid,
        input: ReopenFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.journaled
            .reopen_fulfillment(context, fulfillment_id, input)
            .await
    }
    pub async fn reship_fulfillment(
        &self,
        context: PortContext,
        fulfillment_id: Uuid,
        input: ReshipFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.journaled
            .reship_fulfillment(context, fulfillment_id, input)
            .await
    }
    pub async fn cancel_fulfillment(
        &self,
        context: PortContext,
        fulfillment_id: Uuid,
        input: CancelFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.journaled
            .cancel_fulfillment(context, fulfillment_id, input)
            .await
    }
}

fn legacy_with_journaled_labels(
    db: DatabaseConnection,
    registry: FulfillmentProviderRegistry,
) -> (LegacyFulfillmentOrchestrationService, Option<String>) {
    match wrap_create_label_providers(db.clone(), registry.clone()) {
        Ok(wrapped) => (
            LegacyFulfillmentOrchestrationService::new(db).with_provider_registry(wrapped),
            None,
        ),
        Err(error) => (
            LegacyFulfillmentOrchestrationService::new(db).with_provider_registry(registry),
            Some(error.to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn facade_rejects_nil_uuids() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        let service = FulfillmentOrchestrationService::new(db);

        let err = service
            .create_manual_fulfillment(
                Uuid::nil(),
                CreateFulfillmentInput {
                    order_id: Uuid::new_v4(),
                    shipping_option_id: None,
                    customer_id: None,
                    carrier: None,
                    tracking_number: None,
                    items: None,
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, FulfillmentOrchestrationError::Validation(msg) if msg.contains("tenant_id"))
        );

        let err = service
            .ship_fulfillment(
                crate::services::shipment_stock::test_shipment_context(Uuid::nil()),
                Uuid::new_v4(),
                ShipFulfillmentInput {
                    carrier: "carrier".to_string(),
                    tracking_number: "track".to_string(),
                    items: None,
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, FulfillmentOrchestrationError::Validation(msg) if msg.contains("tenant_id"))
        );

        let err = service
            .ship_fulfillment(
                crate::services::shipment_stock::test_shipment_context(Uuid::new_v4()),
                Uuid::nil(),
                ShipFulfillmentInput {
                    carrier: "carrier".to_string(),
                    tracking_number: "track".to_string(),
                    items: None,
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, FulfillmentOrchestrationError::Validation(msg) if msg.contains("fulfillment_id"))
        );
    }
}
