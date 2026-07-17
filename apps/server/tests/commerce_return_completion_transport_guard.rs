use std::path::Path;

#[test]
fn return_completion_uses_one_commerce_orchestration_boundary() {
    let rest = include_str!("../../../crates/rustok-commerce/src/controllers/admin/returns.rs");
    let graphql =
        include_str!("../../../crates/rustok-commerce/src/graphql/mutations/fulfillment.rs");
    let graphql_runtime = include_str!("../../../crates/rustok-commerce/src/graphql_runtime.rs");
    let recovery =
        include_str!("../../../crates/rustok-commerce/src/services/return_completion_recovery.rs");
    let orchestration = include_str!(
        "../../../crates/rustok-commerce/src/services/return_completion_orchestration.rs"
    );
    let legacy_helper = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/rustok-commerce/src/graphql/mutations/provider_return_helpers.rs");

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

    let validation = recovery
        .find("validate_completion_shape(&input)")
        .expect("return completion must validate the complete command");
    let admission = recovery
        .find(".admit_command_and_operation(")
        .expect("return completion must atomically admit command and operation");
    let delegation = recovery
        .find("self.core_service()")
        .expect("recovery facade must delegate execution to the core orchestration");
    assert!(
        validation < admission && admission < delegation,
        "return completion must validate and durably admit before execution"
    );

    let journal_admission = orchestration
        .find(".begin(BeginReturnCompletionOperation")
        .expect("core return completion must reuse the execution journal");
    let first_effect = orchestration
        .find(".create_refund_idempotent(")
        .expect("return completion refund path must exist");
    assert!(journal_admission < first_effect);
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
    let entity =
        include_str!("../../../crates/rustok-commerce/src/entities/return_completion_operation.rs");
    let migration = include_str!(
        "../../../crates/rustok-commerce/src/migrations/m20260716_000004_create_return_completion_operations.rs"
    );
    let resolution_identity_migration = include_str!(
        "../../../crates/rustok-commerce/src/migrations/m20260716_000005_enforce_return_completion_resolution_identity.rs"
    );
    let journal =
        include_str!("../../../crates/rustok-commerce/src/services/return_completion_operation.rs");
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
        "DROP FUNCTION IF EXISTS enforce_return_completion_operation_integrity() CASCADE",
    ] {
        assert!(
            migration.contains(marker),
            "return completion migration is missing invariant {marker}"
        );
    }
    for marker in [
        "ck_return_completion_operations_resolution_identity",
        "stage <> 'resolution_created'",
        "return completion resolution identity is required",
    ] {
        assert!(
            resolution_identity_migration.contains(marker),
            "return completion resolution identity migration is missing invariant {marker}"
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
        "validate_explicit_resolution_links(",
        "is not attached to order",
        "without a refund identity",
        "without an order-change identity",
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

#[test]
fn return_completion_command_inbox_and_operator_surface_are_safe() {
    let entity =
        include_str!("../../../crates/rustok-commerce/src/entities/return_completion_command.rs");
    let migration = include_str!(
        "../../../crates/rustok-commerce/src/migrations/m20260716_000006_create_return_completion_commands.rs"
    );
    let recovery =
        include_str!("../../../crates/rustok-commerce/src/services/return_completion_recovery.rs");
    let services = include_str!("../../../crates/rustok-commerce/src/services/mod.rs");
    let controller = include_str!(
        "../../../crates/rustok-commerce/src/controllers/return_completion_operations.rs"
    );
    let controllers = include_str!("../../../crates/rustok-commerce/src/controllers/mod.rs");
    let openapi = include_str!("../../../crates/rustok-commerce/src/openapi.rs");

    assert!(entity.contains("table_name = \"return_completion_commands\""));
    for marker in [
        "ux_return_completion_commands_return",
        "request_payload",
        "request completion command identity and payload are immutable",
        "return completion command return tenant mismatch",
        "retry_count cannot decrease",
        "DROP FUNCTION IF EXISTS enforce_return_completion_command_integrity() CASCADE",
    ] {
        assert!(
            migration.contains(marker)
                || migration.contains(&marker.replace("request completion", "return completion")),
            "return completion command migration is missing invariant {marker}"
        );
    }

    for marker in [
        "admit_command_and_operation(",
        "let txn = self.db.begin()",
        ".insert(&txn)",
        "txn.commit()",
        "ensure_same_command(",
        "ensure_same_operation(",
        "record_retry(tenant_id, command.id, retry_actor_id)",
        "Column::TenantId.eq(tenant_id)",
        "command.requested_by_actor_id",
    ] {
        assert!(
            recovery.contains(marker),
            "return completion recovery facade is missing invariant {marker}"
        );
    }
    assert!(
        !recovery.contains("tenant_id_for_command_placeholder"),
        "return completion retry must not retain placeholder tenant scope"
    );
    let response_start = recovery
        .find("pub struct ReturnCompletionOperationResponse")
        .expect("safe operation response must exist");
    let response_end = recovery[response_start..]
        .find("/// Durable return-completion facade")
        .map(|offset| response_start + offset)
        .expect("safe operation response boundary must exist");
    assert!(
        !recovery[response_start..response_end].contains("request_payload"),
        "operator projections must not expose the stored command payload"
    );

    assert!(services.contains("mod return_completion_recovery;"));
    assert!(services.contains("ReturnCompletionOrchestrationService,"));
    assert!(controller.contains("Permission::ORDERS_READ"));
    assert!(controller.contains("Permission::ORDERS_MANAGE, Permission::PAYMENTS_MANAGE"));
    assert!(controller.contains(".retry_operation(tenant.id, auth.user_id, id)"));
    assert!(controllers.contains("/admin/return-completion-operations"));
    for marker in [
        "list_return_completion_operations",
        "show_return_completion_operation",
        "retry_return_completion_operation",
        "ReturnCompletionOperationResponse",
    ] {
        assert!(openapi.contains(marker), "OpenAPI is missing {marker}");
    }
}
