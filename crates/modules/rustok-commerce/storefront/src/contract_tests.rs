use crate::core::{
    SELECTED_CART_QUERY_KEY, build_cart_checkout_handoff_labels,
    build_fetch_commerce_request, build_fulfillment_delivery_groups,
    build_fulfillment_shipping_selection_labels, build_order_checkout_action_labels,
    build_order_checkout_result_data, build_order_checkout_result_labels,
    build_payment_collection_action_labels, build_payment_collection_card_data,
    build_payment_collection_card_labels, build_select_shipping_option_request,
    build_storefront_context_view_model, build_storefront_route_state,
    build_storefront_shell_view_model, error_with_context,
};
use crate::model::{
    StorefrontCheckoutCart, StorefrontCheckoutCompletion,
    StorefrontCheckoutDeliveryGroup, StorefrontCheckoutPaymentCollection,
    StorefrontCheckoutShippingOption, StorefrontCommerceData,
};

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
        include_str!("core/mod.rs"),
        include_str!("core/presentation.rs"),
        include_str!("core/requests.rs"),
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
fn storefront_route_state_and_request_normalization() {
    assert_eq!(SELECTED_CART_QUERY_KEY, "cart_id");

    // Empty and whitespace are normalized to None
    let state_blank = build_storefront_route_state(Some("   ".to_string()));
    assert_eq!(state_blank.selected_cart_id, None);
    assert_eq!(state_blank.selected_cart_query_key, "cart_id");

    let state_none = build_storefront_route_state(None);
    assert_eq!(state_none.selected_cart_id, None);

    // Whitespace is trimmed
    let state_val = build_storefront_route_state(Some("  cart-xyz-123  ".to_string()));
    assert_eq!(state_val.selected_cart_id.as_deref(), Some("cart-xyz-123"));

    // Fetch commerce request normalizes cart and locale
    let req = build_fetch_commerce_request(Some("  cart-abc  ".to_string()), Some("  ru  ".to_string()));
    assert_eq!(req.selected_cart_id.as_deref(), Some("cart-abc"));
    assert_eq!(req.locale.as_deref(), Some("ru"));

    let req_empty = build_fetch_commerce_request(Some("  ".to_string()), Some("".to_string()));
    assert_eq!(req_empty.selected_cart_id, None);
    assert_eq!(req_empty.locale, None);
}

#[test]
fn storefront_shell_view_model_bilingual_labels() {
    let shell_en = build_storefront_shell_view_model(Some("en"));
    assert_eq!(shell_en.badge, "commerce");
    assert_eq!(shell_en.title, "Commerce orchestration hub");
    assert_eq!(shell_en.load_error, "Failed to load commerce storefront data");
    assert_eq!(shell_en.action_error, "Failed to update aggregate checkout state");

    let shell_ru = build_storefront_shell_view_model(Some("ru"));
    assert_eq!(shell_ru.badge, "commerce");
    assert_eq!(shell_ru.title, "Commerce orchestration hub");
    assert_eq!(shell_ru.load_error, "Не удалось загрузить storefront-данные commerce");
    assert_eq!(shell_ru.action_error, "Не удалось обновить aggregate checkout state");
}

#[test]
fn storefront_context_view_model_fallbacks_and_trimming() {
    let data_full = StorefrontCommerceData {
        effective_locale: "  ru  ".to_string(),
        tenant_slug: Some("  primary-shop  ".to_string()),
        tenant_default_locale: "  en  ".to_string(),
        channel_slug: Some("  mobile-app  ".to_string()),
        channel_resolution_source: Some("  header  ".to_string()),
        selected_cart_id: Some("cart-1".to_string()),
        checkout: None,
    };

    let vm = build_storefront_context_view_model(data_full, Some("en"));
    assert_eq!(vm.effective_locale, "ru");
    assert_eq!(vm.tenant, "primary-shop");
    assert_eq!(vm.tenant_default_locale, "en");
    assert_eq!(vm.channel, "mobile-app");
    assert_eq!(vm.channel_resolution_source, "header");

    // All empty/None values fall back to localized defaults
    let data_empty = StorefrontCommerceData {
        effective_locale: "".to_string(),
        tenant_slug: None,
        tenant_default_locale: "".to_string(),
        channel_slug: None,
        channel_resolution_source: None,
        selected_cart_id: None,
        checkout: None,
    };

    let vm_fallback = build_storefront_context_view_model(data_empty, Some("en"));
    assert_eq!(vm_fallback.effective_locale, "not resolved");
    assert_eq!(vm_fallback.tenant, "host tenant");
    assert_eq!(vm_fallback.tenant_default_locale, "not resolved");
    assert_eq!(vm_fallback.channel, "not resolved");
    assert_eq!(vm_fallback.channel_resolution_source, "not resolved");
}

