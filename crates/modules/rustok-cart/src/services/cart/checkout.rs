use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set,
    TransactionTrait, sea_query::Expr,
};
use std::collections::BTreeMap;
use uuid::Uuid;
use validator::Validate;

use crate::atomic_checkout_port::{CartCheckoutLineItemPricingUpdate, CartCheckoutPricingPlan};
use crate::checkout_snapshot::PrepareCartCheckoutSnapshotRequest;
use crate::dto::{CartResponse, CartStatus, UpdateCartContextInput};
use crate::entities;
use crate::error::{CartError, CartResult};

use super::CartService;
use super::helpers::{
    apply_shipping_selection_patch, load_cart_in_tx, normalize_country_code, normalize_locale_code,
    recalculate_totals, replace_pricing_adjustments,
};

const CHECKOUT_PRICING_CHANGED_PREFIX: &str = "checkout pricing snapshot changed:";

impl CartService {
    pub async fn complete_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_status(
            tenant_id,
            cart_id,
            &[CartStatus::Active, CartStatus::CheckingOut],
            CartStatus::Completed,
            true,
        )
        .await
    }

    pub async fn abandon_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_status(
            tenant_id,
            cart_id,
            &[CartStatus::Active],
            CartStatus::Abandoned,
            false,
        )
        .await
    }

    pub async fn begin_checkout(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_status(
            tenant_id,
            cart_id,
            &[CartStatus::Active],
            CartStatus::CheckingOut,
            false,
        )
        .await
    }

    /// Atomically applies checkout context and totals without an external
    /// pricing plan. Kept for compatibility with non-storefront callers.
    pub async fn prepare_checkout(
        &self,
        tenant_id: Uuid,
        request: PrepareCartCheckoutSnapshotRequest,
    ) -> CartResult<CartResponse> {
        self.prepare_checkout_with_pricing(tenant_id, request, None)
            .await
    }

    /// Atomically claims the active cart, verifies and applies a resolved
    /// pricing plan, applies checkout context, recalculates cart-owned totals
    /// and commits the `checking_out` state.
    pub async fn prepare_checkout_with_pricing(
        &self,
        tenant_id: Uuid,
        request: PrepareCartCheckoutSnapshotRequest,
        pricing_plan: Option<CartCheckoutPricingPlan>,
    ) -> CartResult<CartResponse> {
        for selection in request.input.shipping_selections.iter().flatten() {
            selection
                .validate()
                .map_err(|error| CartError::Validation(error.to_string()))?;
        }

        let txn = self.db.begin().await?;
        let now = Utc::now().fixed_offset();
        let claimed = entities::cart::Entity::update_many()
            .col_expr(
                entities::cart::Column::Status,
                Expr::value(CartStatus::CheckingOut.as_str()),
            )
            .col_expr(entities::cart::Column::UpdatedAt, Expr::value(now))
            .filter(entities::cart::Column::TenantId.eq(tenant_id))
            .filter(entities::cart::Column::Id.eq(request.cart_id))
            .filter(entities::cart::Column::Status.eq(CartStatus::Active.as_str()))
            .exec(&txn)
            .await?;

        if claimed.rows_affected != 1 {
            let current = load_cart_in_tx(&txn, tenant_id, request.cart_id).await?;
            let from = current.status;
            txn.rollback().await?;
            return Err(CartError::InvalidTransition {
                from,
                to: CartStatus::CheckingOut.to_string(),
            });
        }

        let cart = load_cart_in_tx(&txn, tenant_id, request.cart_id).await?;
        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
            .all(&txn)
            .await?;
        if line_items.is_empty() {
            txn.rollback().await?;
            return Err(CartError::Validation(format!(
                "cart {} has no line items",
                cart.id
            )));
        }

        if let Some(pricing_plan) = pricing_plan {
            apply_checkout_pricing_plan(&txn, &cart, &line_items, &request, pricing_plan).await?;
        }

        let shipping_patch_requested = request.input.shipping_selections.is_some()
            || request.input.selected_shipping_option_id.is_some();
        let country_code = match cart.country_code.clone() {
            Some(country_code) => Some(country_code),
            None => request
                .input
                .country_code
                .as_deref()
                .map(normalize_country_code)
                .transpose()?,
        };
        let locale_code = match cart.locale_code.clone() {
            Some(locale_code) => Some(locale_code),
            None => request
                .input
                .locale_code
                .as_deref()
                .map(normalize_locale_code)
                .transpose()?,
        };
        let selected_shipping_option_id = if shipping_patch_requested {
            request.input.selected_shipping_option_id
        } else {
            cart.selected_shipping_option_id
        };
        let context_input = UpdateCartContextInput {
            email: cart.email.clone(),
            region_id: cart.region_id.or(request.input.region_id),
            country_code,
            locale_code,
            selected_shipping_option_id,
            shipping_selections: request.input.shipping_selections.clone(),
        };

        let mut active: entities::cart::ActiveModel = cart.clone().into();
        active.email = Set(context_input.email.clone());
        active.region_id = Set(context_input.region_id);
        active.country_code = Set(context_input.country_code.clone());
        active.locale_code = Set(context_input.locale_code.clone());
        active.selected_shipping_option_id = Set(context_input.selected_shipping_option_id);
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;

        if shipping_patch_requested {
            apply_shipping_selection_patch(&txn, &cart, &context_input).await?;
        }

        let prepared_cart = load_cart_in_tx(&txn, tenant_id, cart.id).await?;
        recalculate_totals(&txn, self.tax_calculation_port.as_ref(), prepared_cart).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart.id).await
    }

    pub async fn release_checkout(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CartResult<CartResponse> {
        self.transition_status(
            tenant_id,
            cart_id,
            &[CartStatus::CheckingOut],
            CartStatus::Active,
            false,
        )
        .await
    }

    async fn transition_status(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        allowed_from: &[CartStatus],
        target: CartStatus,
        mark_completed: bool,
    ) -> CartResult<CartResponse> {
        let txn = self.db.begin().await?;
        let now = Utc::now().fixed_offset();
        let completed_at = mark_completed.then_some(now);
        let allowed_from = allowed_from
            .iter()
            .map(|status| status.as_str())
            .collect::<Vec<_>>();

        let result = entities::cart::Entity::update_many()
            .col_expr(entities::cart::Column::Status, Expr::value(target.as_str()))
            .col_expr(entities::cart::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                entities::cart::Column::CompletedAt,
                Expr::value(completed_at),
            )
            .filter(entities::cart::Column::TenantId.eq(tenant_id))
            .filter(entities::cart::Column::Id.eq(cart_id))
            .filter(entities::cart::Column::Status.is_in(allowed_from))
            .exec(&txn)
            .await?;

        if result.rows_affected != 1 {
            let current = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
            let from = current.status;
            txn.rollback().await?;
            return Err(CartError::InvalidTransition {
                from,
                to: target.to_string(),
            });
        }

        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }
}

