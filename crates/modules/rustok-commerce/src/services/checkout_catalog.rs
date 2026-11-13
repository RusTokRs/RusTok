//! Commerce checkout preconditions, not another Product identity or Inventory owner.
//!
//! Inventory demand is grouped by stable Variant UUID only. Cart/Order lines, quantity-tier
//! prices, titles, SKU text and durable request identities are never merged or rewritten.

use std::collections::{BTreeMap, BTreeSet};

use rustok_cart::{CartLineFulfillmentRequirement, CartLineItemResponse, CartResponse};
use rustok_product::{
    ProductFulfillmentRequirement, ProductStatus, dto::ProductResponse,
};
use uuid::Uuid;

use crate::storefront_channel::is_metadata_visible_for_public_channel;
use crate::storefront_shipping::effective_shipping_profile_slug;

use super::{CheckoutError, CheckoutResult};

/// Checkout processing bound, not a SKU matrix or Cart storage/retention limit. Oversized
/// carts remain readable/editable; reduce their lines before beginning a new checkout.
pub(crate) const MAX_CHECKOUT_LINE_ITEMS: usize = 1024;

pub(crate) struct CheckoutVariantDemand<'a> {
    pub variant_id: Uuid,
    pub product_id: Option<Uuid>,
    pub quantity: i32,
    pub line_items: Vec<&'a CartLineItemResponse>,
}

/// A guest checkout delegates through Commerce; a nil UUID must never fabricate a User
/// actor. This supplies diagnostic/call identity only, not Cart/customer authorization.
pub(super) fn checkout_actor(actor_id: Uuid) -> rustok_api::PortActor {
    if actor_id.is_nil() {
        rustok_api::PortActor::service("rustok-commerce.checkout")
    } else {
        rustok_api::PortActor::user(actor_id.to_string())
    }
}

/// Validate the whole snapshot before owner IO and total the requested physical SKU units.
/// Digital SKU-backed lines still follow the Inventory owner's existing policy; an aggregate
/// must not invent an inventory exemption from a display label or Product kind.
pub(crate) fn variant_demands(
    cart: &CartResponse,
) -> CheckoutResult<Vec<CheckoutVariantDemand<'_>>> {
    if cart.id.is_nil() || cart.tenant_id.is_nil() {
        return Err(CheckoutError::Validation(
            "Cart identity is invalid".to_string(),
        ));
    }
    if cart.line_items.len() > MAX_CHECKOUT_LINE_ITEMS {
        return Err(CheckoutError::Validation(format!(
            "Checkout supports at most {MAX_CHECKOUT_LINE_ITEMS} line items; remove lines before retrying"
        )));
    }
    let mut line_ids = BTreeSet::new();
    let mut demands = BTreeMap::<Uuid, CheckoutVariantDemand<'_>>::new();
    for line in &cart.line_items {
        if line.id.is_nil()
            || line.cart_id != cart.id
            || !line_ids.insert(line.id)
            || line.quantity <= 0
            || line.product_id.is_some_and(|id| id.is_nil())
            || line.variant_id.is_some_and(|id| id.is_nil())
            || !line.currency_code.eq_ignore_ascii_case(&cart.currency_code)
        {
            return Err(CheckoutError::Validation(
                "Cart line identity, quantity or currency is inconsistent; inspect the cart before retrying"
                    .to_string(),
            ));
        }
        let Some(variant_id) = line.variant_id else {
            // Explicit custom lines have no SKU identity and must not acquire a substitute.
            continue;
        };
        let demand = demands
            .entry(variant_id)
            .or_insert_with(|| CheckoutVariantDemand {
                variant_id,
                product_id: line.product_id,
                quantity: 0,
                line_items: Vec::new(),
            });
        if let (Some(expected), Some(actual)) = (demand.product_id, line.product_id)
            && expected != actual
        {
            return Err(CheckoutError::Validation(
                "The same SKU is associated with different products in this cart".to_string(),
            ));
        }
        // A variant-first historical line remains unchanged. A known parent on another line
        // supplies only the expected Product owner for the shared verification read.
        demand.product_id = demand.product_id.or(line.product_id);
        demand.quantity = demand.quantity.checked_add(line.quantity).ok_or_else(|| {
            CheckoutError::Validation(
                "Combined quantity for one SKU exceeds the supported inventory quantity; reduce the cart quantity"
                    .to_string(),
            )
        })?;
        demand.line_items.push(line);
    }
    Ok(demands.into_values().collect())
}

