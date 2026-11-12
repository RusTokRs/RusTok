use crate::core::{
    CartPromotionForm, DEFAULT_ORDER_CHANGE_STATUS, DEFAULT_PROMOTION_AMOUNT,
    DEFAULT_PROMOTION_KIND, DEFAULT_PROMOTION_SCOPE, DEFAULT_PROMOTION_SOURCE_ID,
    active_badge_class, empty_shipping_profile_form_state, localized_order_change_status,
    optional_value, order_change_resolution_summary, order_change_status_badge_class,
    prepare_cart_promotion_command, prepare_order_change_action_command,
    prepare_shipping_profile_draft, trimmed_non_empty,
};
use crate::model::{CommerceCartPromotionKind, CommerceCartPromotionScope, CommerceOrderChange};

#[test]
fn test_crate_api_compliance() {
    let api_doc = include_str!("../CRATE_API.md");
    assert!(api_doc.contains("## Platform Capability Bindings"));
    assert!(api_doc.contains("## Domain Collaborations"));
    assert!(api_doc.contains("## Minimum Contract Set"));
    assert!(api_doc.contains("### Input DTOs/Commands"));
    assert!(api_doc.contains("### Domain Invariants"));
    assert!(api_doc.contains("### Events / Outbox Side Effects"));
    assert!(api_doc.contains("### Errors / Failure Codes"));
}

#[test]
fn ffa_core_boundary_contains_no_leptos_dependencies() {
    let core_files = [
        include_str!("core/common.rs"),
        include_str!("core/order_change.rs"),
        include_str!("core/presentation.rs"),
        include_str!("core/promotion.rs"),
        include_str!("core/shipping_profile.rs"),
    ];

    for code in core_files {
        assert!(
            !code.contains("use leptos"),
            "FFA core boundary must not depend on leptos framework"
        );
        assert!(
            !code.contains("view!"),
            "FFA core boundary must not contain view! macros"
        );
    }
}

#[test]
fn shipping_profile_form_state_and_draft_invariants() {
    let empty_state = empty_shipping_profile_form_state();
    assert_eq!(empty_state.slug, "");
    assert_eq!(empty_state.name, "");
    assert_eq!(empty_state.description, "");
    assert_eq!(empty_state.metadata_json, "");

    let prepared = prepare_shipping_profile_draft(
        "  Express-Air  ",
        "  Express Air Delivery  ",
        "  Fast shipping  ",
        "{\"carrier\":\"DHL\"}",
        "en".to_string(),
    )
    .expect("valid draft");

    assert_eq!(prepared.slug, "Express-Air");
    assert_eq!(prepared.name, "Express Air Delivery");
    assert_eq!(prepared.description, "Fast shipping");
    assert_eq!(prepared.metadata_json, "{\"carrier\":\"DHL\"}");
    assert_eq!(prepared.locale, "en");

    // Invalid slug/name yields None
    assert!(prepare_shipping_profile_draft("", "Name", "", "{}", "en".to_string()).is_none());
    assert!(prepare_shipping_profile_draft("slug", "  ", "", "{}", "en".to_string()).is_none());
    // Invalid JSON yields None
    assert!(
        prepare_shipping_profile_draft("slug", "Name", "", "{not-json", "en".to_string()).is_none()
    );
}

#[test]
fn cart_promotion_command_normalizes_kind_and_scope() {
    // 1. Percentage discount: amount must be cleared, line_item_id cleared for Cart scope
    let form = CartPromotionForm {
        cart_id: "cart-123",
        kind: "percentage_discount",
        scope: "cart",
        line_item_id: "ignored-for-cart",
        source_id: "PROMO-10",
        discount_percent: "15",
        amount: "50.00",
        metadata_json: "{}",
    };

    let cmd = prepare_cart_promotion_command(form).expect("valid promo");
    assert_eq!(cmd.cart_id, "cart-123");
    assert_eq!(cmd.draft.source_id, "PROMO-10");
    assert_eq!(
        cmd.draft.kind,
        CommerceCartPromotionKind::PercentageDiscount
    );
    assert_eq!(cmd.draft.scope, CommerceCartPromotionScope::Cart);
    assert_eq!(cmd.draft.discount_percent, "15");
    assert_eq!(
        cmd.draft.amount, "",
        "amount must be empty for PercentageDiscount"
    );
    assert_eq!(
        cmd.draft.line_item_id, "",
        "line_item_id must be empty for Cart scope"
    );

    // 2. Fixed discount: discount_percent must be cleared, line_item_id preserved
    let form = CartPromotionForm {
        cart_id: "cart-123",
        kind: "fixed_discount",
        scope: "line_item",
        line_item_id: "line-item-456",
        source_id: "PROMO-FLAT",
        discount_percent: "20",
        amount: "10.00",
        metadata_json: "{}",
    };

    let cmd = prepare_cart_promotion_command(form).expect("valid promo");
    assert_eq!(cmd.draft.kind, CommerceCartPromotionKind::FixedDiscount);
    assert_eq!(cmd.draft.scope, CommerceCartPromotionScope::LineItem);
    assert_eq!(
        cmd.draft.discount_percent, "",
        "discount_percent must be empty for FixedDiscount"
    );
    assert_eq!(cmd.draft.amount, "10.00");
    assert_eq!(cmd.draft.line_item_id, "line-item-456");
}

