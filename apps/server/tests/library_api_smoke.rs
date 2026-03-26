#[test]
fn order_scenario_uses_library_contract_types_not_local_shadows() {
    let scenario = include_str!("integration/order_flow_test.rs");

    for forbidden in [
        "enum OrderStatus",
        "struct Order {",
        "enum DomainEvent",
        "struct Payment",
    ] {
        assert!(
            !scenario.contains(forbidden),
            "order flow scenario must not reimplement domain contract type: {forbidden}"
        );
    }

    for required in [
        "use rustok_commerce::{Order, OrderError};",
        "use rustok_events::DomainEvent;",
        "Order::new_pending",
        "DomainEvent::OrderStatusChanged",
    ] {
        assert!(
            scenario.contains(required),
            "order flow scenario must use library API marker: {required}"
        );
    }
}
