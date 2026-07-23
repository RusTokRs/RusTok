use rustok_forum::ForumError;

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

#[test]
fn sensitive_forum_error_display_is_redacted() {
    let secret_database_detail = "password=secret database host=private";
    let database_error = ForumError::Database(sea_orm::DbErr::Custom(
        secret_database_detail.to_string(),
    ));
    assert_eq!(
        database_error.to_string(),
        "Forum persistence operation failed"
    );
    assert!(!database_error.to_string().contains(secret_database_detail));
    assert_eq!(database_error.stable_code(), "FORUM_INTERNAL_ERROR");
    assert!(database_error.is_retryable());

    let capability_error = ForumError::capability_failure(
        "profiles",
        "PRIVATE_PROVIDER_CODE",
        "private upstream response",
        true,
    );
    assert_eq!(
        capability_error.to_string(),
        "Forum capability operation failed"
    );
    assert!(!capability_error.to_string().contains("PRIVATE_PROVIDER_CODE"));
    assert!(!capability_error.to_string().contains("private upstream response"));
    assert_eq!(
        capability_error.stable_code(),
        "FORUM_CAPABILITY_FAILURE"
    );
    assert!(capability_error.is_retryable());
}

#[test]
fn forum_error_annotations_do_not_reintroduce_sensitive_sources() {
    let source = include_str!("../src/error.rs");

    for unsafe_annotation in [
        "Database error: {0}",
        "Content error: {0}",
        "Internal error: {0}",
        "failed with `{source_code}`",
    ] {
        assert!(
            !source.contains(unsafe_annotation),
            "public ForumError display leaks a sensitive source: {unsafe_annotation}"
        );
    }
}

#[test]
fn public_topic_and_reply_reads_fail_closed_without_localized_content() {
    let topic_facade = include_str!("../src/services/topic_facade.rs");
    let reply_facade = include_str!("../src/services/reply_facade.rs");

    assert!(topic_facade.contains("require_localized_topic_response"));
    assert!(topic_facade.contains("require_localized_topic_page"));
    assert!(topic_facade.contains("has no localized translation"));
    assert!(topic_facade.contains("available_locales.is_empty()"));

    assert!(reply_facade.contains("require_localized_reply_response"));
    assert!(reply_facade.contains("require_localized_reply_list_page"));
    assert!(reply_facade.contains("require_localized_reply_response_page"));
    assert!(reply_facade.contains("has no localized body"));
}

#[test]
fn topic_delete_does_not_materialize_every_reply() {
    let topic_owner = include_str!("../src/services/topic_owner.rs");
    let user_stats = include_str!("../src/services/user_stats.rs");

    assert!(!topic_owner.contains("public_reply_author_ids"));
    assert!(!topic_owner.contains(".all(&txn)"));
    assert!(topic_owner.contains(".count(&txn)"));
    assert!(topic_owner.contains("decrement_topic_thread_aggregated_in_tx"));
    assert!(user_stats.contains("UPDATE forum_user_stats"));
    assert!(user_stats.contains("SELECT COUNT(*) FROM forum_replies"));
}
