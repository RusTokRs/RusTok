//! Order shipment status as a projection of fulfillment truth.
//!
//! Shipment truth lives in the fulfillment owner (`shipped_quantity` / `delivered_quantity` per
//! item). The order's `shipped` / `delivered` statuses used to be independent manual admin commands,
//! so the two could disagree (`DECISIONS/2026-10-21-order-shipment-status-projection.md`).
//!
//! Rules implemented here:
//!
//! * after a fulfillment `ship` / `deliver` command succeeds, the order is advanced to the status
//!   the fulfillments justify ([`OrderShipmentProjector::project`]); a projection failure never
//!   fails the already committed fulfillment command and is repaired by the next projection or by
//!   the manual command below;
//! * the manual order `ship` / `deliver` commands stay available, but when the order has live
//!   fulfillment records they are refused while those fulfillments do not justify the status
//!   ([`OrderShipmentProjector::ensure_manual_command_allowed`]). Orders without fulfillment
//!   records keep the legacy manual behaviour.
//!
//! Status never moves backwards: a reopened or reshipped fulfillment does not revert an order that
//! is already `shipped`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_fulfillment::{
    CancelAdminFulfillmentRequest, DeliverAdminFulfillmentRequest, FulfillmentAdminCommandPort,
    FulfillmentAdminCommandRuntime, FulfillmentResponse, FulfillmentService, FulfillmentStatusKind,
    ReopenAdminFulfillmentRequest, ReshipAdminFulfillmentRequest, ShipAdminFulfillmentRequest,
};
use rustok_order::dto::OrderLineFulfillmentRequirement;
use rustok_order::{OrderResponse, OrderService};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::order_cancellation::{map_order_error, parse_uuid, unavailable};

const SHIPMENT_PROJECTION_BOUNDARY: &str = "commerce_order_shipment_projection";
const FALLBACK_CARRIER: &str = "unspecified";
const FALLBACK_TRACKING_NUMBER: &str = "unspecified";

/// What the order's live fulfillments say about shipment. Ordered by progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShipmentTruth {
    /// No live (non-cancelled) fulfillment exists: there is no shipment truth to compare with.
    NoFulfillments,
    /// Some physical quantity is not shipped yet.
    Open,
    /// Every physical line is fully shipped.
    Shipped,
    /// Every physical line is fully delivered.
    Delivered,
}

/// Derives [`ShipmentTruth`] from the order lines and its fulfillments.
pub fn shipment_truth(
    order: &OrderResponse,
    fulfillments: &[FulfillmentResponse],
) -> ShipmentTruth {
    let live: Vec<&FulfillmentResponse> = fulfillments
        .iter()
        .filter(|fulfillment| fulfillment.status_kind() != FulfillmentStatusKind::Cancelled)
        .collect();
    if live.is_empty() {
        return ShipmentTruth::NoFulfillments;
    }
    if live
        .iter()
        .any(|fulfillment| fulfillment.status_kind() == FulfillmentStatusKind::Unknown)
    {
        return ShipmentTruth::Open;
    }

    let status_truth = if live
        .iter()
        .all(|fulfillment| fulfillment.status_kind() == FulfillmentStatusKind::Delivered)
    {
        ShipmentTruth::Delivered
    } else if live.iter().all(|fulfillment| {
        matches!(
            fulfillment.status_kind(),
            FulfillmentStatusKind::Shipped | FulfillmentStatusKind::Delivered
        )
    }) {
        ShipmentTruth::Shipped
    } else {
        return ShipmentTruth::Open;
    };

    // A fulfillment without item rows covers the whole order; only its status is known.
    if live.iter().any(|fulfillment| fulfillment.items.is_empty()) {
        return status_truth;
    }

    let mut shipped_by_line: HashMap<Uuid, i64> = HashMap::new();
    let mut delivered_by_line: HashMap<Uuid, i64> = HashMap::new();
    for item in live.iter().flat_map(|fulfillment| &fulfillment.items) {
        *shipped_by_line.entry(item.order_line_item_id).or_insert(0) +=
            i64::from(item.shipped_quantity.max(0));
        *delivered_by_line.entry(item.order_line_item_id).or_insert(0) +=
            i64::from(item.delivered_quantity.max(0));
    }

    let mut quantity_truth = ShipmentTruth::Delivered;
    for line in order.line_items.iter().filter(|line| {
        line.fulfillment_requirement == OrderLineFulfillmentRequirement::Physical
    }) {
        let ordered = i64::from(line.quantity);
        if shipped_by_line.get(&line.id).copied().unwrap_or(0) < ordered {
            return ShipmentTruth::Open;
        }
        if delivered_by_line.get(&line.id).copied().unwrap_or(0) < ordered {
            quantity_truth = ShipmentTruth::Shipped;
        }
    }
    quantity_truth.min(status_truth)
}

