//! Order cancellation orchestration.
//!
//! `OrderService::cancel_order` is an order-owned state change (its status trigger also releases the
//! order's active inventory reservations in the same transaction). Cancelling an order that still
//! has open fulfillments or customer money must also unwind those effects; this module is the
//! commerce-owned orchestration boundary that does it
//! (`DECISIONS/2026-10-21-order-cancellation-orchestration.md`).
//!
//! The orchestration is a stateless, idempotent saga:
//!
//! 1. validate everything that can reject the command (order status, shipped items, payment
//!    collection states) before any effect is executed;
//! 2. cancel open fulfillments, void or refund the payment collections, and only then flip the
//!    order to `cancelled`.
//!
//! Every step is idempotent and keyed by stable identities, and the order status is the last
//! write, so a failure in the middle leaves the order in its previous status and the same command
//! can simply be repeated to resume.

use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortContext, PortError};
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_fulfillment::{FulfillmentService, FulfillmentStatusKind};
use rustok_order::{
    CancelOrderRequest, DeliverOrderRequest, MarkOrderPaidRequest, OrderAdminCommandPort,
    OrderAdminCommandRuntime, OrderError, OrderResponse, OrderService, ShipOrderRequest,
};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::PaymentService;
use rustok_payment::dto::{
    CancelPaymentInput, CreateRefundInput, ListPaymentCollectionsInput, PaymentCollectionResponse,
    PaymentCollectionStatusKind, RefundStatusKind,
};
use rustok_payment::providers::PaymentProviderRegistry;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::dto::{CancelFulfillmentInput, FulfillmentResponse};

use super::fulfillment_orchestration_facade::FulfillmentOrchestrationService;
use super::order_shipment_projection::{ManualShipmentCommand, OrderShipmentProjector};
use super::payment_orchestration::PaymentOrchestrationService;

const ORDER_CANCELLATION_BOUNDARY: &str = "commerce_order_cancellation";
const ORDER_CANCELLATION_METADATA_KEY: &str = "order_cancellation";
const COLLECTION_PAGE_SIZE: u64 = 100;

/// Order statuses an operator may cancel from (mirrors the order owner's `can_cancel`).
const CANCELLABLE_ORDER_STATUSES: [&str; 4] = ["pending", "confirmed", "paid", "shipped"];

/// A payment effect the cancellation has to perform for one collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PaymentCancellationAction {
    /// Void an unpaid or only authorized collection through the provider.
    Void { collection_id: Uuid },
    /// Refund the captured amount that no other refund already claims.
    Refund {
        collection_id: Uuid,
        amount: Decimal,
    },
}

/// Why a cancellation cannot be planned. Reported before any effect is executed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CancellationBlock {
    ShippedItems { fulfillment_id: Uuid },
    UnknownFulfillmentStatus { fulfillment_id: Uuid },
    UnknownCollectionStatus { collection_id: Uuid },
}

impl CancellationBlock {
    fn into_port_error(self) -> PortError {
        match self {
            Self::ShippedItems { fulfillment_id } => PortError::conflict(
                "order_cancellation.shipped_items",
                format!(
                    "fulfillment {fulfillment_id} already shipped items; use a return instead of a cancellation"
                ),
            ),
            Self::UnknownFulfillmentStatus { fulfillment_id } => PortError::conflict(
                "order_cancellation.unknown_fulfillment_status",
                format!("fulfillment {fulfillment_id} has an unknown status"),
            ),
            Self::UnknownCollectionStatus { collection_id } => PortError::conflict(
                "order_cancellation.unknown_collection_status",
                format!("payment collection {collection_id} has an unknown status"),
            ),
        }
    }
}

/// Fulfillments that must be cancelled. Cancelled fulfillments are already done; a fulfillment
/// with shipped quantity is physically out of the warehouse and blocks the cancellation.
pub(crate) fn plan_fulfillment_cancellation(
    fulfillments: &[FulfillmentResponse],
) -> Result<Vec<Uuid>, CancellationBlock> {
    let mut to_cancel = Vec::new();
    for fulfillment in fulfillments {
        match fulfillment.status_kind() {
            FulfillmentStatusKind::Cancelled => {}
            FulfillmentStatusKind::Unknown => {
                return Err(CancellationBlock::UnknownFulfillmentStatus {
                    fulfillment_id: fulfillment.id,
                });
            }
            FulfillmentStatusKind::Shipped | FulfillmentStatusKind::Delivered => {
                return Err(CancellationBlock::ShippedItems {
                    fulfillment_id: fulfillment.id,
                });
            }
            FulfillmentStatusKind::Pending => {
                if fulfillment.items.iter().any(|item| item.shipped_quantity > 0) {
                    return Err(CancellationBlock::ShippedItems {
                        fulfillment_id: fulfillment.id,
                    });
                }
                to_cancel.push(fulfillment.id);
            }
        }
    }
    Ok(to_cancel)
}

