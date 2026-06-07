pub const DEFAULT_PROMOTION_KIND: &str = "fixed_discount";
pub const DEFAULT_PROMOTION_SCOPE: &str = "shipping";
pub const DEFAULT_PROMOTION_SOURCE_ID: &str = "promo-operator";
pub const DEFAULT_PROMOTION_AMOUNT: &str = "4.99";
pub const DEFAULT_ORDER_CHANGE_STATUS: &str = "pending";

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_defaults_are_stable_for_admin_form() {
        assert_eq!(DEFAULT_PROMOTION_KIND, "fixed_discount");
        assert_eq!(DEFAULT_PROMOTION_SCOPE, "shipping");
        assert_eq!(DEFAULT_PROMOTION_SOURCE_ID, "promo-operator");
        assert_eq!(DEFAULT_PROMOTION_AMOUNT, "4.99");
    }

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

    #[test]
    fn order_change_action_command_trims_payload_and_requires_id() {
        assert!(prepare_order_change_action_command(" ", "{}", "reason").is_none());

        let command = prepare_order_change_action_command(
            " change-1 ",
            " {\"operator\":true} ",
            " cancelled ",
        )
        .expect("valid command");

        assert_eq!(command.change_id, "change-1");
        assert_eq!(command.draft.metadata_json, "{\"operator\":true}");
        assert_eq!(command.draft.reason, "cancelled");
    }

    #[test]
    fn badge_classes_are_stable_for_host_adapters() {
        assert_eq!(
            active_badge_class(true),
            "border-emerald-200 bg-emerald-50 text-emerald-700"
        );
        assert_eq!(
            active_badge_class(false),
            "border-slate-200 bg-slate-100 text-slate-700"
        );
        assert_eq!(
            order_change_status_badge_class("applied"),
            "border-emerald-200 bg-emerald-50 text-emerald-700"
        );
        assert_eq!(
            order_change_status_badge_class("cancelled"),
            "border-rose-200 bg-rose-50 text-rose-700"
        );
        assert_eq!(
            order_change_status_badge_class("pending"),
            "border-amber-200 bg-amber-50 text-amber-700"
        );
    }
}

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

pub struct OrderChangeActionCommand {
    pub change_id: String,
    pub draft: crate::model::CommerceOrderChangeActionDraft,
}

pub fn prepare_order_change_action_command(
    change_id: &str,
    metadata_json: &str,
    reason: &str,
) -> Option<OrderChangeActionCommand> {
    let change_id = change_id.trim().to_string();

    if change_id.is_empty() {
        return None;
    }

    Some(OrderChangeActionCommand {
        change_id,
        draft: crate::model::CommerceOrderChangeActionDraft {
            metadata_json: metadata_json.trim().to_string(),
            reason: reason.trim().to_string(),
        },
    })
}

pub fn active_badge_class(active: bool) -> &'static str {
    if active {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    } else {
        "border-slate-200 bg-slate-100 text-slate-700"
    }
}

pub fn order_change_status_badge_class(status: &str) -> &'static str {
    match status {
        "applied" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "cancelled" => "border-rose-200 bg-rose-50 text-rose-700",
        _ => "border-amber-200 bg-amber-50 text-amber-700",
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrderChangeResolutionSummary {
    pub order_return_id: Option<String>,
    pub return_decision_action: Option<String>,
    pub return_decision_source: Option<String>,
    pub cancellation_reason: Option<String>,
}

impl OrderChangeResolutionSummary {
    pub fn has_any(&self) -> bool {
        self.order_return_id.is_some()
            || self.return_decision_action.is_some()
            || self.return_decision_source.is_some()
            || self.cancellation_reason.is_some()
    }
}

pub fn order_change_resolution_summary(
    change: &crate::model::CommerceOrderChange,
) -> OrderChangeResolutionSummary {
    let preview = parse_json_object(change.preview.as_str());
    let metadata = parse_json_object(change.metadata.as_str());

    OrderChangeResolutionSummary {
        order_return_id: json_string(&metadata, "order_return_id")
            .or_else(|| json_string(&preview, "order_return_id")),
        return_decision_action: json_string(&metadata, "return_decision_action")
            .or_else(|| json_string(&preview, "return_decision_action"))
            .or_else(|| {
                Some(change.change_type.clone())
                    .filter(|value| value == "exchange" || value == "claim")
            }),
        return_decision_source: json_string(&metadata, "return_decision_source")
            .or_else(|| json_string(&preview, "return_decision_source")),
        cancellation_reason: json_string(&metadata, "cancellation_reason"),
    }
}

fn parse_json_object(value: &str) -> Option<serde_json::Map<String, serde_json::Value>> {
    serde_json::from_str::<serde_json::Value>(value)
        .ok()
        .and_then(|value| value.as_object().cloned())
}

fn json_string(
    object: &Option<serde_json::Map<String, serde_json::Value>>,
    key: &str,
) -> Option<String> {
    object
        .as_ref()
        .and_then(|object| object.get(key))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod order_change_resolution_tests {
    use super::*;
    use crate::model::CommerceOrderChange;

    fn order_change_with_payload(preview: &str, metadata: &str) -> CommerceOrderChange {
        CommerceOrderChange {
            id: "change-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            order_id: "order-1".to_string(),
            created_by: "operator-1".to_string(),
            change_type: "exchange".to_string(),
            status: "pending".to_string(),
            description: None,
            preview: preview.to_string(),
            metadata: metadata.to_string(),
            created_at: "2026-06-02T00:00:00Z".to_string(),
            updated_at: "2026-06-02T00:00:00Z".to_string(),
            applied_at: None,
            cancelled_at: None,
        }
    }

    #[test]
    fn order_change_resolution_summary_prefers_metadata_context() {
        let change = order_change_with_payload(
            r#"{"order_return_id":"preview-return","return_decision_action":"claim"}"#,
            r#"{"order_return_id":"metadata-return","return_decision_action":"exchange","return_decision_source":"rustok-commerce","cancellation_reason":"operator rejected"}"#,
        );

        let summary = order_change_resolution_summary(&change);

        assert_eq!(summary.order_return_id.as_deref(), Some("metadata-return"));
        assert_eq!(summary.return_decision_action.as_deref(), Some("exchange"));
        assert_eq!(
            summary.return_decision_source.as_deref(),
            Some("rustok-commerce")
        );
        assert_eq!(
            summary.cancellation_reason.as_deref(),
            Some("operator rejected")
        );
        assert!(summary.has_any());
    }

    #[test]
    fn order_change_resolution_summary_falls_back_to_preview_and_change_type() {
        let change = order_change_with_payload(r#"{"order_return_id":"preview-return"}"#, "{}");

        let summary = order_change_resolution_summary(&change);

        assert_eq!(summary.order_return_id.as_deref(), Some("preview-return"));
        assert_eq!(summary.return_decision_action.as_deref(), Some("exchange"));
        assert!(summary.return_decision_source.is_none());
        assert!(summary.cancellation_reason.is_none());
    }
}
