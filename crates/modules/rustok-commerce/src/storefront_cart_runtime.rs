//! Commerce-owned cross-owner add-to-cart workflow, shared by native and GraphQL adapters.
//!
//! Buyer observations are optional preconditions, never prices or price-list eligibility.
//! No cross-owner price/stock reservation or durable command receipt is promised here.

use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_cart::{
    AddCartLineItemInput, CartLineFulfillmentRequirement, CartPricingContextGuard, CartResponse,
    CartStorefrontAddLineItemRequest, CartStorefrontLineItemPricingRequest, CartStorefrontPort,
    CartStorefrontReadRequest,
};
use rustok_inventory::{InventoryAvailabilityRequest, InventoryReservationPort};
use rustok_pricing::{PriceResolutionContext, PricingReadPort, ResolveProductPriceRequest};
use rustok_product::{
    ProductCatalogReadPort, ProductFulfillmentRequirement,
    StorefrontVariantProductProjectionRequest,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::storefront_checkout_pricing::{cart_line_pricing_snapshot, cart_quote_matches};
use crate::storefront_shipping::effective_shipping_profile_slug;

#[derive(Clone, Debug)]
pub struct StorefrontCartAddCommand {
    pub cart_id: Uuid,
    pub variant_id: Uuid,
    pub quantity: i32,
    pub expected_context: Option<PriceResolutionContext>,
    pub expected_locale: Option<String>,
    pub metadata: Value,
}

/// Composition supplies selected owner ports; this workflow never constructs a second provider
/// after a selected provider fails and never reads another owner's private tables.
pub struct StorefrontCartRuntime {
    product: Arc<dyn ProductCatalogReadPort>,
    pricing: Arc<dyn PricingReadPort>,
    cart: Arc<dyn CartStorefrontPort>,
    inventory: Arc<dyn InventoryReservationPort>,
}

impl StorefrontCartRuntime {
    pub fn new(
        product: Arc<dyn ProductCatalogReadPort>,
        pricing: Arc<dyn PricingReadPort>,
        cart: Arc<dyn CartStorefrontPort>,
        inventory: Arc<dyn InventoryReservationPort>,
    ) -> Self {
        Self {
            product,
            pricing,
            cart,
            inventory,
        }
    }

    /// `customer_id` must be resolved from the authenticated request by the transport, never
    /// deserialized from the buyer payload. The supplied Cart storefront port must be the
    /// owner-guarded boundary: it admits guest capability and binds its locked access fence.
    pub async fn add_line_item(
        &self,
        context: PortContext,
        customer_id: Option<Uuid>,
        fallback_locale: &str,
        command: StorefrontCartAddCommand,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        if command.cart_id.is_nil() || command.variant_id.is_nil() || command.quantity < 1 {
            return Err(PortError::validation(
                "commerce.cart_add_invalid",
                "A valid cart, SKU and positive quantity are required",
            ));
        }
        if command.expected_context.as_ref().is_some_and(|expected| {
            expected.price_list_id.is_some() || expected.quantity != Some(command.quantity)
        }) {
            return Err(PortError::validation(
                "commerce.cart_preview_not_purchasable",
                "A price-list preview or another quantity cannot authorize a cart addition",
            ));
        }
        let cart = self
            .cart
            .read_storefront_cart(
                context.clone(),
                CartStorefrontReadRequest {
                    cart_id: command.cart_id,
                },
            )
            .await
            .map_err(|error| dependency_error("cart", error))?;
        let tenant_id = context
            .tenant_id
            .parse::<Uuid>()
            .ok()
            .filter(|id| !id.is_nil())
            .ok_or_else(|| {
                PortError::validation(
                    "commerce.cart_tenant_invalid",
                    "A valid cart tenant is required",
                )
            })?;
        if cart.id != command.cart_id || cart.tenant_id != tenant_id {
            return Err(PortError::invariant_violation(
                "commerce.cart_identity_mismatch",
                "Cart owner returned an inconsistent identity",
            ));
        }
        if cart.customer_id.is_some() && cart.customer_id != customer_id {
            return Err(PortError::forbidden(
                "commerce.cart_customer_mismatch",
                "This cart belongs to another customer",
            ));
        }
        require_active_cart(&cart)?;
        let mut read_context =
            crate::storefront_checkout_pricing::cart_pricing_context(context.clone(), &cart)?;
        let effective_locale = read_context.locale.clone();
        if let Some(expected) = command.expected_locale.as_deref() {
            if expected.trim().is_empty()
                || !rustok_api::locale_tags_match(&effective_locale, expected)
            {
                return Err(PortError::conflict(
                    "commerce.cart_locale_mismatch",
                    "Cart locale differs from the observed product; use the cart locale and request a new quote",
                ));
            }
        }
        let pricing_context = CartPricingContextGuard::capture_add(&cart)?;
        // Only the Cart-owned observation is adopted later; no post-IO snapshot refresh.
        let pricing_request = ResolveProductPriceRequest {
            product_id: None,
            variant_id: command.variant_id,
            currency_code: cart.currency_code.clone(),
            region_id: cart.region_id,
            price_list_id: None,
            channel_id: cart.channel_id,
            channel_slug: cart.channel_slug.clone(),
            quantity: Some(command.quantity),
        };
        let price = self
            .pricing
            .resolve_product_price(read_context.clone(), pricing_request.clone())
            .await
            .map_err(|error| dependency_error("pricing", error))?;
        if !cart_quote_matches(&pricing_request, &price) {
            return Err(PortError::invariant_violation(
                "commerce.cart_quote_mismatch",
                "Cart pricing returned an inconsistent SKU or context",
            ));
        }
        verify_buyer_context(command.expected_context.as_ref(), &price.context)?;
        // Pricing normalizes channel identity through the Channel owner. Use that exact scope
        // for Product publication/stock projection, even when the cart stored only one selector.
        read_context.channel = price.context.channel_slug.clone();
        let product = self
            .product
            .read_storefront_variant_product_projection(
                read_context.clone(),
                StorefrontVariantProductProjectionRequest {
                    variant_id: command.variant_id,
                    locale: Some(read_context.locale.clone()),
                    fallback_locale: Some(fallback_locale.to_string()),
                    public_channel_slug: price.context.channel_slug.clone(),
                },
            )
            .await
            .map_err(|error| dependency_error("product", error))?
            .ok_or_else(|| {
                PortError::not_found(
                    "commerce.cart_product_unavailable",
                    "The selected SKU is no longer available",
                )
            })?;
        let variant = product
            .variants
            .iter()
            .find(|variant| variant.id == command.variant_id)
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "commerce.cart_variant_missing",
                    "The public SKU projection is inconsistent",
                )
            })?;
        if product.tenant_id != tenant_id
            || product.id != price.product_id
            || variant.product_id != product.id
            || cart.line_items.iter().any(|line| {
                line.variant_id == Some(variant.id)
                    && line.product_id.is_some_and(|id| id != product.id)
            })
            || product
                .variants
                .iter()
                .filter(|item| item.id == command.variant_id)
                .count()
                != 1
        {
            return Err(PortError::invariant_violation(
                "commerce.cart_product_mismatch",
                "Cart pricing returned a different product",
            ));
        }
        require_cart_inventory(
            self.inventory.as_ref(),
            read_context.clone(),
            &cart,
            command.variant_id,
            command.quantity,
            None,
        )
        .await?;
        let product_title = product
            .translations
            .iter()
            .find(|translation| {
                rustok_api::locale_tags_match(&translation.locale, &read_context.locale)
            })
            .or_else(|| {
                product.translations.iter().find(|translation| {
                    rustok_api::locale_tags_match(&translation.locale, fallback_locale)
                })
            })
            .map(|translation| translation.title.as_str())
            .filter(|title| !title.trim().is_empty())
            .unwrap_or(variant.title.as_str());
        let title = if variant.title.trim().is_empty() || variant.title == product_title {
            product_title.to_string()
        } else {
            format!("{product_title} / {}", variant.title)
        };
        let shipping_profile_slug = match product.fulfillment_requirement {
            ProductFulfillmentRequirement::Digital => None,
            ProductFulfillmentRequirement::Physical => Some(effective_shipping_profile_slug(
                product.shipping_profile_slug.as_deref(),
                &product.metadata,
                variant.shipping_profile_slug.as_deref(),
            )),
        };
        let (unit_price, pricing_adjustment) = cart_line_pricing_snapshot(command.quantity, &price);
        let mut metadata = sanitized_line_metadata(command.metadata);
        if let Some(seller_id) = product.seller_id.as_deref() {
            metadata["seller"] = json!({ "id": seller_id });
        }
        self.cart
            .add_storefront_line_item(
                context,
                CartStorefrontAddLineItemRequest {
                    cart_id: command.cart_id,
                    pricing_context,
                    input: AddCartLineItemInput {
                        product_id: Some(product.id),
                        variant_id: Some(variant.id),
                        fulfillment_requirement: match product.fulfillment_requirement {
                            ProductFulfillmentRequirement::Digital => {
                                CartLineFulfillmentRequirement::Digital
                            }
                            ProductFulfillmentRequirement::Physical => {
                                CartLineFulfillmentRequirement::Physical
                            }
                        },
                        shipping_profile_slug,
                        sku: variant.sku.clone(),
                        title,
                        quantity: command.quantity,
                        unit_price,
                        metadata,
                    },
                    pricing_adjustment,
                },
            )
            .await
            .map_err(|error| dependency_error("cart", error))
    }
    /// Adopt an explicit quantity against the same admitted Cart observation used for the quote
    /// and aggregate stock check. In particular, native decrement must not reread/rebase here.
    pub async fn update_line_item_quantity(
        &self,
        context: PortContext,
        customer_id: Option<Uuid>,
        fallback_locale: &str,
        cart: CartResponse,
        line_item_id: Uuid,
        quantity: i32,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        let pricing_context = CartPricingContextGuard::capture(&cart)?;
        if cart.customer_id.is_some() && cart.customer_id != customer_id {
            return Err(PortError::forbidden(
                "commerce.cart_customer_mismatch",
                "This cart belongs to another customer",
            ));
        }
        require_active_cart(&cart)?;
        let mut read_context =
            crate::storefront_checkout_pricing::cart_pricing_context(context.clone(), &cart)?;
        let price = crate::read_storefront_cart_line_quote(
            self.pricing.as_ref(),
            read_context.clone(),
            &cart,
            line_item_id,
            quantity,
        )
        .await
        .map_err(|error| dependency_error("pricing", error))?;
        let line = cart
            .line_items
            .iter()
            .find(|line| line.id == line_item_id)
            .ok_or_else(|| {
                PortError::not_found(
                    "commerce.cart_pricing_line_not_found",
                    "The selected Cart line is no longer available",
                )
            })?;
        let variant_id = line.variant_id.filter(|id| !id.is_nil()).ok_or_else(|| {
            PortError::validation("commerce.cart_pricing_variant_required", "Cart quantity changes require the original SKU; remove or reconcile this line instead")
        })?;
        read_context.channel = price.context.channel_slug.clone();
        let product = self
            .product
            .read_storefront_variant_product_projection(
                read_context.clone(),
                StorefrontVariantProductProjectionRequest {
                    variant_id,
                    locale: Some(read_context.locale.clone()),
                    fallback_locale: Some(fallback_locale.to_string()),
                    public_channel_slug: price.context.channel_slug.clone(),
                },
            )
            .await
            .map_err(|error| dependency_error("product", error))?
            .ok_or_else(|| {
                PortError::not_found(
                    "commerce.cart_product_unavailable",
                    "The selected SKU is no longer available",
                )
            })?;
        let mut variants = product
            .variants
            .iter()
            .filter(|variant| variant.id == variant_id);
        let variant = variants.next().ok_or_else(|| {
            PortError::invariant_violation(
                "commerce.cart_product_mismatch",
                "Product owner omitted the original SKU",
            )
        })?;
        if product.tenant_id != cart.tenant_id
            || product.id != price.product_id
            || variants.next().is_some()
            || variant.product_id != product.id
            || line.product_id.is_some_and(|id| id != product.id)
        {
            return Err(PortError::invariant_violation(
                "commerce.cart_product_mismatch",
                "Cart pricing returned an incompatible SKU or parent",
            ));
        }
        let (requirement, profile) = match product.fulfillment_requirement {
            ProductFulfillmentRequirement::Digital => {
                (CartLineFulfillmentRequirement::Digital, None)
            }
            ProductFulfillmentRequirement::Physical => (
                CartLineFulfillmentRequirement::Physical,
                Some(effective_shipping_profile_slug(
                    product.shipping_profile_slug.as_deref(),
                    &product.metadata,
                    variant.shipping_profile_slug.as_deref(),
                )),
            ),
        };
        if line.fulfillment_requirement != requirement || line.shipping_profile_slug != profile {
            return Err(PortError::conflict(
                "commerce.cart_product_snapshot_changed",
                "Cart fulfillment snapshot changed; reconcile the line before changing quantity",
            ));
        }
        require_cart_inventory(
            self.inventory.as_ref(),
            read_context,
            &cart,
            variant_id,
            quantity,
            Some(line_item_id),
        )
        .await?;
        let (unit_price, pricing_adjustment) = cart_line_pricing_snapshot(quantity, &price);
        self.cart
            .update_storefront_line_item_pricing(
                context,
                CartStorefrontLineItemPricingRequest {
                    cart_id: cart.id,
                    line_item_id,
                    quantity,
                    unit_price,
                    pricing_adjustment,
                    pricing_context,
                },
            )
            .await
            .map_err(|error| dependency_error("cart", error))
    }
}

