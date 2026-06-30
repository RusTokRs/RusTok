mod graphql_adapter;
mod native_server_adapter;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutCompletionCommandMetadata {
    pub source_module: String,
    pub source_surface: String,
    pub command: String,
    pub owner_module: String,
    pub create_fulfillment: bool,
}

impl CheckoutCompletionCommandMetadata {
    pub fn storefront_complete() -> Self {
        Self {
            source_module: "rustok-commerce".into(),
            source_surface: "storefront_checkout_workspace".into(),
            command: "complete_checkout".into(),
            owner_module: "rustok-order".into(),
            create_fulfillment: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompleteCheckoutRequest {
    pub cart_id: String,
    pub metadata: CheckoutCompletionCommandMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutAdjustment {
    pub id: String,
    pub line_item_id: Option<String>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub scope: Option<String>,
    pub amount: String,
    pub currency_code: String,
    pub metadata: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutCompletion {
    pub order_id: String,
    pub order_status: String,
    pub currency_code: String,
    pub shipping_total: String,
    pub adjustment_total: String,
    pub total_amount: String,
    pub adjustments: Vec<CheckoutAdjustment>,
    pub payment_collection_id: String,
    pub payment_collection_status: String,
    pub fulfillment_count: u64,
    pub context_locale: String,
    pub context_currency_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckoutCompletionTransportError {
    Graphql(String),
    ServerFn(String),
    Validation(String),
}

impl CheckoutCompletionTransportError {
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

pub async fn complete_checkout(
    request: CompleteCheckoutRequest,
) -> Result<CheckoutCompletion, CheckoutCompletionTransportError> {
    match native_server_adapter::complete_checkout(request.clone()).await {
        Ok(completion) => Ok(completion),
        Err(error) if error.should_fallback_to_graphql() => {
            graphql_adapter::complete_checkout(request).await
        }
        Err(error) => Err(error),
    }
}

pub fn build_complete_checkout_request(cart_id: String) -> CompleteCheckoutRequest {
    CompleteCheckoutRequest {
        cart_id: normalize_required(cart_id),
        metadata: CheckoutCompletionCommandMetadata::storefront_complete(),
    }
}

fn normalize_required(value: String) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_request_trims_cart_id() {
        let request = build_complete_checkout_request(" cart-1 ".into());
        assert_eq!(request.cart_id, "cart-1");
    }

    #[test]
    fn complete_request_carries_order_owned_command_metadata() {
        let request = build_complete_checkout_request("cart-1".into());
        assert_eq!(request.metadata.owner_module, "rustok-order");
        assert_eq!(request.metadata.command, "complete_checkout");
        assert!(request.metadata.create_fulfillment);
    }

    #[test]
    fn server_function_missing_error_can_fallback_to_graphql() {
        assert!(
            CheckoutCompletionTransportError::ServerFn("MissingServerFunction".into())
                .should_fallback_to_graphql()
        );
        assert!(CheckoutCompletionTransportError::ServerFn(
            "server function is not available on this target".into()
        )
        .should_fallback_to_graphql());
        assert!(
            !CheckoutCompletionTransportError::Validation("bad cart".into())
                .should_fallback_to_graphql()
        );
        assert!(!CheckoutCompletionTransportError::Graphql("network".into())
            .should_fallback_to_graphql());
    }
}
