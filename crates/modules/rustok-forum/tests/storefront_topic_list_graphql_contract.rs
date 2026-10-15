use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use rustok_forum::ForumQuery;

#[tokio::test]
async fn public_topic_list_has_only_the_audience_owner_field() {
    let schema = Schema::build(ForumQuery::default(), EmptyMutation, EmptySubscription).finish();
    let sdl = schema.sdl();
    assert!(sdl.contains("forumStorefrontAudienceTopics("));
    assert!(!sdl.contains("forumStorefrontTopics("));

    // Validation rejects the old field before any context or DB access; no
    // compatibility resolver may expose hidden titles, metadata or cursors.
    let response = schema
        .execute("{ forumStorefrontTopics { items { id title } nextCursor } }")
        .await;
    assert!(!response.errors.is_empty());
    assert_eq!(response.data, async_graphql::Value::Null);
}
