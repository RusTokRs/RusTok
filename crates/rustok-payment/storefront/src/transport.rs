mod graphql_adapter;
mod native_server_adapter;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentCollectionCommandMetadata {
    pub source_module: String,
    pub source_surface: String,
    pub command: String,
    pub owner_module: String,
}

impl PaymentCollectionCommandMetadata {
    pub fn storefront_create() -> Self {
        Self {
            source_module: "rustok-commerce".into(),
            source_surface: "storefront_checkout_workspace".into(),
            command: "create_or_reuse_payment_collection".into(),
            owner_module: "rustok-payment".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentCollectionCreateRequest {
    pub cart_id: String,
    pub metadata: PaymentCollectionCommandMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentCollection {
    pub id: String,
    pub status: String,
    pub currency_code: String,
    pub amount: String,
    pub authorized_amount: String,
    pub captured_amount: String,
    pub order_id: Option<String>,
    pub provider_id: Option<String>,
    pub payment_count: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentCollectionTransportError {
    Graphql(String),
    ServerFn(String),
    Validation(String),
}

impl PaymentCollectionTransportError {
    pub fn message(&self) -> &str {
        match self {
            Self::Graphql(message) | Self::ServerFn(message) | Self::Validation(message) => message,
        }
    }

    pub fn should_fallback_to_graphql(&self) -> bool {
        match self {
            Self::ServerFn(server_error) => {
                server_error.contains("MissingServer")
                    || server_error.contains("missing server")
                    || server_error.contains("not available on this target")
            }
            _ => false,
        }
    }
}

pub async fn create_payment_collection(
    request: PaymentCollectionCreateRequest,
) -> Result<PaymentCollection, PaymentCollectionTransportError> {
    match native_server_adapter::create_payment_collection(request.clone()).await {
        Ok(collection) => Ok(collection),
        Err(error) if error.should_fallback_to_graphql() => {
            graphql_adapter::create_payment_collection(request).await
        }
        Err(error) => Err(error),
    }
}

pub fn build_payment_collection_create_request(cart_id: String) -> PaymentCollectionCreateRequest {
    PaymentCollectionCreateRequest {
        cart_id: normalize_required(cart_id),
        metadata: PaymentCollectionCommandMetadata::storefront_create(),
    }
}

fn normalize_required(value: String) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_request_trims_cart_id() {
        let request = build_payment_collection_create_request(" cart-1 ".into());
        assert_eq!(request.cart_id, "cart-1");
    }

    #[test]
    fn create_request_carries_payment_owned_command_metadata() {
        let request = build_payment_collection_create_request("cart-1".into());
        assert_eq!(request.metadata.owner_module, "rustok-payment");
        assert_eq!(
            request.metadata.command,
            "create_or_reuse_payment_collection"
        );
    }

    #[test]
    fn server_function_missing_error_can_fallback_to_graphql() {
        assert!(
            PaymentCollectionTransportError::ServerFn("MissingServerFunction".into())
                .should_fallback_to_graphql()
        );
        assert!(PaymentCollectionTransportError::ServerFn(
            "server function is not available on this target".into()
        )
        .should_fallback_to_graphql());
        assert!(
            !PaymentCollectionTransportError::Validation("bad cart".into())
                .should_fallback_to_graphql()
        );
        assert!(!PaymentCollectionTransportError::Graphql("network".into())
            .should_fallback_to_graphql());
    }
}
