pub struct CartPromotionCommand {
    pub cart_id: String,
    pub draft: crate::model::CommerceCartPromotionDraft,
}

pub fn prepare_cart_promotion_command(
    cart_id: &str,
    kind: &str,
    scope: &str,
    line_item_id: &str,
    source_id: &str,
    discount_percent: &str,
    amount: &str,
    metadata_json: &str,
) -> Option<CartPromotionCommand> {
    let cart_id = cart_id.trim().to_string();
    let source_id = source_id.trim().to_string();

    if cart_id.is_empty() || source_id.is_empty() {
        return None;
    }

    Some(CartPromotionCommand {
        cart_id,
        draft: crate::model::CommerceCartPromotionDraft {
            kind: parse_promotion_kind(kind),
            scope: parse_promotion_scope(scope),
            line_item_id: line_item_id.trim().to_string(),
            source_id,
            discount_percent: discount_percent.trim().to_string(),
            amount: amount.trim().to_string(),
            metadata_json: metadata_json.trim().to_string(),
        },
    })
}

pub fn parse_promotion_kind(value: &str) -> crate::model::CommerceCartPromotionKind {
    match value {
        "percentage_discount" => crate::model::CommerceCartPromotionKind::PercentageDiscount,
        _ => crate::model::CommerceCartPromotionKind::FixedDiscount,
    }
}

pub fn parse_promotion_scope(value: &str) -> crate::model::CommerceCartPromotionScope {
    match value {
        "cart" => crate::model::CommerceCartPromotionScope::Cart,
        "line_item" => crate::model::CommerceCartPromotionScope::LineItem,
        _ => crate::model::CommerceCartPromotionScope::Shipping,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{DEFAULT_PROMOTION_AMOUNT, DEFAULT_PROMOTION_KIND, DEFAULT_PROMOTION_SCOPE};
    use super::*;

    #[test]
    fn cart_promotion_command_trims_and_maps_form_values() {
        let command = prepare_cart_promotion_command(
            " cart-1 ",
            "percentage_discount",
            "line_item",
            " line-1 ",
            " promo ",
            " 10 ",
            " 4.99 ",
            " { } ",
        )
        .expect("valid command");

        assert_eq!(command.cart_id, "cart-1");
        assert_eq!(
            command.draft.kind,
            crate::model::CommerceCartPromotionKind::PercentageDiscount
        );
        assert_eq!(
            command.draft.scope,
            crate::model::CommerceCartPromotionScope::LineItem
        );
        assert_eq!(command.draft.line_item_id, "line-1");
        assert_eq!(command.draft.source_id, "promo");
        assert_eq!(command.draft.discount_percent, "10");
        assert_eq!(command.draft.amount, "4.99");
        assert_eq!(command.draft.metadata_json, "{ }");
    }

    #[test]
    fn cart_promotion_command_requires_cart_and_source() {
        assert!(prepare_cart_promotion_command(
            "",
            DEFAULT_PROMOTION_KIND,
            DEFAULT_PROMOTION_SCOPE,
            "",
            "source",
            "",
            DEFAULT_PROMOTION_AMOUNT,
            "",
        )
        .is_none());
        assert!(prepare_cart_promotion_command(
            "cart",
            DEFAULT_PROMOTION_KIND,
            DEFAULT_PROMOTION_SCOPE,
            "",
            "  ",
            "",
            DEFAULT_PROMOTION_AMOUNT,
            "",
        )
        .is_none());
    }
}
