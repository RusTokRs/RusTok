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
fn product_ai_constants_and_descriptors() {
    use crate::{
        PRODUCT_ATTRIBUTES_TASK_SLUG, PRODUCT_ATTRIBUTES_TOOL_NAME,
        PRODUCT_COPY_TASK_SLUG, PRODUCT_COPY_TOOL_NAME, product_ai_verticals,
    };

    assert_eq!(PRODUCT_COPY_TASK_SLUG, "product_copy");
    assert_eq!(PRODUCT_ATTRIBUTES_TASK_SLUG, "product_attributes");
    assert_eq!(PRODUCT_COPY_TOOL_NAME, "direct.commerce.product_copy");
    assert_eq!(PRODUCT_ATTRIBUTES_TOOL_NAME, "direct.commerce.product_attributes");

    let verticals = product_ai_verticals();
    assert_eq!(verticals.len(), 2);
    assert_eq!(verticals[0].task_slug, "product_copy");
    assert!(!verticals[0].sensitive);
    assert_eq!(verticals[1].task_slug, "product_attributes");
    assert!(!verticals[1].sensitive);
}


