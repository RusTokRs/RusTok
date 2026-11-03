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
fn media_ai_constants_and_descriptors() {
    use crate::{
        IMAGE_ASSET_TASK_SLUG, IMAGE_ASSET_TOOL_NAME, media_ai_verticals,
    };

    assert_eq!(IMAGE_ASSET_TASK_SLUG, "image_asset");
    assert_eq!(IMAGE_ASSET_TOOL_NAME, "direct.media.generate_image");

    let verticals = media_ai_verticals();
    assert_eq!(verticals.len(), 1);
    assert_eq!(verticals[0].task_slug, "image_asset");
    assert_eq!(verticals[0].tool_name, "direct.media.generate_image");
    assert!(!verticals[0].sensitive);
}


