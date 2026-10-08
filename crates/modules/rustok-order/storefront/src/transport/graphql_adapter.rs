use rustok_graphql::{GraphqlRequest, execute, graphql_url};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::model::{
    StorefrontOrder, StorefrontOrderAdjustment, StorefrontOrderLineItem, StorefrontOrdersResponse,
};

use super::{
    CheckoutAdjustment, CheckoutCompletion, CheckoutCompletionTransportError,
    CompleteCheckoutRequest,
};

const COMPLETE_STOREFRONT_CHECKOUT_MUTATION: &str = "mutation CompleteStorefrontCheckout($idempotencyKey: String!, $input: CompleteStorefrontCheckoutInput!) { completeStorefrontCheckout(idempotencyKey: $idempotencyKey, input: $input) { order { id status currencyCode shippingTotal adjustmentTotal totalAmount adjustments { id lineItemId sourceType sourceId amount currencyCode metadata } } paymentCollection { id status currencyCode } fulfillments { id } context { locale currencyCode } } }";

const STOREFRONT_ORDER_QUERY: &str = "query StorefrontOrder($id: UUID!) { storefrontOrder(id: $id) { id status currencyCode subtotalAmount adjustmentTotal shippingTotal totalAmount taxTotal taxIncluded metadata paymentId paymentMethod trackingNumber carrier cancellationReason deliveredSignature createdAt updatedAt confirmedAt paidAt shippedAt deliveredAt cancelledAt lineItems { id productId variantId sku title quantity unitPrice totalPrice currencyCode } adjustments { id lineItemId sourceType sourceId amount currencyCode } } }";

const STOREFRONT_ORDERS_QUERY: &str = "query StorefrontOrders($page: Int, $perPage: Int, $status: String) { storefrontOrders(page: $page, perPage: $perPage, status: $status) { items { id status currencyCode subtotalAmount adjustmentTotal shippingTotal totalAmount taxTotal taxIncluded metadata paymentId paymentMethod trackingNumber carrier cancellationReason deliveredSignature createdAt updatedAt confirmedAt paidAt shippedAt deliveredAt cancelledAt lineItems { id productId variantId sku title quantity unitPrice totalPrice currencyCode } adjustments { id lineItemId sourceType sourceId amount currencyCode } } total page perPage hasNext } }";

#[derive(Debug, Deserialize)]
struct CompleteStorefrontCheckoutResponse {
    #[serde(rename = "completeStorefrontCheckout")]
    completion: GraphqlCheckoutCompletion,
}

#[derive(Debug, Serialize)]
struct CompleteStorefrontCheckoutVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
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

pub(super) async fn complete_checkout(
    request: CompleteCheckoutRequest,
) -> Result<CheckoutCompletion, CheckoutCompletionTransportError> {
    let cart_id = Uuid::parse_str(request.cart_id.trim()).map_err(|_| {
        CheckoutCompletionTransportError::Validation("cart_id must be a valid UUID".to_string())
    })?;
    let idempotency_key = request.idempotency_key.trim().to_string();
    if idempotency_key.is_empty() || idempotency_key.len() > 191 {
        return Err(CheckoutCompletionTransportError::Validation(
            "checkout idempotency key must contain 1 to 191 bytes".to_string(),
        ));
    }
    let metadata = request.metadata;
    let response: CompleteStorefrontCheckoutResponse = execute(
        &graphql_url(),
        GraphqlRequest::new(
            COMPLETE_STOREFRONT_CHECKOUT_MUTATION,
            Some(CompleteStorefrontCheckoutVariables {
                idempotency_key,
                input: CompleteStorefrontCheckoutInput {
                    cart_id,
                    create_fulfillment: metadata.create_fulfillment,
                    metadata: Some(
                        json!({
                            "source_module": metadata.source_module,
                            "source_surface": metadata.source_surface,
                            "command": metadata.command,
                            "owner_module": metadata.owner_module,
                        })
                        .to_string(),
                    ),
                },
            }),
        ),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| CheckoutCompletionTransportError::Graphql(error.to_string()))?;

    let value = response.completion;
    let adjustments = value
        .order
        .adjustments
        .into_iter()
        .map(|adjustment| CheckoutAdjustment {
            id: adjustment.id,
            line_item_id: adjustment.line_item_id,
            source_type: adjustment.source_type,
            source_id: adjustment.source_id,
            scope: serde_json::from_str::<Value>(&adjustment.metadata)
                .ok()
                .and_then(|metadata| {
                    metadata
                        .get("scope")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                }),
            amount: adjustment.amount,
            currency_code: adjustment.currency_code,
            metadata: adjustment.metadata,
        })
        .collect();

    Ok(CheckoutCompletion {
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
    })
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
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        })
    })
}

