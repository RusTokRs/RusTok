pub mod carts;
pub mod checkout;
pub mod orders;
pub mod products;

pub use carts::*;
pub use checkout::*;
pub use orders::*;
pub use products::*;

#[cfg(test)]
mod tests;

use loco_rs::{app::AppContext, controller::Routes, Error, Result};
use rust_decimal::Decimal;
use rustok_api::{loco::transactional_event_bus_from_context, RequestContext};
use rustok_cart::CartError;
use rustok_cart::CartService;
use rustok_core::locale_tags_match;
use rustok_customer::CustomerService;
use rustok_fulfillment::FulfillmentService;
use rustok_inventory::{check_variant_availability_for_public_channel, InventoryReservationPort};
use rustok_order::OrderService;
use rustok_pricing::{PriceResolutionContext, PricingService};
use rustok_product::entities::{
    product, product_translation, product_variant, variant_translation,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::common::PaginationParams;
use crate::{
    dto::{
        AddCartLineItemInput, CartResponse, ResolveStoreContextInput, StoreContextResponse,
        UpdateCartContextInput,
    },
    storefront_channel::{
        is_metadata_visible_for_public_channel, is_module_enabled_for_request_channel,
        normalize_public_channel_slug, public_channel_slug_from_request,
    },
    storefront_shipping::{
        effective_shipping_profile_slug, enrich_cart_delivery_groups,
        is_shipping_option_compatible_with_profiles, normalize_shipping_profile_slug,
    },
    StoreContextService,
};

pub const MODULE_SLUG: &str = "commerce";

pub fn routes() -> Routes {
    Routes::new()
        .add("/products", axum::routing::get(products::list_products))
        .add("/products/{id}", axum::routing::get(products::show_product))
        .add("/regions", axum::routing::get(products::list_regions))
        .add(
            "/shipping-options",
            axum::routing::get(products::list_shipping_options),
        )
        .add("/carts", axum::routing::post(carts::create_cart))
        .add(
            "/carts/{id}",
            axum::routing::get(carts::get_cart).post(carts::update_cart_context),
        )
        .add(
            "/carts/{id}/line-items",
            axum::routing::post(carts::add_cart_line_item),
        )
        .add(
            "/carts/{id}/line-items/{line_id}",
            axum::routing::post(carts::update_cart_line_item).delete(carts::remove_cart_line_item),
        )
        .add(
            "/carts/{id}/complete",
            axum::routing::post(checkout::complete_cart_checkout),
        )
        .add(
            "/payment-collections",
            axum::routing::post(checkout::create_payment_collection),
        )
        .add("/orders/{id}", axum::routing::get(orders::get_order))
        .add(
            "/orders/{id}/returns",
            axum::routing::get(orders::list_order_returns).post(orders::create_order_return),
        )
        .add(
            "/orders/{id}/refunds",
            axum::routing::get(orders::list_order_refunds),
        )
        .add(
            "/orders/{id}/changes",
            axum::routing::get(orders::list_order_changes),
        )
        .add("/customers/me", axum::routing::get(orders::get_me))
}

pub(crate) async fn resolve_context(
    ctx: &AppContext,
    tenant_id: Uuid,
    request_context: &RequestContext,
    region_id: Option<Uuid>,
    country_code: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
) -> Result<StoreContextResponse> {
    let service = StoreContextService::new(
        ctx.db.clone(),
        std::sync::Arc::new(rustok_region::RegionService::new(ctx.db.clone())),
    );
    service
        .resolve_context(
            tenant_id,
            ResolveStoreContextInput {
                region_id,
                country_code,
                locale: locale.or_else(|| Some(request_context.locale.clone())),
                currency_code,
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))
}

pub(crate) async fn resolve_context_from_cart(
    ctx: &AppContext,
    tenant_id: Uuid,
    request_context: &RequestContext,
    cart: &CartResponse,
) -> Result<StoreContextResponse> {
    resolve_context(
        ctx,
        tenant_id,
        request_context,
        cart.region_id,
        cart.country_code.clone(),
        cart.locale_code.clone(),
        Some(cart.currency_code.clone()),
    )
    .await
}

pub(crate) async fn ensure_customer_owns_order(
    ctx: &AppContext,
    tenant_id: Uuid,
    auth: Option<&rustok_api::AuthContext>,
    order_id: Uuid,
) -> Result<()> {
    let customer_id = current_customer_id(ctx, tenant_id, auth)
        .await?
        .ok_or_else(|| Error::Unauthorized("Customer account required".to_string()))?;
    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(ctx))
        .get_order(tenant_id, order_id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    if order.customer_id != Some(customer_id) {
        return Err(Error::Unauthorized(
            "Order does not belong to the current customer".to_string(),
        ));
    }

    Ok(())
}

pub(crate) async fn current_customer_id(
    ctx: &AppContext,
    tenant_id: Uuid,
    auth: Option<&rustok_api::AuthContext>,
) -> Result<Option<Uuid>> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    let service = CustomerService::new(ctx.db.clone());
    match service.get_customer_by_user(tenant_id, auth.user_id).await {
        Ok(customer) => Ok(Some(customer.id)),
        Err(rustok_customer::CustomerError::CustomerByUserNotFound(_)) => Ok(None),
        Err(err) => Err(Error::BadRequest(err.to_string())),
    }
}

