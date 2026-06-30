use loco_rs::app::AppContext;
use rustok_api::{OptionalAuthContext, RequestContext, TenantContext};
use serde_json::{json, Value};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct StorefrontPaymentCollectionCommand {
    pub cart_id: Uuid,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct StorefrontShippingSelectionUpdateInput {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub struct StorefrontShippingSelectionCommand {
    pub cart_id: Uuid,
    pub shipping_selections: Vec<StorefrontShippingSelectionUpdateInput>,
}

#[derive(Clone, Debug)]
pub struct StorefrontCheckoutCompletionCommand {
    pub cart_id: Uuid,
    pub create_fulfillment: bool,
    pub metadata: Value,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct StorefrontCheckoutRuntimeError {
    message: String,
}

impl StorefrontCheckoutRuntimeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub async fn read_storefront_payment_collection(
    app_ctx: &AppContext,
    tenant: &TenantContext,
    auth: OptionalAuthContext,
    cart_id: Uuid,
) -> Result<Option<rustok_payment::dto::PaymentCollectionResponse>, StorefrontCheckoutRuntimeError>
{
    let cart = rustok_cart::CartService::new(app_ctx.db.clone())
        .get_cart(tenant.id, cart_id)
        .await
        .map_err(runtime_error)?;
    let storefront_customer_id =
        resolve_storefront_customer_id(app_ctx.db.clone(), tenant.id, auth.0).await?;
    ensure_storefront_cart_access(&cart, storefront_customer_id)?;

    rustok_payment::PaymentService::new(app_ctx.db.clone())
        .find_reusable_collection_by_cart(tenant.id, cart.id)
        .await
        .map_err(runtime_error)
}

pub async fn read_storefront_order_refunds(
    app_ctx: &AppContext,
    tenant: &TenantContext,
    request_context: &RequestContext,
    auth: OptionalAuthContext,
    order_id: Uuid,
) -> Result<(Vec<rustok_payment::dto::RefundResponse>, u64), StorefrontCheckoutRuntimeError> {
    let auth = auth.0.ok_or_else(|| {
        StorefrontCheckoutRuntimeError::new("Authentication required to access order refunds")
    })?;
    let customer = rustok_customer::CustomerService::new(app_ctx.db.clone())
        .get_customer_by_user(tenant.id, auth.user_id)
        .await
        .map_err(runtime_error)?;
    let event_bus = rustok_api::loco::transactional_event_bus_from_context(app_ctx);
    let order = match rustok_order::OrderService::new(app_ctx.db.clone(), event_bus)
        .get_order_with_locale_fallback(
            tenant.id,
            order_id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
    {
        Ok(order) => order,
        Err(rustok_order::OrderError::OrderNotFound(_)) => return Ok((Vec::new(), 0)),
        Err(error) => return Err(runtime_error(error)),
    };
    if order.customer_id != Some(customer.id) {
        return Err(StorefrontCheckoutRuntimeError::new(
            "Order does not belong to the current storefront customer",
        ));
    }

    rustok_payment::PaymentService::new(app_ctx.db.clone())
        .list_refunds(
            tenant.id,
            rustok_payment::dto::ListRefundsInput {
                page: 1,
                per_page: 50,
                payment_collection_id: None,
                order_id: Some(order_id),
                status: None,
            },
        )
        .await
        .map_err(runtime_error)
}

pub async fn create_storefront_payment_collection(
    app_ctx: &AppContext,
    tenant: &TenantContext,
    request_context: &RequestContext,
    auth: OptionalAuthContext,
    command: StorefrontPaymentCollectionCommand,
) -> Result<rustok_payment::dto::PaymentCollectionResponse, StorefrontCheckoutRuntimeError> {
    let cart_service = rustok_cart::CartService::new(app_ctx.db.clone());
    let cart = cart_service
        .get_cart(tenant.id, command.cart_id)
        .await
        .map_err(runtime_error)?;
    let storefront_customer_id =
        resolve_storefront_customer_id(app_ctx.db.clone(), tenant.id, auth.0).await?;
    ensure_storefront_cart_access(&cart, storefront_customer_id)?;
    let cart = reprice_storefront_cart_line_items(
        app_ctx,
        tenant.id,
        &cart_service,
        cart,
        Some(request_context),
    )
    .await?;

    let service = rustok_payment::PaymentService::new(app_ctx.db.clone());
    if let Some(existing) = service
        .find_reusable_collection_by_cart(tenant.id, cart.id)
        .await
        .map_err(runtime_error)?
    {
        return Ok(existing);
    }

    let context = crate::StoreContextService::new(
        app_ctx.db.clone(),
        std::sync::Arc::new(rustok_region::RegionService::new(app_ctx.db.clone())),
    )
    .resolve_context(
        tenant.id,
        crate::dto::ResolveStoreContextInput {
            region_id: cart.region_id,
            country_code: cart.country_code.clone(),
            locale: Some(resolve_requested_locale(
                cart.locale_code.clone(),
                Some(request_context.locale.as_str()),
                tenant.default_locale.as_str(),
            )),
            currency_code: Some(cart.currency_code.clone()),
        },
    )
    .await
    .map_err(runtime_error)?;

    service
        .create_collection(
            tenant.id,
            rustok_payment::dto::CreatePaymentCollectionInput {
                cart_id: Some(cart.id),
                order_id: None,
                customer_id: cart.customer_id,
                currency_code: cart.currency_code.clone(),
                amount: cart.total_amount,
                metadata: merge_metadata(command.metadata, cart_context_metadata(&cart, &context)),
            },
        )
        .await
        .map_err(runtime_error)
}

pub async fn select_storefront_shipping_option(
    app_ctx: &AppContext,
    tenant: &TenantContext,
    request_context: Option<&RequestContext>,
    auth: OptionalAuthContext,
    command: StorefrontShippingSelectionCommand,
) -> Result<(), StorefrontCheckoutRuntimeError> {
    let cart_service = rustok_cart::CartService::new(app_ctx.db.clone());
    let cart = cart_service
        .get_cart(tenant.id, command.cart_id)
        .await
        .map_err(runtime_error)?;
    let storefront_customer_id =
        resolve_storefront_customer_id(app_ctx.db.clone(), tenant.id, auth.0).await?;
    ensure_storefront_cart_access(&cart, storefront_customer_id)?;

    let shipping_selections = command
        .shipping_selections
        .into_iter()
        .map(|selection| rustok_cart::dto::CartShippingSelectionInput {
            shipping_profile_slug: selection.shipping_profile_slug,
            seller_id: selection.seller_id,
            seller_scope: None,
            selected_shipping_option_id: selection.selected_shipping_option_id,
        })
        .collect::<Vec<_>>();

    let updated_cart = cart_service
        .update_context(
            tenant.id,
            command.cart_id,
            rustok_cart::dto::UpdateCartContextInput {
                email: cart.email.clone(),
                region_id: cart.region_id,
                country_code: cart.country_code.clone(),
                locale_code: cart.locale_code.clone(),
                selected_shipping_option_id: None,
                shipping_selections: Some(shipping_selections),
            },
        )
        .await
        .map_err(runtime_error)?;
    let _ = reprice_storefront_cart_line_items(
        app_ctx,
        tenant.id,
        &cart_service,
        updated_cart,
        request_context,
    )
    .await?;

    Ok(())
}

pub async fn complete_storefront_checkout(
    app_ctx: &AppContext,
    tenant: &TenantContext,
    request_context: &RequestContext,
    auth: OptionalAuthContext,
    command: StorefrontCheckoutCompletionCommand,
) -> Result<crate::dto::CompleteCheckoutResponse, StorefrontCheckoutRuntimeError> {
    let auth_context = auth.0;
    let cart_service = rustok_cart::CartService::new(app_ctx.db.clone());
    let cart = cart_service
        .get_cart(tenant.id, command.cart_id)
        .await
        .map_err(runtime_error)?;
    let storefront_customer_id =
        resolve_storefront_customer_id(app_ctx.db.clone(), tenant.id, auth_context.clone()).await?;
    ensure_storefront_cart_access(&cart, storefront_customer_id)?;
    let _ = reprice_storefront_cart_line_items(
        app_ctx,
        tenant.id,
        &cart_service,
        cart,
        Some(request_context),
    )
    .await?;
    let actor_id = auth_context
        .map(|auth| auth.user_id)
        .unwrap_or_else(Uuid::nil);

    let event_bus = rustok_api::loco::transactional_event_bus_from_context(app_ctx);
    crate::CheckoutService::new(
        app_ctx.db.clone(),
        event_bus.clone(),
        std::sync::Arc::new(rustok_region::RegionService::new(app_ctx.db.clone())),
        std::sync::Arc::new(rustok_inventory::InventoryService::new(
            app_ctx.db.clone(),
            event_bus,
        )),
    )
    .complete_checkout(
        tenant.id,
        actor_id,
        crate::dto::CompleteCheckoutInput {
            cart_id: command.cart_id,
            shipping_option_id: None,
            shipping_selections: None,
            region_id: None,
            country_code: None,
            locale: None,
            create_fulfillment: command.create_fulfillment,
            metadata: command.metadata,
        },
    )
    .await
    .map_err(runtime_error)
}

async fn resolve_storefront_customer_id(
    db: sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    auth: Option<rustok_api::AuthContext>,
) -> Result<Option<Uuid>, StorefrontCheckoutRuntimeError> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    match rustok_customer::CustomerService::new(db)
        .get_customer_by_user(tenant_id, auth.user_id)
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(rustok_customer::CustomerError::CustomerByUserNotFound(_)) => Ok(None),
        Err(err) => Err(runtime_error(err)),
    }
}

fn ensure_storefront_cart_access(
    cart: &rustok_cart::dto::CartResponse,
    storefront_customer_id: Option<Uuid>,
) -> Result<(), StorefrontCheckoutRuntimeError> {
    if let Some(owner_customer_id) = cart.customer_id {
        match storefront_customer_id {
            Some(customer_id) if customer_id == owner_customer_id => Ok(()),
            Some(_) => Err(StorefrontCheckoutRuntimeError::new(
                "Cart does not belong to the current storefront customer",
            )),
            None => Err(StorefrontCheckoutRuntimeError::new(
                "Authentication required to access this cart",
            )),
        }
    } else {
        Ok(())
    }
}

fn merge_metadata(current: Value, patch: Value) -> Value {
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

fn cart_context_metadata(
    cart: &rustok_cart::dto::CartResponse,
    context: &crate::dto::StoreContextResponse,
) -> Value {
    json!({
        "cart_context": {
            "region_id": cart.region_id,
            "country_code": cart.country_code,
            "locale": context.locale,
            "currency_code": context.currency_code,
            "selected_shipping_option_id": cart.selected_shipping_option_id,
            "email": cart.email,
        }
    })
}

async fn reprice_storefront_cart_line_items(
    app_ctx: &AppContext,
    tenant_id: Uuid,
    cart_service: &rustok_cart::CartService,
    cart: rustok_cart::CartResponse,
    request_context: Option<&RequestContext>,
) -> Result<rustok_cart::CartResponse, StorefrontCheckoutRuntimeError> {
    if cart.line_items.is_empty() {
        return Ok(cart);
    }

    let pricing_service = rustok_pricing::PricingService::new(
        app_ctx.db.clone(),
        rustok_api::loco::transactional_event_bus_from_context(app_ctx),
    );
    let channel_id = cart
        .channel_id
        .or_else(|| request_context.and_then(|ctx| ctx.channel_id));
    let channel_slug = normalize_public_channel_slug(cart.channel_slug.as_deref()).or_else(|| {
        request_context.and_then(|ctx| normalize_public_channel_slug(ctx.channel_slug.as_deref()))
    });
    let mut updates = Vec::new();
    for line_item in &cart.line_items {
        let Some(variant_id) = line_item.variant_id else {
            continue;
        };
        let pricing_context = rustok_pricing::PriceResolutionContext {
            currency_code: cart.currency_code.to_ascii_uppercase(),
            region_id: cart.region_id,
            price_list_id: None,
            channel_id,
            channel_slug: channel_slug.clone(),
            quantity: Some(line_item.quantity),
        };
        let resolved_price = pricing_service
            .resolve_variant_price(tenant_id, variant_id, pricing_context)
            .await
            .map_err(runtime_error)?
            .ok_or_else(|| {
                StorefrontCheckoutRuntimeError::new(
                    "Unable to resolve storefront price for cart line item",
                )
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
            .map_err(runtime_error)
    }
}

fn storefront_cart_pricing_update(
    line_item_id: Uuid,
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedPrice,
) -> rustok_cart::services::cart::CartLineItemPricingUpdate {
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
            amount: (base_unit_price - resolved_price.amount)
                * rust_decimal::Decimal::from(quantity),
            metadata: Value::Object(metadata),
        })
    } else {
        None
    };

    rustok_cart::services::cart::CartLineItemPricingUpdate {
        line_item_id,
        unit_price: base_unit_price,
        pricing_adjustment,
    }
}

fn resolve_requested_locale(
    requested: Option<String>,
    request_context_locale: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    normalize_optional(requested)
        .or_else(|| {
            request_context_locale.and_then(|value| normalize_optional(Some(value.to_string())))
        })
        .or_else(|| normalize_optional(Some(tenant_default_locale.to_string())))
        .unwrap_or_default()
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_public_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn runtime_error(error: impl std::fmt::Display) -> StorefrontCheckoutRuntimeError {
    StorefrontCheckoutRuntimeError::new(error.to_string())
}
