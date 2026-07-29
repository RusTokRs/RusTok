const FORUM_SOURCE: &str = include_str!("../../rustok-forum/src/search_projection.rs");
const FORUM_REPLY_UPDATE: &str = include_str!("../../rustok-forum/src/services/reply_inline.rs");
const SEARCH_PROJECTOR: &str = include_str!("../src/forum_projector.rs");
const SEARCH_ENGINE: &str = include_str!("../src/engine.rs");
const ADMIN_GLOBAL_SEARCH: &str =
    include_str!("../../../apps/admin/src/widgets/app_shell/native_server_adapter.rs");

fn require(source: &str, marker: &str) {
    assert!(source.contains(marker), "missing source marker: {marker}");
}

#[test]
fn forum_source_publishes_only_exact_public_approved_reply_documents() {
    for marker in [
        "const FORUM_REPLY_ENTITY_TYPE: &str = \"forum_reply\"",
        "forum_reply_body::Entity::find()",
        "ProjectionCandidate::Reply",
        "ProjectionCursor::Reply",
        "if owner.status != ReplyStatus::Approved",
        ".get_public_topic_with_locale_fallback(",
        ".get_public_category_with_locale_fallback(",
        "document_key: format!(\"forum_reply:{reply_id}:{locale}\")",
        "\"kind\": \"forum_reply\"",
        "\"reply_id\": reply_id",
        "\"topic_id\": topic.id",
        "\"is_solution\": is_solution",
    ] {
        require(FORUM_SOURCE, marker);
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
        "FORUM_TOPIC_ENTITY_TYPE | FORUM_REPLY_ENTITY_TYPE",
        "if entity_type == FORUM_TOPIC_ENTITY_TYPE",
        "return self.rebuild_tenant(tenant_id).await",
        "'forum_category', 'forum_topic', 'forum_reply'",
    ] {
        require(SEARCH_PROJECTOR, marker);
    }
}

#[test]
fn canonical_reply_route_is_bound_to_result_and_parent_topic_identity() {
    for marker in [
        "const FORUM_REPLY_ENTITY_TYPE: &str = \"forum_reply\"",
        "canonical_forum_reply_result_url(value)",
        "parse_payload_uuid(&value.payload, \"reply_id\")",
        "if reply_id != value.id",
        "parse_payload_uuid(&value.payload, \"topic_id\")",
        "?topic={topic_id}&reply={reply_id}",
        "canonical_url_rejects_spoofed_forum_source_entity_pairs_and_reply_payloads",
    ] {
        require(SEARCH_ENGINE, marker);
    }
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