pub(crate) async fn ensure_storefront_channel_enabled(
    ctx: &AppContext,
    request_context: &RequestContext,
) -> Result<()> {
    let enabled = is_module_enabled_for_request_channel(&ctx.db, request_context, MODULE_SLUG)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    if !enabled {
        return Err(Error::Unauthorized(format!(
            "Module '{MODULE_SLUG}' is not enabled for channel '{}'",
            request_context.channel_slug.as_deref().unwrap_or("current"),
        )));
    }

    Ok(())
}

pub(crate) fn storefront_public_channel_slug_for_cart(
    cart: &CartResponse,
    request_context: &RequestContext,
) -> Option<String> {
    normalize_public_channel_slug(cart.channel_slug.as_deref())
        .or_else(|| public_channel_slug_from_request(request_context))
}

pub(crate) fn ensure_store_cart_access(
    cart: &CartResponse,
    customer_id: Option<Uuid>,
) -> Result<()> {
    if let Some(expected_customer_id) = cart.customer_id {
        if customer_id != Some(expected_customer_id) {
            return Err(Error::Unauthorized(
                "Cart belongs to another customer".to_string(),
            ));
        }
    }

    Ok(())
}

pub(crate) fn ensure_cart_allows_payment_collection(cart: &CartResponse) -> Result<()> {
    if cart.status == "completed" {
        return Err(Error::BadRequest(
            "Cannot create payment collection for completed cart".to_string(),
        ));
    }

    Ok(())
}

pub(crate) fn checkout_actor_id(auth: Option<&rustok_api::AuthContext>) -> Uuid {
    auth.map(|auth| auth.user_id).unwrap_or_else(Uuid::nil)
}

pub(crate) async fn apply_cart_context_patch(
    ctx: &AppContext,
    tenant_id: Uuid,
    request_context: &RequestContext,
    tenant_default_locale: &str,
    cart: &CartResponse,
    patch: StoreCartContextPatch,
) -> Result<StoreCartResponse> {
    let requested = requested_cart_context(cart, request_context, patch);

    let context = resolve_context(
        ctx,
        tenant_id,
        request_context,
        requested.region_id,
        requested.country_code.clone(),
        requested.locale,
        Some(cart.currency_code.clone()),
    )
    .await?;

    validate_selected_shipping_option(
        ctx,
        tenant_id,
        cart,
        requested.selected_shipping_option_id,
        Some(requested.shipping_selections.as_slice()),
        &cart.currency_code,
        storefront_public_channel_slug_for_cart(cart, request_context).as_deref(),
        Some(request_context.locale.as_str()),
        Some(tenant_default_locale),
    )
    .await?;

    let cart_service = CartService::new(ctx.db.clone());
    let updated_cart = cart_service
        .update_context(
            tenant_id,
            cart.id,
            UpdateCartContextInput {
                email: requested.email,
                region_id: context.region.as_ref().map(|region| region.id),
                country_code: requested.country_code,
                locale_code: Some(context.locale.clone()),
                selected_shipping_option_id: requested.selected_shipping_option_id,
                shipping_selections: Some(requested.shipping_selections.clone()),
            },
        )
        .await
        .map_err(map_cart_error)?;
    let updated_cart = reprice_storefront_cart_line_items(
        ctx,
        tenant_id,
        request_context,
        &cart_service,
        updated_cart,
    )
    .await?;
    let updated_cart = enrich_storefront_cart(
        ctx,
        tenant_id,
        request_context,
        tenant_default_locale,
        updated_cart,
    )
    .await?;

    Ok(StoreCartResponse {
        cart: updated_cart,
        context,
    })
}

