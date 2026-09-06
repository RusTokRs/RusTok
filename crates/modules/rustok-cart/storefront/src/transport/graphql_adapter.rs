use super::native_server_adapter::ApiError;
use crate::core::{
    CartFetchRequest, CartLineItemDecrementRequest, CartLineItemMutationRequest,
    CartLineItemQuantityCommand, parse_adjustment_scope, parse_cart_id, parse_line_item_id,
};
use crate::model::{
    StorefrontCart, StorefrontCartAdjustment, StorefrontCartData, StorefrontCartDeliveryGroup,
    StorefrontCartLineItem, StorefrontCartShippingOption,
};
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

impl From<rustok_graphql::GraphqlHttpError> for ApiError {
    fn from(value: rustok_graphql::GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

const STOREFRONT_CART_QUERY: &str = "query StorefrontCart($id: UUID!) { storefrontCart(id: $id) { id status currencyCode subtotalAmount adjustmentTotal shippingTotal totalAmount channelSlug email customerId regionId countryCode localeCode lineItems { id title sku quantity unitPrice totalPrice currencyCode shippingProfileSlug sellerId } adjustments { id lineItemId sourceType sourceId amount currencyCode metadata } deliveryGroups { shippingProfileSlug sellerId lineItemIds selectedShippingOptionId availableShippingOptions { id name currencyCode amount providerId active } } } }";
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
    #[serde(rename = "shippingTotal")]
    shipping_total: String,
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
    metadata: String,
}

#[derive(Debug, Deserialize)]
struct GraphqlCartDeliveryGroup {
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: String,
    #[serde(rename = "sellerId")]
    seller_id: Option<String>,
    #[serde(rename = "lineItemIds")]
    line_item_ids: Vec<String>,
    #[serde(rename = "selectedShippingOptionId")]
    selected_shipping_option_id: Option<String>,
    #[serde(rename = "availableShippingOptions")]
    available_shipping_options: Vec<GraphqlCartShippingOption>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCartShippingOption {
    id: String,
    name: String,
    #[serde(rename = "currencyCode")]
    currency_code: String,
    amount: String,
    #[serde(rename = "providerId")]
    provider_id: String,
    active: bool,
}

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
        let origin = leptos::web_sys::window()
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

pub async fn fetch_cart(request: CartFetchRequest) -> Result<StorefrontCartData, ApiError> {
    let _ = request.locale;
    let Some((normalized_cart_id, cart_id)) = parse_cart_id(request.selected_cart_id)? else {
        return Ok(StorefrontCartData {
            selected_cart_id: None,
            cart: None,
        });
    };

    let response: StorefrontCartResponse = request_graphql_cart(cart_id).await?;

    Ok(StorefrontCartData {
        selected_cart_id: Some(normalized_cart_id),
        cart: response.storefront_cart.map(map_graphql_cart),
    })
}

pub async fn decrement_line_item(request: CartLineItemDecrementRequest) -> Result<(), ApiError> {
    match request.command {
        CartLineItemQuantityCommand::Remove => {
            remove_storefront_cart_line_item(request.cart_id, request.line_item_id).await
        }
        CartLineItemQuantityCommand::Update { next_quantity } => {
            update_storefront_cart_line_item_quantity(
                request.cart_id,
                request.line_item_id,
                next_quantity,
            )
            .await
        }
    }
}

pub async fn remove_line_item(request: CartLineItemMutationRequest) -> Result<(), ApiError> {
    remove_storefront_cart_line_item(request.cart_id, request.line_item_id).await
}

async fn request_graphql_cart(cart_id: Uuid) -> Result<StorefrontCartResponse, ApiError> {
    request(
        STOREFRONT_CART_QUERY,
        StorefrontCartVariables { id: cart_id },
    )
    .await
}

async fn update_storefront_cart_line_item_quantity(
    cart_id: String,
    line_item_id: String,
    next_quantity: i32,
) -> Result<(), ApiError> {
    let Some((_, parsed_cart_id)) = parse_cart_id(Some(cart_id))? else {
        return Err(ApiError::Validation(
            "cart_id must not be empty".to_string(),
        ));
    };
    let (_, parsed_line_item_id) = parse_line_item_id(line_item_id)?;

    let response: UpdateStorefrontCartLineItemResponse = request(
        UPDATE_STOREFRONT_CART_LINE_ITEM_MUTATION,
        UpdateStorefrontCartLineItemVariables {
            cart_id: parsed_cart_id,
            line_id: parsed_line_item_id,
            input: UpdateStorefrontCartLineItemInput {
                quantity: next_quantity,
            },
        },
    )
    .await?;
    let _ = response.updated_cart.id;
    Ok(())
}

async fn remove_storefront_cart_line_item(
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
        shipping_total: value.shipping_total,
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
                scope: parse_adjustment_scope(&adjustment.metadata),
                amount: adjustment.amount,
                currency_code: adjustment.currency_code,
                metadata: adjustment.metadata,
            })
            .collect(),
        delivery_groups: value
            .delivery_groups
            .into_iter()
            .map(|group| StorefrontCartDeliveryGroup {
                shipping_profile_slug: group.shipping_profile_slug,
                seller_id: group.seller_id,
                line_item_count: group.line_item_ids.len() as u64,
                selected_shipping_option_id: group.selected_shipping_option_id,
                available_option_count: group.available_shipping_options.len() as u64,
                available_shipping_options: group
                    .available_shipping_options
                    .into_iter()
                    .map(|option| StorefrontCartShippingOption {
                        id: option.id,
                        name: option.name,
                        currency_code: option.currency_code,
                        amount: option.amount,
                        provider_id: option.provider_id,
                        active: option.active,
                    })
                    .collect(),
            })
            .collect(),
    }
}
