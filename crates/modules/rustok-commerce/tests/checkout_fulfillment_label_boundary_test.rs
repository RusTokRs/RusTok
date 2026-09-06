#[test]
fn checkout_does_not_execute_fulfillment_labels_before_payment() {
    let checkout = include_str!("../src/services/checkout.rs");

    assert!(
        !checkout.contains(".execute_create_label("),
        "checkout must persist fulfillments without invoking carrier label side effects"
    );
    assert!(
        !checkout.contains("fulfillment_provider_registry: FulfillmentProviderRegistry"),
        "checkout must not retain a fulfillment provider registry"
    );
}

#[test]
fn paid_order_recovery_owns_fulfillment_label_execution() {
    let handler = include_str!("../src/services/paid_order_create_label.rs");
    let recovery = include_str!("../src/services/fulfillment_create_label_recovery.rs");
    let sweep = include_str!("../src/services/paid_order_create_label_sweep.rs");

    assert!(handler.contains("DomainEvent::OrderStatusChanged"));
    assert!(handler.contains("new_status == \"paid\""));
    assert!(recovery.contains(".execute_create_label("));
    assert!(recovery.contains("claim_execution("));
    assert!(recovery.contains("mark_provider_succeeded("));
    assert!(sweep.contains("PaidOrderCreateLabelSweepService"));
}
