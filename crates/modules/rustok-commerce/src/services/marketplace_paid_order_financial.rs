use async_trait::async_trait;
use rustok_core::events::{DomainEvent, EventEnvelope, EventHandler, HandlerResult};
use rustok_marketplace_ledger::MarketplaceLedgerCommandPort;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::PaymentService;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;
use uuid::Uuid;

use crate::entities::checkout_operation;

use super::{
    CheckoutOrderPlanJournal, IngestMarketplacePaidEvent, MarketplacePaidEventInboxService,
};

pub(crate) struct MarketplacePaidOrderFinancialHandler {
    db: DatabaseConnection,
    payment_service: PaymentService,
    plan_journal: CheckoutOrderPlanJournal,
    inbox: Arc<MarketplacePaidEventInboxService>,
}

impl MarketplacePaidOrderFinancialHandler {
    pub(crate) fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
    ) -> Self {
        Self {
            payment_service: PaymentService::new(db.clone()),
            plan_journal: CheckoutOrderPlanJournal::new(db.clone()),
            inbox: Arc::new(MarketplacePaidEventInboxService::new(
                db.clone(),
                event_bus,
                ledger_port,
            )),
            db,
        }
    }

    async fn dispatch(&self, envelope: &EventEnvelope, order_id: Uuid) -> HandlerResult {
        let operation = checkout_operation::Entity::find()
            .filter(checkout_operation::Column::TenantId.eq(envelope.tenant_id))
            .filter(checkout_operation::Column::OrderId.eq(order_id))
            .one(&self.db)
            .await?;
        let Some(operation) = operation else {
            return Ok(());
        };
        let plan = self
            .plan_journal
            .get(envelope.tenant_id, operation.id)
            .await
            .map_err(|error| rustok_core::Error::External(error.to_string()))?;
        if plan.payload.marketplace_lines.is_empty() {
            return Ok(());
        }
        let collection_id = operation.payment_collection_id.ok_or_else(|| {
            rustok_core::Error::External(format!(
                "paid marketplace order {order_id} checkout operation {} has no payment collection",
                operation.id
            ))
        })?;
        let payment = self
            .payment_service
            .get_collection(envelope.tenant_id, collection_id)
            .await
            .map_err(|error| rustok_core::Error::External(error.to_string()))?;
        let captured_at = payment.captured_at.ok_or_else(|| {
            rustok_core::Error::External(format!(
                "paid marketplace order {order_id} payment collection {} has no captured_at",
                payment.id
            ))
        })?;
        if payment.status != "captured" || payment.order_id != Some(order_id) {
            return Err(rustok_core::Error::External(format!(
                "paid marketplace order {order_id} payment collection {} is `{}`",
                payment.id, payment.status
            )));
        }

        self.inbox
            .ingest_and_process(IngestMarketplacePaidEvent {
                tenant_id: envelope.tenant_id,
                event_source: "order-domain".to_string(),
                event_id: envelope.id.to_string(),
                checkout_operation_id: operation.id,
                order_id,
                payment_collection_id: payment.id,
                captured_at: captured_at.fixed_offset(),
                currency_code: payment.currency_code,
                captured_amount: payment.captured_amount,
            })
            .await
            .map(|_| ())
            .map_err(|error| rustok_core::Error::External(error.to_string()))
    }
}

#[async_trait]
impl EventHandler for MarketplacePaidOrderFinancialHandler {
    fn name(&self) -> &'static str {
        "commerce_marketplace_paid_order_financial"
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
        self.dispatch(envelope, *order_id).await
    }
}
