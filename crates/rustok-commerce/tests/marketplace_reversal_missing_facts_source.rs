#[test]
fn marketplace_reversal_missing_facts_fail_closed_for_marketplace_orders() {
    let runtime = include_str!("../src/services/marketplace_financial_runtime.rs");
    let guard = include_str!("../src/services/marketplace_reversal_fact_guard.rs");

    assert!(runtime.contains("MarketplaceAllocationReadPort"));
    assert!(runtime.contains("with_allocation_reader"));
    assert!(runtime.contains("MarketplaceReversalFactGuardObserver::new"));
    assert!(runtime.contains("with_observer(Arc::new(guarded))"));
    assert!(!runtime.contains("with_observer(Arc::new(\n            self.provider_reversal_event_adapter"));

    assert!(guard.contains("list_allocations_by_order"));
    assert!(guard.contains("resolve_authoritative_order_id"));
    assert!(guard.contains("PaymentService::new"));
    assert!(guard.contains("PortActor::service"));
    assert!(guard.contains("with_deadline(ASSOCIATION_READ_DEADLINE)"));
    assert!(guard.contains("marketplace_reversal_adapter.marketplace_facts_missing"));
    assert!(guard.contains("Marketplace reversal facts are missing"));
    assert!(guard.contains("return Ok(())"));
    assert!(guard.contains("Err(non_retryable(MISSING_FACTS_CODE, MISSING_FACTS_MESSAGE))"));
    assert!(guard.contains("return self.delegate.observe(context, event).await"));
    assert!(!guard.contains("raw_payload"));
    assert!(!guard.contains("error.to_string()"));
}
