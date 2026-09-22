use async_graphql::{EmptySubscription, Schema};
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumMutation, ForumQuery};

const TOPIC_MERGE_GRAPHQL: &str = include_str!("../src/graphql/topic_merge_mutation.rs");

#[test]
fn graphql_schema_exposes_idempotent_topic_merge_command() {
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();
    let sdl = schema.sdl();

    for marker in [
        "mergeForumTopic",
        "MergeForumTopicGraphqlInput",
        "GqlForumTopicMerge",
        "operationId",
        "sourceTopicId",
        "targetTopicId",
        "eventId",
        "categoryId",
        "actorId",
        "movedReplyCount",
        "movedPublishedReplyCount",
        "resultingPublishedReplyCount",
        "positionOffset",
        "mergedAt",
    ] {
        assert!(
            sdl.contains(marker),
            "missing GraphQL merge marker {marker}"
        );
    }
}

#[test]
fn graphql_merge_adapter_uses_routed_tenant_manage_scope_and_owner_service() {
    for marker in [
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "Permission::FORUM_TOPICS_MANAGE",
        "Permission denied: forum_topics:manage required",
        "resolve_tenant_scope",
        "ForumTopicMergeService::new(db.clone(), event_bus.clone())",
        ".merge_topic(",
        "SecurityContext::from_permission_snapshot",
        "operation_id: input.operation_id",
        "source_topic_id: input.source_topic_id",
        "reason: input.reason",
    ] {
        assert!(
            TOPIC_MERGE_GRAPHQL.contains(marker),
            "missing merge adapter marker {marker}"
        );
    }

    assert!(!TOPIC_MERGE_GRAPHQL.contains("ForumTopicMoveService"));
    assert!(!TOPIC_MERGE_GRAPHQL.contains("resolve_canonical_topic"));
    assert!(!TOPIC_MERGE_GRAPHQL.contains("forum_topic_merge_operations"));
}
