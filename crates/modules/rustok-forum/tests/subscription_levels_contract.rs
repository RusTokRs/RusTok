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
    for token in ["forum.subscription.changed", "forum_subscription_policies"] {
        assert!(
            migration.contains(token),
            "missing subscription contract token {token}"
        );
    }
    // Auto-subscribe must never come back as a database trigger: triggers
    // upgraded an explicit `normal` (cleared) row without consent. The Rust
    // owner helper is the only implementation; trigger names may survive
    // only in the drop migration for already-migrated databases.
    for forbidden in [
        "CREATE TRIGGER forum_70_auto_subscribe",
        "CREATE OR REPLACE FUNCTION forum_auto_subscribe",
        "WHERE forum_topic_subscriptions.level = 'normal'",
        "WHERE forum_topic_subscriptions.level='normal'",
    ] {
        assert!(
            !migration.contains(forbidden),
            "forbidden auto-subscribe trigger token {forbidden}"
        );
    }
    let removal_migration =
        include_str!("../src/migrations/m20261102_000040_remove_forum_auto_subscribe_triggers.rs");
    assert!(removal_migration.contains("forum_70_auto_subscribe_topic_author"));
    let owner = include_str!("../src/services/subscription/auto_subscribe.rs");
    for token in [
        "auto_subscribe_topic_author_in_tx",
        "auto_subscribe_reply_participant_in_tx",
        "do_nothing",
    ] {
        assert!(
            owner.contains(token),
            "missing auto-subscribe owner token {token}"
        );
    }
}
