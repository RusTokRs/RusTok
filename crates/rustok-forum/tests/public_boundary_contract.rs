#[test]
fn category_owner_does_not_expose_raw_persistence_service() {
    let source = include_str!("../src/services/category_owner.rs");

    assert!(
        !source.contains("impl Deref for CategoryService"),
        "the public category owner must not dereference into raw persistence"
    );
    assert!(
        !source.contains("std::ops::Deref"),
        "the category owner must expose only explicit domain operations"
    );
    for method in [
        "pub async fn create(",
        "pub async fn get(",
        "pub async fn update(",
        "pub async fn delete(",
        "pub async fn move_category(",
        "pub async fn archive_subtree(",
    ] {
        assert!(source.contains(method), "missing explicit owner method: {method}");
    }
    assert!(
        source.contains("archive_subtree_for_delete"),
        "normal category deletion must route through lifecycle archival"
    );
}

#[test]
fn http_controllers_use_stable_forum_error_mapping() {
    let controllers = [
        include_str!("../src/controllers/categories.rs"),
        include_str!("../src/controllers/category_commands.rs"),
        include_str!("../src/controllers/category_lifecycle.rs"),
        include_str!("../src/controllers/category_policy.rs"),
        include_str!("../src/controllers/category_tree.rs"),
        include_str!("../src/controllers/content_commands.rs"),
        include_str!("../src/controllers/quote_commands.rs"),
        include_str!("../src/controllers/replies.rs"),
        include_str!("../src/controllers/subscriptions.rs"),
        include_str!("../src/controllers/topics.rs"),
        include_str!("../src/controllers/users.rs"),
        include_str!("../src/controllers/widgets.rs"),
    ];

    for source in controllers {
        assert!(
            !source.contains("HttpError::bad_request(\"forum_operation_failed\""),
            "controller bypasses the stable ForumError mapper"
        );
        assert!(
            !source.contains("HttpError::internal(error.to_string())"),
            "controller exposes an internal Forum error message"
        );
        assert!(
            !source.contains(
                "HttpError::unauthorized(\n            \"forum_permission_denied\""
            ),
            "authenticated permission failures must be HTTP 403"
        );
    }
}

#[test]
fn category_reads_do_not_silently_default_missing_translations() {
    let source = include_str!("../src/services/category.rs");

    assert!(source.contains("has no localized translation"));
    assert!(!source.contains("name: resolved"));
    assert!(!source.contains("slug: resolved"));
    assert!(source.contains("Column::TenantId.eq(tenant_id)"));
}

#[test]
fn page_builder_is_an_optional_forum_capability() {
    let module = include_str!("../src/lib.rs");
    let manifest = include_str!("../rustok-module.toml");

    assert!(module.contains("&[\"content\", \"taxonomy\"]"));
    assert!(!module.contains("&[\"content\", \"taxonomy\", \"page_builder\"]"));
    assert!(!manifest.contains("[dependencies.page_builder]"));
    assert!(manifest.contains("[fba.builder_consumer]"));
    assert!(manifest.contains("builder_disabled"));
}