/// Payment effects for the collections of an order.
///
/// A captured collection is refunded for `captured - other refunds`, where "other" excludes a
/// refund this cancellation already created (it carries the cancellation marker in its metadata),
/// so a retry after a provider failure re-submits the same amount under the same creation key
/// instead of concluding that nothing is left to refund.
pub(crate) fn plan_payment_cancellation(
    collections: &[PaymentCollectionResponse],
) -> Result<Vec<PaymentCancellationAction>, CancellationBlock> {
    let mut actions = Vec::new();
    for collection in collections {
        match collection.status_kind() {
            PaymentCollectionStatusKind::Cancelled => {}
            PaymentCollectionStatusKind::Unknown => {
                return Err(CancellationBlock::UnknownCollectionStatus {
                    collection_id: collection.id,
                });
            }
            PaymentCollectionStatusKind::Pending | PaymentCollectionStatusKind::Authorized => {
                actions.push(PaymentCancellationAction::Void {
                    collection_id: collection.id,
                });
            }
            PaymentCollectionStatusKind::Captured => {
                let claimed_by_others = collection
                    .refunds
                    .iter()
                    .filter(|refund| refund.status_kind() != RefundStatusKind::Cancelled)
                    .filter(|refund| !is_cancellation_refund(&refund.metadata))
                    .fold(Decimal::ZERO, |acc, refund| acc + refund.amount);
                let refundable = collection.captured_amount - claimed_by_others;
                if refundable > Decimal::ZERO {
                    actions.push(PaymentCancellationAction::Refund {
                        collection_id: collection.id,
                        amount: refundable,
                    });
                }
            }
        }
    }
    Ok(actions)
}

fn is_cancellation_refund(metadata: &Value) -> bool {
    metadata.get(ORDER_CANCELLATION_METADATA_KEY).is_some()
}

fn refund_creation_key(collection_id: Uuid) -> String {
    format!("order_cancellation:{collection_id}:refund")
}

/// Orchestrates the cancellation of an order across the fulfillment and payment owners.
#[derive(Clone)]
pub struct OrderCancellationOrchestrator {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    payment_provider_registry: PaymentProviderRegistry,
    fulfillment_provider_registry: FulfillmentProviderRegistry,
}