pub(crate) async fn reprice_storefront_cart_line_items(
    ctx: &AppContext,
    tenant_id: Uuid,
    request_context: &RequestContext,
    cart_service: &CartService,
    cart: CartResponse,
) -> Result<CartResponse> {
    if cart.line_items.is_empty() {
        return Ok(cart);
    }

    let pricing_service =
        PricingService::new(ctx.db.clone(), transactional_event_bus_from_context(ctx));
    let mut updates = Vec::new();
    for line_item in &cart.line_items {
        let Some(variant_id) = line_item.variant_id else {
            continue;
        };
        let pricing_context =
            build_store_pricing_context(&cart, request_context, line_item.quantity);
        let resolved_price = pricing_service
            .resolve_variant_price(tenant_id, variant_id, pricing_context)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?
            .ok_or_else(|| {
                Error::BadRequest(format!(
                    "No storefront price for variant {} in currency {}",
                    variant_id, cart.currency_code
                ))
            })?;
        updates.push(storefront_cart_pricing_update(
            line_item.id,
            line_item.quantity,
            &resolved_price,
        ));
    }

    if updates.is_empty() {
        Ok(cart)
    } else {
        cart_service
            .reprice_line_items(tenant_id, cart.id, updates)
            .await
            .map_err(map_cart_error)
    }
}

pub(crate) fn storefront_cart_pricing_update(
    line_item_id: Uuid,
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedPrice,
) -> rustok_cart::services::cart::CartLineItemPricingUpdate {
    let (base_unit_price, pricing_adjustment) =
        storefront_cart_pricing_snapshot(quantity, resolved_price);

    rustok_cart::services::cart::CartLineItemPricingUpdate {
        line_item_id,
        unit_price: base_unit_price,
        pricing_adjustment,
    }
}

pub(crate) fn storefront_cart_pricing_snapshot(
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedPrice,
) -> (
    Decimal,
    Option<rustok_cart::services::cart::CartPricingAdjustmentUpdate>,
) {
    let base_unit_price = resolved_price
        .compare_at_amount
        .filter(|compare_at| *compare_at > resolved_price.amount)
        .unwrap_or(resolved_price.amount);
    let pricing_adjustment = if base_unit_price > resolved_price.amount {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "kind".to_string(),
            Value::from(if resolved_price.price_list_id.is_some() {
                "price_list"
            } else {
                "sale"
            }),
        );
        metadata.insert(
            "base_amount".to_string(),
            Value::from(base_unit_price.normalize().to_string()),
        );
        metadata.insert(
            "effective_amount".to_string(),
            Value::from(resolved_price.amount.normalize().to_string()),
        );
        if let Some(compare_at_amount) = resolved_price.compare_at_amount {
            metadata.insert(
                "compare_at_amount".to_string(),
                Value::from(compare_at_amount.normalize().to_string()),
            );
        }
        if let Some(discount_percent) = resolved_price.discount_percent {
            metadata.insert(
                "discount_percent".to_string(),
                Value::from(discount_percent.normalize().to_string()),
            );
        }
        if let Some(price_list_id) = resolved_price.price_list_id {
            metadata.insert(
                "price_list_id".to_string(),
                Value::from(price_list_id.to_string()),
            );
        }
        if let Some(channel_id) = resolved_price.channel_id {
            metadata.insert(
                "channel_id".to_string(),
                Value::from(channel_id.to_string()),
            );
        }
        if let Some(channel_slug) = resolved_price.channel_slug.as_deref() {
            metadata.insert("channel_slug".to_string(), Value::from(channel_slug));
        }

        Some(rustok_cart::services::cart::CartPricingAdjustmentUpdate {
            source_id: resolved_price.price_list_id.map(|value| value.to_string()),
            amount: (base_unit_price - resolved_price.amount) * Decimal::from(quantity),
            metadata: Value::Object(metadata),
        })
    } else {
        None
    };

    (base_unit_price, pricing_adjustment)
}

#[derive(Debug)]
pub(crate) struct ResolvedStoreLineItemInput {
    pub(crate) add_line_item: AddCartLineItemInput,
    pub(crate) pricing_adjustment: Option<rustok_cart::services::cart::CartPricingAdjustmentUpdate>,
}

