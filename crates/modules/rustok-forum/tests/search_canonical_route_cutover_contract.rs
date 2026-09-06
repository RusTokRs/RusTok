use std::{fs, path::PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"))
}

#[test]
fn forum_projection_publishes_only_exact_owner_routes() {
    let source = read("crates/rustok-forum/src/search_projection.rs");

    for marker in [
        "ForumCategoryRouteService",
        "ForumTopicRouteService",
        "exact_category_route",
        "exact_topic_route",
        ".canonical_descriptor(",
        "category.effective_locale != locale",
        "topic.effective_locale != locale",
        "descriptor.category_id != category_id || descriptor.locale != locale",
        "descriptor.topic_id != topic_id || descriptor.locale != locale",
        "format!(\"{topic_route}?reply={reply_id}\")",
        "\"route\": route",
    ] {
        assert!(
            source.contains(marker),
            "missing projection marker: {marker}"
        );
    }

    for forbidden in ["\"/modules/forum?category=", "\"/modules/forum?topic="] {
        assert!(
            !source.contains(forbidden),
            "Forum projection retains UUID route construction: {forbidden}"
        );
    }
}

#[test]
fn search_validates_owner_route_without_rebuilding_forum_identity() {
    let source = read("crates/rustok-search/src/engine.rs");

    for marker in [
        "canonical_forum_projected_result_url(value)",
        "value.payload.get(\"route\")",
        "canonical_forum_category_route",
        "canonical_forum_topic_route",
        "exact_forum_locale",
        "rustok_api::normalize_locale_tag",
        "forum_topic_short_identity",
        "valid_forum_short_identity",
        "valid_forum_slug",
        "route.starts_with(\"//\")",
        "route.contains('#')",
        "canonical_url_accepts_owner_projected_forum_category_topic_and_reply_routes",
        "canonical_url_rejects_stale_or_malformed_forum_route_projections",
    ] {
        assert!(source.contains(marker), "missing Search marker: {marker}");
    }

    for forbidden in [
        "const FORUM_STOREFRONT_ROUTE",
        "canonical_forum_reply_result_url",
        "{FORUM_STOREFRONT_ROUTE}?category=",
        "{FORUM_STOREFRONT_ROUTE}?topic=",
    ] {
        assert!(
            !source.contains(forbidden),
            "Search retains obsolete Forum URL policy: {forbidden}"
        );
    }
}

#[test]
fn contract_locks_reindex_fail_closed_and_transport_compatibility() {
    let contract = read("crates/rustok-forum/contracts/forum-search-canonical-route-cutover.json");
    let docs = read("crates/rustok-forum/docs/forum-24q-search-canonical-route-cutover.md");
    let evidence =
        read("crates/rustok-search/contracts/evidence/search-canonical-url-contract.json");

    for marker in [
        "\"task\": \"FORUM-24Q\"",
        "\"search_reconstructs_forum_slug\": false",
        "\"search_reconstructs_topic_short_id\": false",
        "\"legacy_uuid_query_routes_rejected\": true",
        "\"legacy_documents_fail_closed_until_reindexed\": true",
        "\"compatibility_fallback_added\": false",
        "\"search_storage_schema_changed\": false",
        "\"owner_event_schema_changed\": false",
        "\"new_migration\": false",
        "\"executed_by_implementation_agent\": false",
    ] {
        assert!(
            contract.contains(marker),
            "missing contract marker: {marker}"
        );
    }

    for marker in [
        "Existing indexed Forum documents",
        "No compatibility fallback is added",
        "A full Forum Search projection rebuild is therefore required",
        "A canonical route is not an authorization token",
        "No tests, Node verifiers, formatting, Cargo commands",
        "implementation-plan.md` remains the only authoritative roadmap",
    ] {
        assert!(
            docs.contains(marker),
            "missing documentation marker: {marker}"
        );
    }

    for marker in [
        "\"forum_projection_owner\": \"crates/rustok-forum/src/search_projection.rs\"",
        "\"name\": \"forum_projection_owner_routes\"",
        "\"name\": \"forum_stale_projection_fail_closed\"",
        "no compatibility fallback exists",
        "verify no UUID Forum query route is emitted after reindex",
    ] {
        assert!(
            evidence.contains(marker),
            "missing Search evidence marker: {marker}"
        );
    }
}
