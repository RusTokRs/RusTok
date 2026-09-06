const MIGRATION: &str = include_str!(
    "../src/migrations/m20260806_000025_add_forum_topic_route_tombstone_visibility.rs"
);
const MIGRATIONS_MOD: &str = include_str!("../src/migrations/mod.rs");
const TOPIC_OWNER: &str = include_str!("../src/services/topic_owner.rs");
const TOPIC_OWNER_INLINE: &str = include_str!("../src/services/topic_owner_inline.rs");
const SNAPSHOT_OWNER: &str = include_str!("../src/services/topic_route_tombstone_visibility.rs");
const CATEGORY_VISIBILITY: &str = include_str!("../src/services/category_visibility.rs");

fn require_order(source: &str, markers: &[&str]) {
    let mut previous = 0usize;
    for marker in markers {
        let position = source
            .find(marker)
            .unwrap_or_else(|| panic!("missing source marker {marker}"));
        assert!(
            position >= previous,
            "source marker {marker} appears out of order"
        );
        previous = position;
    }
}

#[test]
fn migration_adds_sealed_append_only_snapshot_storage() {
    for marker in [
        "forum_topic_route_tombstone_visibility",
        "forum_topic_route_tombstone_channels",
        "publicly_disclosable",
        "route_channel_restricted",
        "route_channel_count",
        "route_channel_digest",
        "route_channel_digest ~ '^[0-9a-f]{64}$'",
        "route_channel_digest NOT GLOB '*[^0-9a-f]*'",
        "forum topic route tombstone visibility is append-only",
        "idx_forum_topic_route_tombstone_channel_lookup",
        "DatabaseBackend::Postgres",
        "DatabaseBackend::Sqlite",
    ] {
        assert!(
            MIGRATION.contains(marker),
            "missing migration marker {marker}"
        );
    }
    assert!(MIGRATIONS_MOD.contains("m20260806_000025_add_forum_topic_route_tombstone_visibility"));
}

#[test]
fn delete_matches_canonical_policy_lock_order_before_snapshot() {
    require_order(
        TOPIC_OWNER,
        &[
            "ForumTopicRouteTombstoneVisibilityService::lock_category_scope_in_tx(",
            "claim_topic_delete_in_tx(&txn, tenant_id, topic_id).await?",
            "ForumTopicRouteTombstoneVisibilityService::lock_topic_audience_scope_in_tx(",
            "ForumTopicRouteTombstoneVisibilityService::record_locked_delete_snapshot_in_tx(",
            "ForumTopicRouteService::record_delete_tombstones_in_tx(",
            "mark_topic_thread_deleted_in_tx(&txn, tenant_id, topic_id).await?",
        ],
    );
    assert!(TOPIC_OWNER_INLINE.contains("include!(\"topic_route_tombstone_visibility.rs\")"));
}

#[test]
fn snapshot_reuses_visibility_and_lock_owners_and_seals_exact_channel_scope() {
    for marker in [
        "lock_category_tree_in_tx(txn, tenant_id).await",
        "lock_topic_audience_scopes_in_tx(txn, tenant_id, &[topic_id]).await",
        "is_category_public_to_anonymous(txn, tenant_id, topic.category_id)",
        "load_policy_for_topic(txn, tenant_id, topic)",
        "ForumAudienceEvaluator::decide(",
        "SecurityContext::public_read()",
        "topic.status == TopicStatus::Open",
        "route_channel_digest(&channel_slugs)",
        "Some(existing)",
        "None =>",
        "stored_channels != channel_slugs",
        "validate_sealed_channel_scope",
        "can_disclose_public_gone",
    ] {
        assert!(
            SNAPSHOT_OWNER.contains(marker),
            "missing snapshot owner marker {marker}"
        );
    }
    assert!(CATEGORY_VISIBILITY.contains("pub(crate) async fn is_category_public_to_anonymous"));
    assert!(
        CATEGORY_VISIBILITY
            .contains("super::category_audience::lock_category_tree_in_tx(&txn, tenant_id).await?")
    );

    for forbidden in [
        "forum_category_policy::",
        "hashtextextended",
        "async_graphql",
        "axum::",
        "StatusCode::GONE",
        "GqlForumStorefrontTopicRouteDisposition::Gone",
        "forumStorefrontTopicRoute",
    ] {
        assert!(
            !SNAPSHOT_OWNER.contains(forbidden),
            "snapshot owner contains forbidden marker {forbidden}"
        );
    }
}

#[test]
fn this_slice_does_not_publish_gone_transport_or_http_policy() {
    for source in [SNAPSHOT_OWNER, TOPIC_OWNER, TOPIC_OWNER_INLINE, MIGRATION] {
        for forbidden in [
            "GqlForumStorefrontTopicRouteDisposition::Gone",
            "StorefrontForumTopicRouteDisposition::Gone",
            "StatusCode::GONE",
            "410 Gone",
        ] {
            assert!(
                !source.contains(forbidden),
                "FORUM-24J contains premature public gone marker {forbidden}"
            );
        }
    }
}
