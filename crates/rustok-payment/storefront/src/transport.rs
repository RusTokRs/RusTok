#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentCollectionCreateRequest {
    pub cart_id: String,
}

pub fn build_payment_collection_create_request(cart_id: String) -> PaymentCollectionCreateRequest {
    PaymentCollectionCreateRequest {
        cart_id: normalize_required(cart_id),
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
}
