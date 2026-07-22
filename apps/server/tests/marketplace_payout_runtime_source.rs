#[test]
fn marketplace_payout_runtime_composes_one_write_capable_owner_chain() {
    let services = include_str!("../src/services/mod.rs");
    let attachment = include_str!("../src/services/commerce_provider_runtime.rs");
    let runtime = include_str!("../src/services/marketplace_payout_runtime.rs");
    let registry = include_str!(
        "../../../crates/rustok-marketplace-payout/contracts/marketplace-payout-fba-registry.json"
    );

    assert!(services.contains("pub mod marketplace_payout_runtime"));
    assert!(attachment.contains("attach_marketplace_payout_runtime"));
    assert!(attachment.contains("feature = \"mod-marketplace_payout\""));

    assert!(runtime.contains("MarketplaceAllocationService::new"));
    assert!(runtime.contains("MarketplaceCommissionService::new"));
    assert!(runtime.contains("MarketplaceLedgerService::new"));
    assert!(runtime.contains("MarketplacePayoutService::new"));
    assert!(runtime.contains("with_ledger_writer(ledger_writer)"));
    assert!(runtime.contains("Arc<dyn MarketplaceLedgerReadPort>"));
    assert!(runtime.contains("Arc<dyn MarketplaceLedgerCommandPort>"));

    assert!(runtime.contains("PayoutProviderRegistry::with_manual_provider"));
    assert!(runtime.contains("MarketplacePayoutProviderSubmissionService::new"));
    assert!(runtime.contains("host.shared_get::<Arc<PayoutProviderRegistry>>()"));
    assert!(runtime.contains("assert_registry_identity"));
    assert!(runtime.contains("shared_insert_if_absent(provider_submission_service.clone())"));
    assert!(runtime.contains("with_shared_value(provider_registry)"));
    assert!(runtime.contains("with_shared_value(provider_submission_service)"));

    assert!(runtime.contains("host.shared_get::<MarketplacePayoutRuntime>()"));
    assert!(runtime.contains("shared_insert_if_absent(host_runtime.clone())"));
    assert!(runtime.contains("assert_runtime_identity"));
    assert!(runtime.contains("shared_insert_if_absent(ledger_service.clone())"));
    assert!(runtime.contains("shared_insert_if_absent(payout_service.clone())"));
    assert!(runtime.contains("with_shared_value(payout_service)"));

    assert!(registry.contains("\"host_composition_ready\": true"));
    assert!(registry.contains("\"runtime_execution_ready\": true"));
    assert!(registry.contains("\"host_provider_registry_ready\": true"));
    assert!(registry.contains("\"provider_submission_service_ready\": true"));
    assert!(
        registry.contains("\"ledger_read_and_command_owner_identity\": \"same_process_instance\"")
    );
    assert!(registry.contains("\"provider_registry_owner_identity\": \"same_process_instance\""));
}
