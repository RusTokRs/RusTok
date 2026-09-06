use rustok_graphql::{GraphqlRequest, execute};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::{
    PaymentCollection, PaymentCollectionCreateRequest, PaymentCollectionFetchRequest,
    PaymentTransportError, RefundSummary, RefundSummaryFetchRequest,
};

const STOREFRONT_REFUNDS_QUERY: &str = "query StorefrontRefundsSummary($orderId: UUID!, $filter: StorefrontRefundsFilter) { storefrontRefunds(orderId: $orderId, filter: $filter) { total items { amount status } } }";
const STOREFRONT_PAYMENT_COLLECTION_QUERY: &str = "query StorefrontPaymentCollection($cartId: UUID!) { storefrontPaymentCollection(cartId: $cartId) { id status currencyCode amount authorizedAmount capturedAmount orderId providerId createdAt updatedAt payments { id } } }";
const CREATE_STOREFRONT_PAYMENT_COLLECTION_MUTATION: &str = "mutation CreateStorefrontPaymentCollection($input: CreateStorefrontPaymentCollectionInput!) { createStorefrontPaymentCollection(input: $input) { id status currencyCode amount authorizedAmount capturedAmount orderId providerId createdAt updatedAt payments { id } } }";

#[derive(Debug, Deserialize)]
struct StorefrontPaymentCollectionResponse {
    #[serde(rename = "storefrontPaymentCollection")]
    payment_collection: Option<GraphqlPaymentCollection>,
}

#[derive(Debug, Serialize)]
struct StorefrontPaymentCollectionVariables {
    #[serde(rename = "cartId")]
    cart_id: Uuid,
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
struct StorefrontRefundsSummaryResponse {
    #[serde(rename = "storefrontRefunds")]
    storefront_refunds: GraphqlRefundList,
}

#[derive(Debug, Deserialize)]
struct GraphqlRefundList {
    total: u64,
    items: Vec<GraphqlRefundItem>,
}

#[derive(Debug, Deserialize)]
struct GraphqlRefundItem {
    amount: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct StorefrontRefundsSummaryVariables {
    #[serde(rename = "orderId")]
    order_id: Uuid,
    filter: StorefrontRefundsSummaryFilter,
}

#[derive(Debug, Serialize)]
struct StorefrontRefundsSummaryFilter {
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}

pub(super) async fn fetch_refund_summary(
    request: RefundSummaryFetchRequest,
) -> Result<RefundSummary, PaymentTransportError> {
    let order_id = parse_uuid(&request.order_id, "order_id")?;
    let response: StorefrontRefundsSummaryResponse = execute(
        &graphql_url(),
        GraphqlRequest::new(
            STOREFRONT_REFUNDS_QUERY,
            Some(StorefrontRefundsSummaryVariables {
                order_id,
                filter: StorefrontRefundsSummaryFilter {
                    page: 1,
                    per_page: 50,
                },
            }),
        ),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| PaymentTransportError::Graphql(error.to_string()))?;

    Ok(summarize_refunds(
        &response.storefront_refunds.items,
        response.storefront_refunds.total,
    ))
}

pub(super) async fn fetch_payment_collection(
    request: PaymentCollectionFetchRequest,
) -> Result<Option<PaymentCollection>, PaymentTransportError> {
    let cart_id = parse_cart_id(&request.cart_id)?;
    let response: StorefrontPaymentCollectionResponse = execute(
        &graphql_url(),
        GraphqlRequest::new(
            STOREFRONT_PAYMENT_COLLECTION_QUERY,
            Some(StorefrontPaymentCollectionVariables { cart_id }),
        ),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| PaymentTransportError::Graphql(error.to_string()))?;

    Ok(response.payment_collection.map(map_payment_collection))
}

pub(super) async fn create_payment_collection(
    request: PaymentCollectionCreateRequest,
) -> Result<PaymentCollection, PaymentTransportError> {
    let cart_id = parse_cart_id(&request.cart_id)?;
    let metadata = request.metadata;
    let response: CreateStorefrontPaymentCollectionResponse = execute(
        &graphql_url(),
        GraphqlRequest::new(
            CREATE_STOREFRONT_PAYMENT_COLLECTION_MUTATION,
            Some(CreateStorefrontPaymentCollectionVariables {
                input: CreateStorefrontPaymentCollectionInput {
                    cart_id,
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
    .map_err(|error| PaymentTransportError::Graphql(error.to_string()))?;

    Ok(map_payment_collection(response.payment_collection))
}

fn parse_cart_id(value: &str) -> Result<Uuid, PaymentTransportError> {
    parse_uuid(value, "cart_id")
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid, PaymentTransportError> {
    Uuid::parse_str(value.trim())
        .map_err(|_| PaymentTransportError::Validation(format!("{field} must be a valid UUID")))
}

fn summarize_refunds(items: &[GraphqlRefundItem], total: u64) -> RefundSummary {
    use std::str::FromStr;

    let refunded_amount = items
        .iter()
        .filter_map(|item| rust_decimal::Decimal::from_str(item.amount.trim()).ok())
        .fold(rust_decimal::Decimal::ZERO, |acc, value| acc + value);
    RefundSummary {
        total,
        refunded_amount: (total > 0).then(|| refunded_amount.normalize().to_string()),
        latest_status: items.first().map(|item| item.status.clone()),
    }
}

fn map_payment_collection(value: GraphqlPaymentCollection) -> PaymentCollection {
    PaymentCollection {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refund_summary_uses_decimal_safe_total_and_latest_status() {
        let summary = summarize_refunds(
            &[
                GraphqlRefundItem {
                    amount: "0.10".into(),
                    status: "pending".into(),
                },
                GraphqlRefundItem {
                    amount: "0.20".into(),
                    status: "refunded".into(),
                },
            ],
            2,
        );

        assert_eq!(summary.refunded_amount.as_deref(), Some("0.3"));
        assert_eq!(summary.latest_status.as_deref(), Some("pending"));
    }

    #[test]
    fn empty_refund_summary_has_no_amount() {
        let summary = summarize_refunds(&[], 0);
        assert_eq!(summary.refunded_amount, None);
        assert_eq!(summary.latest_status, None);
    }
}
