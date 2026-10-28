pub struct CartPromotionCommand {
    pub cart_id: String,
    pub draft: crate::model::CommerceCartPromotionDraft,
}

pub struct CartPromotionForm<'a> {
    pub cart_id: &'a str,
    pub kind: &'a str,
    pub scope: &'a str,
    pub line_item_id: &'a str,
    pub source_id: &'a str,
    pub discount_percent: &'a str,
    pub amount: &'a str,
    pub metadata_json: &'a str,
}

pub fn prepare_cart_promotion_command(form: CartPromotionForm<'_>) -> Option<CartPromotionCommand> {
    let cart_id = form.cart_id.trim().to_string();
    let source_id = form.source_id.trim().to_string();

    if cart_id.is_empty() || source_id.is_empty() {
        return None;
    }

    let kind = parse_promotion_kind(form.kind);
    let scope = parse_promotion_scope(form.scope);

    let (discount_percent, amount) = match kind {
        crate::model::CommerceCartPromotionKind::PercentageDiscount => {
            let discount_percent = form.discount_percent.trim().to_string();
            if discount_percent.is_empty() {
                return None;
            }
            (discount_percent, String::new())
        }
        crate::model::CommerceCartPromotionKind::FixedDiscount => {
            let amount = form.amount.trim().to_string();
            if amount.is_empty() {
                return None;
            }
            (String::new(), amount)
        }
    };

    let line_item_id = match scope {
        crate::model::CommerceCartPromotionScope::LineItem => {
            let line_item_id = form.line_item_id.trim().to_string();
            if line_item_id.is_empty() {
                return None;
            }
            line_item_id
        }
        crate::model::CommerceCartPromotionScope::Cart
        | crate::model::CommerceCartPromotionScope::Shipping => String::new(),
    };

    let metadata_trimmed = form.metadata_json.trim();
    if !metadata_trimmed.is_empty()
        && serde_json::from_str::<serde_json::Value>(metadata_trimmed).is_err()
    {
        return None;
    }

    Some(CartPromotionCommand {
        cart_id,
        draft: crate::model::CommerceCartPromotionDraft {
            kind,
            scope,
            line_item_id,
            source_id,
            discount_percent,
            amount,
            metadata_json: metadata_trimmed.to_string(),
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
        let command = prepare_cart_promotion_command(CartPromotionForm {
            cart_id: " cart-1 ",
            kind: "percentage_discount",
            scope: "line_item",
            line_item_id: " line-1 ",
            source_id: " promo ",
            discount_percent: " 10 ",
            amount: " 4.99 ",
            metadata_json: " { } ",
        })
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
        assert_eq!(command.draft.amount, "");
        assert_eq!(command.draft.metadata_json, "{ }");
    }

    #[test]
    fn cart_promotion_command_normalizes_fixed_discount_and_shipping_scope() {
        let command = prepare_cart_promotion_command(CartPromotionForm {
            cart_id: "cart-2",
            kind: "fixed_discount",
            scope: "shipping",
            line_item_id: "item-should-be-cleared",
            source_id: "promo-shipping",
            discount_percent: "25",
            amount: "7.50",
            metadata_json: "{}",
        })
        .expect("valid command");

        assert_eq!(command.cart_id, "cart-2");
        assert_eq!(
            command.draft.kind,
            crate::model::CommerceCartPromotionKind::FixedDiscount
        );
        assert_eq!(
            command.draft.scope,
            crate::model::CommerceCartPromotionScope::Shipping
        );
        assert_eq!(command.draft.line_item_id, "");
        assert_eq!(command.draft.discount_percent, "");
        assert_eq!(command.draft.amount, "7.50");
    }

    #[test]
    fn cart_promotion_command_requires_cart_and_source() {
        assert!(
            prepare_cart_promotion_command(CartPromotionForm {
                cart_id: "",
                kind: DEFAULT_PROMOTION_KIND,
                scope: DEFAULT_PROMOTION_SCOPE,
                line_item_id: "",
                source_id: "source",
                discount_percent: "",
                amount: DEFAULT_PROMOTION_AMOUNT,
                metadata_json: "",
            })
            .is_none()
        );
        assert!(
            prepare_cart_promotion_command(CartPromotionForm {
                cart_id: "cart",
                kind: DEFAULT_PROMOTION_KIND,
                scope: DEFAULT_PROMOTION_SCOPE,
                line_item_id: "",
                source_id: "  ",
                discount_percent: "",
                amount: DEFAULT_PROMOTION_AMOUNT,
                metadata_json: "",
            })
            .is_none()
        );
    }

    #[test]
    fn cart_promotion_command_requires_kind_and_scope_fields() {
        assert!(
            prepare_cart_promotion_command(CartPromotionForm {
                cart_id: "cart-1",
                kind: "percentage_discount",
                scope: "shipping",
                line_item_id: "",
                source_id: "promo",
                discount_percent: "  ",
                amount: "5.00",
                metadata_json: "",
            })
            .is_none()
        );

        assert!(
            prepare_cart_promotion_command(CartPromotionForm {
                cart_id: "cart-1",
                kind: "fixed_discount",
                scope: "shipping",
                line_item_id: "",
                source_id: "promo",
                discount_percent: "10",
                amount: "  ",
                metadata_json: "",
            })
            .is_none()
        );

        assert!(
            prepare_cart_promotion_command(CartPromotionForm {
                cart_id: "cart-1",
                kind: "fixed_discount",
                scope: "line_item",
                line_item_id: "  ",
                source_id: "promo",
                discount_percent: "",
                amount: "5.00",
                metadata_json: "",
            })
            .is_none()
        );

        assert!(
            prepare_cart_promotion_command(CartPromotionForm {
                cart_id: "cart-1",
                kind: "fixed_discount",
                scope: "shipping",
                line_item_id: "",
                source_id: "promo",
                discount_percent: "",
                amount: "5.00",
                metadata_json: "{not-json",
            })
            .is_none()
        );
    }
}
