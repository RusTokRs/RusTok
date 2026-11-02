//! One Cart-owned transaction for context and prepared unit prices. Pricing is observed by
//! Commerce beforehand; tax/shipping recalculation retains its existing transactional boundary.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set, TransactionTrait};
use validator::Validate;

use super::helpers::*;
use super::pricing::{PricingCoverage, apply_pricing_updates};
use super::{CartService, DeliveryGroupKey, DeliveryGroupSnapshot};
use crate::{
    CartContextPricing, CartContextUpdateRequest, CartError, CartPricingScope, CartResponse,
    CartResult, MAX_PRICED_CART_LINES, UpdateCartContextInput, entities,
};

pub(super) fn normalize_context_input(
    mut input: UpdateCartContextInput,
) -> CartResult<UpdateCartContextInput> {
    input
        .validate()
        .map_err(|error| CartError::Validation(error.to_string()))?;
    if input.region_id.is_some_and(|id| id.is_nil())
        || input
            .selected_shipping_option_id
            .is_some_and(|id| id.is_nil())
    {
        return Err(CartError::Validation(
            "Context UUIDs must not be nil".into(),
        ));
    }
    input.email = input
        .email
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    input.country_code = input
        .country_code
        .as_deref()
        .map(normalize_country_code)
        .transpose()?;
    input.locale_code = input
        .locale_code
        .as_deref()
        .map(normalize_locale_code)
        .transpose()?;
    if let Some(selections) = input.shipping_selections.as_mut() {
        if selections.len() > MAX_PRICED_CART_LINES {
            return Err(CartError::Validation(
                "Shipping context exceeds the bounded Cart group count".into(),
            ));
        }
        let mut groups = BTreeSet::new();
        for selection in selections {
            selection
                .validate()
                .map_err(|error| CartError::Validation(error.to_string()))?;
            if selection.shipping_profile_slug.trim().is_empty()
                || selection
                    .selected_shipping_option_id
                    .is_some_and(|id| id.is_nil())
            {
                return Err(CartError::Validation(
                    "Shipping context contains an invalid profile or option".into(),
                ));
            }
            selection.shipping_profile_slug =
                normalize_shipping_profile_slug(Some(&selection.shipping_profile_slug));
            selection.seller_id = normalize_seller_id(selection.seller_id.as_deref());
            if !groups.insert((
                selection.shipping_profile_slug.clone(),
                selection.seller_id.clone(),
            )) {
                return Err(CartError::Validation(
                    "Shipping context contains a duplicate delivery group".into(),
                ));
            }
        }
    }
    Ok(input)
}

