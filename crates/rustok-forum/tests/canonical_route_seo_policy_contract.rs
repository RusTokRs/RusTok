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
fn seo_wrapper_uses_route_owners_for_canonical_and_alternate_paths() {
    let source = read("crates/rustok-forum/src/seo_audience_targets.rs");

    for marker in [
        "const MAX_FORUM_SEO_ALTERNATE_ROUTES: usize = 64",
        "ForumCategoryRouteService",
        "ForumTopicRouteService",
        "rewrite_category_target",
        "rewrite_topic_target",
        "parse_canonical_forum_route",
        "ForumTopicRouteDisposition::Gone",
        "Forum category SEO alternate resolved through an unexpected locale fallback",
        "Forum topic SEO alternate resolved through an unexpected locale fallback",
        "BTreeMap<String, String>",
        "BTreeSet<String>",
        "record.template_fields.insert(\"route\", canonical_route)",
        "return category_provider().load_target(runtime, request).await",
        "return topic_provider().load_target(runtime, request).await",
    ] {
        assert!(
            source.contains(marker),
            "missing SEO owner marker: {marker}"
        );
    }

    for forbidden in [
        "format!(\"/modules/forum?category=",
        "format!(\"/modules/forum?category={}&topic={}",
        "CategoryService::new",
        "TopicService::new",
        "SecurityContext::system()",
    ] {
        assert!(
            !source.contains(forbidden),
            "SEO wrapper contains forbidden duplicated policy: {forbidden}"
        );
    }
}

#[test]
fn canonical_route_handlers_compose_head_without_replacing_route_authority() {
    let category = read("apps/storefront/src/forum_category_route.rs");
    let topic = read("apps/storefront/src/forum_topic_route.rs");

    for (source, owner, removed_selector, error_message) in [
        (
            category.as_str(),
            "resolve_storefront_category_route",
            "query_params.remove(\"topic\")",
            "failed to resolve Forum category SEO context",
        ),
        (
            topic.as_str(),
            "resolve_storefront_topic_route",
            "query_params.remove(\"category\")",
            "failed to resolve Forum topic SEO context",
        ),
    ] {
        for marker in [
            owner,
            "fetch_seo_page_context(",
            removed_selector,
            "seo_context.as_ref()",
            error_message,
            "None",
        ] {
            assert!(source.contains(marker), "missing host SEO marker: {marker}");
        }
    }

    assert!(category.contains("ForumCategoryHostAction::Redirect"));
    assert!(topic.contains("ForumTopicHostAction::Gone"));
    assert!(topic.contains("fn valid_topic_descriptor"));
    assert!(topic.contains("!path.starts_with(\"//\")"));
    assert!(topic.contains("!path.chars().any(char::is_control)"));
}

#[test]
fn contract_preserves_visibility_schema_and_compatibility_boundaries() {
    let contract = read("crates/rustok-forum/contracts/forum-canonical-route-seo-policy.json");
    let docs = read("crates/rustok-forum/docs/forum-24p-canonical-route-seo-policy.md");

    for marker in [
        "\"task\": \"FORUM-24P\"",
        "\"legacy_uuid_module_routes_emitted_as_public_canonical\": false",
        "\"managed_authoring_legacy_load_preserved\": true",
        "\"available_locale_fallback_not_emitted_as_exact_alternate\": true",
        "\"maximum_alternates\": 64",
        "\"authorized_topic_gone_excluded_from_seo\": true",
        "\"private_pending_archived_or_hidden_targets_absent\": true",
        "\"topic_schema\": \"DiscussionForumPosting\"",
        "\"question_answer_schema_added\": false",
        "\"seo_failure_turns_public_route_into_outage\": false",
        "\"search_result_routes_changed\": false",
        "\"next_storefront_changed\": false",
        "\"new_migration\": false",
        "\"executed_by_implementation_agent\": false",
    ] {
        assert!(
            contract.contains(marker),
            "missing contract marker: {marker}"
        );
    }

    for marker in [
        "Legacy UUID module routes remain accepted",
        "deduplicated and sorted",
        "An owner-authorized topic `gone` decision produces no SEO target",
        "optional SEO transport failure",
        "This slice does not emit `QAPage`",
        "No tests, Node verifiers, formatting, Cargo commands",
        "implementation-plan.md` remains the only authoritative roadmap",
    ] {
        assert!(
            docs.contains(marker),
            "missing documentation marker: {marker}"
        );
    }
}

#[test]
fn stale_public_discovery_guard_tracks_current_canonical_card_routes() {
    let verifier = read("scripts/verify/verify-forum-public-discovery-seo.mjs");

    for marker in [
        "category_provider().load_target",
        "topic_provider().load_target",
        "ForumCategoryRouteService",
        "ForumTopicRouteService",
        "Some(format!(\"/{locale}/forum/c/{slug}\"))",
        "Some(format!(\"/{locale}/forum/t/{short_id}/{slug}\"))",
        "retired category UUID card route",
        "retired topic UUID card route",
    ] {
        assert!(
            verifier.contains(marker),
            "missing refreshed guard marker: {marker}"
        );
    }
}
