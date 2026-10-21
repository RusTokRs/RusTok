//! Server-side valuation of a return refund.
//!
//! A return refund must never be driven by caller-supplied numbers: the amount is derived from the
//! order's own line snapshot (price, line discounts, an allocated share of order-level discounts and
//! the line's tax) and the quantity being returned. A caller may ask for *less* than that value
//! (goodwill partial refund) but never for more
//! (`DECISIONS/2026-10-21-return-refund-server-side-valuation.md`).
//!
//! Shipping charges are not part of a return's value; refunding them is a separate operator
//! decision made through the plain refund path.

use std::collections::HashMap;

use rust_decimal::Decimal;
use rustok_fulfillment::FulfillmentResponse;
use rustok_order::dto::{OrderLineFulfillmentRequirement, OrderResponse, OrderReturnResponse};

use super::post_order::{PostOrderOrchestrationError, PostOrderOrchestrationResult};

fn validation(message: impl Into<String>) -> PostOrderOrchestrationError {
    PostOrderOrchestrationError::Validation(message.into())
}

fn overflow() -> PostOrderOrchestrationError {
    validation("return refund valuation overflowed Decimal")
}

/// Value of the goods in `order_return`, or `None` when the return carries no line items (the
/// value cannot be derived, so only an explicit operator amount is possible).
pub(crate) fn return_refundable_value(
    order: &OrderResponse,
    order_return: &OrderReturnResponse,
) -> PostOrderOrchestrationResult<Option<Decimal>> {
    if order_return.items.is_empty() {
        return Ok(None);
    }

    let subtotal = order
        .line_items
        .iter()
        .try_fold(Decimal::ZERO, |total, line| total.checked_add(line.total_price))
        .ok_or_else(overflow)?;
    let order_level_discount = order
        .adjustments
        .iter()
        .filter(|adjustment| adjustment.line_item_id.is_none())
        .try_fold(Decimal::ZERO, |total, adjustment| {
            total.checked_add(adjustment.amount)
        })
        .ok_or_else(overflow)?;

    let mut returned_quantity_by_line: HashMap<uuid::Uuid, i32> = HashMap::new();
    for item in &order_return.items {
        if item.quantity <= 0 {
            return Err(validation("return item quantity must be positive"));
        }
        let entry = returned_quantity_by_line.entry(item.line_item_id).or_insert(0);
        *entry = entry
            .checked_add(item.quantity)
            .ok_or_else(|| validation("return item quantity overflowed"))?;
    }

    let mut total = Decimal::ZERO;
    for (line_item_id, returned_quantity) in returned_quantity_by_line {
        let line = order
            .line_items
            .iter()
            .find(|line| line.id == line_item_id)
            .ok_or_else(|| {
                validation(format!(
                    "return item references line item {line_item_id} that is not part of order {}",
                    order.id
                ))
            })?;
        if line.quantity <= 0 || returned_quantity > line.quantity {
            return Err(validation(format!(
                "returned quantity {returned_quantity} exceeds ordered quantity {} for line item {line_item_id}",
                line.quantity
            )));
        }

        let line_discount = order
            .adjustments
            .iter()
            .filter(|adjustment| adjustment.line_item_id == Some(line.id))
            .try_fold(Decimal::ZERO, |sum, adjustment| {
                sum.checked_add(adjustment.amount)
            })
            .ok_or_else(overflow)?;
        let order_discount_share = if subtotal > Decimal::ZERO {
            order_level_discount
                .checked_mul(line.total_price)
                .and_then(|scaled| scaled.checked_div(subtotal))
                .ok_or_else(overflow)?
        } else {
            Decimal::ZERO
        };
        let net = (line.total_price - line_discount - order_discount_share).max(Decimal::ZERO);

        // Tax lines are computed on the discounted price. When prices already include tax the
        // price is the gross amount the customer paid and must not be topped up again.
        let tax = if order.tax_included {
            Decimal::ZERO
        } else {
            order
                .tax_lines
                .iter()
                .filter(|tax_line| tax_line.line_item_id == Some(line.id))
                .try_fold(Decimal::ZERO, |sum, tax_line| sum.checked_add(tax_line.amount))
                .ok_or_else(overflow)?
        };

        let line_paid = net.checked_add(tax).ok_or_else(overflow)?;
        let line_refund = line_paid
            .checked_mul(Decimal::from(returned_quantity))
            .and_then(|scaled| scaled.checked_div(Decimal::from(line.quantity)))
            .ok_or_else(overflow)?;
        total = total.checked_add(line_refund).ok_or_else(overflow)?;
    }

    rustok_core::money::round_to_currency(total, order.currency_code.as_str())
        .map(Some)
        .map_err(|error| validation(format!("return refund valuation failed: {error}")))
}