impl OrderCancellationOrchestrator {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            db,
            event_bus,
            payment_provider_registry: PaymentProviderRegistry::with_manual_provider(),
            fulfillment_provider_registry: FulfillmentProviderRegistry::with_manual_provider(),
        }
    }

    pub fn with_payment_provider_registry(mut self, registry: PaymentProviderRegistry) -> Self {
        self.payment_provider_registry = registry;
        self
    }

    pub fn with_fulfillment_provider_registry(
        mut self,
        registry: FulfillmentProviderRegistry,
    ) -> Self {
        self.fulfillment_provider_registry = registry;
        self
    }

    /// Cancels the order and unwinds its fulfillments and payments (stock is released by the order
    /// status trigger). Safe to repeat.
    pub async fn cancel(
        &self,
        context: PortContext,
        request: CancelOrderRequest,
    ) -> Result<OrderResponse, PortError> {
        context.require_write_semantics()?;
        if request.order_id.is_nil() {
            return Err(PortError::validation(
                "order.order_id_nil",
                "order_id must not be a nil UUID",
            ));
        }
        let tenant_id = parse_uuid(context.tenant_id.as_str(), "tenant_id")?;
        let actor_id = parse_uuid(context.actor.id.as_str(), "actor_id")?;
        let order_id = request.order_id;
        tracing::debug!(
            boundary = ORDER_CANCELLATION_BOUNDARY,
            tenant_id = %tenant_id,
            order_id = %order_id,
            "order cancellation entered"
        );

        let order_service = OrderService::new(self.db.clone(), self.event_bus.clone());
        let order = order_service
            .get_order(tenant_id, order_id)
            .await
            .map_err(map_order_error)?;
        if order.status == "cancelled" {
            return Ok(order);
        }
        if !CANCELLABLE_ORDER_STATUSES.contains(&order.status.as_str()) {
            return Err(PortError::conflict(
                "order_cancellation.status_not_cancellable",
                format!("order in status `{}` cannot be cancelled", order.status),
            ));
        }

        // Phase 1: plan. Nothing below may reject after an effect has been executed.
        let fulfillments = FulfillmentService::new(self.db.clone())
            .list_by_order(tenant_id, order_id)
            .await
            .map_err(|error| unavailable("order_cancellation.fulfillment_read_failed", error))?;
        let fulfillment_ids =
            plan_fulfillment_cancellation(&fulfillments).map_err(CancellationBlock::into_port_error)?;

        let payment_service = PaymentService::new(self.db.clone());
        let (collections, _) = payment_service
            .list_collections(
                tenant_id,
                ListPaymentCollectionsInput {
                    page: 1,
                    per_page: COLLECTION_PAGE_SIZE,
                    status: None,
                    order_id: Some(order_id),
                    cart_id: None,
                    customer_id: None,
                },
            )
            .await
            .map_err(|error| unavailable("order_cancellation.payment_read_failed", error))?;
        let payment_actions =
            plan_payment_cancellation(&collections).map_err(CancellationBlock::into_port_error)?;

        // Phase 2: effects, safest first. Money moves last, right before the order status flips.
        self.cancel_fulfillments(tenant_id, &request, &fulfillment_ids)
            .await?;
        self.unwind_payments(tenant_id, order_id, &request, &payment_actions)
            .await?;

        match order_service
            .cancel_order(tenant_id, actor_id, order_id, request.reason)
            .await
        {
            Ok(order) => Ok(order),
            // A concurrent cancellation won the race; the result is the same cancelled order.
            Err(OrderError::InvalidTransition { from, .. }) if from == "cancelled" => order_service
                .get_order(tenant_id, order_id)
                .await
                .map_err(map_order_error),
            Err(error) => Err(map_order_error(error)),
        }
    }

    async fn cancel_fulfillments(
        &self,
        tenant_id: Uuid,
        request: &CancelOrderRequest,
        fulfillment_ids: &[Uuid],
    ) -> Result<(), PortError> {
        if fulfillment_ids.is_empty() {
            return Ok(());
        }
        let service = FulfillmentOrchestrationService::new(self.db.clone())
            .with_provider_registry(self.fulfillment_provider_registry.clone());
        for fulfillment_id in fulfillment_ids {
            service
                .cancel_fulfillment(
                    tenant_id,
                    *fulfillment_id,
                    CancelFulfillmentInput {
                        reason: request.reason.clone(),
                        metadata: cancellation_metadata(request.order_id),
                    },
                )
                .await
                .map_err(|error| unavailable("order_cancellation.fulfillment_cancel_failed", error))?;
        }
        Ok(())
    }

    async fn unwind_payments(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
        request: &CancelOrderRequest,
        actions: &[PaymentCancellationAction],
    ) -> Result<(), PortError> {
        if actions.is_empty() {
            return Ok(());
        }
        let service = PaymentOrchestrationService::new(self.db.clone())
            .with_provider_registry(self.payment_provider_registry.clone());
        for action in actions {
            match action {
                PaymentCancellationAction::Void { collection_id } => {
                    service
                        .cancel_collection(
                            tenant_id,
                            *collection_id,
                            CancelPaymentInput {
                                reason: request.reason.clone(),
                                metadata: cancellation_metadata(order_id),
                            },
                        )
                        .await
                        .map_err(|error| {
                            unavailable("order_cancellation.payment_void_failed", error)
                        })?;
                }
                PaymentCancellationAction::Refund {
                    collection_id,
                    amount,
                } => {
                    service
                        .create_refund_idempotent(
                            tenant_id,
                            *collection_id,
                            refund_creation_key(*collection_id),
                            CreateRefundInput {
                                amount: *amount,
                                reason: request
                                    .reason
                                    .clone()
                                    .or_else(|| Some("order_cancelled".to_string())),
                                metadata: cancellation_metadata(order_id),
                            },
                        )
                        .await
                        .map_err(|error| {
                            unavailable("order_cancellation.payment_refund_failed", error)
                        })?;
                }
            }
        }
        Ok(())
    }
}

fn cancellation_metadata(order_id: Uuid) -> Value {
    json!({
        (ORDER_CANCELLATION_METADATA_KEY): {
            "order_id": order_id,
        }
    })
}

pub(super) fn parse_uuid(value: &str, field: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(value.trim()).map_err(|_| {
        PortError::validation(
            format!("order_cancellation.{field}_invalid"),
            format!("{field} must be a UUID"),
        )
    })
}

