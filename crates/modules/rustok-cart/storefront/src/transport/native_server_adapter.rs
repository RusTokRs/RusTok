use leptos::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use serde_json::Value;
use std::fmt::{Display, Formatter};
#[cfg(feature = "ssr")]
use uuid::Uuid;

use crate::core::CartCoreError;
#[cfg(feature = "ssr")]
use crate::core::normalize_public_channel_slug;
#[cfg(feature = "ssr")]
use crate::core::{parse_cart_id, parse_line_item_id};
use crate::model::StorefrontCartData;
#[cfg(feature = "ssr")]
use crate::model::{
    StorefrontCart, StorefrontCartAdjustment, StorefrontCartDeliveryGroup, StorefrontCartLineItem,
    StorefrontCartShippingOption,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
    Validation(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
            Self::Validation(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<CartCoreError> for ApiError {
    fn from(value: CartCoreError) -> Self {
        match value {
            CartCoreError::Validation(error) => Self::Validation(error),
        }
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

#[cfg(feature = "ssr")]
fn transactional_event_bus_from_runtime(
    runtime_ctx: &rustok_api::HostRuntimeContext,
    endpoint: &str,
) -> Result<rustok_outbox::TransactionalEventBus, ServerFnError> {
    runtime_ctx
        .shared_get::<rustok_outbox::TransactionalEventBus>()
        .ok_or_else(|| {
            ServerFnError::new(format!(
                "{endpoint} requires TransactionalEventBus in host runtime context"
            ))
        })
}

#[cfg(feature = "ssr")]
fn port_error_to_server_fn_error(error: rustok_api::PortError) -> ServerFnError {
    // Port errors are structured transport failures. Server functions expose a
    // string error contract, so forward the domain-safe message explicitly
    // instead of relying on a Display implementation that PortError omits.
    ServerFnError::new(error.message)
}

pub async fn fetch_storefront_cart_server(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCartData, ApiError> {
    storefront_cart_native(selected_cart_id, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_cart(
    request: crate::core::CartFetchRequest,
) -> Result<StorefrontCartData, ApiError> {
    fetch_storefront_cart_server(request.selected_cart_id, request.locale).await
}

pub async fn decrement_storefront_cart_line_item_server(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ApiError> {
    storefront_cart_decrement_line_item(cart_id, line_item_id)
        .await
        .map_err(ApiError::from)
}

pub async fn decrement_line_item(
    request: crate::core::CartLineItemDecrementRequest,
) -> Result<(), ApiError> {
    decrement_storefront_cart_line_item_server(request.cart_id, request.line_item_id).await
}

pub async fn remove_storefront_cart_line_item_server(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ApiError> {
    storefront_cart_remove_line_item(cart_id, line_item_id)
        .await
        .map_err(ApiError::from)
}

pub async fn remove_line_item(
    request: crate::core::CartLineItemMutationRequest,
) -> Result<(), ApiError> {
    remove_storefront_cart_line_item_server(request.cart_id, request.line_item_id).await
}

#[cfg(feature = "ssr")]
async fn resolve_storefront_customer_id(
    db: sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    auth: Option<rustok_api::AuthContext>,
) -> Result<Option<Uuid>, ServerFnError> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    match rustok_customer::CustomerService::new(db)
        .get_customer_by_user(tenant_id, auth.user_id)
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(rustok_customer::CustomerError::CustomerByUserNotFound(_)) => Ok(None),
        Err(err) => Err(ServerFnError::new(err.to_string())),
    }
}

#[cfg(feature = "ssr")]
fn ensure_storefront_cart_access(
    cart: &rustok_cart::CartResponse,
    storefront_customer_id: Option<Uuid>,
) -> Result<(), ServerFnError> {
    if let Some(owner_customer_id) = cart.customer_id {
        match storefront_customer_id {
            Some(customer_id) if customer_id == owner_customer_id => Ok(()),
            Some(_) => Err(ServerFnError::new(
                "Cart does not belong to the current storefront customer",
            )),
            None => Err(ServerFnError::new(
                "Authentication required to access this cart",
            )),
        }
    } else {
        Ok(())
    }
}

#[cfg(feature = "ssr")]
fn map_native_cart(value: rustok_cart::CartResponse) -> StorefrontCart {
    StorefrontCart {
        id: value.id.to_string(),
        status: value.status,
        currency_code: value.currency_code,
        subtotal_amount: value.subtotal_amount.normalize().to_string(),
        adjustment_total: value.adjustment_total.normalize().to_string(),
        shipping_total: value.shipping_total.normalize().to_string(),
        total_amount: value.total_amount.normalize().to_string(),
        channel_slug: value.channel_slug,
        email: value.email,
        customer_id: value.customer_id.map(|value| value.to_string()),
        region_id: value.region_id.map(|value| value.to_string()),
        country_code: value.country_code,
        locale_code: value.locale_code,
        line_items: value
            .line_items
            .into_iter()
            .map(|item| StorefrontCartLineItem {
                id: item.id.to_string(),
                title: item.title,
                sku: item.sku,
                quantity: item.quantity,
                unit_price: item.unit_price.normalize().to_string(),
                total_price: item.total_price.normalize().to_string(),
                currency_code: item.currency_code,
                shipping_profile_slug: item.shipping_profile_slug,
                seller_id: item.seller_id,
            })
            .collect(),
        adjustments: value
            .adjustments
            .into_iter()
            .map(|adjustment| StorefrontCartAdjustment {
                id: adjustment.id.to_string(),
                line_item_id: adjustment.line_item_id.map(|value| value.to_string()),
                source_type: adjustment.source_type,
                source_id: adjustment.source_id,
                scope: adjustment
                    .metadata
                    .get("scope")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                amount: adjustment.amount.normalize().to_string(),
                currency_code: adjustment.currency_code,
                metadata: adjustment.metadata.to_string(),
            })
            .collect(),
        delivery_groups: value
            .delivery_groups
            .into_iter()
            .map(|group| StorefrontCartDeliveryGroup {
                shipping_profile_slug: group.shipping_profile_slug,
                seller_id: group.seller_id,
                line_item_count: group.line_item_ids.len() as u64,
                selected_shipping_option_id: group
                    .selected_shipping_option_id
                    .map(|value| value.to_string()),
                available_option_count: group.available_shipping_options.len() as u64,
                available_shipping_options: group
                    .available_shipping_options
                    .into_iter()
                    .map(|option| StorefrontCartShippingOption {
                        id: option.id.to_string(),
                        name: option.name,
                        currency_code: option.currency_code,
                        amount: option.amount.normalize().to_string(),
                        provider_id: option.provider_id,
                        active: option.active,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[server(prefix = "/api/fn", endpoint = "cart/storefront-data")]
async fn storefront_cart_native(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCartData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let db = runtime_ctx.db_clone();
        let event_bus = transactional_event_bus_from_runtime(&runtime_ctx, "cart/storefront-data")?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let Some((normalized_cart_id, cart_id)) =
            parse_cart_id(selected_cart_id).map_err(|err| ServerFnError::new(err.to_string()))?
        else {
            let _ = locale;
            return Ok(StorefrontCartData {
                selected_cart_id: None,
                cart: None,
            });
        };

        let cart_service = rustok_cart::CartService::new(db.clone());
        let cart = match cart_service.get_cart(tenant.id, cart_id).await {
            Ok(cart) => cart,
            Err(rustok_cart::CartError::CartNotFound(_)) => {
                return Ok(StorefrontCartData {
                    selected_cart_id: Some(normalized_cart_id),
                    cart: None,
                });
            }
            Err(err) => return Err(ServerFnError::new(err.to_string())),
        };
        let storefront_customer_id =
            resolve_storefront_customer_id(db.clone(), tenant.id, auth.0).await?;
        ensure_storefront_cart_access(&cart, storefront_customer_id)?;
        let cart = reprice_storefront_cart_line_items(
            db,
            event_bus,
            tenant.id,
            &cart_service,
            cart,
            request_context.as_ref(),
        )
        .await?;

        let _ = locale;
        Ok(StorefrontCartData {
            selected_cart_id: Some(normalized_cart_id),
            cart: Some(map_native_cart(cart)),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (selected_cart_id, locale);
        Err(ServerFnError::new(
            "cart/storefront-data requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
async fn reprice_storefront_cart_line_items(
    db: sea_orm::DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
    tenant_id: Uuid,
    cart_service: &rustok_cart::CartService,
    cart: rustok_cart::CartResponse,
    request_context: Option<&rustok_api::RequestContext>,
) -> Result<rustok_cart::CartResponse, ServerFnError> {
    if cart.line_items.is_empty() {
        return Ok(cart);
    }

    use rustok_pricing::{PricingReadPort, ResolveProductPriceRequest};

    let pricing_service = rustok_pricing::PricingService::new(db, event_bus);
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
        let resolved_price = pricing_service
            .resolve_product_price(
                rustok_api::PortContext::new(
                    tenant_id.to_string(),
                    rustok_api::PortActor::service("rustok-cart.storefront"),
                    "en",
                    format!("cart:{}:reprice", cart.id),
                )
                .with_deadline(std::time::Duration::from_secs(2)),
                ResolveProductPriceRequest {
                    product_id: line_item.product_id,
                    variant_id,
                    region_id: cart.region_id,
                    channel_id,
                    channel_slug: channel_slug.clone(),
                    price_list_id: None,
                    quantity: Some(line_item.quantity),
                    currency_code: cart.currency_code.to_ascii_uppercase(),
                },
            )
            .await
            .map_err(port_error_to_server_fn_error)?;
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
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
}

#[server(prefix = "/api/fn", endpoint = "cart/decrement-line-item")]
async fn storefront_cart_decrement_line_item(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_pricing::{PricingReadPort, PricingService, ResolveProductPriceRequest};

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let db = runtime_ctx.db_clone();
        let event_bus =
            transactional_event_bus_from_runtime(&runtime_ctx, "cart/decrement-line-item")?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let Some((_, parsed_cart_id)) =
            parse_cart_id(Some(cart_id)).map_err(|err| ServerFnError::new(err.to_string()))?
        else {
            return Err(ServerFnError::new("cart_id must not be empty"));
        };
        let (_, parsed_line_item_id) =
            parse_line_item_id(line_item_id).map_err(|err| ServerFnError::new(err.to_string()))?;

        let cart_service = rustok_cart::CartService::new(db.clone());
        let cart = cart_service
            .get_cart(tenant.id, parsed_cart_id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        let storefront_customer_id =
            resolve_storefront_customer_id(db.clone(), tenant.id, auth.0).await?;
        ensure_storefront_cart_access(&cart, storefront_customer_id)?;

        let line_item = cart
            .line_items
            .iter()
            .find(|item| item.id == parsed_line_item_id)
            .ok_or_else(|| ServerFnError::new("Cart line item not found"))?;
        if line_item.quantity <= 1 {
            cart_service
                .remove_line_item(tenant.id, parsed_cart_id, parsed_line_item_id)
                .await
                .map_err(|err| ServerFnError::new(err.to_string()))?;
        } else {
            let next_quantity = line_item.quantity - 1;
            let pricing_service = PricingService::new(db, event_bus);
            let variant_id = line_item
                .variant_id
                .ok_or_else(|| ServerFnError::new("Cart line item is missing variant_id"))?;
            let resolved_price = pricing_service
                .resolve_product_price(
                    rustok_api::PortContext::new(
                        tenant.id.to_string(),
                        rustok_api::PortActor::service("rustok-cart.storefront"),
                        "en",
                        format!("cart:{}:decrement", parsed_cart_id),
                    )
                    .with_deadline(std::time::Duration::from_secs(2)),
                    ResolveProductPriceRequest {
                        product_id: line_item.product_id,
                        variant_id,
                        region_id: cart.region_id,
                        channel_id: cart
                            .channel_id
                            .or_else(|| request_context.as_ref().and_then(|ctx| ctx.channel_id)),
                        channel_slug: normalize_public_channel_slug(cart.channel_slug.as_deref())
                            .or_else(|| {
                                request_context.as_ref().and_then(|ctx| {
                                    normalize_public_channel_slug(ctx.channel_slug.as_deref())
                                })
                            }),
                        price_list_id: None,
                        quantity: Some(next_quantity),
                        currency_code: cart.currency_code.to_ascii_uppercase(),
                    },
                )
                .await
                .map_err(port_error_to_server_fn_error)?;

            let pricing_update =
                storefront_cart_pricing_update(parsed_line_item_id, next_quantity, &resolved_price);
            cart_service
                .update_line_item_pricing(
                    tenant.id,
                    parsed_cart_id,
                    parsed_line_item_id,
                    next_quantity,
                    pricing_update.unit_price,
                    pricing_update.pricing_adjustment,
                )
                .await
                .map_err(|err| ServerFnError::new(err.to_string()))?;
        }

        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (cart_id, line_item_id);
        Err(ServerFnError::new(
            "cart/decrement-line-item requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn storefront_cart_pricing_update(
    line_item_id: Uuid,
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedProductPriceSnapshot,
) -> rustok_cart::services::cart::CartLineItemPricingUpdate {
    let base_unit_price = resolved_price
        .compare_at_amount
        .filter(|compare_at| *compare_at > resolved_price.amount)
        .unwrap_or(resolved_price.amount);
    let pricing_adjustment = if base_unit_price > resolved_price.amount {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "kind".to_string(),
            serde_json::Value::from(if resolved_price.price_list_id.is_some() {
                "price_list"
            } else {
                "sale"
            }),
        );
        metadata.insert(
            "base_amount".to_string(),
            serde_json::Value::from(base_unit_price.normalize().to_string()),
        );
        metadata.insert(
            "effective_amount".to_string(),
            serde_json::Value::from(resolved_price.amount.normalize().to_string()),
        );
        if let Some(compare_at_amount) = resolved_price.compare_at_amount {
            metadata.insert(
                "compare_at_amount".to_string(),
                serde_json::Value::from(compare_at_amount.normalize().to_string()),
            );
        }
        if let Some(discount_percent) = resolved_price.discount_percent {
            metadata.insert(
                "discount_percent".to_string(),
                serde_json::Value::from(discount_percent.normalize().to_string()),
            );
        }
        if let Some(price_list_id) = resolved_price.price_list_id {
            metadata.insert(
                "price_list_id".to_string(),
                serde_json::Value::from(price_list_id.to_string()),
            );
        }
        if let Some(channel_id) = resolved_price.channel_id {
            metadata.insert(
                "channel_id".to_string(),
                serde_json::Value::from(channel_id.to_string()),
            );
        }
        if let Some(channel_slug) = resolved_price.channel_slug.as_deref() {
            metadata.insert(
                "channel_slug".to_string(),
                serde_json::Value::from(channel_slug),
            );
        }

        Some(rustok_cart::services::cart::CartPricingAdjustmentUpdate {
            source_id: resolved_price.price_list_id.map(|value| value.to_string()),
            amount: (base_unit_price - resolved_price.amount)
                * rust_decimal::Decimal::from(quantity),
            metadata: serde_json::Value::Object(metadata),
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

#[server(prefix = "/api/fn", endpoint = "cart/remove-line-item")]
async fn storefront_cart_remove_line_item(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let db = runtime_ctx.db_clone();
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let Some((_, parsed_cart_id)) =
            parse_cart_id(Some(cart_id)).map_err(|err| ServerFnError::new(err.to_string()))?
        else {
            return Err(ServerFnError::new("cart_id must not be empty"));
        };
        let (_, parsed_line_item_id) =
            parse_line_item_id(line_item_id).map_err(|err| ServerFnError::new(err.to_string()))?;

        let cart_service = rustok_cart::CartService::new(db.clone());
        let cart = cart_service
            .get_cart(tenant.id, parsed_cart_id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        let storefront_customer_id = resolve_storefront_customer_id(db, tenant.id, auth.0).await?;
        ensure_storefront_cart_access(&cart, storefront_customer_id)?;

        cart_service
            .remove_line_item(tenant.id, parsed_cart_id, parsed_line_item_id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (cart_id, line_item_id);
        Err(ServerFnError::new(
            "cart/remove-line-item requires the `ssr` feature",
        ))
    }
}