/// Picks the refund amount: the returned goods' value by default, an explicit amount only when it
/// does not exceed that value. Returns that cannot be valued accept an explicit positive amount
/// (payment-level caps still apply downstream).
pub(crate) fn resolve_return_refund_amount(
    requested: Option<Decimal>,
    refundable: Option<Decimal>,
) -> PostOrderOrchestrationResult<Decimal> {
    let amount = match (requested, refundable) {
        (Some(requested), Some(refundable)) if requested > refundable => {
            return Err(validation(format!(
                "refund amount {requested} exceeds the value {refundable} of the returned items"
            )));
        }
        (Some(requested), _) => requested,
        (None, Some(refundable)) => refundable,
        (None, None) => Decimal::ZERO,
    };
    if amount <= Decimal::ZERO {
        return Err(validation(
            "refund decision requires a positive amount or priced return items",
        ));
    }
    Ok(amount)
}

/// Rejects returns of goods that were never shipped.
///
/// Only enforced when the order has fulfillment records: orders driven purely by manual status
/// changes carry no shipment truth to compare against. Digital lines need no shipment. Quantities
/// are compared per return; cumulative quantity across several returns is bounded by the order
/// owner against the ordered quantity.
pub(crate) fn validate_return_quantities_were_shipped(
    order: &OrderResponse,
    order_return: &OrderReturnResponse,
    fulfillments: &[FulfillmentResponse],
) -> PostOrderOrchestrationResult<()> {
    if fulfillments.is_empty() {
        return Ok(());
    }

    let mut shipped_by_line: HashMap<uuid::Uuid, i64> = HashMap::new();
    for item in fulfillments.iter().flat_map(|fulfillment| &fulfillment.items) {
        *shipped_by_line.entry(item.order_line_item_id).or_insert(0) +=
            i64::from(item.shipped_quantity.max(0));
    }

    let mut returned_by_line: HashMap<uuid::Uuid, i64> = HashMap::new();
    for item in &order_return.items {
        *returned_by_line.entry(item.line_item_id).or_insert(0) += i64::from(item.quantity);
    }

    for (line_item_id, returned) in returned_by_line {
        let requires_shipment = order
            .line_items
            .iter()
            .find(|line| line.id == line_item_id)
            .is_some_and(|line| line.fulfillment_requirement == OrderLineFulfillmentRequirement::Physical);
        if !requires_shipment {
            continue;
        }
        let shipped = shipped_by_line.get(&line_item_id).copied().unwrap_or(0);
        if returned > shipped {
            return Err(validation(format!(
                "cannot return {returned} unit(s) of line item {line_item_id}: only {shipped} shipped"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use rustok_fulfillment::FulfillmentItemResponse;
    use rustok_order::dto::{
        OrderAdjustmentResponse, OrderLineItemResponse, OrderReturnItemResponse,
        OrderTaxLineResponse,
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    fn line(quantity: i32, unit_price: Decimal) -> OrderLineItemResponse {
        OrderLineItemResponse {
            id: Uuid::new_v4(),
            order_id: Uuid::nil(),
            product_id: None,
            variant_id: None,
            fulfillment_requirement: OrderLineFulfillmentRequirement::Physical,
            shipping_profile_slug: None,
            seller_id: None,
            sku: None,
            title: "line".to_string(),
            quantity,
            unit_price,
            total_price: unit_price * Decimal::from(quantity),
            currency_code: "USD".to_string(),
            metadata: json!({}),
            created_at: Utc::now(),
        }
    }

    fn adjustment(line_item_id: Option<Uuid>, amount: Decimal) -> OrderAdjustmentResponse {
        OrderAdjustmentResponse {
            id: Uuid::new_v4(),
            order_id: Uuid::nil(),
            line_item_id,
            source_type: "promotion".to_string(),
            source_id: None,
            amount,
            currency_code: "USD".to_string(),
            metadata: json!({}),
            created_at: Utc::now(),
        }
    }

    fn tax_line(line_item_id: Uuid, amount: Decimal) -> OrderTaxLineResponse {
        let now = Utc::now();
        OrderTaxLineResponse {
            id: Uuid::new_v4(),
            order_id: Uuid::nil(),
            line_item_id: Some(line_item_id),
            shipping_option_id: None,
            description: None,
            provider_id: "region_default".to_string(),
            rate: dec!(0.1),
            amount,
            currency_code: "USD".to_string(),
            metadata: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    fn order(
        lines: Vec<OrderLineItemResponse>,
        adjustments: Vec<OrderAdjustmentResponse>,
        tax_lines: Vec<OrderTaxLineResponse>,
        tax_included: bool,
    ) -> OrderResponse {
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
            tax_included,
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
            adjustments,
            tax_lines,
        }
    }

    fn order_return(items: &[(Uuid, i32)]) -> OrderReturnResponse {
        let now = Utc::now();
        OrderReturnResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            order_id: Uuid::nil(),
            reason: None,
            note: None,
            status: "requested".to_string(),
            resolution_type: None,
            refund_id: None,
            order_change_id: None,
            metadata: json!({}),
            items: items
                .iter()
                .map(|(line_item_id, quantity)| OrderReturnItemResponse {
                    id: Uuid::new_v4(),
                    tenant_id: Uuid::new_v4(),
                    return_id: Uuid::nil(),
                    order_id: Uuid::nil(),
                    line_item_id: *line_item_id,
                    quantity: *quantity,
                    reason: None,
                    note: None,
                    metadata: json!({"refund_amount": "999999"}),
                    created_at: now,
                    updated_at: now,
                })
                .collect(),
            created_at: now,
            updated_at: now,
            completed_at: None,
            cancelled_at: None,
        }
    }

    fn fulfillment(order_line_item_id: Uuid, shipped_quantity: i32) -> FulfillmentResponse {
        let now = Utc::now();
        let id = Uuid::new_v4();
        FulfillmentResponse {
            id,
            tenant_id: Uuid::new_v4(),
            order_id: Uuid::nil(),
            shipping_option_id: None,
            customer_id: None,
            status: "shipped".to_string(),
            carrier: None,
            tracking_number: None,
            delivered_note: None,
            cancellation_reason: None,
            items: vec![FulfillmentItemResponse {
                id: Uuid::new_v4(),
                fulfillment_id: id,
                order_line_item_id,
                quantity: 5,
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

    #[test]
    fn value_is_price_minus_discounts_plus_tax_pro_rated_by_quantity() {
        let a = line(2, dec!(50)); // 100.00
        let b = line(1, dec!(100)); // 100.00
        let order = order(
            vec![a.clone(), b.clone()],
            vec![
                adjustment(Some(a.id), dec!(10)), // line discount
                adjustment(None, dec!(20)),       // order-level: 10 per 100 of subtotal
            ],
            vec![tax_line(a.id, dec!(8))],
            false,
        );
        // Line a: 100 - 10 - 10 (share) = 80, tax 8 -> 88 for 2 units -> 44 per unit.
        let value = return_refundable_value(&order, &order_return(&[(a.id, 1)]))
            .expect("valuation")
            .expect("priced");
        assert_eq!(value, dec!(44.00));
    }

    #[test]
    fn tax_is_not_added_when_prices_include_tax() {
        let a = line(1, dec!(100));
        let order = order(vec![a.clone()], vec![], vec![tax_line(a.id, dec!(9.09))], true);
        let value = return_refundable_value(&order, &order_return(&[(a.id, 1)]))
            .expect("valuation")
            .expect("priced");
        assert_eq!(value, dec!(100.00));
    }

    #[test]
    fn item_metadata_refund_amount_is_ignored() {
        let a = line(1, dec!(10));
        let order = order(vec![a.clone()], vec![], vec![], false);
        let value = return_refundable_value(&order, &order_return(&[(a.id, 1)]))
            .expect("valuation")
            .expect("priced");
        assert_eq!(value, dec!(10.00));
    }

    #[test]
    fn return_without_items_cannot_be_valued() {
        let order = order(vec![line(1, dec!(10))], vec![], vec![], false);
        assert_eq!(
            return_refundable_value(&order, &order_return(&[])).expect("valuation"),
            None
        );
    }

    #[test]
    fn unknown_line_and_excess_quantity_are_rejected() {
        let a = line(1, dec!(10));
        let order = order(vec![a.clone()], vec![], vec![], false);
        assert!(return_refundable_value(&order, &order_return(&[(Uuid::new_v4(), 1)])).is_err());
        assert!(return_refundable_value(&order, &order_return(&[(a.id, 2)])).is_err());
    }

    #[test]
    fn explicit_amount_may_be_lower_but_not_higher_than_the_value() {
        assert_eq!(
            resolve_return_refund_amount(None, Some(dec!(44))).expect("default"),
            dec!(44)
        );
        assert_eq!(
            resolve_return_refund_amount(Some(dec!(20)), Some(dec!(44))).expect("partial"),
            dec!(20)
        );
        assert!(resolve_return_refund_amount(Some(dec!(45)), Some(dec!(44))).is_err());
        assert_eq!(
            resolve_return_refund_amount(Some(dec!(5)), None).expect("unvalued"),
            dec!(5)
        );
        assert!(resolve_return_refund_amount(None, None).is_err());
        assert!(resolve_return_refund_amount(Some(Decimal::ZERO), Some(dec!(44))).is_err());
    }

    #[test]
    fn returns_are_capped_by_shipped_quantity() {
        let a = line(3, dec!(10));
        let order = order(vec![a.clone()], vec![], vec![], false);
        let shipped = [fulfillment(a.id, 1)];
        assert!(
            validate_return_quantities_were_shipped(&order, &order_return(&[(a.id, 1)]), &shipped)
                .is_ok()
        );
        assert!(
            validate_return_quantities_were_shipped(&order, &order_return(&[(a.id, 2)]), &shipped)
                .is_err()
        );
    }

    #[test]
    fn shipment_cap_is_skipped_without_fulfillments_and_for_digital_lines() {
        let mut digital = line(1, dec!(10));
        digital.fulfillment_requirement = OrderLineFulfillmentRequirement::Digital;
        let order = order(vec![digital.clone()], vec![], vec![], false);
        let ret = order_return(&[(digital.id, 1)]);
        assert!(validate_return_quantities_were_shipped(&order, &ret, &[]).is_ok());
        assert!(
            validate_return_quantities_were_shipped(
                &order,
                &ret,
                &[fulfillment(Uuid::new_v4(), 1)]
            )
            .is_ok()
        );
    }
}