async fn apply_checkout_pricing_plan(
    txn: &DatabaseTransaction,
    cart: &entities::cart::Model,
    line_items: &[entities::cart_line_item::Model],
    request: &PrepareCartCheckoutSnapshotRequest,
    pricing_plan: CartCheckoutPricingPlan,
) -> CartResult<()> {
    let expected_currency = cart.currency_code.trim().to_ascii_uppercase();
    if pricing_plan.currency_code.trim().to_ascii_uppercase() != expected_currency {
        return Err(checkout_pricing_changed(format!(
            "currency changed from {} to {}",
            pricing_plan.currency_code, cart.currency_code
        )));
    }
    if pricing_plan.effective_region_id != cart.region_id.or(request.input.region_id) {
        return Err(checkout_pricing_changed(
            "effective region changed while resolving checkout prices",
        ));
    }
    if pricing_plan.cart_channel_id != cart.channel_id
        || normalize_channel_slug(pricing_plan.cart_channel_slug.as_deref())
            != normalize_channel_slug(cart.channel_slug.as_deref())
    {
        return Err(checkout_pricing_changed(
            "cart channel changed while resolving checkout prices",
        ));
    }

    let mut updates = BTreeMap::<Uuid, CartCheckoutLineItemPricingUpdate>::new();
    for update in pricing_plan.line_items {
        let line_item_id = update.line_item_id;
        if updates.insert(line_item_id, update).is_some() {
            return Err(checkout_pricing_changed(format!(
                "line item {line_item_id} has duplicate pricing updates"
            )));
        }
    }

    let expected_priced_lines = line_items
        .iter()
        .filter(|line_item| line_item.variant_id.is_some())
        .count();
    if updates.len() != expected_priced_lines {
        return Err(checkout_pricing_changed(format!(
            "pricing plan covers {} variant lines, expected {expected_priced_lines}",
            updates.len()
        )));
    }

    let mut pricing_adjustments = Vec::with_capacity(expected_priced_lines);
    for line_item in line_items {
        let Some(variant_id) = line_item.variant_id else {
            continue;
        };
        let Some(update) = updates.remove(&line_item.id) else {
            return Err(checkout_pricing_changed(format!(
                "line item {} is missing from the pricing plan",
                line_item.id
            )));
        };
        validate_checkout_line_pricing(line_item, variant_id, expected_currency.as_str(), &update)?;

        let mut active: entities::cart_line_item::ActiveModel = line_item.clone().into();
        active.unit_price = Set(update.unit_price);
        active.total_price = Set(update.unit_price * Decimal::from(line_item.quantity));
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        pricing_adjustments.push((line_item.id, update.pricing_adjustment));
    }

    if let Some(unexpected_id) = updates.keys().next() {
        return Err(checkout_pricing_changed(format!(
            "pricing plan contains unknown line item {unexpected_id}"
        )));
    }

    replace_pricing_adjustments(
        txn,
        cart.id,
        cart.currency_code.as_str(),
        pricing_adjustments,
    )
    .await
}

fn validate_checkout_line_pricing(
    line_item: &entities::cart_line_item::Model,
    variant_id: Uuid,
    expected_currency: &str,
    update: &CartCheckoutLineItemPricingUpdate,
) -> CartResult<()> {
    if update.variant_id != variant_id || update.quantity != line_item.quantity {
        return Err(checkout_pricing_changed(format!(
            "line item {} variant or quantity changed while resolving checkout prices",
            line_item.id
        )));
    }
    if !line_item
        .currency_code
        .eq_ignore_ascii_case(expected_currency)
    {
        return Err(checkout_pricing_changed(format!(
            "line item {} currency changed while resolving checkout prices",
            line_item.id
        )));
    }
    if update.unit_price < Decimal::ZERO {
        return Err(CartError::Validation(format!(
            "checkout pricing for line item {} cannot be negative",
            line_item.id
        )));
    }
    if let Some(adjustment) = &update.pricing_adjustment {
        let maximum = update.unit_price * Decimal::from(line_item.quantity);
        if adjustment.amount < Decimal::ZERO || adjustment.amount > maximum {
            return Err(CartError::Validation(format!(
                "checkout pricing adjustment for line item {} is outside the line total",
                line_item.id
            )));
        }
    }
    Ok(())
}

fn checkout_pricing_changed(message: impl Into<String>) -> CartError {
    CartError::Validation(format!(
        "{CHECKOUT_PRICING_CHANGED_PREFIX} {}",
        message.into()
    ))
}

fn normalize_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}
