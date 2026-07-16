#[test]
fn graphql_fulfillment_mutations_use_commerce_orchestration() {
    let graphql = include_str!(
        "../../../crates/rustok-commerce/src/graphql/mutations/provider_operations.rs"
    );
    let facade = include_str!(
        "../../../crates/rustok-commerce/src/services/fulfillment_orchestration_facade.rs"
    );

    assert!(
        !graphql.contains("use rustok_fulfillment::FulfillmentService;"),
        "GraphQL transport must not import the fulfillment owner service directly"
    );
    assert!(
        !graphql.contains("FulfillmentService::new("),
        "GraphQL fulfillment mutations must route through commerce orchestration"
    );

    for operation in [
        ".create_manual_fulfillment(",
        ".ship_fulfillment(",
        ".deliver_fulfillment(",
        ".reopen_fulfillment(",
        ".reship_fulfillment(",
        ".cancel_fulfillment(",
    ] {
        assert!(
            graphql.contains(operation),
            "GraphQL fulfillment transport must retain orchestration call {operation}"
        );
    }

    for method in [
        "pub async fn deliver_fulfillment(",
        "pub async fn reopen_fulfillment(",
    ] {
        assert!(
            facade.contains(method),
            "commerce fulfillment facade must expose {method}"
        );
    }
}
