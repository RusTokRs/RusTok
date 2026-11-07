use rustok_core::error::RichError;
use rustok_core::module::{HealthStatus, ModuleKind, RusToKModule};
use uuid::Uuid;

use crate::ContentResult;
use crate::entities::node::ContentStatus;
use crate::error::ContentError;
use crate::richtext::RichTextErrorCode;
use crate::services::{
    CanonicalRouteResolver, ResolvedContentRoute, SharedCanonicalRouteResolver, normalize_route,
};
use crate::state_machine::{ContentNode, InvalidStatusTransition, validate_status_transition};
use crate::{CONTENT_NODE_KINDS, ContentModule};

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public surface",
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

#[tokio::test]
async fn module_metadata_matches_contract() {
    let module = ContentModule;
    assert_eq!(module.slug(), "content");
    assert_eq!(module.name(), "Content");
    assert_eq!(
        module.description(),
        "Shared content helpers and cross-domain orchestration module"
    );
    assert_eq!(module.kind(), ModuleKind::Optional);
    assert_eq!(module.health().await, HealthStatus::Healthy);
    assert_eq!(module.permissions().len(), 12);
}

#[test]
fn content_error_variants_and_kind_mapping() {
    let id = Uuid::new_v4();

    let node_not_found = ContentError::NodeNotFound(id);
    assert_eq!(node_not_found.kind(), "not_found");
    let rich: RichError = node_not_found.into();
    assert_eq!(rich.status_code, 404);
    assert_eq!(rich.error_code.as_deref(), Some("NODE_NOT_FOUND"));

    let cat_not_found = ContentError::CategoryNotFound(id);
    assert_eq!(cat_not_found.kind(), "not_found");
    let rich: RichError = cat_not_found.into();
    assert_eq!(rich.status_code, 404);
    assert_eq!(rich.error_code.as_deref(), Some("CATEGORY_NOT_FOUND"));

    let trans_not_found = ContentError::translation_not_found(id, "en");
    assert_eq!(trans_not_found.kind(), "not_found");
    let rich: RichError = trans_not_found.into();
    assert_eq!(rich.status_code, 404);
    assert_eq!(rich.error_code.as_deref(), Some("TRANSLATION_NOT_FOUND"));

    let dup_slug = ContentError::duplicate_slug("hello", "en");
    assert_eq!(dup_slug.kind(), "conflict");
    let rich: RichError = dup_slug.into();
    assert_eq!(rich.status_code, 409);
    assert_eq!(rich.error_code.as_deref(), Some("DUPLICATE_SLUG"));

    let conc = ContentError::concurrent_modification(2, 1);
    assert_eq!(conc.kind(), "conflict");
    let rich: RichError = conc.into();
    assert_eq!(rich.status_code, 409);
    assert_eq!(rich.error_code.as_deref(), Some("CONCURRENT_MODIFICATION"));

    let forbidden = ContentError::forbidden("Access denied");
    assert_eq!(forbidden.kind(), "forbidden");
    let rich: RichError = forbidden.into();
    assert_eq!(rich.status_code, 403);

    let val = ContentError::validation("Bad input");
    assert_eq!(val.kind(), "validation");
    let rich: RichError = val.into();
    assert_eq!(rich.status_code, 400);
}

#[test]
fn invalid_status_transition_error_and_conversion() {
    let err = validate_status_transition(&ContentStatus::Draft, &ContentStatus::Archived)
        .expect_err("draft to archived transition must be invalid");

    assert_eq!(
        err,
        InvalidStatusTransition {
            from: ContentStatus::Draft,
            to: ContentStatus::Archived,
        }
    );

    // Verify std::error::Error and Display implementations
    let msg = format!("{err}");
    assert!(msg.contains("Draft"));
    assert!(msg.contains("Archived"));
    let std_err: &dyn std::error::Error = &err;
    assert_eq!(std_err.to_string(), msg);

    // Verify conversion to ContentError
    let content_err: ContentError = err.into();
    assert!(matches!(content_err, ContentError::Validation(_)));
    assert_eq!(content_err.kind(), "validation");
}

#[test]
fn richtext_error_code_display_and_as_str() {
    let cases = [
        (RichTextErrorCode::DocumentTooLarge, "document_too_large"),
        (RichTextErrorCode::InvalidJson, "invalid_json"),
        (RichTextErrorCode::UnsupportedProfile, "unsupported_profile"),
        (RichTextErrorCode::InvalidRoot, "invalid_root"),
        (RichTextErrorCode::UnsupportedNode, "unsupported_node"),
        (RichTextErrorCode::UnsupportedMark, "unsupported_mark"),
        (RichTextErrorCode::DuplicateMark, "duplicate_mark"),
        (
            RichTextErrorCode::UnsupportedAttribute,
            "unsupported_attribute",
        ),
        (RichTextErrorCode::InvalidAttribute, "invalid_attribute"),
        (RichTextErrorCode::InvalidStructure, "invalid_structure"),
        (RichTextErrorCode::LimitExceeded, "limit_exceeded"),
        (RichTextErrorCode::EmptyDocument, "empty_document"),
        (RichTextErrorCode::UnsafeLink, "unsafe_link"),
    ];

    for (code, expected_str) in cases {
        assert_eq!(code.as_str(), expected_str);
        assert_eq!(format!("{code}"), expected_str);
    }
}

struct DummyRouteResolver;

#[async_trait::async_trait]
impl CanonicalRouteResolver for DummyRouteResolver {
    async fn resolve_route(
        &self,
        _tenant_id: Uuid,
        _locale: &str,
        _route: &str,
    ) -> ContentResult<Option<ResolvedContentRoute>> {
        Ok(None)
    }
}

#[test]
fn debug_formatting_for_shared_services() {
    let resolver = SharedCanonicalRouteResolver(std::sync::Arc::new(DummyRouteResolver));
    let debug_str = format!("{resolver:?}");
    assert!(debug_str.contains("SharedCanonicalRouteResolver"));
}

#[test]
fn route_normalization_contract_invariants() {
    assert_eq!(normalize_route("/blog/posts").unwrap(), "/blog/posts");
    assert_eq!(
        normalize_route("  /catalog/items  ").unwrap(),
        "/catalog/items"
    );

    assert!(normalize_route("").is_err());
    assert!(normalize_route("   ").is_err());
    assert!(normalize_route("no-slash").is_err());
    assert!(normalize_route("/contains space").is_err());
    assert!(normalize_route("https://example.com/route").is_err());
}

#[test]
fn content_node_kinds_allowlist_invariants() {
    assert!(CONTENT_NODE_KINDS.contains(&"post"));
    assert!(CONTENT_NODE_KINDS.contains(&"article"));
    assert!(CONTENT_NODE_KINDS.contains(&"page"));
    assert!(CONTENT_NODE_KINDS.contains(&"comment"));
    assert!(CONTENT_NODE_KINDS.contains(&"forum_topic"));
    assert!(CONTENT_NODE_KINDS.contains(&"forum_reply"));
    assert!(CONTENT_NODE_KINDS.contains(&"blog_post"));
}

#[test]
fn state_machine_nil_uuid_normalization_contract() {
    let node = ContentNode::new_draft(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Some(Uuid::new_v4()),
        "post".to_string(),
    );

    let parent_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let node = node.set_parent(parent_id).set_category(category_id);
    assert_eq!(node.parent_id, Some(parent_id));
    assert_eq!(node.category_id, Some(category_id));

    let node = node.set_parent(Uuid::nil()).set_category(Uuid::nil());
    assert_eq!(node.parent_id, None);
    assert_eq!(node.category_id, None);
}
