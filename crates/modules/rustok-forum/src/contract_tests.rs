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
fn forum_error_converts_to_core_error() {
    use crate::ForumError;
    use uuid::Uuid;

    let id = Uuid::new_v4();
    let err: rustok_core::Error = ForumError::CategoryNotFound(id).into();
    assert!(matches!(err, rustok_core::Error::NotFound(_)));

    let err: rustok_core::Error = ForumError::TopicNotFound(id).into();
    assert!(matches!(err, rustok_core::Error::NotFound(_)));

    let err: rustok_core::Error = ForumError::ReplyNotFound(id).into();
    assert!(matches!(err, rustok_core::Error::NotFound(_)));

    let err: rustok_core::Error = ForumError::SolutionNotFound(id).into();
    assert!(matches!(err, rustok_core::Error::NotFound(_)));

    let err: rustok_core::Error = ForumError::TopicRouteNotFound.into();
    assert!(matches!(err, rustok_core::Error::NotFound(_)));

    let err: rustok_core::Error = ForumError::CategoryRouteNotFound.into();
    assert!(matches!(err, rustok_core::Error::NotFound(_)));

    let err: rustok_core::Error = ForumError::Forbidden("access denied".into()).into();
    assert!(matches!(err, rustok_core::Error::Forbidden(_)));

    let err: rustok_core::Error = ForumError::Validation("invalid input".into()).into();
    assert!(matches!(err, rustok_core::Error::Validation(_)));
}

#[test]
fn module_contract_metadata_matches() {
    use rustok_core::RusToKModule;
    let module = crate::ForumModule;
    assert_eq!(module.slug(), "forum");
    assert_eq!(module.name(), "Forum");
    assert_eq!(module.kind(), rustok_core::ModuleKind::Optional);
    assert_eq!(module.dependencies(), &["content", "media", "taxonomy"]);
}
