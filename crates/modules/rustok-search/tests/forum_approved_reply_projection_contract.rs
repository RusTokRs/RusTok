const FORUM_SOURCE: &str = include_str!("../../rustok-forum/src/search_projection.rs");
const PUBLIC_DISCOVERY: &str = include_str!("../../rustok-forum/src/services/public_discovery.rs");
const REPLY_AUDIENCE_READ: &str =
    include_str!("../../rustok-forum/src/services/reply_audience_read.rs");
const FORUM_REPLY_UPDATE: &str = include_str!("../../rustok-forum/src/services/reply_inline.rs");
const SEARCH_PROJECTION_DRIVER: &str = include_str!("../src/projection_driver.rs");
const SEARCH_ENGINE: &str = include_str!("../src/engine.rs");
const ADMIN_GLOBAL_SEARCH: &str =
    include_str!("../../../../apps/admin/src/widgets/app_shell/native_server_adapter.rs");

fn require(source: &str, marker: &str) {
    assert!(source.contains(marker), "missing source marker: {marker}");
}

fn reject(source: &str, marker: &str) {
    assert!(
        !source.contains(marker),
        "forbidden source marker: {marker}"
    );
}

#[test]
fn forum_source_publishes_only_exact_public_approved_reply_documents() {
    for marker in [
        "const FORUM_REPLY_ENTITY_TYPE: &str = \"forum_reply\"",
        "forum_reply_body::Entity::find()",
        "ProjectionCandidate::Reply",
        "ProjectionCursor::Reply",
        ".get_public_reply_with_locale_fallback(",
        "Some(&[ReplyStatus::Approved])",
        "if reply.effective_locale != locale",
        ".get_public_topic_with_locale_fallback(",
        "if topic.effective_locale != locale",
        ".get_public_category_with_locale_fallback(",
        "exact_topic_route(&self.db, tenant_id, topic.id, locale)",
        "document_key: format!(\"forum_reply:{reply_id}:{locale}\")",
        "\"kind\": \"forum_reply\"",
        "\"reply_id\": reply_id",
        "\"topic_id\": topic.id",
        "\"is_solution\": is_solution",
        "format!(\"{topic_route}?reply={reply_id}\")",
    ] {
        require(FORUM_SOURCE, marker);
    }
    reject(FORUM_SOURCE, "forum_reply::Entity::find()");
    reject(FORUM_SOURCE, "\"/modules/forum?topic=");

    for marker in [
        "pub async fn get_public_reply_with_locale_fallback",
        ".get_public_storefront_visible_with_locale_fallback(",
    ] {
        require(PUBLIC_DISCOVERY, marker);
    }
    for marker in [
        "pub async fn get_public_storefront_visible_with_locale_fallback",
        "statuses.is_some_and(|allowed| !allowed.contains(&reply.status))",
        ".is_topic_visible(tenant_id, reply.topic_id, channel_slug, &viewer)",
        ".get_with_locale_fallback(",
    ] {
        require(REPLY_AUDIENCE_READ, marker);
    }
}

#[test]
fn reply_edits_reuse_topic_invalidation_and_topic_refresh_rebuilds_child_scope() {
    for marker in [
        "target_type: \"forum_topic\".to_string()",
        "target_id: Some(topic_id)",
    ] {
        require(FORUM_REPLY_UPDATE, marker);
    }
    for marker in [
        // The owner, not Search, decides that a topic refresh replaces its replies.
        "fn refresh_scope(&self, entity_type: &str) -> SearchProjectionRefreshScope",
        "if entity_type == FORUM_TOPIC_ENTITY_TYPE",
        "SearchProjectionRefreshScope::Tenant",
    ] {
        require(FORUM_SOURCE, marker);
    }
    for marker in [
        "if self.source.refresh_scope(entity_type) == SearchProjectionRefreshScope::Tenant",
        "return self.rebuild_tenant(tenant_id).await",
        "DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = $2",
    ] {
        require(SEARCH_PROJECTION_DRIVER, marker);
    }
}

#[test]
fn canonical_reply_route_is_bound_to_owner_topic_route_and_result_identity() {
    for marker in [
        "const FORUM_REPLY_ENTITY_TYPE: &str = \"forum_reply\"",
        "canonical_forum_projected_result_url(value)",
        "parse_payload_uuid(&value.payload, \"reply_id\")",
        "if reply_id != value.id",
        "parse_payload_uuid(&value.payload, \"topic_id\")",
        "canonical_forum_topic_route(route, locale.as_str(), topic_id, Some(reply_id))",
        "forum_topic_short_identity",
        "canonical_url_rejects_stale_or_malformed_forum_route_projections",
    ] {
        require(SEARCH_ENGINE, marker);
    }
    reject(SEARCH_ENGINE, "canonical_forum_reply_result_url");
    reject(SEARCH_ENGINE, "{FORUM_STOREFRONT_ROUTE}?topic=");
}

#[test]
fn admin_global_search_maps_forum_results_to_domain_permissions() {
    for marker in [
        "(\"forum_category\", \"forum\" | \"rustok-forum\")",
        "Permission::FORUM_CATEGORIES_READ",
        "(\"forum_topic\", \"forum\" | \"rustok-forum\")",
        "Permission::FORUM_TOPICS_READ",
        "(\"forum_reply\", \"forum\" | \"rustok-forum\")",
        "Permission::FORUM_REPLIES_READ",
        "required_admin_search_permission(\"forum_reply\", \"content\")",
    ] {
        require(ADMIN_GLOBAL_SEARCH, marker);
    }
}
