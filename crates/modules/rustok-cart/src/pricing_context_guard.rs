//! Cart-owned material-input fence for Pricing/Inventory observations made before the write lock.
//!
//! This is not a monotonic revision, authorization token, durable receipt or cross-owner hold.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rustok_api::PortError;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use crate::{
    CartError, CartLineItemResponse, CartResponse,
    entities::{cart, cart_line_item},
};

/// Bound for one optimistic priced operation, not a historical Cart storage/retention limit.
pub const MAX_PRICED_CART_LINES: usize = 1024;

/// Stored Cart scope. Country participates in context correlation and tax, not current unit-price selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartPricingScope {
    pub currency_code: String,
    pub region_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub locale_code: Option<String>,
    pub country_code: Option<String>,
}

impl CartPricingScope {
    pub fn capture(cart: &CartResponse) -> Self {
        Self {
            currency_code: cart.currency_code.clone(),
            region_id: cart.region_id,
            channel_id: cart.channel_id,
            channel_slug: cart.channel_slug.clone(),
            locale_code: cart.locale_code.clone(),
            country_code: cart.country_code.clone(),
        }
    }

    pub(crate) fn from_model(cart: &cart::Model) -> Self {
        Self {
            currency_code: cart.currency_code.clone(),
            region_id: cart.region_id,
            channel_id: cart.channel_id,
            channel_slug: cart.channel_slug.clone(),
            locale_code: cart.locale_code.clone(),
            country_code: cart.country_code.clone(),
        }
    }

    pub(crate) fn with_input(mut self, input: &crate::UpdateCartContextInput) -> Self {
        self.region_id = input.region_id;
        self.locale_code = input.locale_code.clone();
        self.country_code = input.country_code.clone();
        self
    }