#[derive(Debug, Deserialize)]
struct StorefrontOrderQueryResponse {
    #[serde(rename = "storefrontOrder")]
    order: Option<GqlStorefrontOrderPayload>,
}

#[derive(Debug, Deserialize)]
struct StorefrontOrdersQueryResponse {
    #[serde(rename = "storefrontOrders")]
    orders: GqlStorefrontOrdersPayload,
}

#[derive(Debug, Deserialize)]
struct GqlStorefrontOrdersPayload {
    #[serde(default)]
    items: Vec<GqlStorefrontOrderPayload>,
    #[serde(default)]
    total: u64,
    #[serde(default)]
    page: u64,
    #[serde(rename = "perPage", default)]
    per_page: u64,
    #[serde(rename = "hasNext", default)]
    has_next: bool,
}

#[derive(Debug, Deserialize)]
struct GqlStorefrontOrderPayload {
    id: String,
    status: String,
    #[serde(rename = "currencyCode")]
    currency_code: String,
    #[serde(rename = "subtotalAmount", default)]
    subtotal_amount: String,
    #[serde(rename = "adjustmentTotal", default)]
    adjustment_total: String,
    #[serde(rename = "shippingTotal", default)]
    shipping_total: String,
    #[serde(rename = "totalAmount", default)]
    total_amount: String,
    #[serde(rename = "taxTotal", default)]
    tax_total: String,
    #[serde(rename = "taxIncluded", default)]
    tax_included: bool,
    #[serde(default)]
    metadata: String,
    #[serde(rename = "paymentId")]
    payment_id: Option<String>,
    #[serde(rename = "paymentMethod")]
    payment_method: Option<String>,
    #[serde(rename = "trackingNumber")]
    tracking_number: Option<String>,
    carrier: Option<String>,
    #[serde(rename = "cancellationReason")]
    cancellation_reason: Option<String>,
    #[serde(rename = "deliveredSignature")]
    delivered_signature: Option<String>,
    #[serde(rename = "createdAt", default)]
    created_at: String,
    #[serde(rename = "updatedAt", default)]
    updated_at: String,
    #[serde(rename = "confirmedAt")]
    confirmed_at: Option<String>,
    #[serde(rename = "paidAt")]
    paid_at: Option<String>,
    #[serde(rename = "shippedAt")]
    shipped_at: Option<String>,
    #[serde(rename = "deliveredAt")]
    delivered_at: Option<String>,
    #[serde(rename = "cancelledAt")]
    cancelled_at: Option<String>,
    #[serde(rename = "lineItems", default)]
    line_items: Vec<GqlStorefrontOrderLineItemPayload>,
    #[serde(default)]
    adjustments: Vec<GqlStorefrontOrderAdjustmentPayload>,
}

#[derive(Debug, Deserialize)]
struct GqlStorefrontOrderLineItemPayload {
    id: String,
    #[serde(rename = "productId")]
    product_id: Option<String>,
    #[serde(rename = "variantId")]
    variant_id: Option<String>,
    sku: Option<String>,
    title: String,
    quantity: i32,
    #[serde(rename = "unitPrice", default)]
    unit_price: String,
    #[serde(rename = "totalPrice", default)]
    total_price: String,
    #[serde(rename = "currencyCode", default)]
    currency_code: String,
}

