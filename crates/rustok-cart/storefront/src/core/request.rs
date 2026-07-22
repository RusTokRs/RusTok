use super::{
    CartLineItemQuantityCommand, decrement_quantity_command, identifiers::normalize_cart_id,
};
use rustok_ui_core::{normalize_optional_ui_text, normalize_required_ui_text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartFetchRequest {
    pub selected_cart_id: Option<String>,
    pub locale: Option<String>,
}

pub fn build_cart_fetch_request(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> CartFetchRequest {
    CartFetchRequest {
        selected_cart_id: normalize_cart_id(selected_cart_id),
        locale: normalize_optional_ui_text(locale),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartLineItemMutationRequest {
    pub cart_id: String,
    pub line_item_id: String,
}

pub fn build_remove_line_item_request(
    cart_id: String,
    line_item_id: String,
) -> CartLineItemMutationRequest {
    CartLineItemMutationRequest {
        cart_id: normalize_required_ui_text(cart_id),
        line_item_id: normalize_required_ui_text(line_item_id),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartLineItemDecrementRequest {
    pub cart_id: String,
    pub line_item_id: String,
    pub current_quantity: i32,
    pub command: CartLineItemQuantityCommand,
}

pub fn build_decrement_line_item_request(
    cart_id: String,
    line_item_id: String,
    current_quantity: i32,
) -> CartLineItemDecrementRequest {
    CartLineItemDecrementRequest {
        cart_id: normalize_required_ui_text(cart_id),
        line_item_id: normalize_required_ui_text(line_item_id),
        current_quantity,
        command: decrement_quantity_command(current_quantity),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cart_fetch_request_keeps_route_state_framework_agnostic() {
        let request = build_cart_fetch_request(
            Some(" 550e8400-e29b-41d4-a716-446655440000 ".to_string()),
            Some(" ru ".to_string()),
        );
        let empty = build_cart_fetch_request(Some("  ".to_string()), Some("  ".to_string()));

        assert_eq!(
            request.selected_cart_id,
            Some("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
        assert_eq!(request.locale, Some("ru".to_string()));
        assert_eq!(empty.selected_cart_id, None);
        assert_eq!(empty.locale, None);
    }

    #[test]
    fn decrement_request_carries_core_quantity_policy() {
        let update =
            build_decrement_line_item_request(" cart-1 ".to_string(), " line-1 ".to_string(), 3);
        let remove =
            build_decrement_line_item_request("cart-1".to_string(), "line-1".to_string(), 1);

        assert_eq!(update.cart_id, "cart-1");
        assert_eq!(update.line_item_id, "line-1");
        assert_eq!(update.current_quantity, 3);
        assert_eq!(
            update.command,
            CartLineItemQuantityCommand::Update { next_quantity: 2 }
        );
        assert_eq!(remove.command, CartLineItemQuantityCommand::Remove);
    }

    #[test]
    fn remove_request_keeps_line_item_identity_owned_by_core() {
        let request =
            build_remove_line_item_request(" cart-1 ".to_string(), " line-1 ".to_string());

        assert_eq!(request.cart_id, "cart-1");
        assert_eq!(request.line_item_id, "line-1");
    }
}
