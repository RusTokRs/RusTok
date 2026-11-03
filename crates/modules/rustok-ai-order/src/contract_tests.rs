#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[test]
fn order_ai_constants_and_descriptors() {
    use crate::{
        order_ai_verticals, ORDER_ANALYTICS_TASK_SLUG, ORDER_ANALYTICS_TOOL_NAME,
        ORDER_OPS_ASSISTANT_TASK_SLUG, ORDER_OPS_ASSISTANT_TOOL_NAME,
    };

    assert_eq!(ORDER_ANALYTICS_TASK_SLUG, "order_analytics");
    assert_eq!(ORDER_OPS_ASSISTANT_TASK_SLUG, "order_ops_assistant");
    assert_eq!(ORDER_ANALYTICS_TOOL_NAME, "direct.orders.analytics");
    assert_eq!(ORDER_OPS_ASSISTANT_TOOL_NAME, "direct.orders.ops_assistant");

    let verticals = order_ai_verticals();
    assert_eq!(verticals.len(), 2);
    assert_eq!(verticals[0].task_slug, "order_analytics");
    assert!(!verticals[0].sensitive);
    assert_eq!(verticals[1].task_slug, "order_ops_assistant");
    assert!(verticals[1].sensitive);
}
