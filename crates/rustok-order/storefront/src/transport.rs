#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompleteCheckoutRequest {
    pub cart_id: String,
}

pub fn build_complete_checkout_request(cart_id: String) -> CompleteCheckoutRequest {
    CompleteCheckoutRequest {
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
    fn complete_request_trims_cart_id() {
        let request = build_complete_checkout_request(" cart-1 ".into());
        assert_eq!(request.cart_id, "cart-1");
    }
}
