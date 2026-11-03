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
fn content_ai_constants_and_descriptors() {
    use crate::{
        BLOG_DRAFT_TASK_SLUG, BLOG_DRAFT_TOOL_NAME, CONTENT_MODERATION_TASK_SLUG,
        CONTENT_MODERATION_TOOL_NAME, content_ai_verticals,
    };

    assert_eq!(CONTENT_MODERATION_TASK_SLUG, "content_moderation");
    assert_eq!(BLOG_DRAFT_TASK_SLUG, "blog_draft");
    assert_eq!(CONTENT_MODERATION_TOOL_NAME, "direct.content.moderation");
    assert_eq!(BLOG_DRAFT_TOOL_NAME, "direct.blog.generate_draft");

    let verticals = content_ai_verticals();
    assert_eq!(verticals.len(), 2);
}


