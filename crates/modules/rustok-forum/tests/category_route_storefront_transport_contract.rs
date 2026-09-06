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
fn graphql_route_resolution_rechecks_exact_category_visibility() {
    let source = read("crates/rustok-forum/src/graphql/category_route_query.rs");
    for marker in [
        "forum_storefront_category_route",
        "ForumCategoryRouteService::new(db.clone())",
        "Some(tenant.default_locale.as_str())",
        "runtime.category_audience_read_service(db.clone())",
        "ForumCategoryReadOperation::SelectedCategory",
        "ForumCategoryReadTransport::Graphql",
        "get_authenticated_storefront_list_visible_with_audience_context",
        "get_public_storefront_visible_with_locale_fallback",
        "if !forum_channel_enabled(ctx).await?",
    ] {
        assert!(source.contains(marker), "missing GraphQL marker: {marker}");
    }
    assert!(!source.contains("pub alias_id"));
    assert!(!source.contains("pub alias_reason"));
}

#[test]
fn native_route_resolution_uses_trusted_context_and_same_owners() {
    let source = read(
        "crates/rustok-forum/storefront/src/transport/native_server_adapter_category_route.rs",
    );
    for marker in [
        "expect_context::<HostRuntimeContext>()",
        "extract::<TenantContext>()",
        "extract::<OptionalAuthContext>()",
        "extract::<RequestContext>()",
        "ForumCategoryRouteService::new(db.clone())",
        "ForumCategoryAudienceReadService::with_audience_facts",
        "ForumCategoryReadTransport::NativeServer",
        "ForumCategoryReadOperation::SelectedCategory",
        "get_authenticated_storefront_list_visible_with_audience_context",
        "get_public_storefront_visible_with_locale_fallback",
        "is_module_enabled(channel_id, \"forum\")",
    ] {
        assert!(source.contains(marker), "missing native marker: {marker}");
    }
    assert!(!source.contains("access_token"));
    assert!(!source.contains("tenant_id: String"));
}

#[test]
fn storefront_deep_link_uses_existing_category_list_permission_boundary() {
    let inline_owner = read("crates/rustok-forum/src/services/category_audience_read_inline.rs");
    let contract =
        read("crates/rustok-forum/contracts/forum-category-route-storefront-transport.json");
    assert!(
        inline_owner.contains("enforce_scope(&security, Resource::ForumCategories, Action::List)")
    );
    assert!(
        inline_owner.contains("get_authenticated_storefront_list_visible_with_audience_context")
    );
    assert!(contract.contains("\"authenticated_permission_boundary\": \"forum_categories:list\""));
}

#[test]
fn public_dto_and_adapters_have_graphql_native_parity() {
    let model = read("crates/rustok-forum/storefront/src/model.rs");
    let graphql =
        read("crates/rustok-forum/storefront/src/transport/category_route_graphql_adapter.rs");
    let native = read(
        "crates/rustok-forum/storefront/src/transport/native_server_adapter_category_route.rs",
    );
    let transport = read("crates/rustok-forum/storefront/src/transport/mod.rs");
    let storefront_lib = read("crates/rustok-forum/storefront/src/lib.rs");

    for marker in [
        "StorefrontForumCategoryRouteDisposition",
        "StorefrontForumCategoryRouteDescriptor",
        "StorefrontForumCategoryRouteResolution",
        "requested_locale",
        "requested_slug",
    ] {
        assert!(model.contains(marker), "missing model marker: {marker}");
    }
    assert!(graphql.contains("forumStorefrontCategoryRoute"));
    assert!(graphql.contains("categoryId locale slug path"));
    assert!(native.contains("map_native_category_route_resolution"));
    assert!(transport.contains("resolve_storefront_category_route"));
    assert!(storefront_lib.contains("resolve_storefront_category_route"));
    assert!(!model.contains("StorefrontForumCategoryRouteGone"));
}

#[test]
fn transport_slice_does_not_mount_or_add_seo_policy() {
    let contract =
        read("crates/rustok-forum/contracts/forum-category-route-storefront-transport.json");
    let docs = read("crates/rustok-forum/docs/forum-24n-category-route-storefront-transport.md");
    for marker in [
        "\"category_route_mounted_in_host\": false",
        "\"category_links_changed\": false",
        "\"http_status_mapping_changed\": false",
        "\"seo_or_hreflang_changed\": false",
        "\"new_migration\": false",
    ] {
        assert!(
            contract.contains(marker),
            "missing contract marker: {marker}"
        );
    }
    assert!(docs.contains("No tests, verifiers, formatting, Cargo commands"));
    assert!(docs.contains("No host router or UI invokes it in this slice"));
}