fn require_active_cart(cart: &CartResponse) -> Result<(), PortError> {
    if cart.lifecycle_status().map_err(|_| {
        PortError::invariant_violation(
            "commerce.cart_lifecycle_invalid",
            "Cart lifecycle is invalid",
        )
    })? != rustok_cart::CartStatus::Active
    {
        return Err(PortError::conflict(
            "commerce.cart_inactive",
            "Cart is not active; inspect it before changing it",
        ));
    }
    Ok(())
}

/// Stock is an Inventory-owned observation, not Product's global descriptive quantity and
/// not a hold. Existing same-SKU lines count toward demand without merging their snapshots;
/// the Pricing quote remains for the new line quantity, not an invented aggregate tier.
async fn require_cart_inventory(
    inventory: &dyn InventoryReservationPort,
    context: PortContext,
    cart: &CartResponse,
    variant_id: Uuid,
    additional_quantity: i32,
    replaced_line: Option<Uuid>,
) -> Result<(), PortError> {
    if let Some(id) = replaced_line {
        if cart
            .line_items
            .iter()
            .filter(|line| line.id == id && line.variant_id == Some(variant_id))
            .count()
            != 1
        {
            return Err(PortError::validation(
                "commerce.cart_inventory_line_invalid",
                "Inventory observation requires the original Cart line and SKU",
            ));
        }
    }
    let quantity = cart
        .line_items
        .iter()
        .filter(|line| line.variant_id == Some(variant_id) && Some(line.id) != replaced_line)
        .try_fold(additional_quantity, |quantity, line| {
            if line.quantity <= 0 || quantity <= 0 {
                None
            } else {
                quantity.checked_add(line.quantity)
            }
        })
        .filter(|quantity| *quantity > 0)
        .ok_or_else(|| {
            PortError::validation(
                "commerce.cart_inventory_quantity_invalid",
                "Combined SKU quantity is invalid or exceeds inventory limits; inspect the cart before retrying",
            )
        })?;
    let request = InventoryAvailabilityRequest {
        variant_id,
        requested_quantity: quantity,
        channel_slug: context.channel.clone(),
    };
    context.require_policy(rustok_api::PortCallPolicy::read())?;
    let budget = std::time::Duration::from_millis(context.deadline_ms.unwrap_or(2000).min(2000));
    // Bound caller waiting; dropping this future does not prove database cancellation.
    let availability = tokio::time::timeout(budget, inventory.check_availability(context, request))
        .await
        .map_err(|_| {
            PortError::timeout(
                "commerce.cart_inventory_timeout",
                "Inventory verification exceeded its deadline; refresh the cart before retrying",
            )
        })?
        .map_err(|error| dependency_error("inventory", error))?;
    crate::services::checkout_catalog::validate_inventory_observation(
        &availability,
        variant_id,
        quantity,
    )
    .map_err(|error| dependency_error("inventory", error))?;
    if !availability.available {
        return Err(PortError::conflict(
            "commerce.cart_inventory_insufficient",
            "The combined SKU quantity is not available for the cart channel; reduce the quantity or refresh",
        ));
    }
    Ok(())
}