/// Carrier and tracking number of the most recently shipped live fulfillment, so the order does
/// not carry values the fulfillment owner never saw.
fn shipment_carrier_and_tracking(fulfillments: &[FulfillmentResponse]) -> (String, String) {
    fulfillments
        .iter()
        .filter(|fulfillment| {
            matches!(
                fulfillment.status_kind(),
                FulfillmentStatusKind::Shipped | FulfillmentStatusKind::Delivered
            )
        })
        .max_by_key(|fulfillment| fulfillment.shipped_at)
        .map(|fulfillment| {
            (
                non_blank(fulfillment.carrier.as_deref()).unwrap_or(FALLBACK_CARRIER),
                non_blank(fulfillment.tracking_number.as_deref())
                    .unwrap_or(FALLBACK_TRACKING_NUMBER),
            )
        })
        .map(|(carrier, tracking)| (carrier.to_string(), tracking.to_string()))
        .unwrap_or_else(|| {
            (
                FALLBACK_CARRIER.to_string(),
                FALLBACK_TRACKING_NUMBER.to_string(),
            )
        })
}

fn non_blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// Which manual order command is being guarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualShipmentCommand {
    Ship,
    Deliver,
}

/// Pure guard: may the manual command run given the shipment truth?
pub fn manual_command_verdict(
    command: ManualShipmentCommand,
    truth: ShipmentTruth,
) -> Result<(), PortError> {
    match (command, truth) {
        (_, ShipmentTruth::NoFulfillments) => Ok(()),
        (ManualShipmentCommand::Ship, ShipmentTruth::Open) => Err(PortError::conflict(
            "order_shipment.fulfillment_not_shipped",
            "the order has fulfillments that are not fully shipped; ship the fulfillments instead",
        )),
        (ManualShipmentCommand::Ship, _) => Ok(()),
        (ManualShipmentCommand::Deliver, ShipmentTruth::Delivered) => Ok(()),
        (ManualShipmentCommand::Deliver, _) => Err(PortError::conflict(
            "order_shipment.fulfillment_not_delivered",
            "the order has fulfillments that are not fully delivered; deliver the fulfillments instead",
        )),
    }
}

