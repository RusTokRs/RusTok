//! Cart-owned adoption of previously observed line prices. Product/Pricing/Inventory IO
//! precedes this boundary; existing tax/shipping total recalculation remains transactional.

use std::collections::HashMap;

use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, Set,
    TransactionTrait,
};
use uuid::Uuid;

use super::helpers::*;
use super::{CartLineItemPricingUpdate, CartService};
use crate::{
    CartError, CartPricingContextGuard, CartResponse, CartResult, MAX_PRICED_CART_LINES, entities,
};

pub(super) fn line_total(
    quantity: i32,
    unit_price: Decimal,
    currency: &str,
) -> CartResult<Decimal> {
    if quantity < 1 || unit_price < Decimal::ZERO {
        return Err(CartError::Validation(
            "A priced line requires positive quantity and non-negative money".into(),
        ));
    }
    let exponent = rustok_core::money::currency_exponent(currency)
        .map_err(|_| CartError::Validation("Cart currency is invalid".into()))?;
    rustok_core::money::to_fixed_point_units(unit_price, exponent).map_err(|_| {
        CartError::Validation("Cart unit price exceeds currency precision or money bounds".into())
    })?;
    let total = unit_price
        .checked_mul(Decimal::from(quantity))
        .ok_or_else(|| {
            CartError::Validation("Cart line total exceeds supported money bounds".into())
        })?;
    rustok_core::money::to_fixed_point_units(total, exponent).map_err(|_| {
        CartError::Validation("Cart line total exceeds currency precision or money bounds".into())
    })?;
    Ok(total)
}

impl CartService {
    pub async fn update_line_item_pricing(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        update: CartLineItemPricingUpdate,
        pricing_context: CartPricingContextGuard,
    ) -> CartResult<CartResponse> {
        let CartLineItemPricingUpdate {
            line_item_id,
            quantity,
            unit_price,
            pricing_adjustment,
        } = update;
        if line_item_id.is_nil() {
            return Err(CartError::Validation(
                "line_item_id must not be a nil UUID".into(),
            ));
        }
        let total = line_total(quantity, unit_price, &pricing_context.currency_code)?;
        let txn = self.db.begin().await?;
        let cart = load_cart_for_update_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "update_line_item_pricing")?;
        pricing_context.verify_locked(&txn, &cart).await?;
        let line = entities::cart_line_item::Entity::find_by_id(line_item_id)
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .one(&txn)
            .await?
            .ok_or(CartError::CartLineItemNotFound(line_item_id))?;
        let mut active: entities::cart_line_item::ActiveModel = line.into();
        active.unit_price = Set(unit_price);
        active.quantity = Set(quantity);
        active.total_price = Set(total);
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;
        update_line_item_marketplace_snapshot_in_tx(
            &txn,
            line_item_id,
            quantity,
            unit_price,
            pricing_adjustment
                .as_ref()
                .map(|adjustment| adjustment.amount),
        )
        .await?;
        replace_pricing_adjustments(
            &txn,
            cart.id,
            &cart.currency_code,
            vec![(line_item_id, pricing_adjustment)],
        )
        .await?;
        recalculate_totals(
            &txn,
            self.tax_calculation_port.as_ref(),
            self.shipping_option_read_port.as_ref(),
            cart,
        )
        .await?;
        reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    pub async fn reprice_line_items(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        updates: Vec<CartLineItemPricingUpdate>,
        pricing_context: CartPricingContextGuard,
    ) -> CartResult<CartResponse> {
        let txn = self.db.begin().await?;
        let cart = load_cart_for_update_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "reprice_line_items")?;
        pricing_context.verify_locked(&txn, &cart).await?;
        if updates.is_empty() {
            txn.commit().await?;
            return self.get_cart(tenant_id, cart_id).await;
        }
        apply_pricing_updates(&txn, &cart, updates, PricingCoverage::Supplied).await?;
        recalculate_totals(
            &txn,
            self.tax_calculation_port.as_ref(),
            self.shipping_option_read_port.as_ref(),
            cart,
        )
        .await?;
        reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PricingCoverage {
    Supplied,
    Complete,
}

pub(super) async fn apply_pricing_updates<C: ConnectionTrait>(
    conn: &C,
    cart: &entities::cart::Model,
    updates: Vec<CartLineItemPricingUpdate>,
    coverage: PricingCoverage,
) -> CartResult<()> {
    if updates.len() > MAX_PRICED_CART_LINES {
        return Err(CartError::Validation(
            "Cart repricing exceeds 1024 observed lines".into(),
        ));
    }
    let mut updates_map = HashMap::new();
    for update in updates {
        if update.line_item_id.is_nil() {
            return Err(CartError::Validation(
                "line_item_id must not be a nil UUID".into(),
            ));
        }
        line_total(update.quantity, update.unit_price, &cart.currency_code)?;
        if updates_map.insert(update.line_item_id, update).is_some() {
            return Err(CartError::Validation(
                "Duplicate line in Cart repricing request".into(),
            ));
        }
    }
    let lines = entities::cart_line_item::Entity::find()
        .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
        .limit((MAX_PRICED_CART_LINES + 1) as u64)
        .all(conn)
        .await?;
    if lines.len() > MAX_PRICED_CART_LINES {
        return Err(CartError::PricingContextChanged);
    }
    if coverage == PricingCoverage::Complete && updates_map.len() != lines.len() {
        return Err(CartError::PricingContextChanged);
    }
    let quantities: HashMap<_, _> = lines.iter().map(|line| (line.id, line.quantity)).collect();
    // Validate the entire requested set, including complete coverage, before any write.
    for update in updates_map.values() {
        let quantity = quantities
            .get(&update.line_item_id)
            .ok_or(CartError::CartLineItemNotFound(update.line_item_id))?;
        if *quantity != update.quantity {
            return Err(CartError::PricingContextChanged);
        }
    }
    let mut adjustments = Vec::new();
    for line in lines {
        if let Some(update) = updates_map.get(&line.id) {
            let id = line.id;
            let mut active: entities::cart_line_item::ActiveModel = line.into();
            active.unit_price = Set(update.unit_price);
            active.total_price = Set(line_total(
                update.quantity,
                update.unit_price,
                &cart.currency_code,
            )?);
            active.updated_at = Set(Utc::now().into());
            active.update(conn).await?;
            update_line_item_marketplace_snapshot_in_tx(
                conn,
                id,
                update.quantity,
                update.unit_price,
                update
                    .pricing_adjustment
                    .as_ref()
                    .map(|adjustment| adjustment.amount),
            )
            .await?;
            adjustments.push((id, update.pricing_adjustment.clone()));
        }
    }
    replace_pricing_adjustments(conn, cart.id, &cart.currency_code, adjustments).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priced_totals_refuse_invalid_quantity_negative_money_and_overflow() {
        assert!(line_total(0, Decimal::ONE, "USD").is_err());
        assert!(line_total(1, -Decimal::ONE, "USD").is_err());
        assert!(line_total(2, Decimal::MAX, "USD").is_err());
        assert_eq!(
            line_total(3, Decimal::from(7), "USD").unwrap(),
            Decimal::from(21)
        );
    }
}
