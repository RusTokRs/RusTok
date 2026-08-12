use async_graphql::{EmptySubscription, Schema};
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumMutation, ForumQuery};

const TOPIC_ROUTE_QUERY: &str = include_str!("../src/graphql/topic_route_query.rs");
const GRAPHQL_MOD: &str = include_str!("../src/graphql/mod.rs");

#[test]
fn graphql_schema_retains_legacy_route_and_adds_authorized_decision() {
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();
    let sdl = schema.sdl();

    for marker in [
        "forumStorefrontTopicRoute",
        "GqlForumStorefrontTopicRouteResolution",
        "GqlForumStorefrontTopicRouteDisposition",
        "forumStorefrontTopicRouteDecision",
        "GqlForumStorefrontTopicRouteDecision",
        "GqlForumStorefrontTopicRouteDecisionDisposition",
        "CANONICAL",
        "REDIRECT",
        "GONE",
        "requestedLocale",
        "requestedShortId",
        "requestedSlug",
        "canonical",
        "GqlForumTopicRouteDescriptor",
        "shortId",
        "path",
    ] {
        assert!(
            sdl.contains(marker),
            "missing storefront topic route GraphQL marker {marker}"
        );
    }
}

#[test]
fn route_query_reuses_exact_audience_and_tombstone_decision_owners() {
    for marker in [
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "forum_channel_enabled(ctx).await?",
        "Permission denied: tenant scope mismatch",
        "ForumTopicRouteService::new(db.clone())",
        ".resolve(tenant_id, &locale, &short_id, &slug)",
        ".topic_audience_read_service(db.clone(), event_bus.clone())",
        "topic_read_audience_port_context(",
        "ForumTopicReadTransport::Graphql",
        "ForumTopicReadOperation::SelectedTopic",
        ".get_authenticated_storefront_visible_with_audience_context(",
        ".get_public_storefront_visible_with_locale_fallback(",
        "SecurityContext::from_permission_snapshot",
        "ForumTopicRouteTombstoneVisibilityService::new(db.clone())",
        ".can_disclose_public_gone(",
        "GqlForumStorefrontTopicRouteDecisionDisposition::Gone",
        "ChannelService::new(db.clone())",
    ] {
        assert!(
            TOPIC_ROUTE_QUERY.contains(marker),
            "missing visibility-safe storefront route marker {marker}"
        );
    }

    for forbidden in [
        "TopicService::new",
        "is_topic_visible_for_channel(",
        "crate::constants::topic_status::OPEN",
        "pub requested_topic_id",
        "pub alias_id",
        "GqlForumStorefrontTopicRouteDisposition::Gone",
        "forum_topic_route_aliases",
        "forum_topic_route_tombstone_visibility",
        "forum_topic_route_tombstone_channels",
        "Statement::from_sql_and_values",
        "SELECT ",
        "record_redirect_alias",
        "record_gone_alias",
    ] {
        assert!(
            !TOPIC_ROUTE_QUERY.contains(forbidden),
            "storefront route query contains forbidden marker {forbidden}"
        );
    }
}

#[test]
fn legacy_field_still_hides_gone_while_decision_field_can_expose_it() {
    for marker in [
        "async fn forum_storefront_topic_route(",
        "map_legacy_public_route_resolution(resolution)",
        "ForumTopicRouteDisposition::Gone => return Ok(None)",
        "async fn forum_storefront_topic_route_decision(",
        "map_public_route_decision(resolution).map(Some)",
        "GqlForumStorefrontTopicRouteDecisionDisposition::Gone",
        "pub canonical: Option<GqlForumTopicRouteDescriptor>",
    ] {
        assert!(
            TOPIC_ROUTE_QUERY.contains(marker),
            "missing marker {marker}"
        );
    }
}

#[test]
fn forum_query_registers_the_route_transport_once() {
    assert_eq!(GRAPHQL_MOD.matches("mod topic_route_query;").count(), 1);
    assert_eq!(
        GRAPHQL_MOD
            .matches("topic_route_query::ForumTopicRouteQuery")
            .count(),
        1
    );
}
