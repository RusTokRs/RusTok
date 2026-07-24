use async_graphql::{EmptySubscription, Schema};
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumMutation, ForumQuery};

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
    ] {
        assert!(
            sdl.contains(contract_field),
            "missing Forum storefront read-state field {contract_field}"
        );
    }
}

#[test]
fn storefront_bulk_read_mutations_remain_closed() {
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .finish();
    let sdl = schema.sdl();

    assert!(!sdl.contains("markForumStorefrontCategoryRead"));
    assert!(!sdl.contains("markAllForumStorefrontTopicsRead"));
}
