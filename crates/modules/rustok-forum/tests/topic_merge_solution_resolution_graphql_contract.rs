use async_graphql::{EmptySubscription, Schema};
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumMutation, ForumQuery};

const TOPIC_MERGE_GRAPHQL: &str = include_str!("../src/graphql/topic_merge_mutation.rs");
const TOPIC_MERGE_OWNER: &str = include_str!("../src/services/topic_merge.rs");
const RESOLUTION_ENTITY: &str =
    include_str!("../src/entities/forum_topic_merge_solution_resolution.rs");
const RESOLUTION_MIGRATION: &str =
    include_str!("../src/migrations/m20260803_000018_add_forum_topic_merge_solution_resolution.rs");

#[test]
fn graphql_schema_exposes_explicit_solution_resolution_command() {
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
        "mergeForumTopicResolvingSolution",
        "ResolveForumTopicMergeSolutionGraphqlInput",
        "GqlForumTopicMergeSolutionResolution",
        "selectedSolutionReplyId",
        "operationId",
        "sourceTopicId",
        "targetTopicId",
        "reason",
        "merge",
    ] {
        assert!(
            sdl.contains(marker),
            "missing GraphQL resolution marker {marker}"
        );
    }
}

#[test]
fn resolution_adapter_uses_routed_manager_context_and_same_owner() {
    for marker in [
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "Permission::FORUM_TOPICS_MANAGE",
        "Permission denied: forum_topics:manage required",
        "resolve_tenant_scope",
        "execute_merge_forum_topic_resolving_solution",
        "selected_solution_reply_id = input.selected_solution_reply_id",
        "ForumTopicMergeService::new(db.clone(), event_bus.clone())",
        ".merge_topic_resolving_solution(",
        "SecurityContext::from_permission_snapshot",
        "operation_id: input.operation_id",
        "source_topic_id: input.source_topic_id",
        "reason: input.reason",
    ] {
        assert!(
            TOPIC_MERGE_GRAPHQL.contains(marker),
            "missing resolution adapter marker {marker}"
        );
    }

    assert_eq!(
        TOPIC_MERGE_GRAPHQL
            .matches("pub(crate) struct ForumTopicMergeMutation")
            .count(),
        1
    );
    for forbidden in [
        "resolve_canonical_topic",
        "forum_topic_merge_operations",
        "forum_solutions::",
        "TopicService::new",
        "get_with_locale_fallback",
    ] {
        assert!(
            !TOPIC_MERGE_GRAPHQL.contains(forbidden),
            "GraphQL resolution adapter contains forbidden marker {forbidden}"
        );
    }
}

#[test]
fn ordinary_and_resolved_commands_share_one_private_transaction_owner() {
    for marker in [
        "pub async fn merge_topic(",
        "pub async fn merge_topic_resolving_solution(",
        "async fn merge_topic_internal(",
        "self.merge_topic_internal(tenant_id, target_topic_id, security, None, input)",
        "Some(selected_solution_reply_id)",
        "plan_solution_merge(",
        "FORUM_TOPIC_MERGED_SCHEMA_VERSION",
        "forum_topic_merge_solution_resolution::ActiveModel",
        "load_solution_resolution_audit_in_tx",
        "TopicMergeSolutionConflict(operation_id)",
        "TopicMergeOperationConflict(input.operation_id)",
    ] {
        assert!(
            TOPIC_MERGE_OWNER.contains(marker),
            "missing shared owner marker {marker}"
        );
    }

    assert_eq!(
        TOPIC_MERGE_OWNER.matches("self.db.begin().await?").count(),
        1
    );
    assert_eq!(TOPIC_MERGE_OWNER.matches("txn.commit().await?").count(), 2);
    assert!(!TOPIC_MERGE_OWNER.contains("FORUM_TOPIC_MERGED_SOLUTION_RESOLUTION_SCHEMA_VERSION"));
}

#[test]
fn resolution_audit_is_append_only_and_keeps_merge_event_schema_one() {
    for marker in [
        "forum_topic_merge_solution_resolutions",
        "pub tenant_id: Uuid",
        "pub operation_id: Uuid",
        "pub selected_solution_reply_id: Uuid",
        "pub rejected_solution_reply_id: Uuid",
        "pub rejected_solution_author_id: Option<Uuid>",
    ] {
        assert!(
            RESOLUTION_ENTITY.contains(marker),
            "missing resolution entity marker {marker}"
        );
    }
    for marker in [
        "CREATE TABLE IF NOT EXISTS forum_topic_merge_solution_resolutions",
        "REFERENCES forum_topic_merge_operations (tenant_id, operation_id)",
        "forum topic merge solution resolutions are append-only",
    ] {
        assert!(
            RESOLUTION_MIGRATION.contains(marker),
            "missing resolution migration marker {marker}"
        );
    }
    assert!(TOPIC_MERGE_OWNER.contains("schema_version: Set(FORUM_TOPIC_MERGED_SCHEMA_VERSION)"));
    assert!(!TOPIC_MERGE_OWNER.contains("\"solution_resolution\""));
}