#[derive(Debug, Deserialize)]
struct GqlStorefrontOrderAdjustmentPayload {
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

fn map_gql_order(payload: GqlStorefrontOrderPayload) -> StorefrontOrder {
    StorefrontOrder {
        id: payload.id,
        status: payload.status,
        currency_code: payload.currency_code,
        subtotal_amount: payload.subtotal_amount,
        adjustment_total: payload.adjustment_total,
        shipping_total: payload.shipping_total,
        total_amount: payload.total_amount,
        tax_total: payload.tax_total,
        tax_included: payload.tax_included,
        metadata: payload.metadata,
        payment_id: payload.payment_id,
        payment_method: payload.payment_method,
        tracking_number: payload.tracking_number,
        carrier: payload.carrier,
        cancellation_reason: payload.cancellation_reason,
        delivered_signature: payload.delivered_signature,
        created_at: payload.created_at,
        updated_at: payload.updated_at,
        confirmed_at: payload.confirmed_at,
        paid_at: payload.paid_at,
        shipped_at: payload.shipped_at,
        delivered_at: payload.delivered_at,
        cancelled_at: payload.cancelled_at,
        line_items: payload
            .line_items
            .into_iter()
            .map(|li| StorefrontOrderLineItem {
                id: li.id,
                product_id: li.product_id,
                variant_id: li.variant_id,
                sku: li.sku,
                title: li.title,
                quantity: li.quantity,
                unit_price: li.unit_price,
                total_price: li.total_price,
                currency_code: li.currency_code,
            })
            .collect(),
        adjustments: payload
            .adjustments
            .into_iter()
            .map(|adj| StorefrontOrderAdjustment {
                id: adj.id,
                line_item_id: adj.line_item_id,
                source_type: adj.source_type,
                source_id: adj.source_id,
                amount: adj.amount,
                currency_code: adj.currency_code,
            })
            .collect(),
    }
}

#[derive(Debug)]
pub(super) struct OrderQueryTransportError(pub String);

impl std::fmt::Display for OrderQueryTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for OrderQueryTransportError {}

pub(super) async fn fetch_storefront_order(
    order_id: String,
) -> Result<Option<StorefrontOrder>, OrderQueryTransportError> {
    let order_id = order_id.trim();
    if order_id.is_empty() {
        return Ok(None);
    }
    let Ok(uuid) = Uuid::parse_str(order_id) else {
        return Ok(None);
    };

    let response: StorefrontOrderQueryResponse = execute(
        &graphql_url(),
        GraphqlRequest::new(STOREFRONT_ORDER_QUERY, Some(json!({ "id": uuid }))),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| OrderQueryTransportError(error.to_string()))?;

    Ok(response.order.map(map_gql_order))
}

pub(super) async fn fetch_storefront_orders(
    page: Option<u64>,
    per_page: Option<u64>,
    status: Option<String>,
) -> Result<StorefrontOrdersResponse, OrderQueryTransportError> {
    let mut vars = serde_json::Map::new();
    if let Some(p) = page {
        vars.insert("page".into(), json!(p));
    }
    if let Some(pp) = per_page {
        vars.insert("perPage".into(), json!(pp));
    }
    if let Some(s) = status.filter(|s| !s.trim().is_empty()) {
        vars.insert("status".into(), json!(s));
    }

    let response: StorefrontOrdersQueryResponse = execute(
        &graphql_url(),
        GraphqlRequest::new(
            STOREFRONT_ORDERS_QUERY,
            Some(serde_json::Value::Object(vars)),
        ),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| OrderQueryTransportError(error.to_string()))?;

    Ok(StorefrontOrdersResponse {
        items: response
            .orders
            .items
            .into_iter()
            .map(map_gql_order)
            .collect(),
        total: response.orders.total,
        page: response.orders.page,
        per_page: response.orders.per_page,
        has_next: response.orders.has_next,
    })
}
