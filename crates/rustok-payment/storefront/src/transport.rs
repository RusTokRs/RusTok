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
}
