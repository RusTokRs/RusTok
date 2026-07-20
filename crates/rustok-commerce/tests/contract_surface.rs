#[test]
fn implementation_plan_tracks_contract_test_coverage() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("Targeted checkout, inventory reservation, payment provider-operation")
            && plan.contains("provider-event, replay, recovery, and lifecycle tests"),
        "main ecommerce plan must include targeted contract and recovery test coverage"
    );
}

#[test]
fn implementation_plan_tracks_checkout_guardrail_visibility() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("Add and execute kill points after every owner call")
            && plan.contains("cart_locked")
            && plan.contains("payment_captured")
            && plan.contains("cart_completed"),
        "main ecommerce plan must keep staged-checkout guardrail visibility markers"
    );
}

#[test]
fn ecommerce_owner_plans_keep_ffa_fba_status_blocks() {
    for (module_slug, plan) in [
        ("commerce", include_str!("../docs/implementation-plan.md")),
        (
            "cart",
            include_str!("../../rustok-cart/docs/implementation-plan.md"),
        ),
        (
            "customer",
            include_str!("../../rustok-customer/docs/implementation-plan.md"),
        ),
        (
            "product",
            include_str!("../../rustok-product/docs/implementation-plan.md"),
        ),
        (
            "region",
            include_str!("../../rustok-region/docs/implementation-plan.md"),
        ),
        (
            "pricing",
            include_str!("../../rustok-pricing/docs/implementation-plan.md"),
        ),
        (
            "inventory",
            include_str!("../../rustok-inventory/docs/implementation-plan.md"),
        ),
        (
            "order",
            include_str!("../../rustok-order/docs/implementation-plan.md"),
        ),
        (
            "fulfillment",
            include_str!("../../rustok-fulfillment/docs/implementation-plan.md"),
        ),
    ] {
        assert!(
            plan.contains("FFA status: `in_progress`")
                || plan.contains("FFA status: `phase_b_ready`")
                || plan.contains("FFA status: `parity_verified`"),
            "module `{module_slug}` plan must publish an explicit FFA status"
        );
        assert!(
            plan.contains("FBA status: `in_progress`")
                || plan.contains("FBA status: `boundary_ready`")
                || plan.contains("FBA status: `transport_verified`"),
            "module `{module_slug}` plan must publish an explicit FBA status"
        );
    }
}

#[test]
fn payment_planning_redirects_to_the_main_ecommerce_plan() {
    let commerce_plan = include_str!("../docs/implementation-plan.md");
    let payment_redirect = include_str!("../../rustok-payment/docs/implementation-plan.md");

    assert!(
        commerce_plan.contains("## Payment workstream")
            && commerce_plan.contains("Payment FFA status: `in_progress`")
            && commerce_plan.contains("Payment FBA status: `boundary_ready`"),
        "main ecommerce plan must own payment tasks and boundary status"
    );
    assert!(
        payment_redirect
            .contains("crates/rustok-commerce/docs/implementation-plan.md#payment-workstream"),
        "payment planning file must redirect to the main ecommerce plan"
    );
    assert!(
        !payment_redirect.contains("- [x]") && !payment_redirect.contains("- [ ]"),
        "payment redirect must not contain a second task checklist"
    );
}

#[test]
fn central_registry_tracks_all_ecommerce_modules_in_ffa_fba_board() {
    let registry = include_str!("../../../docs/modules/registry.md");
    for required_row in [
        "| `commerce` | admin + storefront |",
        "| `cart` | storefront |",
        "| `customer` | admin |",
        "| `product` | admin + storefront |",
        "| `region` | admin + storefront |",
        "| `pricing` | admin + storefront |",
        "| `inventory` | admin |",
        "| `order` | admin |",
        "| `payment` | no module-owned UI |",
        "| `fulfillment` | admin |",
    ] {
        assert!(
            registry.contains(required_row),
            "central FFA/FBA board must include `{required_row}`"
        );
    }
}
