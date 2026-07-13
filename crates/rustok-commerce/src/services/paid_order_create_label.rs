use async_trait::async_trait;
use rustok_core::events::{DomainEvent, EventEnvelope, EventHandler, HandlerResult};
use rustok_fulfillment::entities::{fulfillment, provider_operation};
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_fulfillment::{PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_PENDING};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use super::FulfillmentCreateLabelRecoveryService;

/// Executes durable checkout create-label operations only after the parent order is paid.
///
/// The database owns enqueueing and payment guards. This listener is only an execution trigger:
/// replayed order events skip committed operations, while unknown provider outcomes remain in the
/// reconciliation queue and are never invoked a second time automatically.
#[derive(Clone)]
pub(crate) struct PaidOrderCreateLabelHandler {
    db: DatabaseConnection,
    fulfillment_provider_registry: FulfillmentProviderRegistry,
}

impl PaidOrderCreateLabelHandler {
    pub(crate) fn new(
        db: DatabaseConnection,
        fulfillment_provider_registry: FulfillmentProviderRegistry,
    ) -> Self {
        Self {
            db,
            fulfillment_provider_registry,
        }
    }

    async fn dispatch_order(&self, tenant_id: Uuid, order_id: Uuid) -> HandlerResult {
        let fulfillments = fulfillment::Entity::find()
            .filter(fulfillment::Column::TenantId.eq(tenant_id))
            .filter(fulfillment::Column::OrderId.eq(order_id))
            .all(&self.db)
            .await?;
        if fulfillments.is_empty() {
            return Ok(());
        }

        let fulfillment_ids = fulfillments
            .into_iter()
            .map(|fulfillment| fulfillment.id)
            .collect::<Vec<_>>();
        let operations = provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::FulfillmentId.is_in(fulfillment_ids))
            .filter(provider_operation::Column::Operation.eq("create_label"))
            .filter(provider_operation::Column::Status.is_in([
                PROVIDER_OPERATION_PENDING.to_string(),
                PROVIDER_OPERATION_ERROR.to_string(),
            ]))
            .order_by_asc(provider_operation::Column::CreatedAt)
            .all(&self.db)
            .await?;
        if operations.is_empty() {
            return Ok(());
        }

        let recovery = FulfillmentCreateLabelRecoveryService::new(self.db.clone())
            .with_provider_registry(self.fulfillment_provider_registry.clone());
        let mut failures = Vec::new();
        for operation in operations {
            match recovery.retry(tenant_id, operation.id).await {
                Ok(_) => {
                    tracing::info!(
                        tenant_id = %tenant_id,
                        order_id = %order_id,
                        fulfillment_id = %operation.fulfillment_id,
                        operation_id = %operation.id,
                        provider_id = %operation.provider_id,
                        "Checkout fulfillment label operation committed after payment"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        tenant_id = %tenant_id,
                        order_id = %order_id,
                        fulfillment_id = %operation.fulfillment_id,
                        operation_id = %operation.id,
                        provider_id = %operation.provider_id,
                        error = %error,
                        "Checkout fulfillment label operation requires retry or reconciliation"
                    );
                    failures.push(format!("{}: {error}", operation.id));
                }
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(rustok_core::Error::External(format!(
                "failed to dispatch one or more paid-order create-label operations: {}",
                failures.join("; ")
            )))
        }
    }
}

#[async_trait]
impl EventHandler for PaidOrderCreateLabelHandler {
    fn name(&self) -> &'static str {
        "commerce_paid_order_create_label"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(
            event,
            DomainEvent::OrderStatusChanged { new_status, .. } if new_status == "paid"
        )
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let DomainEvent::OrderStatusChanged {
            order_id,
            new_status,
            ..
        } = &envelope.event
        else {
            return Ok(());
        };
        if new_status != "paid" {
            return Ok(());
        }

        self.dispatch_order(envelope.tenant_id, *order_id).await
    }
}
