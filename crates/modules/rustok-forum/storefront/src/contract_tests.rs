#[test]
fn topic_selection_is_available_through_the_public_crate_api() {
    let selection = crate::ForumTopicSelection {
        search: Some("owner-side search".into()),
        order: crate::ForumTopicFeedOrder::Top,
        solved_only: true,
        unread_only: false,
    };
    assert_eq!(selection.order, crate::ForumTopicFeedOrder::Top);
}

#[test]
fn test_crate_api_compliance() {
    let api_doc = include_str!("../CRATE_API.md");
    assert!(api_doc.contains("## Platform Capability Bindings"));
    assert!(api_doc.contains("## Domain Collaborations"));
    assert!(api_doc.contains("## Minimum Contract Set"));
    assert!(api_doc.contains("### Input DTOs/Commands"));
    assert!(api_doc.contains("### Domain Invariants"));
    assert!(api_doc.contains("### Events / Outbox Side Effects"));
    assert!(api_doc.contains("### Errors / Failure Codes"));
}