pub(crate) async fn enrich_storefront_cart(
    ctx: &AppContext,
    tenant_id: Uuid,
    request_context: &RequestContext,
    tenant_default_locale: &str,
    cart: CartResponse,
) -> Result<CartResponse> {
    let public_channel_slug = storefront_public_channel_slug_for_cart(&cart, request_context);
    enrich_cart_delivery_groups(
        &ctx.db,
        tenant_id,
        cart,
        public_channel_slug.as_deref(),
        Some(request_context.locale.as_str()),
        Some(tenant_default_locale),
    )
    .await
    .map_err(|err| Error::BadRequest(err.to_string()))
}

pub(crate) fn requested_cart_context(
    cart: &CartResponse,
    request_context: &RequestContext,
    patch: StoreCartContextPatch,
) -> RequestedCartContext {
    let region_was_explicit = patch.region_id.is_some();

    RequestedCartContext {
        email: patch.email.unwrap_or_else(|| cart.email.clone()),
        region_id: patch.region_id.unwrap_or(cart.region_id),
        country_code: match patch.country_code {
            Some(country_code) => country_code,
            None if region_was_explicit => None,
            None => cart.country_code.clone(),
        },
        locale: patch
            .locale
            .unwrap_or_else(|| cart.locale_code.clone())
            .or_else(|| Some(request_context.locale.clone())),
        selected_shipping_option_id: patch
            .selected_shipping_option_id
            .unwrap_or(cart.selected_shipping_option_id),
        shipping_selections: patch
            .shipping_selections
            .unwrap_or_else(|| current_shipping_selections(cart)),
    }
}

