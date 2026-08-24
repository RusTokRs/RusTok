use async_graphql::{EmptySubscription, Schema};
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumMutation, ForumQuery};

const REPLY_RANGE_MOVE_GRAPHQL: &str =
    include_str!("../src/graphql/topic_reply_range_move_mutation.rs");

#[test]
fn graphql_schema_exposes_idempotent_reply_range_move_command() {
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();
    let sdl = schema.sdl();

    for marker in [
        "moveForumTopicReplyRange",
        "MoveForumTopicReplyRangeGraphqlInput",
        "GqlForumReplyRangeMove",
        "operationId",
        "sourceTopicId",
        "targetTopicId",
        "startPosition",
        "endPosition",
        "eventId",
        "sourceCategoryId",
        "targetCategoryId",
        "actorId",
        "sourceStartPosition",
        "sourceEndPosition",
        "targetStartPosition",
        "targetEndPosition",
        "movedReplyCount",
        "movedPublishedReplyCount",
        "sourceResultingPublishedReplyCount",
        "targetResultingPublishedReplyCount",
        "movedSolutionReplyId",
        "sourceResultingSolutionReplyId",
        "targetResultingSolutionReplyId",
        "movedAt",
    ] {
        assert!(
            sdl.contains(marker),
            "missing GraphQL reply-range marker {marker}"
        );
    }
}

#[test]
fn graphql_reply_range_adapter_uses_routed_tenant_manage_scope_and_owner_service() {
    for marker in [
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "Permission::FORUM_TOPICS_MANAGE",
        "Permission denied: forum_topics:manage required",
        "resolve_tenant_scope",
        "ForumReplyRangeMoveService::new(db.clone(), event_bus.clone())",
        ".move_reply_range(",
        "SecurityContext::from_permission_snapshot",
        "operation_id: input.operation_id",
        "target_topic_id: input.target_topic_id",
        "start_position: input.start_position",
        "end_position: input.end_position",
        "reason: input.reason",
    ] {
        assert!(
            REPLY_RANGE_MOVE_GRAPHQL.contains(marker),
            "missing reply-range adapter marker {marker}"
        );
    }

    for forbidden in [
        "ForumTopicMoveService",
        "ForumTopicMergeService",
        "ForumTopicSplitService",
        "resolve_canonical_topic",
        "forum_reply_range_move_operations",
        "forum_reply_range_move_items",
        "ReplyService::new",
    ] {
        assert!(
            !REPLY_RANGE_MOVE_GRAPHQL.contains(forbidden),
            "reply-range adapter contains forbidden marker {forbidden}"
        );
    }
}
