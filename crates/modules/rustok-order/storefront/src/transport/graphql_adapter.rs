use rustok_graphql::{GraphqlRequest, execute};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    CheckoutAdjustment, CheckoutCompletion, CheckoutCompletionTransportError,
    CompleteCheckoutRequest,
};

const COMPLETE_STOREFRONT_CHECKOUT_MUTATION: &str = "mutation CompleteStorefrontCheckout($idempotencyKey: String!, $input: CompleteStorefrontCheckoutInput!) { completeStorefrontCheckout(idempotencyKey: $idempotencyKey, input: $input) { order { id status currencyCode shippingTotal adjustmentTotal totalAmount adjustments { id lineItemId sourceType sourceId amount currencyCode metadata } } paymentCollection { id status currencyCode } fulfillments { id } context { locale currencyCode } } }";

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

fn graphql_url() -> String {
    if let Ok(url) = std::env::var("RUSTOK_GRAPHQL_URL") {
        return url;
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
