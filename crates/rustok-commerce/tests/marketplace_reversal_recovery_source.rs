#[test]
fn marketplace_reversal_recovery_source_preserves_owner_and_transport_contracts() {
    let adapter = include_str!(
        "../src/services/marketplace_provider_reversal_event_adapter.rs"
    );
    let backfill = include_str!(
        "../src/services/marketplace_provider_reversal_backfill.rs"
    );
    let adaptation_failures = include_str!(
        "../src/services/marketplace_reversal_adaptation_failure.rs"
    );
    let inbox = include_str!("../src/services/marketplace_reversal_event_inbox.rs");
    let operator = include_str!("../src/services/marketplace_reversal_operator.rs");
    let migration = include_str!(
        "../src/migrations/m20260721_000004_create_marketplace_reversal_event_inbox.rs"
    );
    let mysql_integrity = include_str!(
        "../src/migrations/m20260721_000005_enforce_marketplace_reversal_event_mysql_integrity.rs"
    );
    let adaptation_migration = include_str!(
        "../src/migrations/m20260721_000006_create_marketplace_reversal_adaptation_failures.rs"
    );
    let migration_registry = include_str!("../src/migrations/mod.rs");
    let rest = include_str!("../src/controllers/marketplace_reversal_financial.rs");
    let graphql = include_str!("../src/graphql/marketplace_financial.rs");
    let marketplace_worker = include_str!(
        "../../../apps/server/src/services/marketplace_financial_worker.rs"
    );
    let payment_controller = include_str!("../../rustok-payment/src/controllers.rs");
    let payment_recovery = include_str!(
        "../../rustok-payment/src/provider_event_recovery_controller.rs"
    );
    let payment_worker = include_str!(
        "../../../apps/server/src/services/payment_provider_event_worker.rs"
    );
    let dispatcher = include_str!(
        "../../../apps/server/src/services/module_event_dispatcher.rs"
    );

    assert!(adapter.contains("refund.completed"));
    assert!(adapter.contains("chargeback.completed"));
    assert!(adapter.contains("marketplace_reversal"));
    assert!(adapter.contains("decimal_to_minor_exact"));
    assert!(adapter.contains("return Ok(None)"));
    assert!(adapter.contains("DatabaseBackend::Postgres"));
    assert!(adapter.contains("event_metadata::text"));
    assert!(adapter.contains("DatabaseBackend::Sqlite"));
    assert!(adapter.contains("CAST(event_metadata AS TEXT)"));
    assert!(adapter.contains("DatabaseBackend::MySql"));
    assert!(adapter.contains("CAST(event_metadata AS CHAR)"));
    assert!(!adapter.contains("raw_payload"));

    assert!(backfill.contains("marketplace_reversal_adaptation_failures"));
    assert!(backfill.contains("next_retry_at > CURRENT_TIMESTAMP"));
    assert!(backfill.contains("record_failure"));
    assert!(backfill.contains("mark_resolved"));
    assert!(!backfill.contains("raw_payload"));

    assert!(adaptation_failures.contains("RetryableError"));
    assert!(adaptation_failures.contains("OperatorReview"));
    assert!(adaptation_failures.contains("Resolved"));
    assert!(adaptation_failures.contains("retry_backoff"));
    assert!(adaptation_failures.contains("MAX_BACKOFF_SECONDS: i64 = 600"));
    assert!(adaptation_failures.contains("reset_for_retry"));

    assert!(inbox.contains("marketplace-reversal-event"));
    assert!(inbox.contains(":{}:v1"));
    assert!(inbox.contains("process_financial_reversal"));
    assert!(inbox.contains("MarketplaceReversalEventStatus::OperatorReview"));
    assert!(!inbox.contains("PaymentProviderWebhookRequest"));

    assert!(migration.contains("ux_marketplace_reversal_provider_event"));
    assert!(migration.contains("ux_marketplace_reversal_event_source"));
    assert!(migration.contains("ux_marketplace_reversal_source_identity"));
    assert!(migration.contains("normalized facts are immutable"));
    assert!(!migration.contains("foreign_key("));

    assert!(mysql_integrity.contains("DatabaseBackend::MySql"));
    assert!(mysql_integrity.contains("CREATE TRIGGER marketplace_reversal_event_inbox_guard_insert"));
    assert!(mysql_integrity.contains("CREATE TRIGGER marketplace_reversal_event_inbox_guard_update"));
    assert!(mysql_integrity.contains("marketplace reversal normalized facts are immutable"));
    assert!(mysql_integrity.contains("processed marketplace reversal inbox row is immutable"));

    assert!(adaptation_migration.contains("ux_marketplace_reversal_adaptation_provider_event"));
    assert!(adaptation_migration.contains("retryable_error"));
    assert!(adaptation_migration.contains("operator_review"));
    assert!(adaptation_migration.contains("resolved"));
    assert!(adaptation_migration.contains("DatabaseBackend::Postgres"));
    assert!(adaptation_migration.contains("DatabaseBackend::Sqlite"));
    assert!(adaptation_migration.contains("DatabaseBackend::MySql"));
    assert!(!adaptation_migration.contains("foreign_key("));
    assert!(migration_registry.contains("m20260721_000005_enforce_marketplace_reversal_event_mysql_integrity"));
    assert!(migration_registry.contains("m20260721_000006_create_marketplace_reversal_adaptation_failures"));

    assert!(operator.contains("ReversalId.is_null()"));
    assert!(operator.contains("LedgerTransactionId.is_null()"));
    assert!(operator.contains("retry_adaptation_failure"));
    assert!(operator.contains("list_adaptation_failures_operator_review"));
    assert!(!operator.contains("lines_json: model.lines_json"));

    for route in [
        "/reversal-events/operator-review",
        "/reversal-events/{id}",
        "/reversal-events/{id}/retry",
        "/reversal-events/recovery-sweep",
        "/reversal-adaptation-failures/operator-review",
        "/reversal-adaptation-failures/{id}",
        "/reversal-adaptation-failures/{id}/retry",
    ] {
        assert!(rest.contains(route), "missing reversal REST route {route}");
    }
    assert!(rest.contains("Permission::PAYMENTS_READ"));
    assert!(rest.contains("Permission::PAYMENTS_MANAGE"));
    assert!(!rest.contains("Permission::ORDERS_"));

    for operation in [
        "admin_marketplace_reversal_event",
        "admin_marketplace_reversal_events_operator_review",
        "retry_marketplace_reversal_event",
        "run_marketplace_reversal_recovery_sweep",
        "admin_marketplace_reversal_adaptation_failure",
        "admin_marketplace_reversal_adaptation_failures_operator_review",
        "retry_marketplace_reversal_adaptation_failure",
    ] {
        assert!(
            graphql.contains(operation),
            "missing reversal GraphQL operation {operation}"
        );
    }

    assert!(marketplace_worker.contains("MARKETPLACE_FINANCIAL_SWEEP_INTERVAL"));
    assert!(marketplace_worker.contains("MARKETPLACE_FINANCIAL_SWEEP_BATCH: u64 = 100"));
    assert!(marketplace_worker.contains("MissedTickBehavior::Delay"));
    assert!(marketplace_worker.contains("provider_reversal_backfill"));
    assert!(marketplace_worker.contains("adapt_pending"));
    assert!(marketplace_worker.contains("service.sweep(MARKETPLACE_FINANCIAL_SWEEP_BATCH)"));

    assert!(payment_controller.contains("PaymentObservedDomainEventApplier::new"));
    assert!(payment_recovery.contains("PaymentObservedDomainEventApplier::new"));
    assert!(payment_worker.contains("PaymentObservedDomainEventApplier::new"));
    assert!(dispatcher.contains("PaymentProviderEventObservers"));
    assert!(dispatcher.contains("payment_provider_event_observers"));
}
