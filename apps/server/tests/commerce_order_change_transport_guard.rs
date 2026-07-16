#[test]
fn rest_order_change_application_uses_commerce_orchestration() {
    let rest = include_str!(
        "../../../crates/rustok-commerce/src/controllers/admin/changes.rs"
    );
    let orchestration = include_str!(
        "../../../crates/rustok-commerce/src/services/order_change_orchestration.rs"
    );

    assert!(
        rest.contains("OrderChangeOrchestrationService::new("),
        "REST order-change application must use the commerce orchestration boundary"
    );
    assert!(
        rest.contains(".apply_order_change(tenant.id, id, input.difference_refund, input.metadata)"),
        "REST order-change application must pass the complete command to orchestration"
    );
    assert!(
        !rest.contains("match order_change.change_type.as_str()"),
        "REST transport must not dispatch order-change domain types"
    );

    assert!(
        orchestration.contains("match order_change.change_type.as_str()"),
        "commerce orchestration must own order-change type dispatch"
    );
    for operation in [
        ".apply_exchange_order_change(",
        ".apply_claim_order_change(",
        ".apply_order_change(",
    ] {
        assert!(
            orchestration.contains(operation),
            "order-change orchestration must retain {operation}"
        );
    }
}
