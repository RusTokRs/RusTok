use leptos_graphql::{execute, GraphqlRequest};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::{PaymentCollection, PaymentCollectionCreateRequest, PaymentCollectionTransportError};

const CREATE_STOREFRONT_PAYMENT_COLLECTION_MUTATION: &str = "mutation CreateStorefrontPaymentCollection($input: CreateStorefrontPaymentCollectionInput!) { createStorefrontPaymentCollection(input: $input) { id status currencyCode amount authorizedAmount capturedAmount orderId providerId createdAt updatedAt payments { id } } }";

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

pub(super) async fn create_payment_collection(
    request: PaymentCollectionCreateRequest,
) -> Result<PaymentCollection, PaymentCollectionTransportError> {
    let cart_id = Uuid::parse_str(request.cart_id.trim()).map_err(|_| {
        PaymentCollectionTransportError::Validation("cart_id must be a valid UUID".to_string())
    })?;
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
    .map_err(|error| PaymentCollectionTransportError::Graphql(error.to_string()))?;

    let value = response.payment_collection;
    Ok(PaymentCollection {
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