#[test]
fn select_shipping_option_request_mapping() {
    let cart = StorefrontCheckoutCart {
        id: "  cart-42  ".to_string(),
        status: "active".to_string(),
        currency_code: "EUR".to_string(),
        subtotal_amount: "100.00".to_string(),
        adjustment_total: "0.00".to_string(),
        shipping_total: "15.00".to_string(),
        total_amount: "115.00".to_string(),
        channel_slug: Some("web".to_string()),
        email: Some("guest@example.com".to_string()),
        customer_id: None,
        region_id: None,
        country_code: Some("DE".to_string()),
        locale_code: Some("de".to_string()),
        selected_shipping_option_id: Some("opt-dhl".to_string()),
        line_item_count: 2,
        adjustment_count: 0,
        delivery_group_count: 1,
        adjustments: vec![],
        delivery_groups: vec![StorefrontCheckoutDeliveryGroup {
            shipping_profile_slug: "express".to_string(),
            seller_id: Some("vendor-99".to_string()),
            line_item_count: 2,
            selected_shipping_option_id: Some("opt-dhl".to_string()),
            available_shipping_options: vec![
                StorefrontCheckoutShippingOption {
                    id: "opt-dhl".to_string(),
                    name: "DHL Express".to_string(),
                    currency_code: "EUR".to_string(),
                    amount: "15.00".to_string(),
                    provider_id: "dhl".to_string(),
                    active: true,
                },
            ],
        }],
    };

    let req = build_select_shipping_option_request(
        cart,
        "  express  ".to_string(),
        Some("  vendor-99  ".to_string()),
        Some("  opt-dhl  ".to_string()),
    );

    assert_eq!(req.owner_request.cart_id, "cart-42");
    assert_eq!(req.owner_request.shipping_profile_slug, "express");
    assert_eq!(req.owner_request.seller_id.as_deref(), Some("vendor-99"));
    assert_eq!(req.owner_request.shipping_option_id.as_deref(), Some("opt-dhl"));
    assert_eq!(req.owner_request.delivery_groups.len(), 1);
    assert_eq!(req.owner_request.delivery_groups[0].shipping_profile_slug, "express");
    assert_eq!(req.owner_request.delivery_groups[0].available_shipping_option_ids, vec!["opt-dhl"]);
}

#[test]
fn presentation_models_and_labels_mapping() {
    let handoff = build_cart_checkout_handoff_labels(Some("en"));
    assert_eq!(handoff.cart_label, "Cart");
    assert_eq!(handoff.status_label, "Cart status");

    let shipping = build_fulfillment_shipping_selection_labels(Some("en"));
    assert_eq!(shipping.provider_label, "Provider");
    assert_eq!(shipping.select_label, "Select");

    let pay_actions = build_payment_collection_action_labels(Some("en"));
    assert_eq!(pay_actions.pending, "Processing...");

    let order_actions = build_order_checkout_action_labels(Some("en"));
    assert_eq!(order_actions.complete, "Complete checkout");

    let card = build_payment_collection_card_data(StorefrontCheckoutPaymentCollection {
        id: "pay-1".to_string(),
        status: "authorized".to_string(),
        currency_code: "USD".to_string(),
        amount: "50.00".to_string(),
        authorized_amount: "50.00".to_string(),
        captured_amount: "0.00".to_string(),
        order_id: None,
        provider_id: Some("stripe".to_string()),
        payment_count: 1,
        created_at: "2026-11-06T00:00:00Z".to_string(),
        updated_at: "2026-11-06T00:00:00Z".to_string(),
    });
    assert_eq!(card.id, "pay-1");
    assert_eq!(card.status, "authorized");

    let card_labels = build_payment_collection_card_labels(Some("en"));
    assert_eq!(card_labels.badge, "payment collection");
    assert_eq!(card_labels.empty_id, "not attached");

    let result = build_order_checkout_result_data(StorefrontCheckoutCompletion {
        order_id: "order-999".to_string(),
        order_status: "confirmed".to_string(),
        currency_code: "USD".to_string(),
        shipping_total: "5.00".to_string(),
        adjustment_total: "0.00".to_string(),
        total_amount: "55.00".to_string(),
        adjustments: vec![],
        payment_collection_id: "pay-1".to_string(),
        payment_collection_status: "authorized".to_string(),
        fulfillment_count: 1,
        context_locale: "en".to_string(),
        context_currency_code: Some("USD".to_string()),
    });
    assert_eq!(result.order_id, "order-999");
    assert_eq!(result.order_status, "confirmed");

    let result_labels = build_order_checkout_result_labels(Some("en"));
    assert_eq!(result_labels.badge, "checkout result");
    assert_eq!(result_labels.order_status_label, "Order status");

    let delivery_groups = build_fulfillment_delivery_groups(vec![
        StorefrontCheckoutDeliveryGroup {
            shipping_profile_slug: "priority".to_string(),
            seller_id: None,
            line_item_count: 3,
            selected_shipping_option_id: None,
            available_shipping_options: vec![],
        },
    ]);
    assert_eq!(delivery_groups.len(), 1);
    assert_eq!(delivery_groups[0].shipping_profile_slug, "priority");
    assert_eq!(delivery_groups[0].seller_id, None);

    assert_eq!(error_with_context("Context", "details"), "Context: details");
}
