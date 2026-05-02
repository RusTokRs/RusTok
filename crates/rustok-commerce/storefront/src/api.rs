use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

use crate::model::{
    StorefrontCheckoutAdjustment, StorefrontCheckoutCart, StorefrontCheckoutCompletion,
    StorefrontCheckoutDeliveryGroup, StorefrontCheckoutPaymentCollection,
    StorefrontCheckoutShippingOption, StorefrontCheckoutWorkspace, StorefrontCommerceData,
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

const STOREFRONT_CHECKOUT_QUERY: &str = "query StorefrontCheckoutWorkspace($id: UUID!) { storefrontCart(id: $id) { id status currencyCode subtotalAmount adjustmentTotal shippingTotal totalAmount channelSlug email customerId regionId countryCode localeCode selectedShippingOptionId lineItems { id } adjustments { id lineItemId sourceType sourceId amount currencyCode metadata } deliveryGroups { shippingProfileSlug sellerId sellerScope lineItemIds selectedShippingOptionId availableShippingOptions { id name currencyCode amount providerId active } } } }";
const CREATE_STOREFRONT_PAYMENT_COLLECTION_MUTATION: &str = "mutation CreateStorefrontPaymentCollection($input: CreateStorefrontPaymentCollectionInput!) { createStorefrontPaymentCollection(input: $input) { id status currencyCode amount authorizedAmount capturedAmount orderId providerId createdAt updatedAt payments { id } } }";
const COMPLETE_STOREFRONT_CHECKOUT_MUTATION: &str = "mutation CompleteStorefrontCheckout($input: CompleteStorefrontCheckoutInput!) { completeStorefrontCheckout(input: $input) { order { id status currencyCode shippingTotal adjustmentTotal totalAmount adjustments { id lineItemId sourceType sourceId amount currencyCode metadata } } paymentCollection { id status currencyCode } fulfillments { id } context { locale currencyCode } } }";
const SELECT_STOREFRONT_SHIPPING_OPTION_MUTATION: &str = "mutation SelectStorefrontShippingOption($cartId: UUID!, $input: UpdateStorefrontCartContextInput!) { updateStorefrontCartContext(cartId: $cartId, input: $input) { cart { id } } }";

#[derive(Debug, Deserialize)]
struct StorefrontCheckoutResponse {
    #[serde(rename = "storefrontCart")]
    storefront_cart: Option<GraphqlCheckoutCart>,
}

#[derive(Debug, Serialize)]
struct StorefrontCheckoutVariables {
    id: Uuid,
}

#[derive(Debug, Deserialize)]
struct GraphqlCheckoutCart {
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
    #[serde(rename = "selectedShippingOptionId")]
    selected_shipping_option_id: Option<String>,
    #[serde(rename = "lineItems")]
    line_items: Vec<GraphqlCheckoutLineItem>,
    adjustments: Vec<GraphqlCheckoutAdjustment>,
    #[serde(rename = "deliveryGroups")]
    delivery_groups: Vec<GraphqlCheckoutDeliveryGroup>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCheckoutLineItem {}

#[derive(Debug, Deserialize)]
struct GraphqlCheckoutAdjustment {
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
struct GraphqlCheckoutDeliveryGroup {
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
    available_shipping_options: Vec<GraphqlCheckoutShippingOption>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCheckoutShippingOption {
    id: String,
    name: String,
    #[serde(rename = "currencyCode")]
    currency_code: String,
    amount: String,
    #[serde(rename = "providerId")]
    provider_id: String,
    active: bool,
}

#[derive(Debug, Deserialize)]
struct CreateStorefrontPaymentCollectionResponse {
    #[serde(rename = "createStorefrontPaymentCollection")]
    payment_collection: GraphqlPaymentCollection,
}

#[derive(Debug, Serialize)]
struct CreateStorefrontPaymentCollectionVariables {
    input: CreateStorefrontPaymentCollectionInput,
}

#[derive(Debug, Serialize)]
struct CreateStorefrontPaymentCollectionInput {
    #[serde(rename = "cartId")]
    cart_id: Uuid,
    metadata: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphqlPaymentCollection {
    id: String,
    status: String,
    #[serde(rename = "currencyCode")]
    currency_code: String,
    amount: String,
    #[serde(rename = "authorizedAmount")]
    authorized_amount: String,
    #[serde(rename = "capturedAmount")]
    captured_amount: String,
    #[serde(rename = "orderId")]
    order_id: Option<String>,
    #[serde(rename = "providerId")]
    provider_id: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    payments: Vec<GraphqlPayment>,
}

#[derive(Debug, Deserialize)]
struct GraphqlPayment {}

#[derive(Debug, Deserialize)]
struct CompleteStorefrontCheckoutResponse {
    #[serde(rename = "completeStorefrontCheckout")]
    completion: GraphqlCheckoutCompletion,
}

#[derive(Debug, Serialize)]
struct CompleteStorefrontCheckoutVariables {
    input: CompleteStorefrontCheckoutInput,
}

#[derive(Debug, Serialize)]
struct CompleteStorefrontCheckoutInput {
    #[serde(rename = "cartId")]
    cart_id: Uuid,
    #[serde(rename = "createFulfillment")]
    create_fulfillment: bool,
    metadata: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SelectStorefrontShippingOptionResponse {
    #[serde(rename = "updateStorefrontCartContext")]
    updated_cart: GraphqlStorefrontCartContextUpdate,
}

#[derive(Debug, Deserialize)]
struct GraphqlStorefrontCartContextUpdate {
    cart: GraphqlCartMutationPayload,
}

#[derive(Debug, Deserialize)]
struct GraphqlCartMutationPayload {
    id: String,
}

#[derive(Debug, Serialize)]
struct SelectStorefrontShippingOptionVariables {
    #[serde(rename = "cartId")]
    cart_id: Uuid,
    input: UpdateStorefrontCartContextInput,
}

#[derive(Debug, Serialize)]
struct UpdateStorefrontCartContextInput {
    #[serde(rename = "shippingSelections")]
    shipping_selections: Vec<StorefrontShippingSelectionInput>,
}

#[derive(Debug, Serialize)]
struct StorefrontShippingSelectionInput {
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: String,
    #[serde(rename = "sellerId")]
    seller_id: Option<String>,
    #[serde(rename = "sellerScope")]
    seller_scope: Option<String>,
    #[serde(rename = "selectedShippingOptionId")]
    selected_shipping_option_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCheckoutCompletion {
    order: GraphqlOrderSummary,
    #[serde(rename = "paymentCollection")]
    payment_collection: GraphqlCheckoutCompletionPaymentCollection,
    fulfillments: Vec<GraphqlFulfillmentSummary>,
    context: GraphqlStoreContext,
}

#[derive(Debug, Deserialize)]
struct GraphqlOrderSummary {
    id: String,
    status: String,
    #[serde(rename = "currencyCode")]
    currency_code: String,
    #[serde(rename = "shippingTotal")]
    shipping_total: String,
    #[serde(rename = "adjustmentTotal")]
    adjustment_total: String,
    #[serde(rename = "totalAmount")]
    total_amount: String,
    adjustments: Vec<GraphqlCheckoutAdjustment>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCheckoutCompletionPaymentCollection {
    id: String,
    status: String,
    #[serde(rename = "currencyCode")]
    currency_code: String,
}

#[derive(Debug, Deserialize)]
struct GraphqlFulfillmentSummary {}

#[derive(Debug, Deserialize)]
struct GraphqlStoreContext {
    locale: String,
    #[serde(rename = "currencyCode")]
    currency_code: Option<String>,
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn normalize_public_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn normalize_cart_id(value: Option<String>) -> Option<String> {
    normalize_optional(value)
}

fn parse_adjustment_scope(metadata: &str) -> Option<String> {
    serde_json::from_str::<Value>(metadata)
        .ok()
        .and_then(|value| {
            value
                .get("scope")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn normalize_shipping_profile_slug(value: String) -> Result<String, ApiError> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(ApiError::Validation(
            "shipping_profile_slug must not be empty".to_string(),
        ));
    }

    Ok(normalized)
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

fn parse_optional_uuid(value: Option<String>, field_name: &str) -> Result<Option<Uuid>, ApiError> {
    match normalize_optional(value) {
        Some(value) => {
            let parsed = Uuid::parse_str(value.as_str())
                .map_err(|_| ApiError::Validation(format!("{field_name} must be a valid UUID")))?;
            Ok(Some(parsed))
        }
        None => Ok(None),
    }
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

fn fallback_storefront_commerce(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> StorefrontCommerceData {
    let effective_locale = normalize_optional(locale).unwrap_or_default();
    let normalized_cart_id = normalize_cart_id(selected_cart_id);

    StorefrontCommerceData {
        effective_locale: effective_locale.clone(),
        tenant_slug: configured_tenant_slug(),
        tenant_default_locale: effective_locale,
        channel_slug: None,
        channel_resolution_source: None,
        selected_cart_id: normalized_cart_id.clone(),
        checkout: normalized_cart_id.map(|_| StorefrontCheckoutWorkspace {
            cart: None,
            payment_collection: None,
        }),
    }
}

fn map_graphql_shipping_option(
    value: GraphqlCheckoutShippingOption,
) -> StorefrontCheckoutShippingOption {
    StorefrontCheckoutShippingOption {
        id: value.id,
        name: value.name,
        currency_code: value.currency_code,
        amount: value.amount,
        provider_id: value.provider_id,
        active: value.active,
    }
}

fn map_graphql_delivery_group(
    value: GraphqlCheckoutDeliveryGroup,
) -> StorefrontCheckoutDeliveryGroup {
    StorefrontCheckoutDeliveryGroup {
        shipping_profile_slug: value.shipping_profile_slug,
        seller_id: value.seller_id,
        seller_scope: value.seller_scope,
        line_item_count: value.line_item_ids.len() as u64,
        selected_shipping_option_id: value.selected_shipping_option_id,
        available_shipping_options: value
            .available_shipping_options
            .into_iter()
            .map(map_graphql_shipping_option)
            .collect(),
    }
}

fn map_graphql_checkout_cart(value: GraphqlCheckoutCart) -> StorefrontCheckoutCart {
    let adjustments = value
        .adjustments
        .into_iter()
        .map(|adjustment| StorefrontCheckoutAdjustment {
            id: adjustment.id,
            line_item_id: adjustment.line_item_id,
            source_type: adjustment.source_type,
            source_id: adjustment.source_id,
            scope: parse_adjustment_scope(&adjustment.metadata),
            amount: adjustment.amount,
            currency_code: adjustment.currency_code,
            metadata: adjustment.metadata,
        })
        .collect::<Vec<_>>();
    let delivery_groups = value
        .delivery_groups
        .into_iter()
        .map(map_graphql_delivery_group)
        .collect::<Vec<_>>();
    let delivery_group_count = delivery_groups.len() as u64;

    StorefrontCheckoutCart {
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
        selected_shipping_option_id: value.selected_shipping_option_id,
        line_item_count: value.line_items.len() as u64,
        adjustment_count: adjustments.len() as u64,
        delivery_group_count,
        adjustments,
        delivery_groups,
    }
}

fn map_graphql_payment_collection(
    value: GraphqlPaymentCollection,
) -> StorefrontCheckoutPaymentCollection {
    StorefrontCheckoutPaymentCollection {
        id: value.id,
        status: value.status,
        currency_code: value.currency_code,
        amount: value.amount,
        authorized_amount: value.authorized_amount,
        captured_amount: value.captured_amount,
        order_id: value.order_id,
        provider_id: value.provider_id,
        payment_count: value.payments.len() as u64,
        created_at: value.created_at,
        updated_at: value.updated_at,
    }
}

fn map_graphql_checkout_completion(
    value: GraphqlCheckoutCompletion,
) -> StorefrontCheckoutCompletion {
    let adjustments = value
        .order
        .adjustments
        .into_iter()
        .map(|adjustment| StorefrontCheckoutAdjustment {
            id: adjustment.id,
            line_item_id: adjustment.line_item_id,
            source_type: adjustment.source_type,
            source_id: adjustment.source_id,
            scope: parse_adjustment_scope(&adjustment.metadata),
            amount: adjustment.amount,
            currency_code: adjustment.currency_code,
            metadata: adjustment.metadata,
        })
        .collect::<Vec<_>>();
    StorefrontCheckoutCompletion {
        order_id: value.order.id,
        order_status: value.order.status,
        currency_code: value.order.currency_code,
        shipping_total: value.order.shipping_total,
        adjustment_total: value.order.adjustment_total,
        total_amount: value.order.total_amount,
        adjustments,
        payment_collection_id: value.payment_collection.id,
        payment_collection_status: value.payment_collection.status,
        fulfillment_count: value.fulfillments.len() as u64,
        context_locale: value.context.locale,
        context_currency_code: value
            .context
            .currency_code
            .or(Some(value.payment_collection.currency_code)),
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
    cart: &rustok_commerce::CartResponse,
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

#[cfg(feature = "ssr")]
fn cart_context_metadata(
    cart: &rustok_commerce::CartResponse,
    context: &rustok_commerce::StoreContextResponse,
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

#[cfg(feature = "ssr")]
fn map_native_shipping_option(
    value: rustok_commerce::CartShippingOptionSummary,
) -> StorefrontCheckoutShippingOption {
    StorefrontCheckoutShippingOption {
        id: value.id.to_string(),
        name: value.name,
        currency_code: value.currency_code,
        amount: value.amount.normalize().to_string(),
        provider_id: value.provider_id,
        active: value.active,
    }
}

#[cfg(feature = "ssr")]
fn map_native_delivery_group(
    value: rustok_commerce::CartDeliveryGroupResponse,
) -> StorefrontCheckoutDeliveryGroup {
    StorefrontCheckoutDeliveryGroup {
        shipping_profile_slug: value.shipping_profile_slug,
        seller_id: value.seller_id,
        seller_scope: value.seller_scope,
        line_item_count: value.line_item_ids.len() as u64,
        selected_shipping_option_id: value
            .selected_shipping_option_id
            .map(|value| value.to_string()),
        available_shipping_options: value
            .available_shipping_options
            .into_iter()
            .map(map_native_shipping_option)
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_native_checkout_cart(value: rustok_commerce::CartResponse) -> StorefrontCheckoutCart {
    let adjustments = value
        .adjustments
        .into_iter()
        .map(|adjustment| StorefrontCheckoutAdjustment {
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
        .collect::<Vec<_>>();
    let delivery_groups = value
        .delivery_groups
        .into_iter()
        .map(map_native_delivery_group)
        .collect::<Vec<_>>();

    StorefrontCheckoutCart {
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
        selected_shipping_option_id: value
            .selected_shipping_option_id
            .map(|value| value.to_string()),
        line_item_count: value.line_items.len() as u64,
        adjustment_count: adjustments.len() as u64,
        delivery_group_count: delivery_groups.len() as u64,
        adjustments,
        delivery_groups,
    }
}

#[cfg(feature = "ssr")]
fn map_native_payment_collection(
    value: rustok_commerce::PaymentCollectionResponse,
) -> StorefrontCheckoutPaymentCollection {
    StorefrontCheckoutPaymentCollection {
        id: value.id.to_string(),
        status: value.status,
        currency_code: value.currency_code,
        amount: value.amount.normalize().to_string(),
        authorized_amount: value.authorized_amount.normalize().to_string(),
        captured_amount: value.captured_amount.normalize().to_string(),
        order_id: value.order_id.map(|value| value.to_string()),
        provider_id: value.provider_id,
        payment_count: value.payments.len() as u64,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_native_checkout_completion(
    value: rustok_commerce::CompleteCheckoutResponse,
) -> StorefrontCheckoutCompletion {
    let adjustments = value
        .order
        .adjustments
        .into_iter()
        .map(|adjustment| StorefrontCheckoutAdjustment {
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
        .collect::<Vec<_>>();
    StorefrontCheckoutCompletion {
        order_id: value.order.id.to_string(),
        order_status: value.order.status,
        currency_code: value.order.currency_code,
        shipping_total: value.order.shipping_total.normalize().to_string(),
        adjustment_total: value.order.adjustment_total.normalize().to_string(),
        total_amount: value.order.total_amount.normalize().to_string(),
        adjustments,
        payment_collection_id: value.payment_collection.id.to_string(),
        payment_collection_status: value.payment_collection.status,
        fulfillment_count: value.fulfillments.len() as u64,
        context_locale: value.context.locale,
        context_currency_code: value.context.currency_code,
    }
}

fn build_graphql_shipping_selections(
    cart: &StorefrontCheckoutCart,
    shipping_profile_slug: &str,
    seller_id: Option<&str>,
    seller_scope: Option<&str>,
    shipping_option_id: Option<Uuid>,
) -> Result<Vec<StorefrontShippingSelectionInput>, ApiError> {
    let mut matched_target = false;
    let mut selections = Vec::with_capacity(cart.delivery_groups.len());

    for group in &cart.delivery_groups {
        let group_matches = group.shipping_profile_slug == shipping_profile_slug
            && if let Some(seller_id) = seller_id {
                group.seller_id.as_deref() == Some(seller_id)
            } else {
                group.seller_id.is_none() && group.seller_scope.as_deref() == seller_scope
            };
        let selected_shipping_option_id = if group_matches {
            matched_target = true;
            if let Some(shipping_option_id) = shipping_option_id {
                let is_available = group
                    .available_shipping_options
                    .iter()
                    .any(|option| option.id == shipping_option_id.to_string());
                if !is_available {
                    return Err(ApiError::Validation(format!(
                        "shipping option {shipping_option_id} is not available for shipping profile {}",
                        group.shipping_profile_slug
                    )));
                }
            }
            shipping_option_id
        } else {
            parse_optional_uuid(
                group.selected_shipping_option_id.clone(),
                "selected_shipping_option_id",
            )?
        };

        selections.push(StorefrontShippingSelectionInput {
            shipping_profile_slug: group.shipping_profile_slug.clone(),
            seller_id: group.seller_id.clone(),
            seller_scope: group.seller_scope.clone(),
            selected_shipping_option_id,
        });
    }

    if !matched_target {
        return Err(ApiError::Validation(format!(
            "delivery group `{shipping_profile_slug}`/{:?}/{:?} is not present in the checkout cart",
            seller_id,
            seller_scope
        )));
    }

    Ok(selections)
}

#[cfg(feature = "ssr")]
fn build_native_shipping_selections(
    cart: &rustok_commerce::CartResponse,
    shipping_profile_slug: &str,
    seller_id: Option<&str>,
    seller_scope: Option<&str>,
    shipping_option_id: Option<Uuid>,
) -> Result<Vec<rustok_commerce::CartShippingSelectionInput>, ServerFnError> {
    let mut matched_target = false;
    let mut selections = Vec::with_capacity(cart.delivery_groups.len());

    for group in &cart.delivery_groups {
        let group_matches = group.shipping_profile_slug == shipping_profile_slug
            && if let Some(seller_id) = seller_id {
                group.seller_id.as_deref() == Some(seller_id)
            } else {
                group.seller_id.is_none() && group.seller_scope.as_deref() == seller_scope
            };
        let selected_shipping_option_id = if group_matches {
            matched_target = true;
            if let Some(shipping_option_id) = shipping_option_id {
                let is_available = group
                    .available_shipping_options
                    .iter()
                    .any(|option| option.id == shipping_option_id);
                if !is_available {
                    return Err(ServerFnError::new(format!(
                        "shipping option {shipping_option_id} is not available for shipping profile {}",
                        group.shipping_profile_slug
                    )));
                }
            }
            shipping_option_id
        } else {
            group.selected_shipping_option_id
        };

        selections.push(rustok_commerce::CartShippingSelectionInput {
            shipping_profile_slug: group.shipping_profile_slug.clone(),
            seller_id: group.seller_id.clone(),
            seller_scope: group.seller_scope.clone(),
            selected_shipping_option_id,
        });
    }

    if !matched_target {
        return Err(ServerFnError::new(format!(
            "delivery group `{shipping_profile_slug}`/{:?}/{:?} is not present in the checkout cart",
            seller_id,
            seller_scope
        )));
    }

    Ok(selections)
}

pub async fn fetch_storefront_commerce(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ApiError> {
    match fetch_storefront_commerce_server(selected_cart_id.clone(), locale.clone()).await {
        Ok(data) => Ok(data),
        Err(_) => fetch_storefront_commerce_graphql(selected_cart_id, locale).await,
    }
}

pub async fn fetch_storefront_commerce_server(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ApiError> {
    storefront_commerce_native(selected_cart_id, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_commerce_graphql(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ApiError> {
    let mut data = fallback_storefront_commerce(selected_cart_id.clone(), locale);
    let Some((normalized_cart_id, cart_id)) = parse_cart_id(selected_cart_id)? else {
        return Ok(data);
    };

    let response: StorefrontCheckoutResponse = request(
        STOREFRONT_CHECKOUT_QUERY,
        StorefrontCheckoutVariables { id: cart_id },
    )
    .await?;

    data.selected_cart_id = Some(normalized_cart_id);
    data.checkout = Some(StorefrontCheckoutWorkspace {
        cart: response.storefront_cart.map(map_graphql_checkout_cart),
        payment_collection: None,
    });
    Ok(data)
}

pub async fn select_storefront_shipping_option(
    cart: StorefrontCheckoutCart,
    shipping_profile_slug: String,
    seller_id: Option<String>,
    seller_scope: Option<String>,
    shipping_option_id: Option<String>,
) -> Result<(), ApiError> {
    match select_storefront_shipping_option_server(
        cart.id.clone(),
        shipping_profile_slug.clone(),
        seller_id.clone(),
        seller_scope.clone(),
        shipping_option_id.clone(),
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(_) => {
            select_storefront_shipping_option_graphql(
                cart,
                shipping_profile_slug,
                seller_id,
                seller_scope,
                shipping_option_id,
            )
            .await
        }
    }
}

pub async fn select_storefront_shipping_option_server(
    cart_id: String,
    shipping_profile_slug: String,
    seller_id: Option<String>,
    seller_scope: Option<String>,
    shipping_option_id: Option<String>,
) -> Result<(), ApiError> {
    storefront_select_shipping_option(
        cart_id,
        shipping_profile_slug,
        seller_id,
        seller_scope,
        shipping_option_id,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn select_storefront_shipping_option_graphql(
    cart: StorefrontCheckoutCart,
    shipping_profile_slug: String,
    seller_id: Option<String>,
    seller_scope: Option<String>,
    shipping_option_id: Option<String>,
) -> Result<(), ApiError> {
    let Some((_, parsed_cart_id)) = parse_cart_id(Some(cart.id.clone()))? else {
        return Err(ApiError::Validation(
            "cart_id must not be empty".to_string(),
        ));
    };
    let normalized_shipping_profile_slug = normalize_shipping_profile_slug(shipping_profile_slug)?;
    let parsed_shipping_option_id = parse_optional_uuid(shipping_option_id, "shipping_option_id")?;
    let shipping_selections = build_graphql_shipping_selections(
        &cart,
        normalized_shipping_profile_slug.as_str(),
        seller_id.as_deref(),
        seller_scope.as_deref(),
        parsed_shipping_option_id,
    )?;

    let response: SelectStorefrontShippingOptionResponse = request(
        SELECT_STOREFRONT_SHIPPING_OPTION_MUTATION,
        SelectStorefrontShippingOptionVariables {
            cart_id: parsed_cart_id,
            input: UpdateStorefrontCartContextInput {
                shipping_selections,
            },
        },
    )
    .await?;
    let _ = response.updated_cart.cart.id;
    Ok(())
}

pub async fn create_storefront_payment_collection(
    cart_id: String,
) -> Result<StorefrontCheckoutPaymentCollection, ApiError> {
    match create_storefront_payment_collection_server(cart_id.clone()).await {
        Ok(collection) => Ok(collection),
        Err(_) => create_storefront_payment_collection_graphql(cart_id).await,
    }
}

pub async fn create_storefront_payment_collection_server(
    cart_id: String,
) -> Result<StorefrontCheckoutPaymentCollection, ApiError> {
    storefront_create_payment_collection(cart_id)
        .await
        .map_err(ApiError::from)
}

pub async fn create_storefront_payment_collection_graphql(
    cart_id: String,
) -> Result<StorefrontCheckoutPaymentCollection, ApiError> {
    let Some((_, parsed_cart_id)) = parse_cart_id(Some(cart_id))? else {
        return Err(ApiError::Validation(
            "cart_id must not be empty".to_string(),
        ));
    };

    let response: CreateStorefrontPaymentCollectionResponse = request(
        CREATE_STOREFRONT_PAYMENT_COLLECTION_MUTATION,
        CreateStorefrontPaymentCollectionVariables {
            input: CreateStorefrontPaymentCollectionInput {
                cart_id: parsed_cart_id,
                metadata: None,
            },
        },
    )
    .await?;

    Ok(map_graphql_payment_collection(response.payment_collection))
}

pub async fn complete_storefront_checkout(
    cart_id: String,
) -> Result<StorefrontCheckoutCompletion, ApiError> {
    match complete_storefront_checkout_server(cart_id.clone()).await {
        Ok(completion) => Ok(completion),
        Err(_) => complete_storefront_checkout_graphql(cart_id).await,
    }
}

pub async fn complete_storefront_checkout_server(
    cart_id: String,
) -> Result<StorefrontCheckoutCompletion, ApiError> {
    storefront_complete_checkout(cart_id)
        .await
        .map_err(ApiError::from)
}

pub async fn complete_storefront_checkout_graphql(
    cart_id: String,
) -> Result<StorefrontCheckoutCompletion, ApiError> {
    let Some((_, parsed_cart_id)) = parse_cart_id(Some(cart_id))? else {
        return Err(ApiError::Validation(
            "cart_id must not be empty".to_string(),
        ));
    };

    let response: CompleteStorefrontCheckoutResponse = request(
        COMPLETE_STOREFRONT_CHECKOUT_MUTATION,
        CompleteStorefrontCheckoutVariables {
            input: CompleteStorefrontCheckoutInput {
                cart_id: parsed_cart_id,
                create_fulfillment: true,
                metadata: None,
            },
        },
    )
    .await?;

    Ok(map_graphql_checkout_completion(response.completion))
}

#[server(prefix = "/api/fn", endpoint = "commerce/storefront-data")]
async fn storefront_commerce_native(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;

        let app_ctx = expect_context::<AppContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let normalized_locale = resolve_requested_locale(
            locale,
            Some(request_context.locale.as_str()),
            tenant.default_locale.as_str(),
        );
        let mut data = StorefrontCommerceData {
            effective_locale: normalized_locale,
            tenant_slug: Some(tenant.slug),
            tenant_default_locale: tenant.default_locale,
            channel_slug: request_context.channel_slug.clone(),
            channel_resolution_source: request_context
                .channel_resolution_source
                .as_ref()
                .map(|source| source.as_str().to_string()),
            selected_cart_id: None,
            checkout: None,
        };

        let Some((normalized_cart_id, cart_id)) =
            parse_cart_id(selected_cart_id).map_err(|err| ServerFnError::new(err.to_string()))?
        else {
            return Ok(data);
        };

        let cart_service = rustok_commerce::CartService::new(app_ctx.db.clone());
        let cart = match cart_service.get_cart(tenant.id, cart_id).await {
            Ok(cart) => cart,
            Err(rustok_cart::CartError::CartNotFound(_)) => {
                data.selected_cart_id = Some(normalized_cart_id);
                data.checkout = Some(StorefrontCheckoutWorkspace {
                    cart: None,
                    payment_collection: None,
                });
                return Ok(data);
            }
            Err(err) => return Err(ServerFnError::new(err.to_string())),
        };

        let storefront_customer_id =
            resolve_storefront_customer_id(app_ctx.db.clone(), tenant.id, auth.0).await?;
        ensure_storefront_cart_access(&cart, storefront_customer_id)?;
        let cart = reprice_storefront_cart_line_items(
            &app_ctx,
            tenant.id,
            &cart_service,
            cart,
            Some(&request_context),
        )
        .await?;
        let payment_collection = rustok_commerce::PaymentService::new(app_ctx.db.clone())
            .find_reusable_collection_by_cart(tenant.id, cart.id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;

        data.selected_cart_id = Some(normalized_cart_id);
        data.checkout = Some(StorefrontCheckoutWorkspace {
            cart: Some(map_native_checkout_cart(cart)),
            payment_collection: payment_collection.map(map_native_payment_collection),
        });
        Ok(data)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (selected_cart_id, locale);
        Err(ServerFnError::new(
            "commerce/storefront-data requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "commerce/create-payment-collection")]
async fn storefront_create_payment_collection(
    cart_id: String,
) -> Result<StorefrontCheckoutPaymentCollection, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;

        let app_ctx = expect_context::<AppContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
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

        let cart_service = rustok_commerce::CartService::new(app_ctx.db.clone());
        let cart = cart_service
            .get_cart(tenant.id, parsed_cart_id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        let storefront_customer_id =
            resolve_storefront_customer_id(app_ctx.db.clone(), tenant.id, auth.0).await?;
        ensure_storefront_cart_access(&cart, storefront_customer_id)?;
        let cart = reprice_storefront_cart_line_items(
            &app_ctx,
            tenant.id,
            &cart_service,
            cart,
            Some(&request_context),
        )
        .await?;

        let service = rustok_commerce::PaymentService::new(app_ctx.db.clone());
        if let Some(existing) = service
            .find_reusable_collection_by_cart(tenant.id, cart.id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?
        {
            return Ok(map_native_payment_collection(existing));
        }

        let context = rustok_commerce::StoreContextService::new(app_ctx.db.clone())
            .resolve_context(
                tenant.id,
                rustok_commerce::ResolveStoreContextInput {
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
            .map_err(|err| ServerFnError::new(err.to_string()))?;

        let collection = service
            .create_collection(
                tenant.id,
                rustok_commerce::CreatePaymentCollectionInput {
                    cart_id: Some(cart.id),
                    order_id: None,
                    customer_id: cart.customer_id,
                    currency_code: cart.currency_code.clone(),
                    amount: cart.total_amount,
                    metadata: merge_metadata(json!({}), cart_context_metadata(&cart, &context)),
                },
            )
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;

        Ok(map_native_payment_collection(collection))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = cart_id;
        Err(ServerFnError::new(
            "commerce/create-payment-collection requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "commerce/select-shipping-option")]
async fn storefront_select_shipping_option(
    cart_id: String,
    shipping_profile_slug: String,
    seller_id: Option<String>,
    seller_scope: Option<String>,
    shipping_option_id: Option<String>,
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
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let Some((_, parsed_cart_id)) =
            parse_cart_id(Some(cart_id)).map_err(|err| ServerFnError::new(err.to_string()))?
        else {
            return Err(ServerFnError::new("cart_id must not be empty"));
        };
        let normalized_shipping_profile_slug =
            normalize_shipping_profile_slug(shipping_profile_slug)
                .map_err(|err| ServerFnError::new(err.to_string()))?;
        let parsed_shipping_option_id =
            parse_optional_uuid(shipping_option_id, "shipping_option_id")
                .map_err(|err| ServerFnError::new(err.to_string()))?;

        let cart_service = rustok_commerce::CartService::new(app_ctx.db.clone());
        let cart = cart_service
            .get_cart(tenant.id, parsed_cart_id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        let storefront_customer_id =
            resolve_storefront_customer_id(app_ctx.db.clone(), tenant.id, auth.0).await?;
        ensure_storefront_cart_access(&cart, storefront_customer_id)?;

        let shipping_selections = build_native_shipping_selections(
            &cart,
            normalized_shipping_profile_slug.as_str(),
            seller_id.as_deref(),
            seller_scope.as_deref(),
            parsed_shipping_option_id,
        )?;

        let updated_cart = cart_service
            .update_context(
                tenant.id,
                parsed_cart_id,
                rustok_commerce::UpdateCartContextInput {
                    email: cart.email.clone(),
                    region_id: cart.region_id,
                    country_code: cart.country_code.clone(),
                    locale_code: cart.locale_code.clone(),
                    selected_shipping_option_id: None,
                    shipping_selections: Some(shipping_selections),
                },
            )
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        let _ = reprice_storefront_cart_line_items(
            &app_ctx,
            tenant.id,
            &cart_service,
            updated_cart,
            request_context.as_ref(),
        )
        .await?;

        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (cart_id, shipping_profile_slug, shipping_option_id);
        Err(ServerFnError::new(
            "commerce/select-shipping-option requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
async fn reprice_storefront_cart_line_items(
    app_ctx: &loco_rs::app::AppContext,
    tenant_id: Uuid,
    cart_service: &rustok_commerce::CartService,
    cart: rustok_cart::CartResponse,
    request_context: Option<&rustok_api::RequestContext>,
) -> Result<rustok_cart::CartResponse, ServerFnError> {
    if cart.line_items.is_empty() {
        return Ok(cart);
    }

    let pricing_service = rustok_commerce::PricingService::new(
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
        let pricing_context = rustok_commerce::services::PriceResolutionContext {
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
            .map_err(|err| ServerFnError::new(err.to_string()))?
            .ok_or_else(|| {
                ServerFnError::new("Unable to resolve storefront price for cart line item")
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
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
}

#[cfg(feature = "ssr")]
fn storefront_cart_pricing_update(
    line_item_id: Uuid,
    quantity: i32,
    resolved_price: &rustok_commerce::services::ResolvedPrice,
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

#[server(prefix = "/api/fn", endpoint = "commerce/complete-checkout")]
async fn storefront_complete_checkout(
    cart_id: String,
) -> Result<StorefrontCheckoutCompletion, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;

        let app_ctx = expect_context::<AppContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
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

        let cart_service = rustok_commerce::CartService::new(app_ctx.db.clone());
        let cart = cart_service
            .get_cart(tenant.id, parsed_cart_id)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        let storefront_customer_id =
            resolve_storefront_customer_id(app_ctx.db.clone(), tenant.id, auth.0.clone()).await?;
        ensure_storefront_cart_access(&cart, storefront_customer_id)?;
        let _ = reprice_storefront_cart_line_items(
            &app_ctx,
            tenant.id,
            &cart_service,
            cart,
            Some(&request_context),
        )
        .await?;
        let actor_id = auth.0.map(|auth| auth.user_id).unwrap_or_else(Uuid::nil);

        let response = rustok_commerce::CheckoutService::new(
            app_ctx.db.clone(),
            rustok_api::loco::transactional_event_bus_from_context(&app_ctx),
        )
        .complete_checkout(
            tenant.id,
            actor_id,
            rustok_commerce::CompleteCheckoutInput {
                cart_id: parsed_cart_id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: json!({}),
            },
        )
        .await
        .map_err(|err| ServerFnError::new(err.to_string()))?;

        Ok(map_native_checkout_completion(response))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = cart_id;
        Err(ServerFnError::new(
            "commerce/complete-checkout requires the `ssr` feature",
        ))
    }
}
