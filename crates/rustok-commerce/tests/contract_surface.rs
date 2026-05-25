#[test]
fn implementation_plan_tracks_contract_test_coverage() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("контрактные тесты покрывают все публичные use-case"),
        "implementation plan must include contract test checklist item"
    );
}

#[test]
fn implementation_plan_tracks_checkout_guardrail_visibility() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("re-entry/release/complete invariants"),
        "implementation plan must keep checkout guardrail visibility markers"
    );
}

#[test]
fn ecommerce_module_plans_keep_ffa_fba_status_blocks() {
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
            "payment",
            include_str!("../../rustok-payment/docs/implementation-plan.md"),
        ),
        (
            "fulfillment",
            include_str!("../../rustok-fulfillment/docs/implementation-plan.md"),
        ),
    ] {
        assert!(
            plan.contains("## FFA/FBA status"),
            "module `{module_slug}` plan must include FFA/FBA status block"
        );
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