fn verify_buyer_context(
    expected: Option<&PriceResolutionContext>,
    actual: &PriceResolutionContext,
) -> Result<(), PortError> {
    if let Some(expected) = expected {
        if !expected
            .currency_code
            .trim()
            .eq_ignore_ascii_case(&actual.currency_code)
            || expected.region_id != actual.region_id
            || expected.price_list_id != actual.price_list_id
            || expected.channel_id != actual.channel_id
            || expected
                .channel_slug
                .as_deref()
                .map(str::trim)
                .map(str::to_ascii_lowercase)
                != actual
                    .channel_slug
                    .as_deref()
                    .map(str::trim)
                    .map(str::to_ascii_lowercase)
            || expected.quantity != actual.quantity
        {
            return Err(PortError::conflict(
                "commerce.cart_pricing_context_mismatch",
                "Cart context differs from the displayed quote; use the cart context and request a new quote",
            ));
        }
    }
    Ok(())
}

fn sanitized_line_metadata(metadata: Value) -> Value {
    let mut object = metadata.as_object().cloned().unwrap_or_default();
    for key in [
        "customer_tax_exempt",
        "tax_class",
        "tax_rate",
        "seller_id",
        "seller",
        "marketplace",
        "channel_tax_provider_ids",
        "pricing_adjustment",
        "pricing",
        "discounts",
    ] {
        object.remove(key);
    }
    Value::Object(object)
}

