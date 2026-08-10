#[test]
fn order_change_application_uses_commerce_orchestration() {
    let rest = include_str!("../../../crates/rustok-commerce/src/controllers/admin/changes.rs");
    let graphql =
        include_str!("../../../crates/rustok-commerce/src/graphql/mutations/fulfillment.rs");
    let graphql_runtime = include_str!("../../../crates/rustok-commerce/src/graphql_runtime.rs");
    let orchestration =
        include_str!("../../../crates/rustok-commerce/src/services/order_change_orchestration.rs");

    assert!(
        rest.contains("OrderChangeOrchestrationService::from_order_ports("),
        "mounted REST order-change application must compose host-selected owner ports"
    );
    assert!(
        rest.contains("runtime.order_read_port()")
            && rest.contains("runtime.order_post_order_command_port()"),
        "mounted REST order-change application must use the HTTP runtime owner ports"
    );
    assert!(
        rest.contains(".apply_order_change_with_owner_ports("),
        "mounted REST order-change application must use the owner-port orchestration entrypoint"
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
        "GraphQL compatibility path must remain explicit until its owner-port cutover"
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
        "GraphQL runtime must keep the separate order-change orchestration composition point"
    );

    assert!(
        orchestration.contains("pub async fn apply_order_change_with_owner_ports("),
        "commerce orchestration must publish the mounted REST owner-port entrypoint"
    );
    assert!(
        orchestration.contains(".read_order_change_projection(")
            && orchestration.contains(".apply_change("),
        "REST owner-port entrypoint must read and default-apply through Order ports"
    );
    assert!(
        orchestration.contains("pub async fn apply_order_change("),
        "GraphQL compatibility entrypoint must remain explicit for a separate cutover"
    );
    for operation in [".apply_exchange_order_change(", ".apply_claim_order_change("] {
        assert!(
            orchestration.contains(operation),
            "order-change orchestration must retain {operation}"
        );
    }
}