pub(super) fn unavailable(code: &'static str, error: impl std::fmt::Display) -> PortError {
    PortError::unavailable(code, error.to_string())
}

pub(super) fn map_order_error(error: OrderError) -> PortError {
    match error {
        OrderError::OrderNotFound(order_id) => PortError::not_found(
            "order.order_not_found",
            format!("order {order_id} was not found"),
        ),
        OrderError::Validation(message) => PortError::validation("order.validation", message),
        OrderError::InvalidTransition { from, to } => PortError::conflict(
            "order.invalid_transition",
            format!("invalid order status transition: {from} -> {to}"),
        ),
        OrderError::IdempotencyConflict => PortError::conflict(
            "order.idempotency_conflict",
            "order command idempotency key conflicts with another request",
        ),
        other => unavailable("order.storage_unavailable", other),
    }
}

/// Order admin command port that routes `cancel` through [`OrderCancellationOrchestrator`] and
/// delegates every other command to the order owner unchanged.
pub struct OrchestratedOrderAdminCommandPort {
    inner: Arc<dyn OrderAdminCommandPort>,
    orchestrator: OrderCancellationOrchestrator,
    shipment_projector: OrderShipmentProjector,
}

impl OrchestratedOrderAdminCommandPort {
    pub fn new(
        inner: Arc<dyn OrderAdminCommandPort>,
        orchestrator: OrderCancellationOrchestrator,
    ) -> Self {
        let shipment_projector =
            OrderShipmentProjector::new(orchestrator.db.clone(), orchestrator.event_bus.clone());
        Self {
            inner,
            orchestrator,
            shipment_projector,
        }
    }
}

#[async_trait]
impl OrderAdminCommandPort for OrchestratedOrderAdminCommandPort {
    async fn mark_paid(
        &self,
        context: PortContext,
        request: MarkOrderPaidRequest,
    ) -> Result<OrderResponse, PortError> {
        self.inner.mark_paid(context, request).await
    }

    async fn ship(
        &self,
        context: PortContext,
        request: ShipOrderRequest,
    ) -> Result<OrderResponse, PortError> {
        self.shipment_projector
            .ensure_manual_command_allowed(&context, request.order_id, ManualShipmentCommand::Ship)
            .await?;
        self.inner.ship(context, request).await
    }

    async fn deliver(
        &self,
        context: PortContext,
        request: DeliverOrderRequest,
    ) -> Result<OrderResponse, PortError> {
        self.shipment_projector
            .ensure_manual_command_allowed(
                &context,
                request.order_id,
                ManualShipmentCommand::Deliver,
            )
            .await?;
        self.inner.deliver(context, request).await
    }

    async fn cancel(
        &self,
        context: PortContext,
        request: CancelOrderRequest,
    ) -> Result<OrderResponse, PortError> {
        self.orchestrator.cancel(context, request).await
    }
}