    pub(crate) fn same_price_basis(&self, other: &Self) -> bool {
        self.currency_code == other.currency_code
            && self.region_id == other.region_id
            && self.channel_id == other.channel_id
            && self.channel_slug == other.channel_slug
            && self.locale_code == other.locale_code
        // Country/shipping feed the existing transactional tax/shipping calculation. Pricing's
        // unit-price request currently has no country selector; do not invent another price model.
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartPricingContextGuard {
    pub currency_code: String,
    pub region_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub locale_code: Option<String>,
    pub customer_id: Option<Uuid>,
    cart_id: Uuid,
    tenant_id: Uuid,
    country_code: Option<String>,
    cart_updated_at: DateTime<Utc>,
    line_items: Vec<PricingLineSnapshot>,
    purpose: GuardPurpose,
    /// Identity only: never a presented bearer and never authorization by itself.
    guest_access_fingerprint: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GuardPurpose {
    Pricing,
    Removal { line_id: Uuid },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PricingLineSnapshot {
    id: Uuid,
    cart_id: Uuid,
    product_id: Option<Uuid>,
    variant_id: Option<Uuid>,
    quantity: i32,
    fulfillment_requirement: String,
    shipping_profile_slug: Option<String>,
    unit_price: Decimal,
    total_price: Decimal,
    currency_code: String,
    updated_at: DateTime<Utc>,
}

impl PricingLineSnapshot {
    fn from_response(line: &CartLineItemResponse) -> Self {
        Self {
            id: line.id,
            cart_id: line.cart_id,
            product_id: line.product_id,
            variant_id: line.variant_id,
            quantity: line.quantity,
            fulfillment_requirement: line.fulfillment_requirement.as_str().to_string(),
            shipping_profile_slug: line.shipping_profile_slug.clone(),
            unit_price: line.unit_price,
            total_price: line.total_price,
            currency_code: line.currency_code.clone(),
            updated_at: line.updated_at,
        }
    }

    fn from_model(line: &cart_line_item::Model) -> Self {
        Self {
            id: line.id,
            cart_id: line.cart_id,
            product_id: line.product_id,
            variant_id: line.variant_id,
            quantity: line.quantity,
            fulfillment_requirement: line.fulfillment_requirement.clone(),
            shipping_profile_slug:
                crate::services::cart::helpers::normalize_optional_shipping_profile_slug(Some(
                    &line.shipping_profile_slug,
                )),
            unit_price: line.unit_price,
            total_price: line.total_price,
            currency_code: line.currency_code.clone(),
            updated_at: line.updated_at.with_timezone(&Utc),
        }
    }
}

impl CartPricingContextGuard {
    pub fn capture(cart: &CartResponse) -> Result<Self, PortError> {
        if cart.line_items.len() > MAX_PRICED_CART_LINES {
            return Err(PortError::validation(
                "cart.pricing_snapshot_invalid",
                "Cart pricing observation exceeds 1024 lines; inspect the cart before changing it",
            ));
        }
        let mut observation = Self::observation(
            cart,
            cart.line_items
                .iter()
                .map(PricingLineSnapshot::from_response)
                .collect(),
            GuardPurpose::Pricing,
        );
        if !observation.valid_shape() {
            return Err(PortError::validation(
                "cart.pricing_snapshot_invalid",
                "Cart pricing observation is invalid or exceeds 1024 lines; inspect the cart before changing it",
            ));
        }
        observation.line_items.sort_by_key(|line| line.id);
        Ok(observation)
    }

    pub fn capture_add(cart: &CartResponse) -> Result<Self, PortError> {
        let observation = Self::capture(cart)?;
        observation.verify_add_capacity().map_err(|_| {
            PortError::validation(
                "cart.pricing_snapshot_invalid",
                "Cart has reached the priced line limit; remove a line before adding another",
            )
        })?;
        Ok(observation)
    }

    pub(crate) fn observed_line_count(&self) -> usize {
        self.line_items.len()
    }

    pub(crate) fn verify_add_capacity(&self) -> Result<(), CartError> {
        if self.line_items.len() >= MAX_PRICED_CART_LINES {
            return Err(CartError::Validation(
                "Cart has reached the priced line limit; remove a line before adding another"
                    .into(),
            ));
        }
        Ok(())
    }

    fn observation(
        cart: &CartResponse,
        line_items: Vec<PricingLineSnapshot>,
        purpose: GuardPurpose,
    ) -> Self {
        Self {
            currency_code: cart.currency_code.clone(),
            region_id: cart.region_id,
            channel_id: cart.channel_id,
            channel_slug: cart.channel_slug.clone(),
            locale_code: cart.locale_code.clone(),
            customer_id: cart.customer_id,
            cart_id: cart.id,
            tenant_id: cart.tenant_id,
            country_code: cart.country_code.clone(),
            cart_updated_at: cart.updated_at,
            line_items,
            purpose,
            guest_access_fingerprint: guest_access_fingerprint(&cart.metadata),
        }
    }

    pub fn capture_removal(cart: &CartResponse, line_id: Uuid) -> Result<Self, PortError> {
        let mut lines = cart.line_items.iter().filter(|line| line.id == line_id);
        let line = lines.next().ok_or_else(|| {
            PortError::not_found("cart.line_item_not_found", "Cart line item was not found")
        })?;
        if lines.next().is_some() {
            return Err(PortError::validation(
                "cart.pricing_snapshot_invalid",
                "Cart line identity is inconsistent; inspect the cart",
            ));
        }
        let observation = Self::observation(
            cart,
            vec![PricingLineSnapshot::from_response(line)],
            GuardPurpose::Removal { line_id },
        );
        if !observation.valid_shape() {
            return Err(PortError::validation(
                "cart.pricing_snapshot_invalid",
                "Cart removal observation is invalid",
            ));
        }
        Ok(observation)
    }

    fn valid_shape(&self) -> bool {
        if self.cart_id.is_nil() || self.tenant_id.is_nil() {
            return false;
        }
        if let GuardPurpose::Removal { line_id } = self.purpose {
            return !line_id.is_nil()
                && self.line_items.len() == 1
                && self.line_items[0].id == line_id
                && self.line_items[0].cart_id == self.cart_id;
        }
        if self.currency_code.len() != 3 || self.line_items.len() > MAX_PRICED_CART_LINES {
            return false;
        }
        let Ok(exponent) = rustok_core::money::currency_exponent(&self.currency_code) else {
            return false;
        };
        let mut ids = BTreeSet::new();
        let mut parents = BTreeMap::new();
        self.line_items.iter().all(|line| {
            let parent_matches = match (line.variant_id, line.product_id) {
                (Some(variant), Some(parent)) => parents
                    .insert(variant, parent)
                    .is_none_or(|previous| previous == parent),
                _ => true,
            };
            parent_matches
                && !line.id.is_nil()
                && ids.insert(line.id)
                && line.cart_id == self.cart_id
                && !line.product_id.is_some_and(|id| id.is_nil())
                && !line.variant_id.is_some_and(|id| id.is_nil())
                && line.quantity > 0
                && line.unit_price >= Decimal::ZERO
                && line.total_price >= Decimal::ZERO
                && rustok_core::money::to_fixed_point_units(line.unit_price, exponent).is_ok()
                && rustok_core::money::to_fixed_point_units(line.total_price, exponent).is_ok()
                && line.unit_price.checked_mul(Decimal::from(line.quantity))
                    == Some(line.total_price)
                && line.currency_code == self.currency_code
        })
    }

    /// Bind only access evidence after owner authorization. Never refresh the original line/context
    /// observation here: doing so would turn a stale quote/stock check into a successful fresh write.
    pub(crate) fn bind_authorized_access(&mut self, cart: &CartResponse) {
        self.guest_access_fingerprint = guest_access_fingerprint(&cart.metadata);
    }

    fn verify_row(&self, cart: &cart::Model) -> Result<(), CartError> {
        if !self.valid_shape() {
            return Err(CartError::Validation(
                "Cart pricing observation is invalid or too large".into(),
            ));
        }
        if self.cart_id != cart.id
            || self.tenant_id != cart.tenant_id
            || self.currency_code != cart.currency_code
            || self.region_id != cart.region_id
            || self.channel_id != cart.channel_id
            || self.channel_slug != cart.channel_slug
            || self.locale_code != cart.locale_code
            || self.customer_id != cart.customer_id
            || self.country_code != cart.country_code
            || self.cart_updated_at != cart.updated_at.with_timezone(&Utc)
            || self.guest_access_fingerprint != guest_access_fingerprint(&cart.metadata)
        {
            return Err(CartError::PricingContextChanged);
        }
        Ok(())
    }

    fn verify_lines(&self, lines: &[cart_line_item::Model]) -> Result<(), CartError> {
        let mut current: Vec<_> = lines.iter().map(PricingLineSnapshot::from_model).collect();
        current.sort_by_key(|line| line.id);
        if current != self.line_items {
            return Err(CartError::PricingContextChanged);
        }
        Ok(())
    }

    /// The caller must already hold Cart's write lock. Only Cart-owned rows are read here;
    /// Pricing, Product and Inventory IO stays outside this critical section.
    pub(crate) async fn verify_locked<C: ConnectionTrait>(
        &self,
        conn: &C,
        cart: &cart::Model,
    ) -> Result<(), CartError> {
        if self.purpose != GuardPurpose::Pricing {
            return Err(CartError::Validation(
                "A priced mutation requires the full Cart observation".into(),
            ));
        }
        self.verify_row(cart)?;
        let lines = cart_line_item::Entity::find()
            .filter(cart_line_item::Column::CartId.eq(cart.id))
            .order_by_asc(cart_line_item::Column::Id)
            .limit((MAX_PRICED_CART_LINES + 1) as u64)
            .all(conn)
            .await?;
        self.verify_lines(&lines)
    }
    pub(crate) async fn verify_removal_locked<C: ConnectionTrait>(
        &self,
        conn: &C,
        cart: &cart::Model,
        line_id: Uuid,
    ) -> Result<(), CartError> {
        if self.purpose != (GuardPurpose::Removal { line_id }) {
            return Err(CartError::Validation(
                "Removal requires the original target-line observation".into(),
            ));
        }
        self.verify_row(cart)?;
        let line = cart_line_item::Entity::find_by_id(line_id)
            .filter(cart_line_item::Column::CartId.eq(cart.id))
            .one(conn)
            .await?
            .ok_or(CartError::CartLineItemNotFound(line_id))?;
        self.verify_lines(&[line])
    }
}

fn guest_access_fingerprint(metadata: &serde_json::Value) -> Option<String> {
    metadata
        .get(crate::guest_access::GUEST_CART_TOKEN_HASH_METADATA_KEY)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cart() -> cart::Model {
        let now = Utc::now().fixed_offset();
        cart::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            channel_id: Some(Uuid::new_v4()),
            channel_slug: Some("web".into()),
            customer_id: None,
            email: None,
            region_id: Some(Uuid::new_v4()),
            country_code: None,
            locale_code: Some("en".into()),
            selected_shipping_option_id: None,
            status: "active".into(),
            currency_code: "EUR".into(),
            shipping_total: Decimal::ZERO,
            total_amount: Decimal::ZERO,
            tax_total: Decimal::ZERO,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    fn guard(cart: &cart::Model) -> CartPricingContextGuard {
        CartPricingContextGuard {
            currency_code: cart.currency_code.clone(),
            region_id: cart.region_id,
            channel_id: cart.channel_id,
            channel_slug: cart.channel_slug.clone(),
            locale_code: cart.locale_code.clone(),
            customer_id: cart.customer_id,
            cart_id: cart.id,
            tenant_id: cart.tenant_id,
            country_code: cart.country_code.clone(),
            cart_updated_at: cart.updated_at.with_timezone(&Utc),
            line_items: Vec::new(),
            purpose: GuardPurpose::Pricing,
            guest_access_fingerprint: guest_access_fingerprint(&cart.metadata),
        }
    }

    fn line(cart: &cart::Model) -> cart_line_item::Model {
        cart_line_item::Model {
            id: Uuid::new_v4(),
            cart_id: cart.id,
            product_id: Some(Uuid::new_v4()),
            variant_id: Some(Uuid::new_v4()),
            fulfillment_requirement: "digital".into(),
            shipping_profile_slug: String::new(),
            sku: None,
            quantity: 2,
            unit_price: Decimal::from(10),
            total_price: Decimal::from(20),
            currency_code: "EUR".into(),
            metadata: serde_json::json!({}),
            created_at: cart.created_at,
            updated_at: cart.updated_at,
        }
    }

    #[test]
    fn every_pricing_or_title_context_change_refuses_the_locked_insert() {
        let current = cart();
        let guard = guard(&current);
        assert!(guard.verify_row(&current).is_ok());
        let mut changes = Vec::new();
        let mut changed = current.clone();
        changed.currency_code = "USD".into();
        changes.push(changed);
        let mut changed = current.clone();
        changed.region_id = None;
        changes.push(changed);
        let mut changed = current.clone();
        changed.channel_id = None;
        changes.push(changed);
        let mut changed = current.clone();
        changed.channel_slug = None;
        changes.push(changed);
        let mut changed = current.clone();
        changed.customer_id = Some(Uuid::new_v4());
        changes.push(changed);
        let mut changed = current.clone();
        changed.country_code = Some("FR".into());
        changes.push(changed);
        let mut changed = current.clone();
        changed.metadata[crate::guest_access::GUEST_CART_TOKEN_HASH_METADATA_KEY] =
            serde_json::json!("rotated");
        changes.push(changed);
        let mut changed = current.clone();
        changed.locale_code = Some("ru".into());
        changes.push(changed);
        let mut changed = current.clone();
        changed.id = Uuid::new_v4();
        changes.push(changed);
        let mut changed = current.clone();
        changed.tenant_id = Uuid::new_v4();
        changes.push(changed);
        let mut changed = current;
        changed.updated_at += chrono::Duration::microseconds(1);
        changes.push(changed);
        for changed in changes {
            assert!(matches!(
                guard.verify_row(&changed),
                Err(CartError::PricingContextChanged)
            ));
        }
    }

    #[test]
    fn quantity_identity_money_and_line_set_drift_refuse_even_without_a_clock_change() {
        let cart = cart();
        let original = line(&cart);
        let mut observed = guard(&cart);
        observed.line_items = vec![PricingLineSnapshot::from_model(&original)];
        assert!(
            observed
                .verify_lines(std::slice::from_ref(&original))
                .is_ok()
        );
        let mut changes = Vec::new();
        let mut changed = original.clone();
        changed.quantity = 3;
        changes.push(changed);
        let mut changed = original.clone();
        changed.variant_id = Some(Uuid::new_v4());
        changes.push(changed);
        let mut changed = original.clone();
        changed.product_id = None;
        changes.push(changed);
        let mut changed = original.clone();
        changed.fulfillment_requirement = "physical".into();
        changes.push(changed);
        let mut changed = original.clone();
        changed.shipping_profile_slug = "other".into();
        changes.push(changed);
        let mut changed = original.clone();
        changed.unit_price = Decimal::from(9);
        changes.push(changed);
        let mut changed = original.clone();
        changed.total_price = Decimal::from(18);
        changes.push(changed);
        for changed in changes {
            assert!(matches!(
                observed.verify_lines(&[changed]),
                Err(CartError::PricingContextChanged)
            ));
        }
        assert!(matches!(
            observed.verify_lines(&[]),
            Err(CartError::PricingContextChanged)
        ));
        assert!(matches!(
            observed.verify_lines(&[original, line(&cart)]),
            Err(CartError::PricingContextChanged)
        ));
    }

    #[test]
    fn malformed_or_duplicate_observations_do_not_become_a_valid_precondition() {
        let cart = cart();
        let mut observed = guard(&cart);
        let line = PricingLineSnapshot::from_model(&line(&cart));
        observed.line_items = vec![line.clone(), line.clone()];
        assert!(!observed.valid_shape());
        observed.line_items = vec![line.clone()];
        observed.line_items[0].quantity = 0;
        assert!(!observed.valid_shape());
        observed.line_items = vec![line; MAX_PRICED_CART_LINES + 1];
        assert!(!observed.valid_shape());
    }
}