#[test]
fn order_change_resolution_summary_fallbacks() {
    let mut change = CommerceOrderChange {
        id: "chg-1".to_string(),
        tenant_id: "ten-1".to_string(),
        order_id: "ord-1".to_string(),
        created_by: "user-1".to_string(),
        change_type: "exchange".to_string(),
        status: "pending".to_string(),
        description: None,
        preview: "{\"order_return_id\":\"preview-ret\"}".to_string(),
        metadata: "{\"order_return_id\":\"meta-ret\",\"cancellation_reason\":\"Customer request\"}"
            .to_string(),
        created_at: "2026-11-06T00:00:00Z".to_string(),
        updated_at: "2026-11-06T00:00:00Z".to_string(),
        applied_at: None,
        cancelled_at: None,
    };

    let summary = order_change_resolution_summary(&change);
    assert!(summary.has_any());
    assert_eq!(summary.order_return_id.as_deref(), Some("meta-ret"));
    assert_eq!(
        summary.cancellation_reason.as_deref(),
        Some("Customer request")
    );
    assert_eq!(summary.return_decision_action.as_deref(), Some("exchange"));

    // Empty metadata falls back to preview
    change.metadata = "{}".to_string();
    let summary2 = order_change_resolution_summary(&change);
    assert_eq!(summary2.order_return_id.as_deref(), Some("preview-ret"));
    assert_eq!(summary2.cancellation_reason, None);
    assert_eq!(summary2.cancellation_reason_value(), "-");
}

#[test]
fn order_change_action_command_validation() {
    assert!(prepare_order_change_action_command("", "{}", "reason").is_none());
    assert!(prepare_order_change_action_command("  ", "{}", "reason").is_none());
    assert!(prepare_order_change_action_command("chg-1", "{invalid-json", "reason").is_none());

    let cmd =
        prepare_order_change_action_command("  chg-1  ", " {\"source\":\"test\"} ", " Approved ")
            .expect("valid command");
    assert_eq!(cmd.change_id, "chg-1");
    assert_eq!(cmd.draft.metadata_json, "{\"source\":\"test\"}");
    assert_eq!(cmd.draft.reason, "Approved");
    assert!(!cmd.draft.idempotency_key.is_empty());
}

#[test]
fn presentation_helpers_and_badge_classes() {
    assert_eq!(optional_value(None), "-");
    assert_eq!(optional_value(Some("")), "-");
    assert_eq!(optional_value(Some("   ")), "-");
    assert_eq!(optional_value(Some("Present")), "Present");

    assert_eq!(trimmed_non_empty("  hello  "), Some("hello".to_string()));
    assert_eq!(trimmed_non_empty("   "), None);
    assert_eq!(trimmed_non_empty(""), None);

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

    assert_eq!(
        localized_order_change_status(Some("en"), "pending"),
        "Pending"
    );
    assert_eq!(
        localized_order_change_status(Some("ru"), "pending"),
        "В ожидании"
    );
    assert_eq!(
        localized_order_change_status(Some("en"), DEFAULT_ORDER_CHANGE_STATUS),
        "Pending"
    );

    assert_eq!(DEFAULT_PROMOTION_KIND, "fixed_discount");
    assert_eq!(DEFAULT_PROMOTION_SCOPE, "shipping");
    assert_eq!(DEFAULT_PROMOTION_SOURCE_ID, "promo-operator");
    assert_eq!(DEFAULT_PROMOTION_AMOUNT, "4.99");
}
