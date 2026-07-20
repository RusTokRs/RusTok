#[test]
fn order_change_application_uses_commerce_orchestration() {
    let rest = include_str!("../../../crates/rustok-commerce/src/controllers/admin/changes.rs");
    let graphql =
        include_str!("../../../crates/rustok-commerce/src/graphql/mutations/fulfillment.rs");
    let graphql_runtime = include_str!("../../../crates/rustok-commerce/src/graphql_runtime.rs");
    let orchestration =
        include_str!("../../../crates/rustok-commerce/src/services/order_change_orchestration.rs");

    assert!(
        rest.contains("OrderChangeOrchestrationService::new("),
        "REST order-change application must use the commerce orchestration boundary"
    );
    assert!(
        rest.contains(
            ".apply_order_change(tenant.id, id, input.difference_refund, input.metadata)"
        ),
        "REST order-change application must pass the complete command to orchestration"
    );
    assert!(
        !rest.contains("match order_change.change_type.as_str()"),
        "REST transport must not dispatch order-change domain types"
    );

    assert!(
        graphql.contains("order_change_orchestration_from_context("),
        "GraphQL order-change application must use the composed orchestration boundary"
    );
    assert!(
        graphql.contains(".apply_order_change(tenant_id, id, difference_refund, metadata)"),
        "GraphQL order-change application must pass the complete command to orchestration"
    );
    assert!(
        !graphql.contains("match order_change.change_type.as_str()"),
        "GraphQL transport must not dispatch order-change domain types"
    );
    assert!(
        !graphql.contains(".apply_exchange_order_change("),
        "GraphQL transport must not invoke exchange orchestration directly"
    );
    assert!(
        !graphql.contains(".apply_claim_order_change("),
        "GraphQL transport must not invoke claim orchestration directly"
    );
    assert!(
        graphql_runtime.contains("pub(crate) fn order_change_orchestration_from_context("),
        "GraphQL runtime must compose the order-change orchestration service"
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
