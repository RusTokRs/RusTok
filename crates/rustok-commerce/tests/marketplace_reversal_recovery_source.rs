#[test]
fn marketplace_reversal_recovery_source_preserves_owner_and_transport_contracts() {
    let adapter = include_str!(
        "../src/services/marketplace_provider_reversal_event_adapter.rs"
    );
    let inbox = include_str!("../src/services/marketplace_reversal_event_inbox.rs");
    let operator = include_str!("../src/services/marketplace_reversal_operator.rs");
    let migration = include_str!(
        "../src/migrations/m20260721_000004_create_marketplace_reversal_event_inbox.rs"
    );
    let rest = include_str!("../src/controllers/marketplace_reversal_financial.rs");
    let graphql = include_str!("../src/graphql/marketplace_financial.rs");
    let worker = include_str!(
        "../../../apps/server/src/services/marketplace_financial_worker.rs"
    );

    assert!(adapter.contains("refund.completed"));
    assert!(adapter.contains("chargeback.completed"));
    assert!(adapter.contains("marketplace_reversal"));
    assert!(adapter.contains("decimal_to_minor_exact"));
    assert!(adapter.contains("return Ok(None)"));
    assert!(!adapter.contains("raw_payload"));

    assert!(inbox.contains("marketplace-reversal-event:{}:v1"));
    assert!(inbox.contains("process_financial_reversal"));
    assert!(inbox.contains("MarketplaceReversalEventStatus::OperatorReview"));
    assert!(!inbox.contains("PaymentProviderWebhookRequest"));

    assert!(migration.contains("ux_marketplace_reversal_provider_event"));
    assert!(migration.contains("ux_marketplace_reversal_event_source"));
    assert!(migration.contains("ux_marketplace_reversal_source_identity"));
    assert!(migration.contains("normalized facts are immutable"));
    assert!(!migration.contains("foreign_key("));

    assert!(operator.contains("ReversalId.is_null()"));
    assert!(operator.contains("LedgerTransactionId.is_null()"));
    assert!(!operator.contains("lines_json: model.lines_json"));

    for route in [
        "/reversal-events/operator-review",
        "/reversal-events/{id}",
        "/reversal-events/{id}/retry",
        "/reversal-events/recovery-sweep",
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
    ] {
        assert!(
            graphql.contains(operation),
            "missing reversal GraphQL operation {operation}"
        );
    }

    assert!(worker.contains("MARKETPLACE_FINANCIAL_SWEEP_INTERVAL"));
    assert!(worker.contains("MARKETPLACE_FINANCIAL_SWEEP_BATCH: u64 = 100"));
    assert!(worker.contains("MissedTickBehavior::Delay"));
    assert!(worker.contains("adapt_pending"));
    assert!(worker.contains("reversal_events.sweep"));
}
