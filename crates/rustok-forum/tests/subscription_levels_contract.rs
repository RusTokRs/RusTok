#[test]
fn postgres_and_sqlite_subscription_contracts_remain_present() {
    let migration = [
        include_str!("../src/migrations/m20260713_000013_add_forum_subscription_levels.rs"),
        include_str!("../src/migrations/m20260713_000013_add_forum_subscription_levels/postgres_up/schema.rs"),
        include_str!("../src/migrations/m20260713_000013_add_forum_subscription_levels/postgres_up/events.rs"),
        include_str!("../src/migrations/m20260713_000013_add_forum_subscription_levels/postgres_up/automation.rs"),
        include_str!("../src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/schema.rs"),
        include_str!("../src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/validation.rs"),
        include_str!("../src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/events.rs"),
        include_str!("../src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/automation.rs"),
    ]
    .join("\n");
    for token in [
        "forum.subscription.changed.v1",
        "forum_auto_subscribe_topic_author",
        "forum_auto_subscribe_reply_participant",
        "ON CONFLICT (tenant_id, topic_id, user_id) DO UPDATE SET",
        "ON CONFLICT(tenant_id,topic_id,user_id) DO UPDATE SET",
        "forum_subscription_policies",
    ] {
        assert!(
            migration.contains(token),
            "missing subscription contract token {token}"
        );
    }
}
