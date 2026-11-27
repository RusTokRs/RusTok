#![recursion_limit = "512"]

use async_graphql::{EmptySubscription, Schema};
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumMutation, ForumQuery};

const TOPIC_MOVE_GRAPHQL: &str = include_str!("../src/graphql/topic_move_mutation.rs");

#[test]
fn graphql_schema_exposes_idempotent_topic_move_command() {
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();
    let sdl = schema.sdl();

    for marker in [
        "moveForumTopic",
        "MoveForumTopicGraphqlInput",
        "GqlForumTopicMove",
        "operationId",
        "topicId",
        "sourceCategoryId",
        "targetCategoryId",
        "eventId",
        "actorId",
        "publishedReplyCount",
        "movedAt",
    ] {
        assert!(sdl.contains(marker), "missing GraphQL move marker {marker}");
    }
}

#[test]
fn graphql_move_adapter_uses_routed_tenant_manage_scope_and_owner_service() {
    for marker in [
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "Permission::FORUM_TOPICS_MANAGE",
        "Permission denied: forum_topics:manage required",
        "resolve_tenant_scope",
        "ForumTopicMoveService::new(db.clone(), event_bus.clone())",
        ".move_topic(",
        "SecurityContext::from_permission_snapshot",
        "operation_id: input.operation_id",
        "target_category_id: input.target_category_id",
        "reason: input.reason",
    ] {
        assert!(
            TOPIC_MOVE_GRAPHQL.contains(marker),
            "missing move adapter marker {marker}"
        );
    }

    assert!(!TOPIC_MOVE_GRAPHQL.contains("ForumTopicMergeService"));
    assert!(!TOPIC_MOVE_GRAPHQL.contains("resolve_canonical_topic"));
    assert!(!TOPIC_MOVE_GRAPHQL.contains("forum_topic_move_operations"));
}
