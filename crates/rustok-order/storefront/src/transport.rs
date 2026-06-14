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
}