fn dependency_error(owner: &'static str, error: PortError) -> PortError {
    // Retain typed classification and bounded local evidence, never raw storage/owner messages.
    tracing::warn!(owner, kind = ?error.kind, retryable = error.retryable,
        code_length = error.code.len(), boundary = "commerce_storefront_cart_add",
        "storefront cart dependency refused the operation");
    let message = match error.kind {
        rustok_api::PortErrorKind::Conflict => {
            "Cart or stock context changed; refresh before adding the SKU"
        }
        rustok_api::PortErrorKind::Forbidden => {
            "Cart access was refused; reopen the cart in the current session"
        }
        rustok_api::PortErrorKind::NotFound => {
            "The cart or selected SKU/price is no longer available"
        }
        rustok_api::PortErrorKind::Validation => "The cart or pricing request was refused",
        _ => "Adding to cart could not be confirmed; inspect the cart before trying again",
    };
    PortError::new(
        error.kind,
        format!("commerce.cart_{owner}_refused"),
        message,
        error.retryable,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> PriceResolutionContext {
        PriceResolutionContext {
            currency_code: "EUR".into(),
            region_id: Some(Uuid::new_v4()),
            price_list_id: None,
            channel_id: Some(Uuid::new_v4()),
            channel_slug: Some("web".into()),
            quantity: Some(3),
        }
    }

    #[test]
    fn exact_buyer_context_does_not_treat_null_selectors_as_wildcards() {
        let actual = context();
        assert!(verify_buyer_context(Some(&actual), &actual).is_ok());
        let mut expected = actual.clone();
        expected.region_id = None;
        assert_eq!(
            verify_buyer_context(Some(&expected), &actual)
                .unwrap_err()
                .kind,
            rustok_api::PortErrorKind::Conflict
        );
        expected = actual.clone();
        expected.channel_id = None;
        assert!(verify_buyer_context(Some(&expected), &actual).is_err());
        expected = actual.clone();
        expected.quantity = Some(1);
        assert!(verify_buyer_context(Some(&expected), &actual).is_err());
        expected = actual.clone();
        expected.price_list_id = Some(Uuid::new_v4());
        assert!(verify_buyer_context(Some(&expected), &actual).is_err());
    }

    #[test]
    fn buyer_metadata_cannot_supply_seller_or_pricing_authority() {
        let metadata = sanitized_line_metadata(json!({ "seller": { "id": "forged" },
            "pricing": { "amount": "0" }, "customer_tax_exempt": true, "note": "gift" }));
        assert_eq!(metadata, json!({ "note": "gift" }));
    }
}

#[cfg(test)]
#[path = "storefront_cart_inventory_tests.rs"]
mod inventory_tests;