pub(crate) async fn validate_selected_shipping_option(
    ctx: &AppContext,
    tenant_id: Uuid,
    cart: &CartResponse,
    selected_shipping_option_id: Option<Uuid>,
    shipping_selections: Option<&[crate::dto::CartShippingSelectionInput]>,
    currency_code: &str,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> Result<()> {
    let service = FulfillmentService::new(ctx.db.clone());
    let selections = if let Some(shipping_selections) = shipping_selections {
        shipping_selections.to_vec()
    } else if let Some(selected_shipping_option_id) = selected_shipping_option_id {
        if cart.delivery_groups.len() > 1 {
            return Err(Error::BadRequest(
                "selected_shipping_option_id can only be used for carts with a single delivery group"
                    .to_string(),
            ));
        }
        cart.delivery_groups
            .first()
            .map(|group| {
                vec![crate::dto::CartShippingSelectionInput {
                    shipping_profile_slug: group.shipping_profile_slug.clone(),
                    seller_id: group.seller_id.clone(),
                    seller_scope: None,
                    selected_shipping_option_id: Some(selected_shipping_option_id),
                }]
            })
            .unwrap_or_default()
    } else {
        current_shipping_selections(cart)
    };

    for selection in selections {
        let Some(selected_shipping_option_id) = selection.selected_shipping_option_id else {
            continue;
        };
        let required_shipping_profiles = BTreeSet::from([normalize_shipping_profile_slug(
            selection.shipping_profile_slug.as_str(),
        )
        .unwrap_or_else(|| "default".to_string())]);
        let option = service
            .get_shipping_option(
                tenant_id,
                selected_shipping_option_id,
                requested_locale,
                tenant_default_locale,
            )
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?;
        if !option.currency_code.eq_ignore_ascii_case(currency_code) {
            return Err(Error::BadRequest(format!(
                "Shipping option {} uses currency {}, expected {}",
                option.id, option.currency_code, currency_code
            )));
        }
        if !is_metadata_visible_for_public_channel(&option.metadata, public_channel_slug) {
            return Err(Error::BadRequest(format!(
                "Shipping option {} is not available for the current channel",
                option.id
            )));
        }
        if !is_shipping_option_compatible_with_profiles(&option, &required_shipping_profiles) {
            return Err(Error::BadRequest(format!(
                "Shipping option {} is not compatible with shipping profile {}",
                option.id, selection.shipping_profile_slug
            )));
        }
    }

    Ok(())
}

pub(crate) fn current_shipping_selections(
    cart: &CartResponse,
) -> Vec<crate::dto::CartShippingSelectionInput> {
    cart.delivery_groups
        .iter()
        .map(|group| crate::dto::CartShippingSelectionInput {
            shipping_profile_slug: group.shipping_profile_slug.clone(),
            seller_id: group.seller_id.clone(),
            seller_scope: None,
            selected_shipping_option_id: group.selected_shipping_option_id,
        })
        .collect()
}

pub(crate) fn build_store_pricing_context(
    cart: &CartResponse,
    request_context: &RequestContext,
    quantity: i32,
) -> PriceResolutionContext {
    PriceResolutionContext {
        currency_code: cart.currency_code.to_ascii_uppercase(),
        region_id: cart.region_id,
        price_list_id: None,
        channel_id: cart.channel_id.or(request_context.channel_id),
        channel_slug: storefront_public_channel_slug_for_cart(cart, request_context),
        quantity: Some(quantity),
    }
}

pub(crate) async fn resolve_store_line_item_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    inventory_port: &dyn InventoryReservationPort,
    pricing_service: &PricingService,
    pricing_context: &PriceResolutionContext,
    locale: &str,
    default_locale: &str,
    public_channel_slug: Option<&str>,
    input: StoreAddCartLineItemInput,
) -> Result<ResolvedStoreLineItemInput> {
    let variant = product_variant::Entity::find_by_id(input.variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or(Error::NotFound)?;

    let product_model = product::Entity::find_by_id(variant.product_id)
        .filter(product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or(Error::NotFound)?;
    if product_model.status != product::ProductStatus::Active
        || product_model.published_at.is_none()
        || !is_metadata_visible_for_public_channel(&product_model.metadata, public_channel_slug)
    {
        return Err(Error::NotFound);
    }

    let product_translation_models = product_translation::Entity::find()
        .filter(product_translation::Column::ProductId.eq(product_model.id))
        .all(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    let variant_translation_models = variant_translation::Entity::find()
        .filter(variant_translation::Column::VariantId.eq(variant.id))
        .all(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let resolved_price = pricing_service
        .resolve_variant_price(tenant_id, variant.id, pricing_context.clone())
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or_else(|| {
            Error::BadRequest(format!(
                "No storefront price for variant {} in currency {}",
                variant.id, pricing_context.currency_code
            ))
        })?;
    let (base_unit_price, pricing_adjustment) =
        storefront_cart_pricing_snapshot(input.quantity, &resolved_price);
    validate_store_variant_inventory(
        inventory_port,
        db,
        tenant_id,
        &variant,
        input.quantity,
        public_channel_slug,
        locale,
    )
    .await?;

    let base_title = pick_product_translation(&product_translation_models, locale, default_locale)
        .map(|translation| translation.title.clone())
        .unwrap_or_else(|| {
            variant
                .sku
                .clone()
                .unwrap_or_else(|| format!("Variant {}", variant.id))
        });
    let title = match pick_variant_translation(&variant_translation_models, locale, default_locale)
        .and_then(|translation| translation.title.clone())
    {
        Some(variant_title) if !variant_title.trim().is_empty() => {
            format!("{base_title} / {}", variant_title.trim())
        }
        _ => base_title,
    };

    Ok(ResolvedStoreLineItemInput {
        add_line_item: AddCartLineItemInput {
            product_id: Some(product_model.id),
            variant_id: Some(variant.id),
            shipping_profile_slug: Some(effective_shipping_profile_slug(
                product_model.shipping_profile_slug.as_deref(),
                &product_model.metadata,
                variant.shipping_profile_slug.as_deref(),
            )),
            sku: variant.sku.clone(),
            title,
            quantity: input.quantity,
            unit_price: base_unit_price,
            metadata: merge_metadata(
                input.metadata,
                seller_snapshot_metadata(product_model.seller_id.as_deref()),
            ),
        },
        pricing_adjustment,
    })
}

pub(crate) fn pick_product_translation<'a>(
    translations: &'a [product_translation::Model],
    locale: &str,
    default_locale: &str,
) -> Option<&'a product_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(default_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, default_locale))
            })?
        })
        .or_else(|| translations.first())
}

pub(crate) fn pick_variant_translation<'a>(
    translations: &'a [variant_translation::Model],
    locale: &str,
    default_locale: &str,
) -> Option<&'a variant_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(default_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, default_locale))
            })?
        })
        .or_else(|| translations.first())
}

