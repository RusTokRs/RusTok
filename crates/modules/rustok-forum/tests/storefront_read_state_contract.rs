use async_graphql::{EmptySubscription, Schema};
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumMutation, ForumQuery};

const GRAPHQL_ADAPTER: &str = include_str!("../storefront/src/transport/graphql_adapter.rs");
const NATIVE_ADAPTER: &str = include_str!("../storefront/src/transport/native_server_adapter.rs");
const NATIVE_BULK_ADAPTER: &str =
    include_str!("../storefront/src/transport/native_server_adapter_bulk.rs");
const TRANSPORT_SELECTOR: &str = include_str!("../storefront/src/transport/mod.rs");

#[test]
fn graphql_schema_exposes_storefront_visible_unread_contract() {
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();
    let sdl = schema.sdl();

    for field in [
        "forumStorefrontUnreadTopics",
        "markForumStorefrontTopicRead",
        "markForumStorefrontCategoryRead",
        "markAllForumStorefrontTopicsRead",
    ] {
        assert!(
            sdl.contains(field),
            "missing Forum storefront read-state field {field}"
        );
    }

    for contract_type in [
        "GqlForumStorefrontUnreadTopic",
        "GqlForumStorefrontUnreadTopicPage",
        "GqlForumStorefrontTopicReadState",
        "GqlForumTopicsReadBatchResult",
        "MarkForumTopicsReadBatchGraphqlInput",
    ] {
        assert!(
            sdl.contains(contract_type),
            "missing Forum storefront read-state type {contract_type}"
        );
    }

    for contract_field in [
        "readStateExplicit",
        "lastReadPosition",
        "lastReadRevision",
        "unreadCount",
        "hasUnreadTopicRevision",
        "isUnread",
        "nextCursor",
        "hasMore",
        "snapshotAt",
    ] {
        assert!(
            sdl.contains(contract_field),
            "missing Forum storefront read-state field {contract_field}"
        );
    }
}

#[test]
fn storefront_adapters_use_only_exact_visibility_safe_composition() {
    for marker in [
        "forumStorefrontUnreadTopics",
        "markForumStorefrontTopicRead",
        "markForumStorefrontCategoryRead",
        "markAllForumStorefrontTopicsRead",
    ] {
        assert!(GRAPHQL_ADAPTER.contains(marker));
    }
    assert!(NATIVE_ADAPTER.contains("list_topics_with_unread_audience_visible"));
    assert!(NATIVE_ADAPTER.contains("mark_topic_read_current_audience_visible"));
    assert!(NATIVE_BULK_ADAPTER.contains("mark_category_read_audience_visible"));
    assert!(NATIVE_BULK_ADAPTER.contains("mark_all_read_audience_visible"));
    assert!(NATIVE_BULK_ADAPTER.contains("MarkCategoryRead"));
    assert!(NATIVE_BULK_ADAPTER.contains("MarkAllRead"));

    for source in [GRAPHQL_ADAPTER, NATIVE_ADAPTER, NATIVE_BULK_ADAPTER] {
        assert!(!source.contains("summarize_topic_ids"));
        assert!(!source.contains("forum_topic_read_states"));
        assert!(!source.contains("forum_replies::"));
        assert!(!source.contains("forum_topic_revisions::"));
    }
}

#[test]
fn storefront_transport_selector_keeps_native_and_graphql_without_fallback() {
    assert!(TRANSPORT_SELECTOR.contains("mark_storefront_category_read_server"));
    assert!(TRANSPORT_SELECTOR.contains("mark_storefront_category_read_graphql"));
    assert!(TRANSPORT_SELECTOR.contains("mark_all_storefront_topics_read_server"));
    assert!(TRANSPORT_SELECTOR.contains("mark_all_storefront_topics_read_graphql"));
    assert!(!TRANSPORT_SELECTOR.contains("or_else"));
}
