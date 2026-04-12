use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

use crate::model::{
    StorefrontCart, StorefrontCartAdjustment, StorefrontCartData, StorefrontCartDeliveryGroup,
    StorefrontCartLineItem,
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

impl From<GraphqlHttpError> for ApiError {
    fn from(value: GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

const STOREFRONT_CART_QUERY: &str = "query StorefrontCart($id: UUID!) { storefrontCart(id: $id) { id status currencyCode subtotalAmount adjustmentTotal totalAmount channelSlug email customerId regionId countryCode localeCode lineItems { id title sku quantity unitPrice totalPrice currencyCode shippingProfileSlug sellerId sellerScope } adjustments { id lineItemId sourceType sourceId amount currencyCode } deliveryGroups { shippingProfileSlug sellerId sellerScope lineItemIds selectedShippingOptionId availableShippingOptions { id } } } }";
const UPDATE_STOREFRONT_CART_LINE_ITEM_MUTATION: &str = "mutation UpdateStorefrontCartLineItem($cartId: UUID!, $lineId: UUID!, $input: UpdateStorefrontCartLineItemInput!) { updateStorefrontCartLineItem(cartId: $cartId, lineId: $lineId, input: $input) { id } }";
const REMOVE_STOREFRONT_CART_LINE_ITEM_MUTATION: &str = "mutation RemoveStorefrontCartLineItem($cartId: UUID!, $lineId: UUID!) { removeStorefrontCartLineItem(cartId: $cartId, lineId: $lineId) { id } }";

#[derive(Debug, Deserialize)]
struct StorefrontCartResponse {
    #[serde(rename = "storefrontCart")]
    storefront_cart: Option<GraphqlCart>,
}

#[derive(Debug, Serialize)]
struct StorefrontCartVariables {
    id: Uuid,
}

#[derive(Debug, Deserialize)]
struct UpdateStorefrontCartLineItemResponse {
    #[serde(rename = "updateStorefrontCartLineItem")]
    updated_cart: GraphqlCartMutationPayload,
}

#[derive(Debug, Serialize)]
struct UpdateStorefrontCartLineItemVariables {
    #[serde(rename = "cartId")]
    cart_id: Uuid,
    #[serde(rename = "lineId")]
    line_id: Uuid,
    input: UpdateStorefrontCartLineItemInput,
}

#[derive(Debug, Serialize)]
struct UpdateStorefrontCartLineItemInput {
    quantity: i32,
}

#[derive(Debug, Deserialize)]
struct RemoveStorefrontCartLineItemResponse {
    #[serde(rename = "removeStorefrontCartLineItem")]
    updated_cart: GraphqlCartMutationPayload,
}

#[derive(Debug, Serialize)]
struct RemoveStorefrontCartLineItemVariables {
    #[serde(rename = "cartId")]
    cart_id: Uuid,
    #[serde(rename = "lineId")]
    line_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct GraphqlCartMutationPayload {
    id: String,
}

#[derive(Debug, Deserialize)]
struct GraphqlCart {
    id: String,
    status: String,
    #[serde(rename = "currencyCode")]
    currency_code: String,
    #[serde(rename = "subtotalAmount")]
    subtotal_amount: String,
    #[serde(rename = "adjustmentTotal")]
    adjustment_total: String,
    #[serde(rename = "totalAmount")]
    total_amount: String,
    #[serde(rename = "channelSlug")]
    channel_slug: Option<String>,
    email: Option<String>,
    #[serde(rename = "customerId")]
    customer_id: Option<String>,
    #[serde(rename = "regionId")]
    region_id: Option<String>,
    #[serde(rename = "countryCode")]
    country_code: Option<String>,
    #[serde(rename = "localeCode")]
    locale_code: Option<String>,
    #[serde(rename = "lineItems")]
    line_items: Vec<GraphqlCartLineItem>,
    adjustments: Vec<GraphqlCartAdjustment>,
    #[serde(rename = "deliveryGroups")]
    delivery_groups: Vec<GraphqlCartDeliveryGroup>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCartLineItem {
    id: String,
    title: String,
    sku: Option<String>,
    quantity: i32,
    #[serde(rename = "unitPrice")]
    unit_price: String,
    #[serde(rename = "totalPrice")]
    total_price: String,
    #[serde(rename = "currencyCode")]
    currency_code: String,
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: String,
    #[serde(rename = "sellerId")]
    seller_id: Option<String>,
    #[serde(rename = "sellerScope")]
    seller_scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCartAdjustment {
    id: String,
    #[serde(rename = "lineItemId")]
    line_item_id: Option<String>,
    #[serde(rename = "sourceType")]
    source_type: String,
    #[serde(rename = "sourceId")]
    source_id: Option<String>,
    amount: String,
    #[serde(rename = "currencyCode")]
    currency_code: String,
}

#[derive(Debug, Deserialize)]
struct GraphqlCartDeliveryGroup {
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: String,
    #[serde(rename = "sellerId")]
    seller_id: Option<String>,
    #[serde(rename = "sellerScope")]
    seller_scope: Option<String>,
    #[serde(rename = "lineItemIds")]
    line_item_ids: Vec<String>,
    #[serde(rename = "selectedShippingOptionId")]
    selected_shipping_option_id: Option<String>,
    #[serde(rename = "availableShippingOptions")]
    available_shipping_options: Vec<GraphqlCartShippingOption>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCartShippingOption {}

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(query: &str, variables: V) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(ApiError::from)
}

fn normalize_cart_id(value: Option<String>) -> Option<String> {
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

fn parse_cart_id(value: Option<String>) -> Result<Option<(String, Uuid)>, ApiError> {
    match normalize_cart_id(value) {
        Some(cart_id) => {
            let parsed = Uuid::parse_str(cart_id.as_str())
                .map_err(|_| ApiError::Validation("cart_id must be a valid UUID".to_string()))?;
            Ok(Some((cart_id, parsed)))
        }
        None => Ok(None),
    }
}

fn parse_line_item_id(value: String) -> Result<(String, Uuid), ApiError> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(ApiError::Validation(
            "line_item_id must not be empty".to_string(),
        ));
    }

    let parsed = Uuid::parse_str(normalized.as_str())
        .map_err(|_| ApiError::Validation("line_item_id must be a valid UUID".to_string()))?;
    Ok((normalized, parsed))
}

pub async fn fetch_storefront_cart(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCartData, ApiError> {
    match fetch_storefront_cart_server(selected_cart_id.clone(), locale.clone()).await {
        Ok(data) => Ok(data),
        Err(_) => fetch_storefront_cart_graphql(selected_cart_id, locale).await,
    }
}

pub async fn fetch_storefront_cart_server(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCartData, ApiError> {
    storefront_cart_native(selected_cart_id, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_cart_graphql(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCartData, ApiError> {
    let _ = locale;
    let Some((normalized_cart_id, cart_id)) = parse_cart_id(selected_cart_id)? else {
        return Ok(StorefrontCartData {
            selected_cart_id: None,
            cart: None,
        });
    };

    let response: StorefrontCartResponse = request(
        STOREFRONT_CART_QUERY,
        StorefrontCartVariables { id: cart_id },
    )
    .await?;

    Ok(StorefrontCartData {
        selected_cart_id: Some(normalized_cart_id),
        cart: response.storefront_cart.map(map_graphql_cart),
    })
}

pub async fn decrement_storefront_cart_line_item(
    cart_id: String,
    line_item_id: String,
    current_quantity: i32,
) -> Result<(), ApiError> {
    match decrement_storefront_cart_line_item_server(cart_id.clone(), line_item_id.clone()).await {
        Ok(()) => Ok(()),
        Err(_) => {
            decrement_storefront_cart_line_item_graphql(cart_id, line_item_id, current_quantity)
                .await
        }
    }
}

pub async fn remove_storefront_cart_line_item(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ApiError> {
    match remove_storefront_cart_line_item_server(cart_id.clone(), line_item_id.clone()).await {
        Ok(()) => Ok(()),
        Err(_) => remove_storefront_cart_line_item_graphql(cart_id, line_item_id).await,
    }
}

pub async fn decrement_storefront_cart_line_item_server(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ApiError> {
    storefront_cart_decrement_line_item(cart_id, line_item_id)
        .await
        .map_err(ApiError::from)
}

pub async fn remove_storefront_cart_line_item_server(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ApiError> {
    storefront_cart_remove_line_item(cart_id, line_item_id)
        .await
        .map_err(ApiError::from)
}

pub async fn decrement_storefront_cart_line_item_graphql(
    cart_id: String,
    line_item_id: String,
    current_quantity: i32,
) -> Result<(), ApiError> {
    let Some((_, parsed_cart_id)) = parse_cart_id(Some(cart_id))? else {
        return Err(ApiError::Validation(
            "cart_id must not be empty".to_string(),
        ));
    };
    let (_, parsed_line_item_id) = parse_line_item_id(line_item_id)?;

    if current_quantity <= 1 {
        return remove_storefront_cart_line_item_graphql(
            parsed_cart_id.to_string(),
            parsed_line_item_id.to_string(),
        )
        .await;
    }

    let response: UpdateStorefrontCartLineItemResponse = request(
        UPDATE_STOREFRONT_CART_LINE_ITEM_MUTATION,
        UpdateStorefrontCartLineItemVariables {
            cart_id: parsed_cart_id,
            line_id: parsed_line_item_id,
            input: UpdateStorefrontCartLineItemInput {
                quantity: current_quantity - 1,
            },
        },
    )
    .await?;
    let _ = response.updated_cart.id;
    Ok(())
}

pub async fn remove_storefront_cart_line_item_graphql(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ApiError> {
    let Some((_, parsed_cart_id)) = parse_cart_id(Some(cart_id))? else {
        return Err(ApiError::Validation(
            "cart_id must not be empty".to_string(),
        ));
    };
    let (_, parsed_line_item_id) = parse_line_item_id(line_item_id)?;

    let response: RemoveStorefrontCartLineItemResponse = request(
        REMOVE_STOREFRONT_CART_LINE_ITEM_MUTATION,
        RemoveStorefrontCartLineItemVariables {
            cart_id: parsed_cart_id,
            line_id: parsed_line_item_id,
        },
    )
    .await?;
    let _ = response.updated_cart.id;
    Ok(())
}

fn map_graphql_cart(value: GraphqlCart) -> StorefrontCart {
    StorefrontCart {
        id: value.id,
        status: value.status,
        currency_code: value.currency_code,
        subtotal_amount: value.subtotal_amount,
        adjustment_total: value.adjustment_total,
        total_amount: value.total_amount,
        channel_slug: value.channel_slug,
        email: value.email,
        customer_id: value.customer_id,
        region_id: value.region_id,
        country_code: value.country_code,
        locale_code: value.locale_code,
        line_items: value
            .line_items
            .into_iter()
            .map(|item| StorefrontCartLineItem {
                id: item.id,
                title: item.title,
                sku: item.sku,
                quantity: item.quantity,
                unit_price: item.unit_price,
                total_price: item.total_price,
                currency_code: item.currency_code,
                shipping_profile_slug: item.shipping_profile_slug,
                seller_id: item.seller_id,
                seller_scope: item.seller_scope,
            })
            .collect(),
        adjustments: value
            .adjustments
            .into_iter()
            .map(|adjustment| StorefrontCartAdjustment {
                id: adjustment.id,
                line_item_id: adjustment.line_item_id,
                source_type: adjustment.source_type,
                source_id: adjustment.source_id,
                amount: adjustment.amount,
                currency_code: adjustment.currency_code,
            })
            .collect(),
        delivery_groups: value
            .delivery_groups
            .into_iter()
            .map(|group| StorefrontCartDeliveryGroup {
                shipping_profile_slug: group.shipping_profile_slug,
                seller_id: group.seller_id,
                seller_scope: group.seller_scope,
                line_item_count: group.line_item_ids.len() as u64,
                selected_shipping_option_id: group.selected_shipping_option_id,
                available_option_count: group.available_shipping_options.len() as u64,
            })
            .collect(),
    }
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
                seller_scope: item.seller_scope,
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
                amount: adjustment.amount.normalize().to_string(),
                currency_code: adjustment.currency_code,
            })
            .collect(),
        delivery_groups: value
            .delivery_groups
            .into_iter()
            .map(|group| StorefrontCartDeliveryGroup {
                shipping_profile_slug: group.shipping_profile_slug,
                seller_id: group.seller_id,
                seller_scope: group.seller_scope,
                line_item_count: group.line_item_ids.len() as u64,
                selected_shipping_option_id: group
                    .selected_shipping_option_id
                    .map(|value| value.to_string()),
                available_option_count: group.available_shipping_options.len() as u64,
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
        use loco_rs::app::AppContext;

        let app_ctx = expect_context::<AppContext>();
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let Some((normalized_cart_id, cart_id)) =
            parse_cart_id(selected_cart_id).map_err(|err| ServerFnError::new(err.to_string()))?
        else {
            let _ = locale;
            return Ok(StorefrontCartData {
                selected_cart_id: None,
                cart: None,
            });
        };

        let cart = match rustok_cart::CartService::new(app_ctx.db.clone())
            .get_cart(tenant.id, cart_id)
            .await
        {
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
            resolve_storefront_customer_id(app_ctx.db.clone(), tenant.id, auth.0).await?;
        ensure_storefront_cart_access(&cart, storefront_customer_id)?;

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

#[server(prefix = "/api/fn", endpoint = "cart/decrement-line-item")]
async fn storefront_cart_decrement_line_item(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_pricing::{PriceResolutionContext, PricingService};

        let app_ctx = expect_context::<AppContext>();
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

        let cart_service = rustok_cart::CartService::new(app_ctx.db.clone());
        let cart = cart_service
            .get_cart(tenant.id, parsed_cart_id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        let storefront_customer_id =
            resolve_storefront_customer_id(app_ctx.db.clone(), tenant.id, auth.0).await?;
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
            let pricing_context = PriceResolutionContext {
                currency_code: cart.currency_code.to_ascii_uppercase(),
                region_id: cart.region_id,
                price_list_id: None,
                channel_id: cart
                    .channel_id
                    .or_else(|| request_context.as_ref().and_then(|ctx| ctx.channel_id)),
                channel_slug: normalize_public_channel_slug(cart.channel_slug.as_deref()).or_else(
                    || {
                        request_context.as_ref().and_then(|ctx| {
                            normalize_public_channel_slug(ctx.channel_slug.as_deref())
                        })
                    },
                ),
                quantity: Some(next_quantity),
            };
            let pricing_service = PricingService::new(
                app_ctx.db.clone(),
                rustok_api::loco::transactional_event_bus_from_context(&app_ctx),
            );
            let variant_id = line_item
                .variant_id
                .ok_or_else(|| ServerFnError::new("Cart line item is missing variant_id"))?;
            let resolved_price = pricing_service
                .resolve_variant_price(tenant.id, variant_id, pricing_context)
                .await
                .map_err(|err| ServerFnError::new(err.to_string()))?
                .ok_or_else(|| {
                    ServerFnError::new("Unable to resolve storefront price for cart line item")
                })?;

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
            amount: (base_unit_price - resolved_price.amount) * rust_decimal::Decimal::from(quantity),
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
        use loco_rs::app::AppContext;

        let app_ctx = expect_context::<AppContext>();
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

        let cart_service = rustok_cart::CartService::new(app_ctx.db.clone());
        let cart = cart_service
            .get_cart(tenant.id, parsed_cart_id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        let storefront_customer_id =
            resolve_storefront_customer_id(app_ctx.db.clone(), tenant.id, auth.0).await?;
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
