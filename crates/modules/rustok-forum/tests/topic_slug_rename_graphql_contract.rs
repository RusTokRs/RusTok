use async_graphql::{EmptySubscription, Schema};
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumMutation, ForumQuery};

const TOPIC_SLUG_RENAME_GRAPHQL: &str =
    include_str!("../src/graphql/topic_slug_rename_mutation.rs");

#[test]
fn graphql_schema_exposes_topic_slug_rename_command() {
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();
    let sdl = schema.sdl();

    for marker in [
        "renameForumTopicSlug",
        "RenameForumTopicSlugGraphqlInput",
        "GqlForumTopicSlugRename",
        "GqlForumTopicRouteDescriptor",
        "topicId",
        "locale",
        "previousSlug",
        "previousPath",
        "canonical",
        "shortId",
        "aliasId",
        "changed",
    ] {
        assert!(
            sdl.contains(marker),
            "missing GraphQL topic slug rename marker {marker}"
        );
    }
}

#[test]
fn graphql_rename_adapter_uses_routed_tenant_update_scope_and_owner_service() {
    for marker in [
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "Permission::FORUM_TOPICS_UPDATE",
        "Permission denied: forum_topics:update required",
        "resolve_tenant_scope",
        "TopicService::new(db.clone(), event_bus.clone())",
        ".rename_slug(",
        "SecurityContext::from_permission_snapshot",
        "locale: input.locale",
        "slug: input.slug",
    ] {
        assert!(
            TOPIC_SLUG_RENAME_GRAPHQL.contains(marker),
            "missing topic slug rename adapter marker {marker}"
        );
    }

    for forbidden in [
        "ForumTopicRouteService::new",
        "rename_topic_slug_in_tx",
        "forum_topic_route_aliases",
        "resolve_canonical_topic",
        "ForumTopicMergeService",
        "ForumTopicMergeRouteBackfillService",
    ] {
        assert!(
            !TOPIC_SLUG_RENAME_GRAPHQL.contains(forbidden),
            "topic slug rename adapter contains forbidden marker {forbidden}"
        );
    }
}