/// Reads fulfillment truth and advances or guards the order accordingly.
#[derive(Clone)]
pub struct OrderShipmentProjector {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl OrderShipmentProjector {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    async fn load(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> Result<(OrderResponse, Vec<FulfillmentResponse>), PortError> {
        let order = OrderService::new(self.db.clone(), self.event_bus.clone())
            .get_order(tenant_id, order_id)
            .await
            .map_err(map_order_error)?;
        let fulfillments = FulfillmentService::new(self.db.clone())
            .list_by_order(tenant_id, order_id)
            .await
            .map_err(|error| unavailable("order_shipment.fulfillment_read_failed", error))?;
        Ok((order, fulfillments))
    }

    /// Refuses a manual `ship` / `deliver` that contradicts live fulfillments.
    pub async fn ensure_manual_command_allowed(
        &self,
        context: &PortContext,
        order_id: Uuid,
        command: ManualShipmentCommand,
    ) -> Result<(), PortError> {
        let tenant_id = parse_uuid(context.tenant_id.as_str(), "tenant_id")?;
        let (order, fulfillments) = self.load(tenant_id, order_id).await?;
        manual_command_verdict(command, shipment_truth(&order, &fulfillments))
    }

    /// Advances the order to the status its fulfillments justify. Idempotent; returns the truth it
    /// acted on.
    pub async fn project(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
    ) -> Result<ShipmentTruth, PortError> {
        let (order, fulfillments) = self.load(tenant_id, order_id).await?;
        let truth = shipment_truth(&order, &fulfillments);
        if truth < ShipmentTruth::Shipped {
            return Ok(truth);
        }

        let order_service = OrderService::new(self.db.clone(), self.event_bus.clone());
        let mut status = order.status.clone();
        if status == "paid" {
            let (carrier, tracking_number) = shipment_carrier_and_tracking(&fulfillments);
            order_service
                .ship_order(tenant_id, actor_id, order_id, tracking_number, carrier)
                .await
                .map_err(map_order_error)?;
            status = "shipped".to_string();
        }
        if truth == ShipmentTruth::Delivered && status == "shipped" {
            order_service
                .deliver_order(tenant_id, actor_id, order_id, None)
                .await
                .map_err(map_order_error)?;
        }
        tracing::debug!(
            boundary = SHIPMENT_PROJECTION_BOUNDARY,
            %tenant_id,
            %order_id,
            truth = ?truth,
            order_status = %order.status,
            "order shipment projected"
        );
        Ok(truth)
    }

    /// Projection after a committed fulfillment command. The fulfillment already changed, so a
    /// projection failure is logged and left for the next projection or a manual command.
    async fn project_after_fulfillment_command(
        &self,
        context: &PortContext,
        fulfillment: &FulfillmentResponse,
    ) {
        let ids = parse_uuid(context.tenant_id.as_str(), "tenant_id").and_then(|tenant_id| {
            parse_uuid(context.actor.id.as_str(), "actor_id").map(|actor_id| (tenant_id, actor_id))
        });
        let outcome = match ids {
            Ok((tenant_id, actor_id)) => {
                self.project(tenant_id, actor_id, fulfillment.order_id)
                    .await
            }
            Err(error) => Err(error),
        };
        if let Err(error) = outcome {
            tracing::warn!(
                boundary = SHIPMENT_PROJECTION_BOUNDARY,
                order_id = %fulfillment.order_id,
                fulfillment_id = %fulfillment.id,
                error = %error,
                "order shipment projection failed after a committed fulfillment command"
            );
        }
    }
}

/// Fulfillment admin command port that projects `ship` / `deliver` results onto the order and
/// delegates everything else unchanged.
pub struct ProjectingFulfillmentAdminCommandPort {
    inner: Arc<dyn FulfillmentAdminCommandPort>,
    projector: OrderShipmentProjector,
}

impl ProjectingFulfillmentAdminCommandPort {
    pub fn new(
        inner: Arc<dyn FulfillmentAdminCommandPort>,
        projector: OrderShipmentProjector,
    ) -> Self {
        Self { inner, projector }
    }
}

#[async_trait]
impl FulfillmentAdminCommandPort for ProjectingFulfillmentAdminCommandPort {
    async fn ship_fulfillment(
        &self,
        context: PortContext,
        request: ShipAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        let fulfillment = self.inner.ship_fulfillment(context.clone(), request).await?;
        self.projector
            .project_after_fulfillment_command(&context, &fulfillment)
            .await;
        Ok(fulfillment)
    }

    async fn deliver_fulfillment(
        &self,
        context: PortContext,
        request: DeliverAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        let fulfillment = self
            .inner
            .deliver_fulfillment(context.clone(), request)
            .await?;
        self.projector
            .project_after_fulfillment_command(&context, &fulfillment)
            .await;
        Ok(fulfillment)
    }

    async fn reopen_fulfillment(
        &self,
        context: PortContext,
        request: ReopenAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        self.inner.reopen_fulfillment(context, request).await
    }

    async fn reship_fulfillment(
        &self,
        context: PortContext,
        request: ReshipAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        self.inner.reship_fulfillment(context, request).await
    }

