#[test]
fn graphql_runtime_parity_refunds_require_creation_identity() {
    let source =
        include_str!("../../../crates/rustok-commerce/tests/graphql_runtime_parity_test/main.rs");
    let start = source
        .find("fn admin_create_refund_mutation(")
        .expect("GraphQL runtime parity refund helper must exist");
    let end = source[start..]
        .find("fn admin_complete_refund_mutation(")
        .map(|offset| start + offset)
        .expect("refund helper must remain isolated from completion helper");
    let helper = &source[start..end];

    assert!(
        helper.contains("idempotencyKey: \"graphql-refund-{step}\""),
        "GraphQL runtime parity refund creation must pass a deterministic idempotencyKey"
    );
    assert!(
        helper.contains("paymentCollectionId: \"{payment_collection_id}\""),
        "refund creation identity must stay scoped to the payment collection helper"
    );
}
