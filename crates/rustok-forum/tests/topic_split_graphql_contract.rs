use async_graphql::{EmptySubscription, Schema};
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumMutation, ForumQuery};

const TOPIC_SPLIT_GRAPHQL: &str = include_str!("../src/graphql/topic_split_mutation.rs");

#[test]
fn graphql_schema_exposes_idempotent_topic_split_command() {
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();
    let sdl = schema.sdl();

    for marker in [
        "splitForumTopicReplies",
        "SplitForumTopicRepliesGraphqlInput",
        "GqlForumTopicSplit",
        "operationId",
        "sourceTopicId",
        "targetTopicId",
        "replyIds",
        "eventId",
        "categoryId",
        "actorId",
        "movedReplyCount",
        "movedPublishedReplyCount",
        "sourceResultingPublishedReplyCount",
        "targetResultingPublishedReplyCount",
        "solutionReplyId",
        "splitAt",
    ] {
        assert!(
            sdl.contains(marker),
            "missing GraphQL split marker {marker}"
        );
    }
}

#[test]
fn graphql_split_adapter_uses_routed_tenant_manage_scope_and_owner_service() {
    for marker in [
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "Permission::FORUM_TOPICS_MANAGE",
        "Permission denied: forum_topics:manage required",
        "resolve_tenant_scope",
        "ForumTopicSplitService::new(db.clone(), event_bus.clone())",
        ".split_selected_replies(",
        "SecurityContext::from_permission_snapshot",
        "operation_id: input.operation_id",
        "target_topic_id: input.target_topic_id",
        "reply_ids: input.reply_ids",
        "locale: input.locale",
        "title: input.title",
        "slug: input.slug",
        "reason: input.reason",
    ] {
        assert!(
            TOPIC_SPLIT_GRAPHQL.contains(marker),
            "missing split adapter marker {marker}"
        );
    }

    for forbidden in [
        "ForumTopicMoveService",
        "ForumTopicMergeService",
        "resolve_canonical_topic",
        "forum_topic_split_operations",
        "forum_topic_split_reply_items",
        "TopicService::new",
    ] {
        assert!(
            !TOPIC_SPLIT_GRAPHQL.contains(forbidden),
            "split adapter contains forbidden marker {forbidden}"
        );
    }
}
