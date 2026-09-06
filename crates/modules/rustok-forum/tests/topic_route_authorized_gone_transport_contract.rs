const GRAPHQL_QUERY: &str = include_str!("../src/graphql/topic_route_query.rs");
const GRAPHQL_MOD: &str = include_str!("../src/graphql/mod.rs");
const SERVICES_MOD: &str = include_str!("../src/services/mod.rs");
const STOREFRONT_MODEL: &str = include_str!("../storefront/src/model.rs");
const GRAPHQL_ADAPTER: &str =
    include_str!("../storefront/src/transport/topic_route_graphql_adapter.rs");
const NATIVE_ADAPTER: &str =
    include_str!("../storefront/src/transport/native_server_adapter_topic_route.rs");
const HOST: &str = include_str!("../../../apps/storefront/src/forum_topic_route.rs");

fn require(source: &str, markers: &[&str]) {
    for marker in markers {
        assert!(source.contains(marker), "missing source marker {marker}");
    }
}

fn forbid(source: &str, markers: &[&str]) {
    for marker in markers {
        assert!(!source.contains(marker), "forbidden source marker {marker}");
    }
}

#[test]
fn graphql_keeps_legacy_field_and_adds_authorized_decision() {
    require(
        GRAPHQL_QUERY,
        &[
            "async fn forum_storefront_topic_route(",
            "map_legacy_public_route_resolution(resolution)",
            "ForumTopicRouteDisposition::Gone => return Ok(None)",
            "async fn forum_storefront_topic_route_decision(",
            "GqlForumStorefrontTopicRouteDecisionDisposition",
            "Gone",
            "pub canonical: Option<GqlForumTopicRouteDescriptor>",
            "ForumTopicRouteTombstoneVisibilityService::new(db.clone())",
            ".can_disclose_public_gone(",
            "public_channel_slug(ctx).as_deref()",
        ],
    );
    require(
        GRAPHQL_MOD,
        &[
            "GqlForumStorefrontTopicRouteDecision",
            "GqlForumStorefrontTopicRouteDecisionDisposition",
        ],
    );
    forbid(
        GRAPHQL_QUERY,
        &[
            "GqlForumStorefrontTopicRouteDisposition::Gone",
            "forum_topic_route_tombstone_visibility",
            "forum_topic_route_tombstone_channels",
            "forum_topic_route_aliases",
            "Statement::from_sql_and_values",
        ],
    );
}

#[test]
fn owner_export_reveals_service_not_snapshot_payload() {
    require(SERVICES_MOD, &["ForumTopicRouteTombstoneVisibilityService"]);
    forbid(
        SERVICES_MOD,
        &[
            "StoredForumTopicRouteTombstoneVisibility",
            "route_channel_digest",
            "load_snapshot_channel_slugs",
        ],
    );
}

#[test]
fn storefront_transports_share_terminal_decision_shape() {
    require(
        STOREFRONT_MODEL,
        &[
            "StorefrontForumTopicRouteDisposition",
            "Gone",
            "pub canonical: Option<StorefrontForumTopicRouteDescriptor>",
        ],
    );
    require(
        GRAPHQL_ADAPTER,
        &[
            "forumStorefrontTopicRouteDecision",
            "StorefrontForumTopicRouteDecision",
            "canonical { topicId locale shortId slug path }",
        ],
    );
    require(
        NATIVE_ADAPTER,
        &[
            "ForumTopicRouteTombstoneVisibilityService",
            ".can_disclose_public_gone(",
            "request.channel_slug.as_deref()",
            "StorefrontForumTopicRouteDisposition::Gone",
            "(StorefrontForumTopicRouteDisposition::Gone, None)",
        ],
    );
    forbid(
        NATIVE_ADAPTER,
        &[
            "forum_topic_route_tombstone_visibility",
            "forum_topic_route_tombstone_channels",
            "forum_topic_route_aliases",
            "Statement::from_sql_and_values",
        ],
    );
}

#[test]
fn host_maps_only_authorized_terminal_decision_to_private_gone() {
    require(
        HOST,
        &[
            "ForumTopicHostAction::Gone",
            "StatusCode::GONE",
            "This Forum topic route is no longer available",
            "ForumTopicHostAction::Invalid",
            "StatusCode::SERVICE_UNAVAILABLE",
            "StorefrontForumTopicRouteDisposition::Gone, None",
            "StorefrontForumTopicRouteDisposition::Gone, Some(_)",
        ],
    );
    forbid(
        HOST,
        &[
            "ForumTopicRouteTombstoneVisibilityService",
            "can_disclose_public_gone",
            "forum_topic_route_tombstone_visibility",
            "forum_topic_route_aliases",
        ],
    );
}
