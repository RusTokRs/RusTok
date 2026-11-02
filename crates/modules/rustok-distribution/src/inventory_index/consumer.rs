use async_trait::async_trait;
use rustok_core::{
    ModuleEventListenerContext,
    events::{
        ContractEventEnvelope, ContractEventPayload, DomainEvent, EventEnvelope, EventHandler,
        HandlerResult,
    },
};
use rustok_index::{
    EntityKey, IndexReplayJobEnqueueRequest, IndexSourceRefreshEventDelivery,
    PostgresIndexReplayJobStore, PostgresMutationStore, PostgresSchemaRegistrationStore,
    SharedIndexMutationEventRegistry, SharedIndexSchemaRegistry, SharedIndexSourceRegistry,
    persist_index_source_refresh,
};
use rustok_inventory::source_history::InventorySourceEvent;
use sea_orm::DatabaseConnection;

use super::{
    INVENTORY_STOCK_INDEX_SOURCE,
    schema::{schema_ref, stock_schema},
    source::InventoryStockSource,
};

pub(super) struct InventoryStockEventHandler {
    db: DatabaseConnection,
    source: InventoryStockSource,
    schemas: Option<SharedIndexSchemaRegistry>,
    sources: Option<SharedIndexSourceRegistry>,
    events: Option<SharedIndexMutationEventRegistry>,
}

impl InventoryStockEventHandler {
    pub(super) fn new(context: &ModuleEventListenerContext<'_>) -> Self {
        Self {
            db: context.db.clone(),
            source: InventoryStockSource::new(
                context.db.clone(),
                context
                    .extensions
                    .get::<rustok_auth::WorkerServiceContextRuntime>()
                    .cloned(),
            ),
            schemas: context
                .extensions
                .get::<SharedIndexSchemaRegistry>()
                .cloned(),
            sources: context
                .extensions
                .get::<SharedIndexSourceRegistry>()
                .cloned(),
            events: context
                .extensions
                .get::<SharedIndexMutationEventRegistry>()
                .cloned(),
        }
    }
}

#[async_trait]
impl EventHandler for InventoryStockEventHandler {
    fn name(&self) -> &'static str {
        "inventory_stock_index"
    }
    fn handles(&self, _event: &DomainEvent) -> bool {
        false
    }
    async fn handle(&self, _envelope: &EventEnvelope) -> HandlerResult {
        Err(refusal("inventory.index_sealed_delivery_required"))
    }
    fn handles_contract(&self, payload: &ContractEventPayload) -> bool {
        matches!(payload, ContractEventPayload::InventorySource(_))
    }

    async fn handle_contract(&self, envelope: &ContractEventEnvelope) -> HandlerResult {
        envelope
            .validate_registered_schema()
            .map_err(|_| refusal("inventory.index_source_contract_invalid"))?;
        let payload = envelope
            .payload()
            .map_err(|_| refusal("inventory.index_source_contract_invalid"))?;
        let ContractEventPayload::InventorySource(fact) = payload else {
            return Err(refusal("inventory.index_source_contract_invalid"));
        };
        let schemas = self
            .schemas
            .as_ref()
            .ok_or_else(|| refusal("inventory.index_schema_capability_unavailable"))?;
        let sources = self
            .sources
            .as_ref()
            .ok_or_else(|| refusal("inventory.index_source_capability_unavailable"))?;
        let events = self
            .events
            .as_ref()
            .ok_or_else(|| refusal("inventory.index_event_capability_unavailable"))?;
        let (baseline_id, changed) = match *fact {
            InventorySourceEvent::Changed {
                baseline_id,
                variant_id,
                source_version,
            } => (
                baseline_id,
                Some((variant_id, source_version, envelope.id())),
            ),
            InventorySourceEvent::BaselineActivated { baseline_id } => {
                if envelope.id() != baseline_id {
                    return Err(refusal("inventory.index_activation_identity_invalid"));
                }
                (baseline_id, None)
            }
        };
        // No envelope actor, creator, receipt or saved job claim participates in this authority.
        self.source
            .verify_fact(
                envelope.tenant_id(),
                envelope.correlation_id(),
                baseline_id,
                changed,
            )
            .await
            .map_err(|error| {
                rustok_core::Error::External(format!(
                    "inventory.index_current_owner_read_refused: {}",
                    error.code()
                ))
            })?;
        let schema = stock_schema().map_err(|_| refusal("inventory.index_schema_invalid"))?;
        PostgresSchemaRegistrationStore::new(self.db.clone())
            .register(envelope.tenant_id(), &schema)
            .await
            .map_err(|_| refusal("inventory.index_schema_persistence_unavailable"))?;
        match *fact {
            InventorySourceEvent::Changed {
                variant_id,
                source_version,
                ..
            } => {
                let delivery = IndexSourceRefreshEventDelivery::new(
                    fact.event_type(),
                    envelope.id(),
                    EntityKey {
                        tenant_id: envelope.tenant_id(),
                        schema: schema_ref()
                            .map_err(|_| refusal("inventory.index_schema_invalid"))?,
                        entity_id: variant_id,
                        locale: None,
                    },
                    source_version,
                    (),
                )
                .map_err(|_| refusal("inventory.index_source_contract_invalid"))?;
                let registry = schemas.shared();
                persist_index_source_refresh(
                    &PostgresMutationStore::new(self.db.clone()),
                    registry.as_ref(),
                    sources,
                    events,
                    &delivery,
                )
                .await
                .map_err(|_| refusal("inventory.index_changed_persistence_unavailable"))?;
                // The canonical module-delivery gate treats the handler's returned success as ACK.
                // There is no dummy acknowledger; the awaited generic store has already committed.
                Ok(())
            }
            InventorySourceEvent::BaselineActivated { .. } => {
                let request = IndexReplayJobEnqueueRequest::new(
                    envelope.tenant_id(),
                    baseline_id,
                    schema.reference,
                    INVENTORY_STOCK_INDEX_SOURCE,
                )
                .map_err(|_| refusal("inventory.index_replay_request_invalid"))?;
                PostgresIndexReplayJobStore::new(self.db.clone())
                    .enqueue(&request)
                    .await
                    .map_err(|_| refusal("inventory.index_replay_request_unavailable"))?;
                // Initial replay is now durable under the original activation UUID, not complete.
                Ok(())
            }
        }
    }
}

fn refusal(code: &'static str) -> rustok_core::Error {
    rustok_core::Error::External(code.into())
}
