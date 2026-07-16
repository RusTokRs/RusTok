#[test]
fn return_completion_uses_one_commerce_orchestration_boundary() {
    let rest = include_str!(
        "../../../crates/rustok-commerce/src/controllers/admin/returns.rs"
    );
    let graphql = include_str!(
        "../../../crates/rustok-commerce/src/graphql/mutations/fulfillment.rs"
    );
    let graphql_runtime = include_str!(
        "../../../crates/rustok-commerce/src/graphql_runtime.rs"
    );
    let orchestration = include_str!(
        "../../../crates/rustok-commerce/src/services/return_completion_orchestration.rs"
    );

    assert!(
        rest.contains("ReturnCompletionOrchestrationService::new("),
        "REST return completion must use the commerce orchestration boundary"
    );
    assert!(
        rest.contains(".complete_return(tenant.id, auth.user_id, id, command)"),
        "REST must pass the complete return command to orchestration"
    );
    assert!(
        !rest.contains(".create_refund_idempotent("),
        "REST return completion must not execute refund provider orchestration"
    );
    assert!(
        !rest.contains("attach_return_order_change_context("),
        "REST transport must not create exchange or claim context"
    );

    assert!(
        graphql.contains("return_completion_orchestration_from_context("),
        "GraphQL return completion must use the composed orchestration boundary"
    );
    assert!(
        graphql.contains(".complete_return(tenant_id, auth.user_id, id, command)"),
        "GraphQL must pass the complete return command to orchestration"
    );
    for forbidden in [
        "build_provider_refund_resolution_return_completion(",
        "build_exchange_resolution_return_completion(",
        "build_claim_resolution_return_completion(",
        ".create_refund_idempotent(",
    ] {
        assert!(
            !graphql.contains(forbidden),
            "GraphQL return completion must not retain transport orchestration {forbidden}"
        );
    }
    assert!(
        graphql_runtime.contains("pub(crate) fn return_completion_orchestration_from_context("),
        "GraphQL runtime must compose return completion orchestration"
    );

    let validation = orchestration
        .find("validate_completion_shape(&input)")
        .expect("return completion must validate the complete command");
    let first_effect = orchestration
        .find("if let Some(refund_input) = refund")
        .expect("return completion refund path must exist");
    assert!(
        validation < first_effect,
        "return completion shape must be validated before provider or owner side effects"
    );
    for marker in [
        "refund, exchange, and claim helpers are mutually exclusive",
        "resolution helpers cannot be combined with explicit refund_id or order_change_id",
        "format!(\"order_return:{return_id}:refund\")",
        ".complete_return(tenant_id, return_id, owner_input)",
    ] {
        assert!(
            orchestration.contains(marker),
            "return completion orchestration is missing invariant {marker}"
        );
    }
}
