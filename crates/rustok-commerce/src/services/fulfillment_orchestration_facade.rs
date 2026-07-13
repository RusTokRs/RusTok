use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, FulfillmentResponse, ReshipFulfillmentInput,
    ShipFulfillmentInput,
};

use super::fulfillment_orchestration::{
    FulfillmentOrchestrationResult, FulfillmentOrchestrationService as LegacyFulfillmentOrchestrationService,
};
use super::journaled_fulfillment_orchestration::JournaledFulfillmentOrchestrationService;

/// Compatibility facade that preserves the existing transport API while routing
/// provider-first lifecycle transitions through the durable operation journal.
pub struct FulfillmentOrchestrationService {
    legacy: LegacyFulfillmentOrchestrationService,
    journaled: JournaledFulfillmentOrchestrationService,
}

impl FulfillmentOrchestrationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            legacy: LegacyFulfillmentOrchestrationService::new(db.clone()),
            journaled: JournaledFulfillmentOrchestrationService::new(db),
        }
    }

    pub fn with_provider_registry(
        mut self,
        fulfillment_provider_registry: FulfillmentProviderRegistry,
    ) -> Self {
        self.legacy = self
            .legacy
            .with_provider_registry(fulfillment_provider_registry.clone());
        self.journaled = self
            .journaled
            .with_provider_registry(fulfillment_provider_registry);
        self
    }

    /// Creation remains local-first because the fulfillment row and item ownership
    /// validation must be persisted before a carrier label can reference it.
    pub async fn create_manual_fulfillment(
        &self,
        tenant_id: Uuid,
        input: CreateFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.legacy
            .create_manual_fulfillment(tenant_id, input)
            .await
    }

    pub async fn ship_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: ShipFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.journaled
            .ship_fulfillment(tenant_id, fulfillment_id, input)
            .await
    }

    pub async fn reship_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: ReshipFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.journaled
            .reship_fulfillment(tenant_id, fulfillment_id, input)
            .await
    }

    pub async fn cancel_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: CancelFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        self.journaled
            .cancel_fulfillment(tenant_id, fulfillment_id, input)
            .await
    }
}