    async fn cancel_fulfillment(
        &self,
        context: PortContext,
        request: CancelAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        self.inner.cancel_fulfillment(context, request).await
    }
}

/// Wraps a fulfillment admin command runtime so shipment results are projected onto the order.
pub fn projecting_fulfillment_admin_command_runtime(
    inner: FulfillmentAdminCommandRuntime,
    projector: OrderShipmentProjector,
) -> FulfillmentAdminCommandRuntime {
    FulfillmentAdminCommandRuntime::new(Arc::new(ProjectingFulfillmentAdminCommandPort::new(
        inner.command_port(),
        projector,
    )))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;
    use rustok_fulfillment::FulfillmentItemResponse;
    use rustok_order::dto::OrderLineItemResponse;
    use serde_json::json;

    use super::*;

    fn line(quantity: i32, requirement: OrderLineFulfillmentRequirement) -> OrderLineItemResponse {
        OrderLineItemResponse {
            id: Uuid::new_v4(),
            order_id: Uuid::nil(),
            product_id: None,
            variant_id: None,
            fulfillment_requirement: requirement,
            shipping_profile_slug: None,
            seller_id: None,
            sku: None,
            title: "line".to_string(),
            quantity,
            unit_price: Decimal::ONE,
            total_price: Decimal::from(quantity),
            currency_code: "USD".to_string(),
            metadata: json!({}),
            created_at: Utc::now(),
        }
    }

    fn order(lines: Vec<OrderLineItemResponse>) -> OrderResponse {
        let now = Utc::now();
        OrderResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            channel_id: None,
            channel_slug: None,
            customer_id: None,
            status: "paid".to_string(),
            currency_code: "USD".to_string(),
            subtotal_amount: Decimal::ZERO,
            adjustment_total: Decimal::ZERO,
            shipping_total: Decimal::ZERO,
            total_amount: Decimal::ZERO,
            tax_total: Decimal::ZERO,
            tax_included: false,
            metadata: json!({}),
            payment_id: None,
            payment_method: None,
            tracking_number: None,
            carrier: None,
            cancellation_reason: None,
            delivered_signature: None,
            created_at: now,
            updated_at: now,
            confirmed_at: None,
            paid_at: None,
            shipped_at: None,
            delivered_at: None,
            cancelled_at: None,
            line_items: lines,
            adjustments: Vec::new(),
            tax_lines: Vec::new(),
        }
    }

    fn fulfillment(
        status: &str,
        items: &[(Uuid, i32, i32, i32)], // line, quantity, shipped, delivered
    ) -> FulfillmentResponse {
        let now = Utc::now();
        let id = Uuid::new_v4();
        FulfillmentResponse {
            id,
            tenant_id: Uuid::new_v4(),
            order_id: Uuid::nil(),
            shipping_option_id: None,
            customer_id: None,
            status: status.to_string(),
            carrier: Some("dhl".to_string()),
            tracking_number: Some("trk-1".to_string()),
            delivered_note: None,
            cancellation_reason: None,
            items: items
                .iter()
                .map(
                    |(line_id, quantity, shipped, delivered)| FulfillmentItemResponse {
                        id: Uuid::new_v4(),
                        fulfillment_id: id,
                        order_line_item_id: *line_id,
                        quantity: *quantity,
                        shipped_quantity: *shipped,
                        delivered_quantity: *delivered,
                        metadata: json!({}),
                        created_at: now,
                        updated_at: now,
                    },
                )
                .collect(),
            metadata: json!({}),
            created_at: now,
            updated_at: now,
            shipped_at: None,
            delivered_at: None,
            cancelled_at: None,
        }
    }

    #[test]
    fn no_live_fulfillments_means_no_truth() {
        let physical = line(1, OrderLineFulfillmentRequirement::Physical);
        let order = order(vec![physical.clone()]);
        assert_eq!(shipment_truth(&order, &[]), ShipmentTruth::NoFulfillments);
        assert_eq!(
            shipment_truth(&order, &[fulfillment("cancelled", &[(physical.id, 1, 0, 0)])]),
            ShipmentTruth::NoFulfillments
        );
    }

