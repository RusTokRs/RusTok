//! Compatibility facade for the canonical Fulfillment-owned command gateway.
//! Never pass journal receipts through ordinary domain metadata (that path strips them).
//! Caller context/key is retained verbatim; recovery uses the original saved provider key,
//! including deployed legacy keys, not a newly-derived payload key or synthetic System actor.
use super::fulfillment_orchestration::{
    FulfillmentOrchestrationError, FulfillmentOrchestrationResult,
};
use crate::dto::{
    CancelFulfillmentInput, FulfillmentResponse, ReopenFulfillmentInput, ReshipFulfillmentInput,
    ShipFulfillmentInput,
};
use rustok_api::PortContext;
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_fulfillment::{
    CancelAdminFulfillmentRequest, FulfillmentAdminCommandPort, FulfillmentAdminCommandRuntime,
    ReopenAdminFulfillmentRequest, ReshipAdminFulfillmentRequest, ShipAdminFulfillmentRequest,
    ShipmentStockPort, UnconfiguredShipmentStockPort,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct JournaledFulfillmentOrchestrationService {
    db: DatabaseConnection,
    fulfillment_provider_registry: FulfillmentProviderRegistry,
    shipment_stock: Arc<dyn ShipmentStockPort>,
    selected_commands: Option<Arc<dyn FulfillmentAdminCommandPort>>,
}

impl JournaledFulfillmentOrchestrationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            fulfillment_provider_registry: FulfillmentProviderRegistry::with_manual_provider(),
            shipment_stock: Arc::new(UnconfiguredShipmentStockPort),
            selected_commands: None,
        }
    }
    pub fn with_provider_registry(mut self, registry: FulfillmentProviderRegistry) -> Self {
        self.fulfillment_provider_registry = registry;
        self
    }
    pub fn with_shipment_stock_port(mut self, port: Arc<dyn ShipmentStockPort>) -> Self {
        self.shipment_stock = port;
        self
    }
    /// Selected owner commands take precedence over native fallback regardless of builder order.
    pub fn with_fulfillment_admin_command_port(
        mut self,
        port: Arc<dyn FulfillmentAdminCommandPort>,
    ) -> Self {
        self.selected_commands = Some(port);
        self
    }
    fn commands(&self) -> Arc<dyn FulfillmentAdminCommandPort> {
        self.selected_commands.clone().unwrap_or_else(|| {
            FulfillmentAdminCommandRuntime::in_process_with_shipment_stock(
                self.db.clone(),
                self.fulfillment_provider_registry.clone(),
                self.shipment_stock.clone(),
            )
            .command_port()
        })
    }
    pub async fn retry_local_persistence(
        &self,
        context: PortContext,
        operation_id: Uuid,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        if operation_id.is_nil() {
            return Err(FulfillmentOrchestrationError::Validation(
                "operation_id must not be nil".into(),
            ));
        }
        self.commands()
            .retry_local_persistence(
                command_context(context, operation_id)?,
                rustok_fulfillment::RetryLocalFulfillmentPersistenceRequest { operation_id },
            )
            .await
            .map_err(FulfillmentOrchestrationError::Owner)
    }
    pub async fn ship_fulfillment(
        &self,
        context: PortContext,
        fulfillment_id: Uuid,
        input: ShipFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.commands()
            .ship_fulfillment(
                command_context(context, fulfillment_id)?,
                ShipAdminFulfillmentRequest {
                    fulfillment_id,
                    input,
                },
            )
            .await
            .map_err(FulfillmentOrchestrationError::Owner)
    }
    pub async fn reship_fulfillment(
        &self,
        context: PortContext,
        fulfillment_id: Uuid,
        input: ReshipFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.commands()
            .reship_fulfillment(
                command_context(context, fulfillment_id)?,
                ReshipAdminFulfillmentRequest {
                    fulfillment_id,
                    input,
                },
            )
            .await
            .map_err(FulfillmentOrchestrationError::Owner)
    }
    pub async fn cancel_fulfillment(
        &self,
        context: PortContext,
        fulfillment_id: Uuid,
        input: CancelFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.commands()
            .cancel_fulfillment(
                command_context(context, fulfillment_id)?,
                CancelAdminFulfillmentRequest {
                    fulfillment_id,
                    input,
                },
            )
            .await
            .map_err(FulfillmentOrchestrationError::Owner)
    }
    pub async fn reopen_fulfillment(
        &self,
        context: PortContext,
        fulfillment_id: Uuid,
        input: ReopenFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.commands()
            .reopen_fulfillment(
                command_context(context, fulfillment_id)?,
                ReopenAdminFulfillmentRequest {
                    fulfillment_id,
                    input,
                },
            )
            .await
            .map_err(FulfillmentOrchestrationError::Owner)
    }
}

fn command_context(
    context: PortContext,
    fulfillment_id: Uuid,
) -> FulfillmentOrchestrationResult<PortContext> {
    if Uuid::parse_str(context.tenant_id.trim())
        .ok()
        .is_none_or(|id| id.is_nil())
    {
        return Err(FulfillmentOrchestrationError::Validation(
            "tenant_id must be a non-nil UUID".into(),
        ));
    }
    if fulfillment_id.is_nil() {
        return Err(FulfillmentOrchestrationError::Validation(
            "fulfillment_id must not be nil".into(),
        ));
    }
    Ok(context)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_context_and_existing_provider_key_are_not_rederived() {
        let context = rustok_api::PortContext::new(
            Uuid::new_v4().to_string(),
            rustok_api::PortActor::user("operator"),
            "fr",
            "correlation",
        )
        .with_claim("fulfillment.update")
        .with_role("operator")
        .with_channel("admin")
        .with_traceparent("trace")
        .with_causation_id("parent")
        .with_idempotency_key("fulfillment:existing-saved-provider-key")
        .with_deadline(std::time::Duration::from_secs(20));
        let validated = command_context(context.clone(), Uuid::new_v4()).expect("valid context");
        assert_eq!(validated, context);
    }

    #[tokio::test]
    async fn journaled_fulfillment_rejects_nil_uuids() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        let service = JournaledFulfillmentOrchestrationService::new(db);

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

        let err = service
            .cancel_fulfillment(
                crate::services::shipment_stock::test_shipment_context(Uuid::nil()),
                Uuid::new_v4(),
                CancelFulfillmentInput {
                    reason: None,
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, FulfillmentOrchestrationError::Validation(msg) if msg.contains("tenant_id"))
        );

        let err = service
            .cancel_fulfillment(
                crate::services::shipment_stock::test_shipment_context(Uuid::new_v4()),
                Uuid::nil(),
                CancelFulfillmentInput {
                    reason: None,
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
