use async_graphql::{EmptySubscription, Schema};
use rustok_forum::{ForumGraphqlErrorExtension, ForumMutation, ForumQuery};

#[test]
fn openapi_exposes_owner_read_state_routes() {
    let document = rustok_forum::openapi::openapi_document();
    let paths = &document.paths.paths;

    for path in [
        "/api/forum/topics/unread",
        "/api/forum/topics/{id}/read-state",
        "/api/forum/categories/{id}/mark-read",
        "/api/forum/topics/mark-all-read",
    ] {
        assert!(paths.contains_key(path), "missing Forum read-state path {path}");
    }

    let topic_read_state = paths
        .get("/api/forum/topics/{id}/read-state")
        .expect("topic read-state path should exist");
    assert!(topic_read_state.get.is_some());
    assert!(topic_read_state.put.is_some());
}

#[test]
fn graphql_schema_exposes_owner_read_state_fields() {
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();
    let sdl = schema.sdl();

    for field in [
        "forumUnreadTopics",
        "forumTopicReadState",
        "markForumTopicRead",
        "markForumCategoryRead",
        "markAllForumTopicsRead",
    ] {
        assert!(sdl.contains(field), "missing Forum GraphQL field {field}");
    }

    for contract_type in [
        "GqlForumTopicUnreadPage",
        "GqlForumTopicReadState",
        "GqlForumTopicsReadBatchResult",
        "MarkForumTopicReadGraphqlInput",
        "MarkForumTopicsReadBatchGraphqlInput",
    ] {
        assert!(
            sdl.contains(contract_type),
            "missing Forum GraphQL contract type {contract_type}"
        );
    }
}
