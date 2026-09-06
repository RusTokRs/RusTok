use async_graphql::{EmptySubscription, Schema};
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumMutation, ForumQuery};

const TOPIC_FORK_GRAPHQL: &str = include_str!("../src/graphql/topic_fork_mutation.rs");

#[test]
fn graphql_schema_exposes_idempotent_topic_fork_command() {
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();
    let sdl = schema.sdl();

    for marker in [
        "forkForumTopicReplyBranch",
        "ForkForumTopicReplyBranchGraphqlInput",
        "GqlForumTopicFork",
        "operationId",
        "sourceTopicId",
        "targetTopicId",
        "rootReplyId",
        "eventId",
        "categoryId",
        "actorId",
        "copiedReplyCount",
        "copiedPublishedReplyCount",
        "copiedBodyCount",
        "copiedReplyRevisionCount",
        "copiedRelationRevisionCount",
        "copiedMentionCount",
        "copiedQuoteCount",
        "forkedAt",
    ] {
        assert!(sdl.contains(marker), "missing GraphQL fork marker {marker}");
    }
}

#[test]
fn graphql_fork_adapter_uses_routed_tenant_manage_scope_and_owner_service() {
    for marker in [
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "Permission::FORUM_TOPICS_MANAGE",
        "Permission denied: forum_topics:manage required",
        "resolve_tenant_scope",
        "ForumTopicForkService::new(db.clone(), event_bus.clone())",
        ".fork_reply_branch(",
        "SecurityContext::from_permission_snapshot",
        "operation_id: input.operation_id",
        "target_topic_id: input.target_topic_id",
        "root_reply_id: input.root_reply_id",
        "locale: input.locale",
        "title: input.title",
        "slug: input.slug",
        "reason: input.reason",
    ] {
        assert!(
            TOPIC_FORK_GRAPHQL.contains(marker),
            "missing fork adapter marker {marker}"
        );
    }

    for forbidden in [
        "resolve_canonical_topic",
        "forum_topic_fork_operations",
        "forum_topic_fork_reply_items",
        "forum_topic_fork_revision_items",
        "ForumTopicMoveService",
        "ForumTopicMergeService",
        "ForumTopicSplitService",
        "TopicService::new",
    ] {
        assert!(
            !TOPIC_FORK_GRAPHQL.contains(forbidden),
            "fork adapter contains forbidden marker {forbidden}"
        );
    }
}
