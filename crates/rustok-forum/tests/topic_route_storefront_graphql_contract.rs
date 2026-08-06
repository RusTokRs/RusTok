use async_graphql::{EmptySubscription, Schema};
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumMutation, ForumQuery};

const TOPIC_ROUTE_QUERY: &str = include_str!("../src/graphql/topic_route_query.rs");
const GRAPHQL_MOD: &str = include_str!("../src/graphql/mod.rs");

#[test]
fn graphql_schema_exposes_visibility_safe_storefront_topic_route_resolution() {
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
        "CANONICAL",
        "REDIRECT",
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
fn storefront_topic_route_query_rechecks_visibility_and_hides_tombstone_identity() {
    for marker in [
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "require_public_forum_channel_enabled(ctx).await?",
        "Permission denied: tenant scope mismatch",
        "ForumTopicRouteService::new(db.clone())",
        ".resolve(tenant_id, &locale, &short_id, &slug)",
        "TopicService::new(db.clone(), event_bus.clone())",
        ".get_with_locale_fallback(",
        "forum_request_security(ctx)",
        "crate::constants::topic_status::OPEN",
        "is_topic_visible_for_channel(",
        "ForumTopicRouteDisposition::Gone => return Ok(None)",
        "SecurityContext::public_read",
        "ChannelService::new(db.clone())",
    ] {
        assert!(
            TOPIC_ROUTE_QUERY.contains(marker),
            "missing visibility-safe storefront route marker {marker}"
        );
    }

    for forbidden in [
        "pub requested_topic_id",
        "pub alias_id",
        "GqlForumStorefrontTopicRouteDisposition::Gone",
        "forum_topic_route_aliases",
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
fn forum_query_registers_the_route_transport_once() {
    assert_eq!(GRAPHQL_MOD.matches("mod topic_route_query;").count(), 1);
    assert_eq!(
        GRAPHQL_MOD
            .matches("topic_route_query::ForumTopicRouteQuery")
            .count(),
        1
    );
}