pub(crate) async fn validate_store_line_item_quantity(
    inventory_port: &dyn InventoryReservationPort,
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
    locale: &str,
) -> Result<()> {
    validate_store_variant_inventory(
        inventory_port,
        db,
        tenant_id,
        &product_variant::Entity::find_by_id(variant_id)
            .filter(product_variant::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?
            .ok_or(Error::NotFound)?,
        requested_quantity,
        public_channel_slug,
        locale,
    )
    .await
}

pub(crate) async fn validate_store_variant_inventory(
    _inventory_port: &dyn InventoryReservationPort,
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant: &product_variant::Model,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
    _locale: &str,
) -> Result<()> {
    let available = check_variant_availability_for_public_channel(
        db,
        tenant_id,
        variant,
        requested_quantity,
        public_channel_slug,
    )
    .await
    .map_err(|error| {
        Error::BadRequest(format!("store-cart-inventory:{}: {}", variant.id, error))
    })?;
    if !available {
        return Err(Error::BadRequest(format!(
            "Variant {} does not have enough available inventory for the current channel",
            variant.id
        )));
    }

    Ok(())
}

pub(crate) fn map_cart_error(error: CartError) -> Error {
    match error {
        CartError::CartNotFound(_) | CartError::CartLineItemNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

pub(crate) fn default_metadata() -> Value {
    json!({})
}

pub(crate) fn deserialize_patch_field<'de, D, T>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

pub(crate) fn merge_metadata(current: Value, patch: Value) -> Value {
    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, patch) => patch,
    }
}

pub(crate) fn normalize_store_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

pub(crate) fn seller_snapshot_metadata(seller_id: Option<&str>) -> Value {
    let seller_id = normalize_store_seller_id(seller_id);

    json!({
        "seller": {
            "id": seller_id,
        }
    })
}

pub(crate) fn cart_context_metadata(cart: &CartResponse, context: &StoreContextResponse) -> Value {
    json!({
        "cart_context": {
            "channel_id": cart.channel_id,
            "channel_slug": cart.channel_slug.clone(),
            "region_id": context.region.as_ref().map(|region| region.id),
            "country_code": cart.country_code.clone(),
            "locale": context.locale.clone(),
            "currency_code": cart.currency_code.clone(),
            "selected_shipping_option_id": cart.selected_shipping_option_id,
            "shipping_selections": current_shipping_selections(cart),
            "customer_id": cart.customer_id,
            "email": cart.email.clone(),
        }
    })
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct StoreListProductsParams {
    #[serde(flatten)]
    pub pagination: Option<PaginationParams>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub search: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
pub struct StoreOrderReturnsParams {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
pub struct StoreOrderRefundsParams {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
pub struct StoreOrderChangesParams {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub status: Option<String>,
    pub change_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
pub struct StoreContextQuery {
    pub cart_id: Option<Uuid>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    pub currency_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreCreateCartInput {
    pub email: Option<String>,
    pub currency_code: Option<String>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StoreCartResponse {
    pub cart: CartResponse,
    pub context: StoreContextResponse,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreUpdateCartInput {
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub email: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub region_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub country_code: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub locale: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub selected_shipping_option_id: Option<Option<Uuid>>,
    #[serde(default)]
    pub shipping_selections: Option<Vec<StoreCartShippingSelectionInput>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreCreatePaymentCollectionInput {
    pub cart_id: Uuid,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreCompleteCartInput {
    pub shipping_option_id: Option<Uuid>,
    pub shipping_selections: Option<Vec<StoreCartShippingSelectionInput>>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    #[serde(default = "default_true")]
    pub create_fulfillment: bool,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreAddCartLineItemInput {
    pub variant_id: Uuid,
    pub quantity: i32,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreUpdateCartLineItemInput {
    pub quantity: i32,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub(crate) struct StoreCartContextPatch {
    pub(crate) email: Option<Option<String>>,
    pub(crate) region_id: Option<Option<Uuid>>,
    pub(crate) country_code: Option<Option<String>>,
    pub(crate) locale: Option<Option<String>>,
    pub(crate) selected_shipping_option_id: Option<Option<Uuid>>,
    pub(crate) shipping_selections: Option<Vec<crate::dto::CartShippingSelectionInput>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequestedCartContext {
    pub(crate) email: Option<String>,
    pub(crate) region_id: Option<Uuid>,
    pub(crate) country_code: Option<String>,
    pub(crate) locale: Option<String>,
    pub(crate) selected_shipping_option_id: Option<Uuid>,
    pub(crate) shipping_selections: Vec<crate::dto::CartShippingSelectionInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct StoreCartShippingSelectionInput {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
}

impl From<StoreCartShippingSelectionInput> for crate::dto::CartShippingSelectionInput {
    fn from(value: StoreCartShippingSelectionInput) -> Self {
        Self {
            shipping_profile_slug: value.shipping_profile_slug,
            seller_id: value.seller_id,
            seller_scope: None,
            selected_shipping_option_id: value.selected_shipping_option_id,
        }
    }
}