    #[test]
    fn partial_shipment_is_open_and_full_shipment_is_shipped() {
        let physical = line(3, OrderLineFulfillmentRequirement::Physical);
        let order = order(vec![physical.clone()]);
        assert_eq!(
            shipment_truth(&order, &[fulfillment("shipped", &[(physical.id, 3, 1, 0)])]),
            ShipmentTruth::Open
        );
        assert_eq!(
            shipment_truth(
                &order,
                &[
                    fulfillment("shipped", &[(physical.id, 1, 1, 0)]),
                    fulfillment("shipped", &[(physical.id, 2, 2, 0)]),
                ]
            ),
            ShipmentTruth::Shipped
        );
    }

    #[test]
    fn pending_sibling_fulfillment_keeps_the_order_open() {
        let physical = line(2, OrderLineFulfillmentRequirement::Physical);
        let order = order(vec![physical.clone()]);
        assert_eq!(
            shipment_truth(
                &order,
                &[
                    fulfillment("shipped", &[(physical.id, 1, 1, 0)]),
                    fulfillment("pending", &[(physical.id, 1, 0, 0)]),
                ]
            ),
            ShipmentTruth::Open
        );
    }

    #[test]
    fn delivered_requires_every_unit_delivered() {
        let physical = line(2, OrderLineFulfillmentRequirement::Physical);
        let order = order(vec![physical.clone()]);
        assert_eq!(
            shipment_truth(&order, &[fulfillment("delivered", &[(physical.id, 2, 2, 2)])]),
            ShipmentTruth::Delivered
        );
        assert_eq!(
            shipment_truth(&order, &[fulfillment("shipped", &[(physical.id, 2, 2, 1)])]),
            ShipmentTruth::Shipped
        );
    }

    #[test]
    fn digital_lines_do_not_need_shipment_and_itemless_fulfillments_use_status() {
        let physical = line(1, OrderLineFulfillmentRequirement::Physical);
        let digital = line(5, OrderLineFulfillmentRequirement::Digital);
        let order = order(vec![physical.clone(), digital]);
        assert_eq!(
            shipment_truth(&order, &[fulfillment("shipped", &[(physical.id, 1, 1, 0)])]),
            ShipmentTruth::Shipped
        );
        assert_eq!(
            shipment_truth(&order, &[fulfillment("delivered", &[])]),
            ShipmentTruth::Delivered
        );
        assert_eq!(
            shipment_truth(&order, &[fulfillment("pending", &[])]),
            ShipmentTruth::Open
        );
    }

    #[test]
    fn manual_commands_are_refused_only_against_contradicting_fulfillments() {
        use ManualShipmentCommand::{Deliver, Ship};
        assert!(manual_command_verdict(Ship, ShipmentTruth::NoFulfillments).is_ok());
        assert!(manual_command_verdict(Deliver, ShipmentTruth::NoFulfillments).is_ok());
        assert!(manual_command_verdict(Ship, ShipmentTruth::Open).is_err());
        assert!(manual_command_verdict(Ship, ShipmentTruth::Shipped).is_ok());
        assert!(manual_command_verdict(Ship, ShipmentTruth::Delivered).is_ok());
        assert!(manual_command_verdict(Deliver, ShipmentTruth::Open).is_err());
        assert!(manual_command_verdict(Deliver, ShipmentTruth::Shipped).is_err());
        assert!(manual_command_verdict(Deliver, ShipmentTruth::Delivered).is_ok());
    }

    #[test]
    fn carrier_and_tracking_come_from_the_shipped_fulfillment() {
        let physical = line(1, OrderLineFulfillmentRequirement::Physical);
        let shipped = fulfillment("shipped", &[(physical.id, 1, 1, 0)]);
        assert_eq!(
            shipment_carrier_and_tracking(&[shipped]),
            ("dhl".to_string(), "trk-1".to_string())
        );
        assert_eq!(
            shipment_carrier_and_tracking(&[]),
            (
                FALLBACK_CARRIER.to_string(),
                FALLBACK_TRACKING_NUMBER.to_string()
            )
        );
    }
}
