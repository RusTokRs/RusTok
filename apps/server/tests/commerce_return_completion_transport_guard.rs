use std::path::Path;

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
    let legacy_helper = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../crates/rustok-commerce/src/graphql/mutations/provider_return_helpers.rs",
    );

    assert!(
        !legacy_helper.exists(),
        "legacy GraphQL provider return helper module must stay removed"
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
    let admission = orchestration
        .find(".begin(BeginReturnCompletionOperation")
        .expect("return completion must durably admit the request");
    let first_effect = orchestration
        .find(".create_refund_idempotent(")
        .expect("return completion refund path must exist");
    assert!(
        validation < admission && admission < first_effect,
        "return completion must validate and journal the command before side effects"
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

#[test]
fn return_completion_journal_preserves_replay_and_recovery_invariants() {
    let entity = include_str!(
        "../../../crates/rustok-commerce/src/entities/return_completion_operation.rs"
    );
    let migration = include_str!(
        "../../../crates/rustok-commerce/src/migrations/m20260716_000004_create_return_completion_operations.rs"
    );
    let journal = include_str!(
        "../../../crates/rustok-commerce/src/services/return_completion_operation.rs"
    );
    let orchestration = include_str!(
        "../../../crates/rustok-commerce/src/services/return_completion_orchestration.rs"
    );

    assert!(entity.contains("table_name = \"return_completion_operations\""));
    for marker in [
        "ux_return_completion_operations_return",
        "idx_return_completion_operations_recovery",
        "return completion operation identity is immutable",
        "return completion operation return tenant mismatch",
        "reconciliation_required",
        "resolution_created",
        "return_completed",
    ] {
        assert!(
            migration.contains(marker),
            "return completion migration is missing invariant {marker}"
        );
    }
    for marker in [
        "pub async fn begin(",
        "pub async fn claim_execution(",
        "pub async fn checkpoint(",
        "pub async fn mark_retryable(",
        "pub async fn mark_reconciliation_required(",
        "pub async fn mark_completed(",
        "ensure_same_request",
        "request_hash must be a 64-character hexadecimal SHA-256 digest",
    ] {
        assert!(
            journal.contains(marker),
            "return completion journal is missing invariant {marker}"
        );
    }
    for marker in [
        "completion_request_hash(&input)",
        "return_completion_operation_id",
        "find_resolution_order_change(",
        "operation.refund_id",
        "operation.order_change_id",
        "FailureDisposition::Reconciliation",
        "mark_reconciliation_required(",
    ] {
        assert!(
            orchestration.contains(marker),
            "return completion recovery is missing invariant {marker}"
        );
    }

    let refund_effect = orchestration
        .find(".create_refund_idempotent(")
        .expect("refund side effect must exist");
    let owner_completion = orchestration
        .find(".complete_return(tenant_id, return_id, owner_input)")
        .expect("owner completion must exist");
    let journal_admission = orchestration
        .find(".begin(BeginReturnCompletionOperation")
        .expect("journal admission must exist");
    assert!(journal_admission < refund_effect && refund_effect < owner_completion);
}
