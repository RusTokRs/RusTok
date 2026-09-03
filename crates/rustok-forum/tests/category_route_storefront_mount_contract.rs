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
fn category_cards_emit_canonical_locale_slug_routes() {
    let core = read("crates/rustok-forum/storefront/src/core.rs");
    for marker in [
        "pub fn category_href(locale: &str, slug: &str) -> Option<String>",
        "item.effective_locale.as_str()",
        "item.slug.as_str()",
        "Some(format!(\"/{locale}/forum/c/{slug}\"))",
        ".unwrap_or_else(|| module_route_base.to_string())",
    ] {
        assert!(
            core.contains(marker),
            "missing category href marker: {marker}"
        );
    }
    assert!(!core.contains("?category={category_id}"));
}

#[test]
fn rust_storefront_mount_executes_transport_decision_without_storage_access() {
    let host = read("apps/storefront/src/forum_category_route.rs");
    let host_lib = read("apps/storefront/src/lib.rs");

    for marker in [
        "rustok_forum_storefront::resolve_storefront_category_route(",
        "ForumCategoryHostAction::Redirect",
        "StatusCode::NOT_FOUND",
        "StatusCode::SERVICE_UNAVAILABLE",
        "private_permanent_redirect(location.as_str())",
        "query_params.insert(\"category\".to_string(), category_id)",
        "query_params.remove(\"topic\")",
        "fn safe_route_segment(segment: &str) -> bool",
        "fn valid_category_descriptor(",
    ] {
        assert!(host.contains(marker), "missing host marker: {marker}");
    }
    for forbidden in [
        "ForumCategoryRouteService",
        "ForumCategoryAudienceReadService",
        "forum_category_route_aliases",
        "Statement::from_sql_and_values",
        "SELECT ",
    ] {
        assert!(
            !host.contains(forbidden),
            "host contains forbidden owner/storage marker: {forbidden}"
        );
    }

    for marker in [
        "mod forum_category_route;",
        "\"/{locale}/forum/c/{slug}\"",
        "forum_category_route::render_forum_category_route_response(",
        "original_uri: axum::extract::OriginalUri",
        "original_uri.0.path().to_string()",
    ] {
        assert!(host_lib.contains(marker), "missing router marker: {marker}");
    }
}

#[test]
fn mount_contract_keeps_private_fail_closed_http_policy() {
    let contract = read("crates/rustok-forum/contracts/forum-category-route-storefront-mount.json");
    let docs = read("crates/rustok-forum/docs/forum-24o-category-route-storefront-mount.md");

    for marker in [
        "\"redirect_or_noncanonical_raw_path_status\": 308",
        "\"not_found_status\": 404",
        "\"transport_or_malformed_status\": 503",
        "\"redirect_cache_control\": \"private, no-store\"",
        "\"gone_status\": null",
        "\"owner_target_must_be_local_absolute_path\": true",
        "\"protocol_relative_or_control_character_target_rejected\": true",
        "\"seo_or_hreflang_changed\": false",
        "\"new_migration\": false",
    ] {
        assert!(
            contract.contains(marker),
            "missing contract marker: {marker}"
        );
    }

    for marker in [
        "private `308 Permanent Redirect`",
        "private `404 Not Found`",
        "private `503 Service Unavailable`",
        "There is no category `GONE` decision",
        "protocol-relative paths",
        "No tests, verifiers, formatting, Cargo commands",
    ] {
        assert!(
            docs.contains(marker),
            "missing documentation marker: {marker}"
        );
    }
}

#[test]
fn topic_mount_and_seo_boundaries_remain_outside_this_slice() {
    let contract = read("crates/rustok-forum/contracts/forum-category-route-storefront-mount.json");
    let host = read("apps/storefront/src/forum_category_route.rs");

    for marker in [
        "\"topic_route_changed\": false",
        "\"category_gone_added\": false",
        "\"seo_or_hreflang_changed\": false",
        "\"next_storefront_changed\": false",
    ] {
        assert!(
            contract.contains(marker),
            "missing compatibility marker: {marker}"
        );
    }
    for forbidden in ["hreflang", "schema.org", "StatusCode::GONE"] {
        assert!(
            !host.contains(forbidden),
            "host contains out-of-scope marker: {forbidden}"
        );
    }
}