/// Wraps an order admin command runtime so that `cancel` unwinds fulfillments and payments.
pub fn orchestrated_order_admin_command_runtime(
    inner: OrderAdminCommandRuntime,
    orchestrator: OrderCancellationOrchestrator,
) -> OrderAdminCommandRuntime {
    OrderAdminCommandRuntime::new(Arc::new(OrchestratedOrderAdminCommandPort::new(
        inner.command_port(),
        orchestrator,
    )))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use rustok_fulfillment::FulfillmentItemResponse;
    use rustok_payment::dto::RefundResponse;

    use super::*;

    fn fulfillment(status: &str, shipped_quantity: i32) -> FulfillmentResponse {
        let now = Utc::now();
        let id = Uuid::new_v4();
        FulfillmentResponse {
            id,
            tenant_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            shipping_option_id: None,
            customer_id: None,
            status: status.to_string(),
            carrier: None,
            tracking_number: None,
            delivered_note: None,
            cancellation_reason: None,
            items: vec![FulfillmentItemResponse {
                id: Uuid::new_v4(),
                fulfillment_id: id,
                order_line_item_id: Uuid::new_v4(),
                quantity: 2,
                shipped_quantity,
                delivered_quantity: 0,
                metadata: json!({}),
                created_at: now,
                updated_at: now,
            }],
            metadata: json!({}),
            created_at: now,
            updated_at: now,
            shipped_at: None,
            delivered_at: None,
            cancelled_at: None,
        }
    }

    fn refund(status: &str, amount: Decimal, metadata: Value) -> RefundResponse {
        let now = Utc::now();
        RefundResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            payment_collection_id: Uuid::new_v4(),
            status: status.to_string(),
            currency_code: "USD".to_string(),
            amount,
            reason: None,
            metadata,
            created_at: now,
            updated_at: now,
            refunded_at: None,
            cancelled_at: None,
        }
    }

    fn collection(
        status: &str,
        captured_amount: Decimal,
        refunds: Vec<RefundResponse>,
    ) -> PaymentCollectionResponse {
        let now = Utc::now();
        PaymentCollectionResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            cart_id: None,
            order_id: Some(Uuid::new_v4()),
            customer_id: None,
            status: status.to_string(),
            currency_code: "USD".to_string(),
            amount: dec!(100),
            authorized_amount: dec!(100),
            captured_amount,
            refunded_amount: Decimal::ZERO,
            provider_id: None,
            cancellation_reason: None,
            metadata: json!({}),
            created_at: now,
            updated_at: now,
            authorized_at: None,
            captured_at: None,
            cancelled_at: None,
            payments: Vec::new(),
            refunds,
        }
    }

    #[test]
    fn pending_fulfillments_are_cancelled_and_cancelled_ones_are_skipped() {
        let pending = fulfillment("pending", 0);
        let cancelled = fulfillment("cancelled", 0);
        let plan = plan_fulfillment_cancellation(&[pending.clone(), cancelled])
            .expect("plan must succeed");
        assert_eq!(plan, vec![pending.id]);
    }

    #[test]
    fn shipped_items_block_the_cancellation() {
        let shipped = fulfillment("shipped", 2);
        assert_eq!(
            plan_fulfillment_cancellation(&[shipped.clone()]),
            Err(CancellationBlock::ShippedItems {
                fulfillment_id: shipped.id
            })
        );
        let partially = fulfillment("pending", 1);
        assert_eq!(
            plan_fulfillment_cancellation(&[partially.clone()]),
            Err(CancellationBlock::ShippedItems {
                fulfillment_id: partially.id
            })
        );
        let unknown = fulfillment("mystery", 0);
        assert!(plan_fulfillment_cancellation(&[unknown]).is_err());
    }

    #[test]
    fn unpaid_collections_are_voided_and_cancelled_ones_skipped() {
        let pending = collection("pending", Decimal::ZERO, Vec::new());
        let authorized = collection("authorized", Decimal::ZERO, Vec::new());
        let cancelled = collection("cancelled", Decimal::ZERO, Vec::new());
        let plan = plan_payment_cancellation(&[pending.clone(), authorized.clone(), cancelled])
            .expect("plan must succeed");
        assert_eq!(
            plan,
            vec![
                PaymentCancellationAction::Void {
                    collection_id: pending.id
                },
                PaymentCancellationAction::Void {
                    collection_id: authorized.id
                },
            ]
        );
    }

    #[test]
    fn captured_collection_is_refunded_for_the_unclaimed_amount() {
        let captured = collection(
            "captured",
            dec!(100),
            vec![
                refund("refunded", dec!(30), json!({})),
                refund("cancelled", dec!(50), json!({})),
            ],
        );
        let plan = plan_payment_cancellation(&[captured.clone()]).expect("plan must succeed");
        assert_eq!(
            plan,
            vec![PaymentCancellationAction::Refund {
                collection_id: captured.id,
                amount: dec!(70)
            }]
        );
    }

    #[test]
    fn retry_resubmits_the_refund_that_the_cancellation_already_reserved() {
        let marker = cancellation_metadata(Uuid::new_v4());
        let captured = collection(
            "captured",
            dec!(100),
            vec![refund("pending", dec!(100), marker)],
        );
        let plan = plan_payment_cancellation(&[captured.clone()]).expect("plan must succeed");
        assert_eq!(
            plan,
            vec![PaymentCancellationAction::Refund {
                collection_id: captured.id,
                amount: dec!(100)
            }]
        );
    }

    #[test]
    fn fully_refunded_collection_needs_no_action() {
        let captured = collection(
            "captured",
            dec!(100),
            vec![refund("refunded", dec!(100), json!({}))],
        );
        assert!(
            plan_payment_cancellation(&[captured])
                .expect("plan must succeed")
                .is_empty()
        );
    }

    #[test]
    fn unknown_collection_status_blocks_the_cancellation() {
        let unknown = collection("mystery", Decimal::ZERO, Vec::new());
        assert!(plan_payment_cancellation(&[unknown]).is_err());
    }
}