impl CartResponse {
    /// Hypothetical owner-normalized quote input only. Old money/totals are deliberately not
    /// recomputed here; this projection is never an acknowledgement or a live Cart response.
    pub fn project_context(&self, input: &UpdateCartContextInput) -> CartResult<Self> {
        let input = normalize_context_input(input.clone())?;
        if self.line_items.len() > MAX_PRICED_CART_LINES
            || self.delivery_groups.len() > MAX_PRICED_CART_LINES
        {
            return Err(CartError::Validation(
                "Cart context projection exceeds 1024 observed lines/groups".into(),
            ));
        }
        let mut target = self.clone();
        target.email = input.email.clone();
        target.region_id = input.region_id;
        target.country_code = input.country_code.clone();
        target.locale_code = input.locale_code.clone();
        target.selected_shipping_option_id = input.selected_shipping_option_id;
        let mut available = BTreeSet::new();
        let mut existing = BTreeMap::new();
        for group in &self.delivery_groups {
            let key = DeliveryGroupKey {
                shipping_profile_slug: normalize_shipping_profile_slug(Some(
                    &group.shipping_profile_slug,
                )),
                seller_id: normalize_seller_id(group.seller_id.as_deref()),
                seller_scope: None,
            };
            available.insert(DeliveryGroupSnapshot { key: key.clone() });
            existing.insert(key, group.selected_shipping_option_id);
        }
        let desired = resolve_shipping_selection_patch(
            &available,
            existing,
            self.selected_shipping_option_id,
            &input,
        )?;
        for group in &mut target.delivery_groups {
            let key = DeliveryGroupKey {
                shipping_profile_slug: normalize_shipping_profile_slug(Some(
                    &group.shipping_profile_slug,
                )),
                seller_id: normalize_seller_id(group.seller_id.as_deref()),
                seller_scope: None,
            };
            group.selected_shipping_option_id = desired.get(&key).copied().flatten();
            // Old availability belongs to the original context, not this hypothetical target.
            group.available_shipping_options.clear();
        }
        target.selected_shipping_option_id = match target.delivery_groups.len() {
            0 if target.line_items.is_empty() => input.selected_shipping_option_id,
            1 => target
                .delivery_groups
                .first()
                .and_then(|group| group.selected_shipping_option_id),
            _ => None,
        };
        Ok(target)
    }
    /// Read-only comparison for orchestration after atomic checkout preparation. It does not
    /// authorize a write or acknowledge a command; locked snapshots must only be recovered/read.
    pub fn context_is_current(&self, input: &UpdateCartContextInput) -> CartResult<bool> {
        let target = self.project_context(input)?;
        Ok(self.email == target.email
            && self.region_id == target.region_id
            && self.country_code == target.country_code
            && self.locale_code == target.locale_code
            && self.selected_shipping_option_id == target.selected_shipping_option_id
            && self
                .delivery_groups
                .iter()
                .zip(&target.delivery_groups)
                .all(|(old, new)| {
                    old.selected_shipping_option_id == new.selected_shipping_option_id
                }))
    }
}

pub(super) fn ensure_preserved_price_basis(
    cart: &entities::cart::Model,
    input: &UpdateCartContextInput,
    has_lines: bool,
) -> CartResult<()> {
    let current = CartPricingScope::from_model(cart);
    let target = current.clone().with_input(input);
    if has_lines && !current.same_price_basis(&target) {
        return Err(CartError::PricingContextChanged);
    }
    Ok(())
}

pub(super) async fn apply_context_input<C: ConnectionTrait>(
    conn: &C,
    cart: &entities::cart::Model,
    input: &UpdateCartContextInput,
) -> CartResult<entities::cart::Model> {
    let mut active: entities::cart::ActiveModel = cart.clone().into();
    active.email = Set(input.email.clone());
    active.region_id = Set(input.region_id);
    active.country_code = Set(input.country_code.clone());
    active.locale_code = Set(input.locale_code.clone());
    active.selected_shipping_option_id = Set(input.selected_shipping_option_id);
    active.updated_at = Set(Utc::now().into());
    let updated = active.update(conn).await?;
    apply_shipping_selection_patch(conn, cart, input).await?;
    Ok(updated)
}

impl CartService {
    pub async fn update_context(
        &self,
        tenant_id: uuid::Uuid,
        request: CartContextUpdateRequest,
    ) -> CartResult<CartResponse> {
        let CartContextUpdateRequest {
            cart_id,
            input,
            pricing_context,
            pricing,
        } = request;
        let input = normalize_context_input(input)?;
        let txn = self.db.begin().await?;
        let cart = load_cart_for_update_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "update_context")?;
        pricing_context.verify_locked(&txn, &cart).await?;
        match pricing {
            CartContextPricing::Preserve => {
                ensure_preserved_price_basis(
                    &cart,
                    &input,
                    pricing_context.observed_line_count() > 0,
                )?;
            }
            CartContextPricing::Reprice(plan) => {
                let target = CartPricingScope::from_model(&cart).with_input(&input);
                if plan.scope != target {
                    return Err(CartError::PricingContextChanged);
                }
                apply_pricing_updates(&txn, &cart, plan.updates, PricingCoverage::Complete).await?;
            }
        }
        let updated = apply_context_input(&txn, &cart, &input).await?;
        recalculate_totals(
            &txn,
            self.tax_calculation_port.as_ref(),
            self.shipping_option_read_port.as_ref(),
            updated,
        )
        .await?;
        reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        // As elsewhere in Cart, a failed post-commit read is not evidence that the write failed.
        self.get_cart(tenant_id, cart_id).await
    }
}