/// Check exact owner correlation and the current fulfillment contract for every original
/// line. Historical title/SKU text, prices and axis labels are intentionally not compared:
/// they are snapshots, not identity. A different tuple or first variant is never a fallback.
pub(super) fn validate_variant_line(
    tenant_id: Uuid,
    line: &CartLineItemResponse,
    product: &ProductResponse,
    public_channel_slug: Option<&str>,
) -> CheckoutResult<()> {
    if product.tenant_id != tenant_id
        || product.id.is_nil()
        || line.product_id.is_some_and(|id| id != product.id)
    {
        return Err(CheckoutError::Validation(
            "Product owner returned an inconsistent tenant or product identity".to_string(),
        ));
    }
    let variant_id = line.variant_id.ok_or_else(|| {
        CheckoutError::Validation(
            "A catalog verification requires the original SKU UUID".to_string(),
        )
    })?;
    let mut matches = product
        .variants
        .iter()
        .filter(|variant| variant.id == variant_id);
    let variant = matches.next().ok_or_else(|| {
        CheckoutError::Validation(format!(
            "Variant {variant_id} is no longer available for checkout"
        ))
    })?;
    if variant.id.is_nil() || variant.product_id != product.id || matches.next().is_some() {
        return Err(CheckoutError::Validation(
            "Product owner returned an inconsistent SKU membership".to_string(),
        ));
    }
    if product.status != ProductStatus::Active
        || product.published_at.is_none()
        || !is_metadata_visible_for_public_channel(&product.metadata, public_channel_slug)
    {
        return Err(CheckoutError::Validation(format!(
            "Product {} is not available for the cart channel",
            product.id
        )));
    }
    let cart_requirement = match line.fulfillment_requirement {
        CartLineFulfillmentRequirement::Digital => ProductFulfillmentRequirement::Digital,
        CartLineFulfillmentRequirement::Physical => ProductFulfillmentRequirement::Physical,
    };
    if product.fulfillment_requirement
        != ProductFulfillmentRequirement::from_product_type(product.product_type.as_deref())
        || product.fulfillment_requirement != cart_requirement
    {
        return Err(CheckoutError::Validation(format!(
            "Line item {} fulfillment requirement is stale relative to the catalog",
            line.id
        )));
    }
    match product.fulfillment_requirement {
        ProductFulfillmentRequirement::Digital if line.shipping_profile_slug.is_some() => {
            Err(CheckoutError::Validation(format!(
                "Digital line item {} must not have a shipping profile",
                line.id
            )))
        }
        ProductFulfillmentRequirement::Digital => Ok(()),
        ProductFulfillmentRequirement::Physical => {
            let current = effective_shipping_profile_slug(
                product.shipping_profile_slug.as_deref(),
                &product.metadata,
                variant.shipping_profile_slug.as_deref(),
            );
            if line.shipping_profile_slug.as_deref() != Some(current.as_str()) {
                return Err(CheckoutError::Validation(format!(
                    "Line item {} uses stale shipping profile snapshot",
                    line.id
                )));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "checkout_catalog_tests.rs"]
pub(crate) mod tests;

pub(crate) fn validate_inventory_observation(
    observation: &rustok_inventory::InventoryAvailabilitySnapshot,
    variant_id: Uuid,
    quantity: i32,
) -> Result<(), rustok_api::PortError> {
    if observation.variant_id != variant_id || observation.requested_quantity != quantity {
        return Err(rustok_api::PortError::invariant_violation(
            "commerce.inventory_observation_mismatch",
            "Inventory owner returned an observation for a different SKU or quantity",
        ));
    }
    Ok(())
}
